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
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::connections::ConnectionRegistryFactory;
    use crate::rpc::RpcServer;
    use crate::testing::{MockEventLog, MockTransport};
    use moor_common::model::CommitResult;
    use moor_common::tasks::Event;
    #[cfg(feature = "trace_events")]
    use moor_common::util::{EventPhase, TraceFile};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_kernel::config::{Config, ImportExportFormat};
    use moor_kernel::tasks::NoopTasksDb;
    use moor_kernel::tasks::scheduler::Scheduler;
    use moor_textdump::textdump_load;
    use moor_var::{Obj, SYSTEM_OBJECT};
    use rpc_common::{AuthToken, ClientToken};
    use rusty_paseto::prelude::Key;
    use semver::Version;

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

    /// Wait for an event with content matching the given predicate
    ///
    /// Searches through events for the specified player and calls the predicate on each event.
    /// Returns when the predicate returns true for any event, or panics on timeout.
    fn wait_for_event_content<F>(
        event_log: &MockEventLog,
        player: Obj,
        predicate: F,
        timeout_secs: u64,
        description: &str,
    ) where
        F: Fn(&moor_common::tasks::Event) -> bool,
    {
        let start_time = Instant::now();
        loop {
            if start_time.elapsed() > Duration::from_secs(timeout_secs) {
                panic!(
                    "No expected {} after {}s",
                    description,
                    start_time.elapsed().as_secs_f32()
                );
            }

            let events = event_log.get_all_events();
            for e in &events {
                if e.player != player {
                    continue;
                }
                match &e.event.event {
                    Event::Traceback(e) => {
                        panic!("Received exception during {description}: {e:?}");
                    }
                    _ => {
                        if predicate(&e.event.event) {
                            return;
                        }
                    }
                }
            }

            // Small sleep to avoid busy polling
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Send a command and wait for output containing the expected text
    #[allow(clippy::too_many_arguments)]
    fn send_command_and_wait_for_output(
        env: &TestEnvironment,
        client_id: Uuid,
        client_token: &ClientToken,
        auth_token: &AuthToken,
        player_obj: Obj,
        command: &str,
        expected_output: &str,
        description: &str,
    ) {
        let message = rpc_common::HostClientToDaemonMessage::Command(
            client_token.clone(),
            auth_token.clone(),
            player_obj,
            command.to_string(),
        );

        let result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            message,
        );

        assert!(
            result.is_ok(),
            "{command} command should succeed: {result:?}"
        );

        wait_for_event_content(
            &env.event_log,
            player_obj,
            |event| {
                if let Event::Notify(content, _) = event {
                    if let Some(str) = content.as_string() {
                        str.contains(expected_output)
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            5,
            description,
        );
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
        let jhcore = manifest_dir.join("../../cores/JHCore-DEV-2.db");

        let mut loader = db.loader_client().unwrap();
        let config = Config::default();
        textdump_load(
            loader.as_mut(),
            jhcore,
            Version::new(0, 1, 0),
            config.features.compile_options(),
        )
        .expect("Failed to load textdump");
        assert!(matches!(loader.commit(), Ok(CommitResult::Success { .. })));

        (db, temp_dir)
    }

    struct TestEnvironment {
        message_handler: Arc<dyn crate::rpc::MessageHandler>,
        transport: Arc<MockTransport>,
        event_log: Arc<MockEventLog>,
        scheduler_client: moor_kernel::SchedulerClient,
        kill_switch: Arc<AtomicBool>,
        _temp_dir: TempDir,
        _temp_output_dir: Option<TempDir>,
        output_dir_path: Option<PathBuf>,
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

    fn setup_test_environment_with_export_format(
        export_format: ImportExportFormat,
    ) -> TestEnvironment {
        // Set up tracing to capture scheduler logs
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        let (public_key, private_key) = create_test_keys();

        // Create a config with a proper output path for textdump
        let temp_output_dir = tempfile::tempdir().expect("Failed to create temp output dir");
        let output_path = temp_output_dir.path().to_path_buf();

        let mut config = Config::default();
        config.import_export.output_path = Some(output_path.clone());
        config.import_export.export_format = export_format;
        // Enable anonymous objects for GC tests
        config.features = Arc::new(moor_kernel::config::FeaturesConfig {
            anonymous_objects: true,
            ..config.features.as_ref().clone()
        });
        let config = Arc::new(config);

        // Create real database with core
        let (db, temp_dir) = setup_test_db_with_core();

        // Create kill switch
        let kill_switch = Arc::new(AtomicBool::new(false));

        // Create mock components
        let connections = ConnectionRegistryFactory::in_memory_only().unwrap();
        let transport = Arc::new(MockTransport::new());
        let event_log = Arc::new(MockEventLog::new());

        // Create RpcServer with MockTransport - this will be the single source of truth!
        let (rpc_server, task_monitor, system_control) = RpcServer::new(
            kill_switch.clone(),
            public_key,
            private_key,
            connections,
            event_log.clone(),
            transport.clone(),
            config.clone(),
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
                    eprintln!("RPC server request loop error: {e:?}");
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
            _temp_output_dir: Some(temp_output_dir),
            output_dir_path: Some(output_path),
            scheduler_thread: Some(scheduler_thread),
            rpc_thread: Some(rpc_thread),
        }
    }

    fn setup_test_environment_with_real_scheduler() -> TestEnvironment {
        setup_test_environment_with_export_format(ImportExportFormat::Textdump)
    }

    #[test]
    fn test_real_scheduler_startup() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None; // Don't need output dir for this test
        env.output_dir_path = None;

        // Wait for scheduler to be ready by attempting a simple operation
        wait_for_scheduler_ready(&env.scheduler_client);
    }

    #[test]
    fn test_connection_establishment_with_real_db() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None; // Don't need output dir for this test
        env.output_dir_path = None;
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // Test establishing a connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
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
            other => panic!("Expected NewConnection, got {other:?}"),
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
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None; // Don't need output dir for this test
        env.output_dir_path = None;
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // First establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
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
            "Welcome message call should succeed: {welcome_result:?}"
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
        let _login_success = env.transport.wait_for_condition(
            |transport| {
                // Look for completion events or specific narrative events indicating login success
                let narrative_events = transport.get_narrative_events();
                let client_events = transport.get_client_events();

                // Login succeeded if we got any narrative events OR client events indicating completion
                !narrative_events.is_empty() || !client_events.is_empty()
            },
            5000,
        );

        // Collect debugging information if login fails
        if login_result.is_err() {
            // Get all events from MockEventLog
            let all_events = env.event_log.get_all_events();
            let narrative_events = env.transport.get_narrative_events();
            let client_replies = env.transport.get_client_replies();
            let client_events = env.transport.get_client_events();
            let host_events = env.transport.get_host_events();

            let recent_events = env.event_log.get_all_events();

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

        // Should be LoginResult(Some(  with a ComnnectType Connected, and a objid 2
        let login_result = login_result.expect("Bad login result");
        let rpc_common::DaemonToClientReply::LoginResult(Some((
            auth_token,
            connect_type,
            player_obj,
        ))) = login_result
        else {
            // Get debugging information for unexpected results too
            let all_events = env.event_log.get_all_events();
            let narrative_events = env.transport.get_narrative_events();
            let client_replies = env.transport.get_client_replies();

            panic!(
                "Unexpected login result: {:?}\n\nMockEventLog events ({} total):\n{:#?}\n\nMockTransport narrative events ({} total):\n{:#?}\n\nMockTransport client replies ({} total):\n{:#?}",
                login_result,
                all_events.len(),
                all_events,
                narrative_events.len(),
                narrative_events,
                client_replies.len(),
                client_replies
            );
        };
        assert_eq!(
            connect_type,
            rpc_common::ConnectType::Connected,
            "Expected connected type"
        );
        assert!(player_obj.id().0 > 0, "Expected valid player object ID");

        // Verify player object is tracked in connections
        let connections = env.message_handler.get_connections();
        assert!(
            connections.contains(&player_obj),
            "Player object should be tracked in connections"
        );

        // Player should be #2
        assert_eq!(player_obj, Obj::mk_id(2));

        // Now we should keep polling received events until we see at lease part of:
        // *** Connected ***
        // Before going anywhere, you might want to describe yourself; type `help describe' for information.
        // #$#mcp version: 2.1 to: 2.1
        // The First Room
        // This is all there is right now.
        // Your previous connection was before we started keeping track.
        wait_for_event_content(
            &env.event_log,
            player_obj,
            |event| {
                if let Event::Notify(content, _) = event {
                    if let Some(str) = content.as_string() {
                        str == "This is all there is right now."
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            5,
            "connection events with room description",
        );

        // Send "@who" command to verify the logged-in player appears in the listing
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@who",
            "Wizard",
            "@who command output with wizard player listing",
        );

        // Send @create $thing named "my thing" command
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@create $thing named \"my thing\"",
            "You now have my thing with object number",
            "@create command output confirming object creation",
        );

        // Send "drop my thing" command
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "drop my thing",
            "You drop the my thing",
            "drop command output",
        );

        // Send @describe my thing command
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@describe my thing as \"A thing that's thingly.\"",
            "Description set",
            "@describe command output",
        );

        // Send @audit command to verify object ownership
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@audit",
            "my thing",
            "@audit command output showing owned objects",
        );

        // Send MOO expression to verify max_object().name
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            ";max_object().name == \"my thing\"",
            "=> 1",
            "MOO expression verification",
        );

        // Send say command
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "say Why hello there...",
            "You say, \"Why hello there...\"",
            "say command output",
        );

        // Verify no tracebacks in the event log
        let events = env.event_log.get_all_events();
        for event in events {
            if let Event::Traceback(traceback) = event.event.event {
                panic!("Unexpected traceback: {traceback:?}");
            }
        }
    }

    #[test]
    fn test_checkpoint_functionality_textdump() {
        test_checkpoint_functionality_impl(ImportExportFormat::Textdump);
    }

    #[test]
    fn test_checkpoint_functionality_objdef() {
        test_checkpoint_functionality_impl(ImportExportFormat::Objdef);
    }

    fn test_checkpoint_functionality_impl(export_format: ImportExportFormat) {
        let env = setup_test_environment_with_export_format(export_format.clone());
        wait_for_scheduler_ready(&env.scheduler_client);

        // Step 1: Verify scheduler is running and responsive
        assert!(
            env.scheduler_client.check_status().is_ok(),
            "Scheduler should be responsive before checkpoint"
        );

        // Step 2: Request a blocking checkpoint from the scheduler
        let checkpoint_result = env.scheduler_client.request_checkpoint_blocking();

        // Step 3: Verify checkpoint completed successfully
        assert!(
            checkpoint_result.is_ok(),
            "Blocking checkpoint should succeed: {checkpoint_result:?}"
        );

        // Step 4: Verify scheduler is still responsive after checkpoint
        assert!(
            env.scheduler_client.check_status().is_ok(),
            "Scheduler should remain responsive after checkpoint"
        );

        // Step 5: Verify that the database is still functional by establishing a connection
        let client_id = Uuid::new_v4();
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        assert!(
            establish_result.is_ok(),
            "Database operations should work after checkpoint: {establish_result:?}"
        );

        // Step 6: Verify the connection was established properly
        match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => {
                assert!(!token.0.is_empty(), "Should receive valid client token");
                assert!(obj.id().0 < 0, "Should receive valid connection object");
            }
            other => panic!("Expected NewConnection, got {other:?}"),
        }

        let output_dir = env.output_dir_path.as_ref().unwrap();
        // Since we used blocking checkpoint, the file should already exist
        let entries =
            std::fs::read_dir(output_dir).expect("Should be able to read output directory");

        let mut export_files: Vec<_> = entries
            .flatten()
            .filter(|entry| {
                if let Some(filename) = entry.file_name().to_str() {
                    // Handle both textdump and objdef formats
                    if export_format == ImportExportFormat::Textdump {
                        filename.starts_with("textdump-")
                            && filename.ends_with(".moo-textdump")
                            && !filename.contains(".in-progress")
                    } else {
                        filename.starts_with("textdump-")
                            && filename.ends_with(".moo")
                            && !filename.contains(".in-progress")
                    }
                } else {
                    false
                }
            })
            .collect();

        let format_name = if export_format == ImportExportFormat::Textdump {
            "Textdump"
        } else {
            "Objdef"
        };
        assert!(
            !export_files.is_empty(),
            "{} file should exist after blocking checkpoint in directory: {}",
            format_name,
            output_dir.display()
        );

        // Get the most recent file (there should be exactly one from our checkpoint)
        export_files.sort_by_key(|entry| entry.metadata().unwrap().modified().unwrap());
        let export_path = export_files.last().unwrap().path();

        // Verify the file has content (JHCore should produce a non-empty export)
        let metadata =
            std::fs::metadata(&export_path).expect("Should be able to read export file metadata");
        assert!(
            metadata.len() > 1000, // JHCore export should be much larger than 1KB
            "{} file should have substantial content, got {} bytes: {}",
            format_name,
            metadata.len(),
            export_path.display()
        );

        // Step 8: Verify there are no errors in the event log
        let events = env.event_log.get_all_events();
        for event in events {
            if let Event::Traceback(traceback) = event.event.event {
                panic!("Unexpected traceback after checkpoint: {traceback:?}");
            }
        }
    }

    #[test]
    fn test_gc_collect_builtin() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None; // Don't need output dir for this test
        env.output_dir_path = None;
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // First establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Welcome message call
        let welcome_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec![], // Empty args for welcome
            do_attach: false,
        };

        let _welcome_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            welcome_message,
        );

        // Wait for welcome message
        assert!(
            env.transport.wait_for_narrative_events(1, 2000),
            "Should receive welcome message events within 2 seconds"
        );

        // Login as wizard
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "wizard".to_string()],
            do_attach: true,
        };

        let login_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            login_message,
        );

        // Wait for login task completion
        let _login_success = env.transport.wait_for_condition(
            |transport| {
                let narrative_events = transport.get_narrative_events();
                let client_events = transport.get_client_events();
                !narrative_events.is_empty() || !client_events.is_empty()
            },
            5000,
        );

        let rpc_common::DaemonToClientReply::LoginResult(Some((
            auth_token,
            connect_type,
            player_obj,
        ))) = login_result.expect("Login should succeed")
        else {
            panic!("Expected successful login result");
        };

        assert_eq!(connect_type, rpc_common::ConnectType::Connected);
        assert_eq!(player_obj, Obj::mk_id(2));

        // Wait for connection events
        wait_for_event_content(
            &env.event_log,
            player_obj,
            |event| {
                if let Event::Notify(content, _) = event {
                    if let Some(str) = content.as_string() {
                        str == "This is all there is right now."
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            5,
            "connection events with room description",
        );

        // Get initial GC stats before running collection
        let initial_stats = env
            .scheduler_client
            .get_gc_stats()
            .expect("Should be able to get GC stats");
        let initial_count = initial_stats.cycle_count;

        // Test gc_collect() builtin - should trigger GC cycle and return nil
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            ";gc_collect()",
            "=> 0", // Should return nil (0)
            "gc_collect() builtin execution",
        );

        // Verify scheduler is still responsive after GC
        assert!(
            env.scheduler_client.check_status().is_ok(),
            "Scheduler should remain responsive after GC"
        );

        // Verify GC counter incremented
        let final_stats = env
            .scheduler_client
            .get_gc_stats()
            .expect("Should be able to get GC stats after GC");
        assert_eq!(
            final_stats.cycle_count,
            initial_count + 1,
            "GC cycle count should increment from {} to {}",
            initial_count,
            final_stats.cycle_count
        );

        // Test that non-wizard cannot call gc_collect()
        // First create a regular player by connecting as a different user
        let non_wizard_client_id = Uuid::new_v4();

        let establish_message_2 = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8081".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
        };

        let establish_result_2 = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            non_wizard_client_id,
            establish_message_2,
        );

        let (client_token_2, _connection_obj_2) = match establish_result_2.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // For non-wizard test, we'll connect as a guest (which should be non-wizard)
        let welcome_message_2 = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token_2.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec![],
            do_attach: false,
        };

        let _welcome_result_2 = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            non_wizard_client_id,
            welcome_message_2,
        );

        // Try to connect as guest
        let login_message_2 = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token_2.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "guest".to_string()],
            do_attach: true,
        };

        let login_result_2 = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            non_wizard_client_id,
            login_message_2,
        );

        // If guest login succeeds, test that gc_collect() fails with permission error
        if let Ok(rpc_common::DaemonToClientReply::LoginResult(Some((
            auth_token_2,
            _connect_type_2,
            player_obj_2,
        )))) = login_result_2
        {
            // Wait for guest connection to complete
            std::thread::sleep(Duration::from_millis(500));

            // Try gc_collect() as non-wizard - should get permission error
            let message_2 = rpc_common::HostClientToDaemonMessage::Command(
                client_token_2.clone(),
                auth_token_2.clone(),
                player_obj_2,
                ";gc_collect()".to_string(),
            );

            let _result_2 = env.transport.process_client_message(
                env.message_handler.as_ref(),
                env.scheduler_client.clone(),
                non_wizard_client_id,
                message_2,
            );

            // Wait for and verify permission error
            wait_for_event_content(
                &env.event_log,
                player_obj_2,
                |event| {
                    if let Event::Notify(content, _) = event {
                        if let Some(str) = content.as_string() {
                            str.contains("Permission denied") || str.contains("E_PERM")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                5,
                "permission error for non-wizard gc_collect() call",
            );
        }

        // Test direct GC calls via scheduler client to verify counter increments properly
        let current_count = final_stats.cycle_count;

        // Request another GC cycle directly via scheduler client
        env.scheduler_client
            .request_gc()
            .expect("Direct GC request should succeed");

        // Verify counter incremented again
        let direct_gc_stats = env
            .scheduler_client
            .get_gc_stats()
            .expect("Should be able to get GC stats after direct GC");
        assert_eq!(
            direct_gc_stats.cycle_count,
            current_count + 1,
            "GC cycle count should increment from {} to {} after direct GC request",
            current_count,
            direct_gc_stats.cycle_count
        );

        // Request one more GC to verify it keeps working
        env.scheduler_client
            .request_gc()
            .expect("Second direct GC request should succeed");

        let final_direct_stats = env
            .scheduler_client
            .get_gc_stats()
            .expect("Should be able to get final GC stats");
        assert_eq!(
            final_direct_stats.cycle_count,
            current_count + 2,
            "GC cycle count should increment to {} after multiple direct requests",
            current_count + 2
        );

        // Verify no unexpected tracebacks in the event log
        let events = env.event_log.get_all_events();
        for event in events {
            if let Event::Traceback(traceback) = event.event.event {
                // Only panic if it's not a permission-related traceback
                let traceback_str = format!("{traceback:?}");
                if !traceback_str.contains("E_PERM") && !traceback_str.contains("Permission denied")
                {
                    panic!("Unexpected traceback during GC test: {traceback:?}");
                }
            }
        }
    }

    #[cfg(feature = "trace_events")]
    #[test]
    fn test_chrome_trace_events_round_trip() {
        // Initialize trace events system before setting up the test environment
        assert!(
            moor_kernel::init_tracing(None),
            "Should be able to initialize trace events"
        );

        let env = setup_test_environment_with_real_scheduler();
        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();

        // Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
            connection_attributes: None,
        };

        let establish_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Welcome message call
        let welcome_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec![],
            do_attach: false,
        };

        let _welcome_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            welcome_message,
        );

        // Wait for welcome message
        assert!(
            env.transport.wait_for_narrative_events(1, 2000),
            "Should receive welcome message events within 2 seconds"
        );

        // Login as wizard
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "wizard".to_string()],
            do_attach: true,
        };

        let login_result = env.transport.process_client_message(
            env.message_handler.as_ref(),
            env.scheduler_client.clone(),
            client_id,
            login_message,
        );

        let rpc_common::DaemonToClientReply::LoginResult(Some((
            auth_token,
            connect_type,
            player_obj,
        ))) = login_result.expect("Login should succeed")
        else {
            panic!("Expected successful login result");
        };

        assert_eq!(connect_type, rpc_common::ConnectType::Connected);
        assert_eq!(player_obj, Obj::mk_id(2));

        // Wait for connection events
        wait_for_event_content(
            &env.event_log,
            player_obj,
            |event| {
                if let Event::Notify(content, _) = event {
                    if let Some(str) = content.as_string() {
                        str == "This is all there is right now."
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            5,
            "connection events with room description",
        );

        // Execute several commands that should generate trace events
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@create $thing named \"test object\"",
            "You now have test object with object number",
            "@create command execution",
        );

        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            ";max_object().name",
            "=> \"test object\"",
            "MOO expression execution",
        );

        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            player_obj,
            "@audit",
            "test object",
            "@audit command execution",
        );

        // Give the trace events system time to flush events
        std::thread::sleep(Duration::from_millis(500));

        // Shut down tracing to force final flush
        moor_kernel::shutdown_tracing();

        // Give additional time for shutdown to complete
        std::thread::sleep(Duration::from_millis(200));

        // Find trace files in the current directory
        let current_dir = std::env::current_dir().expect("Should be able to get current directory");
        let entries =
            std::fs::read_dir(&current_dir).expect("Should be able to read current directory");

        let mut trace_files: Vec<_> = entries
            .flatten()
            .filter(|entry| {
                if let Some(filename) = entry.file_name().to_str() {
                    filename == "moor_trace.json" // The default filename from init_tracing
                } else {
                    false
                }
            })
            .collect();

        assert!(
            !trace_files.is_empty(),
            "Should find Chrome trace files in directory: {}",
            current_dir.display()
        );

        // Get the most recent trace file
        trace_files.sort_by_key(|entry| entry.metadata().unwrap().modified().unwrap());
        let trace_path = trace_files.last().unwrap().path();

        // Read and parse the trace file
        let trace_content =
            std::fs::read_to_string(&trace_path).expect("Should be able to read trace file");

        let trace_file: TraceFile = serde_json::from_str(&trace_content)
            .expect("Trace file should be valid JSON matching TraceFile format");

        // Verify basic structure
        assert!(
            !trace_file.trace_events.is_empty(),
            "Trace file should contain events"
        );

        // Find different types of events we expect
        let mut task_create_events = 0;
        let mut task_start_events = 0;
        let mut task_complete_events = 0;
        let mut verb_begin_events = 0;
        let mut verb_end_events = 0;
        let mut builtin_begin_events = 0;
        let mut builtin_end_events = 0;

        for event in &trace_file.trace_events {
            match event.name.as_str() {
                "Task Create" => task_create_events += 1,
                "Task Execution" => {
                    // Task Execution can be both Begin and End phases
                    match event.ph {
                        EventPhase::Begin => task_start_events += 1,
                        EventPhase::End => task_complete_events += 1,
                        _ => {} // Other phases
                    }
                }
                // Skip metadata and special event types, count function/verb execution events
                name if matches!(event.ph, EventPhase::Begin | EventPhase::End)
                    && name != "Task Execution"
                    && name != "process_name"
                    && name != "Stack Unwind" =>
                {
                    // This is likely a verb or builtin function
                    if name.chars().any(|c| c == ':') {
                        // MOO verb name format (object:verb)
                        match event.ph {
                            EventPhase::Begin => verb_begin_events += 1,
                            EventPhase::End => verb_end_events += 1,
                            _ => {}
                        }
                    } else {
                        // Builtin function or other executable
                        match event.ph {
                            EventPhase::Begin => builtin_begin_events += 1,
                            EventPhase::End => builtin_end_events += 1,
                            _ => {}
                        }
                    }
                }
                _ => {} // Other event types
            }

            // Verify each event has required Chrome trace format fields
            assert!(event.pid > 0, "Event should have valid process ID");
            // tid can be 0 for task_id 0, so just check it's not u64::MAX or something invalid
            // ph is an enum so it's always valid
            assert!(!event.name.is_empty(), "Event should have name");
        }

        // Verify we captured the expected types of events
        assert!(
            task_create_events > 0,
            "Should have captured task creation events"
        );
        assert!(
            task_start_events > 0,
            "Should have captured task start events"
        );
        assert!(
            task_complete_events > 0,
            "Should have captured task completion events"
        );

        // We should have captured either verb events or builtin events (or both)
        let total_execution_events =
            verb_begin_events + verb_end_events + builtin_begin_events + builtin_end_events;
        assert!(
            total_execution_events > 0,
            "Should have captured some execution events (verbs or builtins)"
        );
        assert!(
            builtin_begin_events > 0,
            "Should have captured builtin begin events"
        );

        // Verify event pairing - each begin should have a corresponding end
        // (allowing for substantial variance due to async nature and shutdown timing)
        let verb_diff = (verb_begin_events as i32 - verb_end_events as i32).abs();
        let builtin_diff = (builtin_begin_events as i32 - builtin_end_events as i32).abs();

        // We allow up to 50% imbalance since some functions may not complete before shutdown
        let max_verb_imbalance = std::cmp::max(1, verb_begin_events / 2);
        let max_builtin_imbalance = std::cmp::max(1, builtin_begin_events / 2);

        if verb_begin_events > 0 || verb_end_events > 0 {
            assert!(
                verb_diff as usize <= max_verb_imbalance,
                "Verb begin/end events should be somewhat balanced: {verb_begin_events} begin, {verb_end_events} end (diff: {verb_diff})"
            );
        }

        if builtin_begin_events > 0 || builtin_end_events > 0 {
            assert!(
                builtin_diff as usize <= max_builtin_imbalance,
                "Builtin begin/end events should be somewhat balanced: {builtin_begin_events} begin, {builtin_end_events} end (diff: {builtin_diff})"
            );
        }

        // Verify we have at least some duration events (verb execution)
        let duration_events = trace_file
            .trace_events
            .iter()
            .filter(|e| matches!(e.ph, EventPhase::Begin | EventPhase::End))
            .count();
        assert!(duration_events > 0, "Should have captured duration events");

        // Verify trace file size is reasonable (should contain substantial data)
        let file_size = std::fs::metadata(&trace_path)
            .expect("Should be able to get trace file metadata")
            .len();
        assert!(
            file_size > 1000,
            "Trace file should be substantial size, got {} bytes: {}",
            file_size,
            trace_path.display()
        );

        // Clean up trace file
        let _ = std::fs::remove_file(&trace_path);

        // Verify no unexpected tracebacks in the event log
        let events = env.event_log.get_all_events();
        for event in events {
            if let Event::Traceback(traceback) = event.event.event {
                panic!("Unexpected traceback during trace events test: {traceback:?}");
            }
        }
    }
}
