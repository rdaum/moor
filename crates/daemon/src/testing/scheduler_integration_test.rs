// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Integration tests using real WorldState DB
//!
//! These tests verify the integration between RPC components and a real database
//! loaded with JHCore.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::event_log::EventLogOps;
    use moor_common::model::CommitResult;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_kernel::config::Config;
    use moor_kernel::tasks::NoopTasksDb;
    use moor_kernel::tasks::scheduler::Scheduler;
    use moor_textdump::textdump_load;
    use moor_var::SYSTEM_OBJECT;
    use rusty_paseto::prelude::Key;
    use semver::Version;

    use crate::connections::ConnectionRegistryFactory;
    use crate::rpc::RpcServer;
    use crate::testing::{MockEventLog, MockTransport};

    /// Wait for the scheduler to be ready by attempting simple operations
    fn wait_for_scheduler_ready(scheduler_client: &moor_kernel::SchedulerClient) {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(10);

        while start.elapsed() < timeout {
            if scheduler_client.check_status().is_ok() {
                return;
            }
            // Short sleep to avoid busy waiting, but not load-bearing
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        panic!("Scheduler failed to become ready within timeout");
    }

    fn create_test_keys() -> (Key<32>, Key<64>) {
        // Use the fixed test keys from rpc_integration_test
        const SIGNING_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEILrkKmddHFUDZqRCnbQsPoW/Wsp0fLqhnv5KNYbcQXtk
-----END PRIVATE KEY-----
"#;

        const VERIFYING_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAZQUxGvw8u9CcUHUGLttWFZJaoroXAmQgUGINgbBlVYw=
-----END PUBLIC KEY-----
"#;

        let (private_key, public_key) = rpc_common::parse_keypair(VERIFYING_KEY, SIGNING_KEY)
            .expect("Failed to parse test keypair");
        (public_key, private_key)
    }

    /// Create a temporary database with JHCore loaded
    fn setup_test_db_with_core() -> (Box<dyn Database>, TempDir) {
        // Create temporary directory for database
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        // Create database
        let (db, _) = TxDB::open(Some(&db_path), DatabaseConfig::default());
        let db = Box::new(db) as Box<dyn Database>;

        // Load JHCore
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let jhcore = manifest_dir.join("../../JHCore-DEV-2.db");

        let mut loader = db.loader_client().unwrap();
        let config = Config::default();
        textdump_load(
            loader.as_mut(),
            jhcore,
            Version::new(0, 1, 0),
            config.features.compile_options(),
        )
        .expect("Failed to load textdump");
        assert_eq!(loader.commit().unwrap(), CommitResult::Success);

        (db, temp_dir)
    }

    struct TestEnvironment {
        message_handler: Arc<dyn crate::rpc::MessageHandler>,
        transport: Arc<MockTransport>,
        event_log: Arc<MockEventLog>,
        scheduler_client: moor_kernel::SchedulerClient,
        kill_switch: Arc<AtomicBool>,
        _temp_dir: TempDir,
        scheduler_thread: Option<std::thread::JoinHandle<()>>,
        rpc_thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            // Signal shutdown
            self.kill_switch
                .store(true, std::sync::atomic::Ordering::SeqCst);

            // Send shutdown message to scheduler
            let _ = self.scheduler_client.submit_shutdown("Test complete");

            // Wait for scheduler thread to finish (with timeout)
            if let Some(thread) = self.scheduler_thread.take() {
                // Give it a reasonable time to shut down
                let _ = thread.join();
            }

            // Wait for RPC thread to finish
            if let Some(thread) = self.rpc_thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn setup_test_environment_with_real_scheduler() -> TestEnvironment {
        // Set up tracing to capture scheduler logs
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        let (public_key, private_key) = create_test_keys();
        let config = Arc::new(Config::default());

        // Create real database with core
        let (db, temp_dir) = setup_test_db_with_core();

        // Create kill switch
        let kill_switch = Arc::new(AtomicBool::new(false));

        // Create mock components
        let connections = ConnectionRegistryFactory::in_memory_only().unwrap();
        let transport = Arc::new(MockTransport::new());
        let event_log = Arc::new(MockEventLog::new());

        // Create the event log path for RpcServer
        let events_db_path = temp_dir.path().join("events.db");

        // Create RpcServer with MockTransport - this will be the single source of truth!
        let (rpc_server, task_monitor, system_control) = RpcServer::new(
            kill_switch.clone(),
            public_key,
            private_key,
            connections,
            transport.clone(),
            config.clone(),
            &events_db_path,
        );

        // Get the message handler from the RpcServer for direct testing
        let message_handler = rpc_server.message_handler().clone();

        // Create real scheduler with our test database
        let tasks_db = Box::new(NoopTasksDb {});
        let scheduler = Scheduler::new(
            Version::new(0, 1, 0),
            db,
            tasks_db,
            config,
            Arc::new(system_control),
            None, // No workers for testing
            None, // No worker receiver for testing
        );

        // Get scheduler client before moving scheduler
        let scheduler_client = scheduler.client().expect("Failed to get scheduler client");

        // Run scheduler in background thread like in main.rs
        let rpc_server_arc = Arc::new(rpc_server);

        // Start the RPC server's request loop (handles SessionActions messages)
        let rpc_server_for_loop = rpc_server_arc.clone();
        let scheduler_client_for_rpc = scheduler_client.clone();
        let rpc_thread = std::thread::Builder::new()
            .name("test-rpc-server".to_string())
            .spawn(move || {
                if let Err(e) = rpc_server_for_loop.request_loop(
                    "mock://test".to_string(),
                    scheduler_client_for_rpc,
                    task_monitor,
                ) {
                    eprintln!("RPC server request loop error: {:?}", e);
                }
            })
            .expect("Failed to spawn RPC server thread");

        let scheduler_thread = std::thread::Builder::new()
            .name("test-scheduler".to_string())
            .spawn(move || scheduler.run(rpc_server_arc))
            .expect("Failed to spawn scheduler thread");

        TestEnvironment {
            message_handler,
            transport,
            event_log,
            scheduler_client,
            kill_switch,
            _temp_dir: temp_dir,
            scheduler_thread: Some(scheduler_thread),
            rpc_thread: Some(rpc_thread),
        }
    }

    #[test]
    fn test_real_scheduler_startup() {
        let env = setup_test_environment_with_real_scheduler();

        // Wait for scheduler to be ready by attempting a simple operation
        wait_for_scheduler_ready(&env.scheduler_client);
    }

    #[test]
    fn test_connection_establishment_with_real_db() {
        let env = setup_test_environment_with_real_scheduler();
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // Test establishing a connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        assert!(
            establish_result.is_ok(),
            "Connection establishment should succeed"
        );

        let (client_token, connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {:?}", other),
        };

        assert!(
            !client_token.0.is_empty(),
            "Should receive a valid client token"
        );
        assert!(
            connection_obj.id().0 < 0,
            "Should receive a valid connection object"
        );

        // Verify connection is tracked
        let connections = env.message_handler.get_connections();
        assert!(
            connections.contains(&connection_obj),
            "Connection should be tracked"
        );
    }

    #[test]
    fn test_wizard_login_with_real_scheduler() {
        let env = setup_test_environment_with_real_scheduler();
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // First establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {:?}", other),
        };

        // Follow the proper telnet-host sequence:
        // 1. First call welcome message (empty args, do_attach: false)
        // 2. Then actual login (with args, do_attach: true)

        // Step 1: Welcome message call
        let welcome_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec![], // Empty args for welcome
            do_attach: false,
        };

        let welcome_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            welcome_message,
        );

        // Welcome call should succeed (it just triggers welcome message)
        assert!(
            welcome_result.is_ok(),
            "Welcome message call should succeed: {:?}",
            welcome_result
        );

        // Wait for welcome message to be processed - check transport for narrative events
        // since EventLog skips events for negative connection objects
        assert!(
            env.transport.wait_for_narrative_events(1, 2000),
            "Should receive welcome message events within 2 seconds"
        );

        // Step 2: Actual login as wizard
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "wizard".to_string()], // Same as user typing "connect wizard"
            do_attach: true,
        };

        let login_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            login_message,
        );

        // Wait for login task to be processed and events to be generated
        let login_success = env.transport.wait_for_condition(
            |transport| {
                // Look for completion events or specific narrative events indicating login success
                let narrative_events = transport.get_narrative_events();
                let client_events = transport.get_client_events();

                // Login succeeded if we got any narrative events OR client events indicating completion
                !narrative_events.is_empty() || !client_events.is_empty()
            },
            5000,
        );

        if !login_success {
            // Give a bit more time for any late events
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Collect debugging information if login fails
        if login_result.is_err() {
            // Get all events from MockEventLog
            let all_events = env.event_log.get_all_events();
            let narrative_events = env.transport.get_narrative_events();
            let client_replies = env.transport.get_client_replies();
            let client_events = env.transport.get_client_events();
            let host_events = env.transport.get_host_events();

            // Also check for any recent events since the start of login
            let recent_events = env
                .event_log
                .events_for_player_since_seconds(SYSTEM_OBJECT, 10);

            panic!(
                "Login failed: {:?}\n\nMockEventLog all events ({} total):\n{:#?}\n\nMockEventLog recent events for SYSTEM_OBJECT ({} total):\n{:#?}\n\nMockTransport narrative events ({} total):\n{:#?}\n\nMockTransport client events ({} total):\n{:#?}\n\nMockTransport host events ({} total):\n{:#?}\n\nMockTransport client replies ({} total):\n{:#?}",
                login_result,
                all_events.len(),
                all_events,
                recent_events.len(),
                recent_events,
                narrative_events.len(),
                narrative_events,
                client_events.len(),
                client_events,
                host_events.len(),
                host_events,
                client_replies.len(),
                client_replies
            );
        }

        // Check that we got some kind of response (TaskSubmitted at minimum)
        match login_result.unwrap() {
            rpc_common::DaemonToClientReply::TaskSubmitted(_) => {
                // This is expected - task was submitted to scheduler
            }
            rpc_common::DaemonToClientReply::LoginResult(Some(_)) => {
                // This would be ideal - login completed synchronously
            }
            other => {
                // Get debugging information for unexpected results too
                let all_events = env.event_log.get_all_events();
                let narrative_events = env.transport.get_narrative_events();
                let client_replies = env.transport.get_client_replies();

                panic!(
                    "Unexpected login result: {:?}\n\nMockEventLog events ({} total):\n{:#?}\n\nMockTransport narrative events ({} total):\n{:#?}\n\nMockTransport client replies ({} total):\n{:#?}",
                    other,
                    all_events.len(),
                    all_events,
                    narrative_events.len(),
                    narrative_events,
                    client_replies.len(),
                    client_replies
                );
            }
        }
    }
}
