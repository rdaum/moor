// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Shared daemon integration-test environment.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use moor_common::model::CommitResult;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{
        NoopTasksDb,
        scheduler::{Scheduler, SchedulerThreads},
    },
};
use moor_textdump::{TextdumpImportOptions, textdump_load};
use rusty_paseto::prelude::Key;
use semver::Version;
use tempfile::TempDir;

use crate::{
    connections::{ConnectionRegistry, ConnectionRegistryFactory},
    event_log::EventLogOps,
    rpc::{MessageHandler, RpcServer, Transport},
    system_control::NoopWorkerInfoSource,
    testing::MockEventLog,
};

pub struct TestEnvironment<T: Transport + 'static> {
    pub message_handler: Arc<dyn MessageHandler>,
    pub transport: Arc<T>,
    pub event_log: Arc<MockEventLog>,
    pub connections: Arc<dyn ConnectionRegistry>,
    pub scheduler_client: SchedulerClient,
    pub rpc_server: Arc<RpcServer>,
    pub kill_switch: Arc<AtomicBool>,
    pub _temp_dir: TempDir,
    pub _temp_output_dir: Option<TempDir>,
    pub output_dir_path: Option<PathBuf>,
    scheduler_thread: Option<SchedulerThreads>,
    rpc_thread: Option<std::thread::JoinHandle<()>>,
}

impl<T: Transport + 'static> Drop for TestEnvironment<T> {
    fn drop(&mut self) {
        self.kill_switch.store(true, Ordering::SeqCst);
        let _ = self.scheduler_client.submit_shutdown("Test complete");

        if let Some(thread) = self.scheduler_thread.take() {
            let _ = thread.join();
        }

        if let Some(thread) = self.rpc_thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();
}

pub fn create_test_keys() -> (Key<32>, Key<64>) {
    const SIGNING_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEILrkKmddHFUDZqRCnbQsPoW/Wsp0fLqhnv5KNYbcQXtk
-----END PRIVATE KEY-----
"#;

    const VERIFYING_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAZQUxGvw8u9CcUHUGLttWFZJaoroXAmQgUGINgbBlVYw=
-----END PUBLIC KEY-----
"#;

    let (private_key, public_key) = moor_runtime_api::parse_keypair(VERIFYING_KEY, SIGNING_KEY)
        .expect("Failed to parse test keypair");
    (public_key, private_key)
}

pub fn setup_test_db_with_core() -> (Box<dyn Database>, TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    let (db, _) = TxDB::try_open_fjall(&db_path, DatabaseConfig::default()).unwrap();
    let db = Box::new(db) as Box<dyn Database>;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let jhcore = manifest_dir.join("../../cores/JHCore-DEV-2.db");

    let mut loader = db.loader_client().unwrap();
    let config = Config::default();
    textdump_load(
        loader.as_mut(),
        jhcore,
        Version::new(0, 1, 0),
        config.features.compile_options(),
        TextdumpImportOptions::default(),
    )
    .expect("Failed to load textdump");
    assert!(matches!(loader.commit(), Ok(CommitResult::Success { .. })));

    (db, temp_dir)
}

pub fn wait_for_scheduler_ready(scheduler_client: &SchedulerClient) {
    let start = Instant::now();
    let timeout = Duration::from_secs(10);

    while start.elapsed() < timeout {
        if scheduler_client.check_status().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    panic!("Scheduler failed to become ready within timeout");
}

pub fn setup_test_environment<T>(
    transport: Arc<T>,
    configure: impl FnOnce(&mut Config),
) -> TestEnvironment<T>
where
    T: Transport + 'static,
{
    setup_test_environment_with_core(transport, configure, None)
}

pub fn setup_test_environment_with_core<T>(
    transport: Arc<T>,
    configure: impl FnOnce(&mut Config),
    core: Option<&Path>,
) -> TestEnvironment<T>
where
    T: Transport + 'static,
{
    setup_tracing();

    let (public_key, private_key) = create_test_keys();

    let mut config = Config::default();
    configure(&mut config);
    let config = Arc::new(config);

    let (db, temp_dir) = if let Some(core) = core {
        let temp_dir = tempfile::tempdir().unwrap();
        let (db, _) =
            TxDB::try_open_fjall(temp_dir.path().join("test.db"), DatabaseConfig::default())
                .unwrap();
        let mut loader = db.loader_client().unwrap();
        moor_objdef::ObjectDefinitionLoader::new(loader.as_mut())
            .load_objdef_directory(
                config.features.compile_options(),
                core,
                moor_objdef::ObjDefLoaderOptions::default(),
            )
            .unwrap();
        assert!(matches!(loader.commit(), Ok(CommitResult::Success { .. })));
        (Box::new(db) as Box<dyn Database>, temp_dir)
    } else {
        setup_test_db_with_core()
    };
    let kill_switch = Arc::new(AtomicBool::new(false));
    let connections = ConnectionRegistryFactory::in_memory_only().unwrap();
    let event_log = Arc::new(MockEventLog::new());

    let transport_for_server: Arc<dyn Transport> = transport.clone();
    let (rpc_server, task_monitor, system_control) = RpcServer::new(
        kill_switch.clone(),
        public_key,
        private_key,
        connections.clone(),
        event_log.clone() as Arc<dyn EventLogOps>,
        transport_for_server,
        config.clone(),
        None,
        Arc::new(NoopWorkerInfoSource),
    );

    let message_handler = rpc_server.message_handler().clone();
    let tasks_db = Box::new(NoopTasksDb {});
    let scheduler = Scheduler::new(
        Version::new(0, 1, 0),
        db,
        tasks_db,
        config,
        Arc::new(system_control),
        None,
        None,
    );

    let scheduler_client = scheduler.client().expect("Failed to get scheduler client");
    let rpc_server = Arc::new(rpc_server);

    let rpc_server_for_loop = rpc_server.clone();
    let scheduler_client_for_rpc = scheduler_client.clone();
    let rpc_thread = std::thread::Builder::new()
        .name("test-rpc-server".to_string())
        .spawn(move || {
            if let Err(e) = rpc_server_for_loop.request_loop(
                "mock://test".to_string(),
                scheduler_client_for_rpc,
                task_monitor,
            ) {
                eprintln!("RPC server request loop error: {e:?}");
            }
        })
        .expect("Failed to spawn RPC server thread");

    let scheduler_thread = scheduler
        .start(rpc_server.clone())
        .expect("Failed to start scheduler");
    wait_for_scheduler_ready(&scheduler_client);

    TestEnvironment {
        message_handler,
        transport,
        event_log,
        connections,
        scheduler_client,
        rpc_server,
        kill_switch,
        _temp_dir: temp_dir,
        _temp_output_dir: None,
        output_dir_path: None,
        scheduler_thread: Some(scheduler_thread),
        rpc_thread: Some(rpc_thread),
    }
}

pub fn setup_test_environment_with_checkpoint_output<T>(transport: Arc<T>) -> TestEnvironment<T>
where
    T: Transport + 'static,
{
    let temp_output_dir = tempfile::tempdir().expect("Failed to create temp output dir");
    let output_path = temp_output_dir.path().to_path_buf();

    let mut env = setup_test_environment(transport, |config| {
        config.import_export.output_path = Some(output_path.clone());
        config.features = Arc::new(FeaturesConfig {
            anonymous_objects: true,
            ..config.features.as_ref().clone()
        });
    });

    env._temp_output_dir = Some(temp_output_dir);
    env.output_dir_path = Some(output_path);
    env
}
