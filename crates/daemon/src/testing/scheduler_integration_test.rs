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
        test_object: moor_var::Obj,
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
        let config = Arc::new(config);

        // Create real database with core
        let (db, temp_dir) = setup_test_db_with_core();

        // Create a test object in the database before starting scheduler
        let test_object = {
            let mut ws = db.new_world_state().expect("Should get world state");
            let wizard = moor_var::Obj::mk_id(2);
            let thing_parent = moor_var::Obj::mk_id(5); // $thing

            let test_obj = ws
                .create_object(
                    &wizard,                           // perms - wizard can create objects
                    &thing_parent,                     // parent - $thing (#5)
                    &wizard,                           // owner - wizard owns it
                    moor_common::util::BitEnum::new(), // flags - no special flags
                    None,                              // id - let system assign
                )
                .expect("Should be able to create test object");

            // Set a name property
            ws.update_property(
                &wizard,                         // perms
                &test_obj,                       // object
                moor_var::Symbol::mk("name"),    // property name
                &moor_var::v_str("test object"), // value
            )
            .expect("Should be able to set name");

            ws.commit()
                .expect("Should be able to commit object creation");
            test_obj
        };

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
            test_object,
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
    fn test_command_suggestions_object_actions() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None;
        env.output_dir_path = None;

        wait_for_scheduler_ready(&env.scheduler_client);

        let wizard = Obj::mk_id(2);

        let suggestions = env
            .scheduler_client
            .get_command_suggestions(
                &wizard,
                Some(&env.test_object),
                moor_kernel::tasks::SuggestionMode::ObjectActions,
                10,
            )
            .expect("Should be able to get command suggestions");

        // Verify we got action suggestions
        assert!(
            !suggestions.action_suggestions.is_empty(),
            "Should have action suggestions for test object"
        );

        // Verify all suggestions have proper structure
        for action in &suggestions.action_suggestions {
            assert!(
                !action.verb.as_string().is_empty(),
                "Verb should not be empty"
            );

            // For object actions, the target object should be the direct object (if any)
            if action.dobj.is_some() {
                assert_eq!(
                    action.dobj.unwrap(),
                    env.test_object,
                    "Direct object should be test object"
                );
                assert!(
                    action.dobjstr.is_some(),
                    "Should have display string for direct object"
                );
            }
        }

        // Verify context is correct
        assert_eq!(
            suggestions.suggestion_context,
            moor_kernel::tasks::SuggestionContext::ObjectActions(env.test_object)
        );

        // Should not have other suggestion types for ObjectActions mode
        assert!(suggestions.verb_suggestions.is_empty());
        assert!(suggestions.object_suggestions.is_empty());
        assert!(suggestions.builtin_suggestions.is_empty());

        // Verify we get expected object-related verbs
        let verb_names: Vec<String> = suggestions
            .action_suggestions
            .iter()
            .map(|action| action.verb.as_string())
            .collect();

        // Things should have basic verbs like look/examine/take
        assert!(
            !verb_names.is_empty(),
            "Should have some verbs, got: {:?}",
            verb_names
        );

        // If we have verbs, verify they're structured properly
        if let Some(first_action) = suggestions.action_suggestions.first() {
            // Verify the action has proper structure
            assert!(
                !first_action.verb.as_string().is_empty(),
                "Verb should not be empty"
            );

            if first_action.dobj.is_some() {
                assert_eq!(
                    first_action.dobj.unwrap(),
                    env.test_object,
                    "Should target test object"
                );
                assert!(
                    first_action.dobjstr.is_some(),
                    "Should have object name for display"
                );

                let object_name = first_action.dobjstr.as_ref().unwrap();
                assert!(!object_name.is_empty(), "Object name should not be empty");
            }
        }
    }

    #[test]
    fn test_command_suggestions_environment_actions() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None;
        env.output_dir_path = None;

        wait_for_scheduler_ready(&env.scheduler_client);

        let wizard = Obj::mk_id(2);

        let suggestions = env
            .scheduler_client
            .get_command_suggestions(
                &wizard,
                None, // No specific object - get environment actions
                moor_kernel::tasks::SuggestionMode::EnvironmentActions,
                20, // Higher limit to see more environmental verbs
            )
            .expect("Should be able to get environment action suggestions");

        // Verify we got action suggestions
        assert!(
            !suggestions.action_suggestions.is_empty(),
            "Should have environment action suggestions"
        );

        // Verify context is correct
        assert_eq!(
            suggestions.suggestion_context,
            moor_kernel::tasks::SuggestionContext::Environment
        );

        // Should not have other suggestion types for EnvironmentActions mode
        assert!(suggestions.verb_suggestions.is_empty());
        assert!(suggestions.object_suggestions.is_empty());
        assert!(suggestions.builtin_suggestions.is_empty());

        // Collect all verb names for analysis
        let verb_names: Vec<String> = suggestions
            .action_suggestions
            .iter()
            .map(|action| action.verb.as_string())
            .collect();

        // We should get verbs from:
        // 1. The player themselves (like 'inventory', 'look')
        // 2. The test object in inventory (inherited $thing verbs like 'get', 'take', 'drop')
        // 3. The room (like directional verbs or room-specific actions)

        // Verify we get some expected verbs that should be in any environment
        let has_look_verb = verb_names.iter().any(|v| v == "look" || v == "l");
        let has_thing_verbs = verb_names
            .iter()
            .any(|v| v == "get" || v == "take" || v == "drop");

        // At minimum we should have basic player verbs and inherited $thing verbs
        assert!(
            has_look_verb || has_thing_verbs,
            "Should have basic verbs like 'look' or thing verbs like 'get/take/drop'. Got: {:?}",
            verb_names
        );

        // Verify all suggestions have proper structure
        for action in &suggestions.action_suggestions {
            assert!(
                !action.verb.as_string().is_empty(),
                "Verb should not be empty"
            );

            // Each action should either have no object (like 'look') or a specific target
            if let Some(dobj) = action.dobj {
                assert!(
                    action.dobjstr.is_some(),
                    "Should have display string for direct object"
                );
                // The object should be either the wizard, the test object, or the room
                let obj_id = dobj.id().0;
                assert!(
                    obj_id == 2 || obj_id == env.test_object.id().0 || obj_id == 70, // wizard, test_object, or room
                    "Direct object should be wizard (#2), test object (#{}) or room (#70), got #{}",
                    env.test_object.id().0,
                    obj_id
                );
            }
        }
    }

    #[test]
    fn test_command_suggestions_limits() {
        let mut env = setup_test_environment_with_real_scheduler();
        env._temp_output_dir = None;
        env.output_dir_path = None;

        wait_for_scheduler_ready(&env.scheduler_client);

        let client_id = Uuid::new_v4();
        let wizard = Obj::mk_id(2);

        // Establish connection and login (same setup as first test)
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

        let (client_token, _) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "wizard".to_string()],
            do_attach: true,
        };

        let login_result = env
            .transport
            .process_client_message(
                env.message_handler.as_ref(),
                env.scheduler_client.clone(),
                client_id,
                login_message,
            )
            .expect("Login should succeed");

        let (auth_token, _, _) = match login_result {
            rpc_common::DaemonToClientReply::LoginResult(Some((
                auth_token,
                connect_type,
                player_obj,
            ))) => (auth_token, connect_type, player_obj),
            other => panic!("Expected successful login, got {other:?}"),
        };

        // Create a test object for limits testing
        send_command_and_wait_for_output(
            &env,
            client_id,
            &client_token,
            &auth_token,
            wizard,
            "@create $thing named \"limits test object\"",
            "You now have limits test object with object number",
            "@create limits test object",
        );

        // Use the test object created during environment setup
        let test_object = env.test_object;

        // Test with different limits
        let suggestions_3 = env
            .scheduler_client
            .get_command_suggestions(
                &wizard,
                Some(&test_object),
                moor_kernel::tasks::SuggestionMode::ObjectActions,
                3,
            )
            .expect("Should get suggestions with limit 3");

        let suggestions_10 = env
            .scheduler_client
            .get_command_suggestions(
                &wizard,
                Some(&test_object),
                moor_kernel::tasks::SuggestionMode::ObjectActions,
                10,
            )
            .expect("Should get suggestions with limit 10");

        // Verify limits are respected
        assert!(
            suggestions_3.action_suggestions.len() <= 3,
            "Should respect limit of 3"
        );
        assert!(
            suggestions_10.action_suggestions.len() <= 10,
            "Should respect limit of 10"
        );

        // If the room has more than 3 verbs, the 10-limit should get more suggestions
        if suggestions_10.action_suggestions.len() > 3 {
            assert!(
                suggestions_10.action_suggestions.len() > suggestions_3.action_suggestions.len(),
                "Higher limit should return more suggestions when available"
            );
        }

        // Verify the first N suggestions are the same (due to priority ordering)
        let min_len = suggestions_3
            .action_suggestions
            .len()
            .min(suggestions_10.action_suggestions.len());
        for i in 0..min_len {
            assert_eq!(
                suggestions_3.action_suggestions[i].verb, suggestions_10.action_suggestions[i].verb,
                "Priority ordering should be consistent"
            );
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

        println!(
            "✓ Blocking checkpoint completed successfully: {} ({} bytes)",
            export_path.display(),
            metadata.len()
        );

        // Step 8: Verify there are no errors in the event log
        let events = env.event_log.get_all_events();
        for event in events {
            if let Event::Traceback(traceback) = event.event.event {
                panic!("Unexpected traceback after checkpoint: {traceback:?}");
            }
        }
    }
}
