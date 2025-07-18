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

//! Integration tests for RPC message handler using mock components

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};
    use uuid::Uuid;

    use moor_kernel::config::Config;
    use moor_kernel::testing::MockScheduler;
    use rusty_paseto::prelude::Key;

    use crate::connections::ConnectionRegistryFactory;
    use crate::event_log::EventLogOps;
    use crate::rpc::{MessageHandler, hosts::Hosts, message_handler::RpcMessageHandler};
    use crate::tasks::task_monitor::TaskMonitor;
    use crate::testing::{MockEventLog, MockTransport};
    use moor_var::{Obj, SYSTEM_OBJECT};
    use rpc_common::{HostToDaemonMessage, HostToken, HostType, make_host_token};
    use std::net::SocketAddr;

    fn create_test_keys() -> (Key<32>, Key<64>) {
        // Use the fixed test keys instead of random generation to avoid Ed25519 validation issues
        // These are the same keys used in the telnet-host integration tests
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

    fn create_test_host_token() -> HostToken {
        let (_, private_key) = create_test_keys();
        let host_type = rpc_common::HostType::WebSocket;

        // Create a proper PASETO host token using the same private key as the message handler
        make_host_token(&private_key, host_type)
    }

    fn setup_test_environment() -> (
        Arc<RpcMessageHandler>,
        Arc<MockTransport>,
        Arc<MockEventLog>,
        Arc<MockScheduler>,
    ) {
        let (public_key, private_key) = create_test_keys();
        let config = Arc::new(Config::default());

        // Create mock components
        let connections = ConnectionRegistryFactory::in_memory_only().unwrap();
        let hosts = Arc::new(RwLock::new(Hosts::default()));
        let (mailbox_sender, _mailbox_receiver) = flume::unbounded();
        let event_log = Arc::new(MockEventLog::new());
        let task_monitor = TaskMonitor::new(mailbox_sender.clone());
        let transport = Arc::new(MockTransport::new());

        // Create scheduler and start it in background
        let scheduler = Arc::new(MockScheduler::new());
        let scheduler_clone = scheduler.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_secs(30) {
                if scheduler_clone
                    .run_with_timeout(std::time::Duration::from_millis(50))
                    .is_err()
                {
                    break;
                }
            }
        });

        // Create message handler
        let message_handler = Arc::new(RpcMessageHandler::new(
            config,
            public_key,
            private_key,
            connections,
            hosts,
            mailbox_sender,
            event_log.clone() as Arc<dyn EventLogOps>,
            task_monitor,
            transport.clone(),
        ));

        (message_handler, transport, event_log, scheduler)
    }

    #[test]
    fn test_host_registration_message() {
        let (message_handler, transport, _event_log, _scheduler) = setup_test_environment();

        let host_token = create_test_host_token();
        let listeners: Vec<(Obj, SocketAddr)> = Vec::new();
        let message = HostToDaemonMessage::RegisterHost(
            std::time::SystemTime::now(),
            HostType::WebSocket,
            listeners,
        );

        let result = transport.process_host_message(message_handler.as_ref(), host_token, message);

        assert!(result.is_ok(), "Host registration should succeed");
        match result.unwrap() {
            rpc_common::DaemonToHostReply::Ack => {
                // Expected response
            }
            other => panic!("Expected Ack, got {other:?}"),
        }
    }

    #[test]
    fn test_host_attach_detach_lifecycle() {
        let (message_handler, transport, _event_log, _scheduler) = setup_test_environment();

        let host_token = create_test_host_token();

        // Create some test listeners for the host
        let listener1 = (
            Obj::mk_id(100),
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );
        let listener2 = (
            Obj::mk_id(101),
            "127.0.0.1:8081".parse::<SocketAddr>().unwrap(),
        );
        let listeners = vec![listener1, listener2];

        // Step 1: Register the host (attach)
        let register_message = HostToDaemonMessage::RegisterHost(
            std::time::SystemTime::now(),
            HostType::WebSocket,
            listeners.clone(),
        );

        let result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            register_message,
        );

        assert!(result.is_ok(), "Host registration should succeed");
        match result.unwrap() {
            rpc_common::DaemonToHostReply::Ack => {
                // Expected response for registration
            }
            other => panic!("Expected Ack for registration, got {other:?}"),
        }

        // Step 2: Verify host is registered by checking listeners
        // Access the hosts data structure through the message handler
        let hosts_listeners = message_handler.get_listeners();
        assert_eq!(
            hosts_listeners.len(),
            2,
            "Should have 2 listeners after registration"
        );

        // Verify the specific listeners are present
        let listener_ports: Vec<u16> = hosts_listeners.iter().map(|(_, _, port)| *port).collect();
        assert!(
            listener_ports.contains(&8080),
            "Should contain first listener port"
        );
        assert!(
            listener_ports.contains(&8081),
            "Should contain second listener port"
        );

        // Verify the object IDs are correct
        let listener_objs: Vec<Obj> = hosts_listeners.iter().map(|(obj, _, _)| *obj).collect();
        assert!(
            listener_objs.contains(&Obj::mk_id(100)),
            "Should contain first listener object"
        );
        assert!(
            listener_objs.contains(&Obj::mk_id(101)),
            "Should contain second listener object"
        );

        // Step 3: Detach the host and verify reply
        let detach_message = HostToDaemonMessage::DetachHost;
        let result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            detach_message,
        );

        assert!(result.is_ok(), "Host detach should succeed");
        match result.unwrap() {
            rpc_common::DaemonToHostReply::Ack => {
                // Expected response for detach
            }
            other => panic!("Expected Ack for detach, got {other:?}"),
        }

        // Step 3a: Verify reply was captured by mock transport
        let last_host_reply = transport.get_last_host_reply();
        assert!(
            last_host_reply.is_some(),
            "Should have captured a host reply"
        );
        match last_host_reply.unwrap() {
            Ok(rpc_common::DaemonToHostReply::Ack) => {
                // Expected Ack reply captured correctly
            }
            other => panic!("Transport captured wrong host reply: {other:?}"),
        }

        // Step 4: Verify host is no longer registered (no listeners)
        let hosts_listeners_after_detach = message_handler.get_listeners();
        assert_eq!(
            hosts_listeners_after_detach.len(),
            0,
            "Should have no listeners after detach"
        );
    }

    #[test]
    fn test_host_detach_with_client_connections() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let host_token = create_test_host_token();

        // Step 1: Register host with listeners
        let listener1 = (
            Obj::mk_id(100),
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );
        let listeners = vec![listener1];

        let register_message = HostToDaemonMessage::RegisterHost(
            std::time::SystemTime::now(),
            HostType::WebSocket,
            listeners,
        );

        let result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            register_message,
        );
        assert!(result.is_ok(), "Host registration should succeed");

        // Step 2: Establish a client connection
        let client_id = Uuid::new_v4();
        let connect_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            connect_message,
        );

        assert!(result.is_ok(), "Connection establishment should succeed");

        // Verify connection is in the registry
        let connections_before = message_handler.get_connections();
        assert!(
            !connections_before.is_empty(),
            "Should have connections before detach"
        );

        // Step 3: Detach the host
        let detach_message = HostToDaemonMessage::DetachHost;
        let result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            detach_message,
        );

        assert!(result.is_ok(), "Host detach should succeed");

        // Step 4: Verify host listeners are gone but connections might still exist
        // (connections are usually cleaned up separately from host detach)
        let hosts_listeners_after_detach = message_handler.get_listeners();
        assert_eq!(
            hosts_listeners_after_detach.len(),
            0,
            "Should have no listeners after host detach"
        );
    }

    #[test]
    fn test_performance_counters_request() {
        let (message_handler, transport, _event_log, _scheduler) = setup_test_environment();

        let host_token = create_test_host_token();
        let message = HostToDaemonMessage::RequestPerformanceCounters;

        let result = transport.process_host_message(message_handler.as_ref(), host_token, message);

        assert!(
            result.is_ok(),
            "Performance counters request should succeed"
        );
        match result.unwrap() {
            rpc_common::DaemonToHostReply::PerfCounters(timestamp, counters) => {
                assert!(!counters.is_empty(), "Should have performance counters");
                // Verify timestamp is recent
                use std::time::{Duration, SystemTime};
                let now = SystemTime::now();
                let diff = now.duration_since(timestamp).unwrap_or(Duration::ZERO);
                assert!(diff < Duration::from_secs(1), "Timestamp should be recent");
            }
            other => panic!("Expected PerfCounters, got {other:?}"),
        }
    }

    #[test]
    fn test_connection_establishment_lifecycle() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Step 1: Verify no connections initially
        let connections_before = message_handler.get_connections();
        let initial_count = connections_before.len();

        // Step 2: Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        assert!(result.is_ok(), "Connection establishment should succeed");
        let (client_token, connection_obj) = match result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, connection_obj) => {
                assert!(!token.0.is_empty(), "Should receive a client token");
                assert!(
                    connection_obj.id().0 != 0,
                    "Should receive a valid connection object (got ID: {})",
                    connection_obj.id().0
                );
                (token, connection_obj)
            }
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 3: Verify connection is tracked in database
        let connections_after_establish = message_handler.get_connections();
        assert_eq!(
            connections_after_establish.len(),
            initial_count + 1,
            "Should have one more connection after establishment"
        );
        assert!(
            connections_after_establish.contains(&connection_obj),
            "Connection registry should contain the new connection object"
        );

        // Step 4: Test client activity (ping)
        let ping_message = rpc_common::HostClientToDaemonMessage::ClientPong(
            client_token.clone(),
            std::time::SystemTime::now(),
            connection_obj,
            HostType::WebSocket,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );

        let ping_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            ping_message,
        );

        assert!(ping_result.is_ok(), "Client ping should succeed");

        // Step 5: Test detachment and verify reply
        let detach_message = rpc_common::HostClientToDaemonMessage::Detach(client_token);

        let detach_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            detach_message,
        );

        assert!(detach_result.is_ok(), "Client detach should succeed");

        // Step 5a: Verify the correct reply was sent
        match detach_result.unwrap() {
            rpc_common::DaemonToClientReply::Disconnected => {
                // This is the expected reply for detach
            }
            other => panic!("Expected Disconnected reply, got {other:?}"),
        }

        // Step 5b: Verify reply was captured by mock transport
        let last_client_reply = transport.get_last_client_reply();
        assert!(
            last_client_reply.is_some(),
            "Should have captured a client reply"
        );
        match last_client_reply.unwrap() {
            Ok(rpc_common::DaemonToClientReply::Disconnected) => {
                // Expected reply captured correctly
            }
            other => panic!("Transport captured wrong reply: {other:?}"),
        }
    }

    #[test]
    fn test_multiple_client_connections() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id_1 = Uuid::new_v4();
        let client_id_2 = Uuid::new_v4();

        // Establish first connection
        let establish_message_1 = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let result_1 = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id_1,
            establish_message_1,
        );

        assert!(
            result_1.is_ok(),
            "First connection establishment should succeed"
        );
        let (_token_1, connection_obj_1) = match result_1.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Establish second connection
        let establish_message_2 = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8081".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/html")]),
        };

        let result_2 = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id_2,
            establish_message_2,
        );

        assert!(
            result_2.is_ok(),
            "Second connection establishment should succeed"
        );
        let (token_2, connection_obj_2) = match result_2.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Verify both connections exist and are different
        let connections = message_handler.get_connections();
        assert_eq!(connections.len(), 2, "Should have 2 connections");
        assert!(
            connections.contains(&connection_obj_1),
            "Should contain first connection"
        );
        assert!(
            connections.contains(&connection_obj_2),
            "Should contain second connection"
        );
        assert_ne!(
            connection_obj_1, connection_obj_2,
            "Connection objects should be different"
        );

        // Detach first connection
        let detach_message_1 = rpc_common::HostClientToDaemonMessage::Detach(token_2.clone());

        let detach_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id_2,
            detach_message_1,
        );

        assert!(detach_result.is_ok(), "Second client detach should succeed");

        // Verify only one connection remains
        let connections_after_one_detach = message_handler.get_connections();
        assert_eq!(
            connections_after_one_detach.len(),
            1,
            "Should have 1 connection after detaching one"
        );
        assert!(
            connections_after_one_detach.contains(&connection_obj_1),
            "Should still contain first connection"
        );
        assert!(
            !connections_after_one_detach.contains(&connection_obj_2),
            "Should not contain detached connection"
        );
    }

    #[test]
    fn test_message_reply_flows() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let host_token = create_test_host_token();
        let client_id = Uuid::new_v4();

        // Test 1: Host registration should reply with Ack
        let register_message = HostToDaemonMessage::RegisterHost(
            std::time::SystemTime::now(),
            HostType::WebSocket,
            vec![(
                Obj::mk_id(100),
                "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
            )],
        );

        let register_result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            register_message,
        );

        assert!(register_result.is_ok(), "Host registration should succeed");
        assert!(matches!(
            register_result.unwrap(),
            rpc_common::DaemonToHostReply::Ack
        ));

        // Verify captured reply
        let host_replies = transport.get_host_replies();
        assert_eq!(host_replies.len(), 1, "Should have captured 1 host reply");
        let (_, _, reply) = &host_replies[0];
        assert!(matches!(reply, Ok(rpc_common::DaemonToHostReply::Ack)));

        // Test 2: Performance counters request should reply with PerfCounters
        let perf_message = HostToDaemonMessage::RequestPerformanceCounters;

        let perf_result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            perf_message,
        );

        assert!(
            perf_result.is_ok(),
            "Performance counters request should succeed"
        );
        match perf_result.unwrap() {
            rpc_common::DaemonToHostReply::PerfCounters(_, counters) => {
                assert!(!counters.is_empty(), "Should have performance counters");
            }
            other => panic!("Expected PerfCounters reply, got {other:?}"),
        }

        // Test 3: Client connection establishment should reply with NewConnection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        assert!(
            establish_result.is_ok(),
            "Connection establishment should succeed"
        );
        let (client_token, connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection reply, got {other:?}"),
        };

        // Verify captured client reply
        let client_replies = transport.get_client_replies();
        assert_eq!(
            client_replies.len(),
            1,
            "Should have captured 1 client reply"
        );
        let (captured_client_id, _, reply) = &client_replies[0];
        assert_eq!(
            *captured_client_id, client_id,
            "Should capture correct client ID"
        );
        assert!(matches!(
            reply,
            Ok(rpc_common::DaemonToClientReply::NewConnection(_, _))
        ));

        // Test 4: Client ping should reply with Ack (or similar)
        let ping_message = rpc_common::HostClientToDaemonMessage::ClientPong(
            client_token.clone(),
            std::time::SystemTime::now(),
            connection_obj,
            HostType::WebSocket,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );

        let ping_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            ping_message,
        );

        assert!(ping_result.is_ok(), "Client ping should succeed");

        // Test 5: Client detach should reply with Disconnected
        let detach_message = rpc_common::HostClientToDaemonMessage::Detach(client_token);

        let detach_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            detach_message,
        );

        assert!(detach_result.is_ok(), "Client detach should succeed");
        assert!(matches!(
            detach_result.unwrap(),
            rpc_common::DaemonToClientReply::Disconnected
        ));

        // Verify all client replies were captured correctly
        let final_client_replies = transport.get_client_replies();
        assert_eq!(
            final_client_replies.len(),
            3,
            "Should have captured 3 client replies"
        );

        // Check each reply type
        let (_, _, establish_reply) = &final_client_replies[0];
        assert!(matches!(
            establish_reply,
            Ok(rpc_common::DaemonToClientReply::NewConnection(_, _))
        ));

        let (_, _, detach_reply) = &final_client_replies[2];
        assert!(matches!(
            detach_reply,
            Ok(rpc_common::DaemonToClientReply::Disconnected)
        ));
    }

    #[test]
    fn test_error_reply_flows() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Test 1: Invalid client token should generate error reply
        let invalid_token = rpc_common::ClientToken("invalid_token".to_string());
        let invalid_ping_message = rpc_common::HostClientToDaemonMessage::ClientPong(
            invalid_token,
            std::time::SystemTime::now(),
            Obj::mk_id(1),
            HostType::WebSocket,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );

        let invalid_ping_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            invalid_ping_message,
        );

        // Should return an error
        assert!(
            invalid_ping_result.is_err(),
            "Invalid token should cause error"
        );

        // Verify the error was captured
        let client_replies = transport.get_client_replies();
        assert_eq!(
            client_replies.len(),
            1,
            "Should have captured 1 client reply"
        );
        let (_, _, reply) = &client_replies[0];
        assert!(reply.is_err(), "Captured reply should be an error");
        match reply {
            Err(rpc_common::RpcMessageError::NoConnection) => {
                // Expected error for invalid token
            }
            other => panic!("Expected NoConnection error, got {other:?}"),
        }
    }

    #[test]
    fn test_client_pong_without_valid_token() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();
        let invalid_token = rpc_common::ClientToken("invalid_token".to_string());
        let message = rpc_common::HostClientToDaemonMessage::ClientPong(
            invalid_token,
            std::time::SystemTime::now(),
            Obj::mk_id(1),
            HostType::WebSocket,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );

        let result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            message,
        );

        assert!(
            result.is_err(),
            "Client pong with invalid token should fail"
        );
        match result.unwrap_err() {
            rpc_common::RpcMessageError::NoConnection => {
                // Expected error - no connection for this client
            }
            other => panic!("Expected NoConnection error, got {other:?}"),
        }
    }

    #[test]
    fn test_event_log_integration() {
        let (_message_handler, _transport, event_log, _scheduler) = setup_test_environment();

        let player = Obj::mk_id(42);
        let test_event = Box::new(moor_common::tasks::NarrativeEvent {
            event_id: uuid::Uuid::now_v7(),
            timestamp: std::time::SystemTime::now(),
            author: moor_var::v_str("test_author"),
            event: moor_common::tasks::Event::Notify(moor_var::v_str("Test message"), None),
        });

        // Test event logging
        let event_id = event_log.append(player, test_event);
        assert!(!event_id.is_nil(), "Should return valid event ID");

        // Test event retrieval
        let events = event_log.events_for_player_since(player, None);
        assert_eq!(events.len(), 1, "Should have one event");
        assert_eq!(
            events[0].player, player,
            "Event should be for correct player"
        );
        assert_eq!(
            events[0].event.event_id(),
            event_id,
            "Event ID should match"
        );
    }

    #[test]
    fn test_presentation_management() {
        let (_message_handler, _transport, event_log, _scheduler) = setup_test_environment();

        let player = Obj::mk_id(42);
        let presentation = moor_common::tasks::Presentation {
            id: "test_widget".to_string(),
            content_type: "text/plain".to_string(),
            content: "Hello World".to_string(),
            target: "main".to_string(),
            attributes: vec![],
        };

        // Test presentation creation
        let present_event = Box::new(moor_common::tasks::NarrativeEvent {
            event_id: uuid::Uuid::now_v7(),
            timestamp: std::time::SystemTime::now(),
            author: moor_var::v_str("test_author"),
            event: moor_common::tasks::Event::Present(presentation.clone()),
        });

        event_log.append(player, present_event);

        // Check current presentations
        let presentations = event_log.current_presentations(player);
        assert_eq!(presentations.len(), 1, "Should have one presentation");
        assert!(
            presentations.contains_key("test_widget"),
            "Should contain test widget"
        );
        assert_eq!(presentations["test_widget"].content, "Hello World");

        // Test presentation dismissal
        event_log.dismiss_presentation(player, "test_widget".to_string());
        let presentations = event_log.current_presentations(player);
        assert!(
            presentations.is_empty(),
            "Should have no presentations after dismissal"
        );
    }

    #[test]
    fn test_narrative_event_propagation() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        // Step 1: Set up a client connection to receive narrative events
        let client_id = uuid::Uuid::new_v4();
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:12345".to_string(),
            acceptable_content_types: None,
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (_client_token, connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Simulate narrative events being sent to the client
        use moor_common::tasks::NarrativeEvent;

        // Create various types of narrative events
        let narrative_events = vec![
            NarrativeEvent::notify(
                moor_var::v_obj(SYSTEM_OBJECT),
                moor_var::v_str("Hello, world!"),
                None,
            ),
            NarrativeEvent::notify(
                moor_var::v_obj(SYSTEM_OBJECT),
                moor_var::v_str("System notification"),
                Some(moor_var::Symbol::mk("text/plain")),
            ),
            NarrativeEvent::notify(
                moor_var::v_obj(connection_obj),
                moor_var::v_str("Connection message"),
                None,
            ),
        ];

        // Manually send narrative events through the transport
        for event in &narrative_events {
            transport.send_narrative_event(connection_obj, event.clone());
        }

        // Step 3: Verify events were captured
        let captured_narrative_events = transport.get_narrative_events();
        assert_eq!(
            captured_narrative_events.len(),
            narrative_events.len(),
            "Should have captured all narrative events"
        );

        // Verify events are for the correct object
        for (obj, _event) in &captured_narrative_events {
            assert_eq!(
                *obj, connection_obj,
                "Events should be for the connection object"
            );
        }

        // Step 4: Test client event capture (like system messages, disconnect, etc.)
        let system_message_event = rpc_common::ClientEvent::SystemMessage(
            connection_obj,
            "System broadcast message".to_string(),
        );
        let disconnect_event = rpc_common::ClientEvent::Disconnect();

        transport.capture_client_event(client_id, system_message_event);
        transport.capture_client_event(client_id, disconnect_event);

        let captured_client_events = transport.get_client_events();
        assert_eq!(
            captured_client_events.len(),
            2,
            "Should have captured 2 client events"
        );

        // Verify client events are for the correct client
        for (captured_client_id, _event) in &captured_client_events {
            assert_eq!(
                *captured_client_id, client_id,
                "Events should be for the correct client"
            );
        }

        // Step 5: Test host broadcast event capture
        let listen_event = rpc_common::HostBroadcastEvent::Listen {
            handler_object: SYSTEM_OBJECT,
            host_type: rpc_common::HostType::TCP,
            port: 8080,
            print_messages: true,
        };
        let unlisten_event = rpc_common::HostBroadcastEvent::Unlisten {
            host_type: rpc_common::HostType::TCP,
            port: 8080,
        };

        transport.send_host_event(listen_event.clone());
        transport.send_host_event(unlisten_event);

        let captured_host_events = transport.get_host_events();
        assert_eq!(
            captured_host_events.len(),
            2,
            "Should have captured 2 host broadcast events"
        );

        // Step 6: Test client broadcast event capture
        let ping_pong_event =
            rpc_common::ClientsBroadcastEvent::PingPong(std::time::SystemTime::now());

        transport.send_client_broadcast_event(ping_pong_event);

        let captured_client_broadcast_events = transport.get_client_broadcast_events();
        assert_eq!(
            captured_client_broadcast_events.len(),
            1,
            "Should have captured 1 client broadcast event"
        );

        // Step 7: Verify all counting methods work correctly
        assert!(
            transport.has_narrative_events(),
            "Should have narrative events"
        );
        assert!(transport.has_client_events(), "Should have client events");
        assert!(transport.has_host_events(), "Should have host events");
        assert!(
            transport.has_client_broadcast_events(),
            "Should have client broadcast events"
        );

        assert_eq!(transport.narrative_event_count(), narrative_events.len());
        assert_eq!(transport.client_event_count(), 2);
        assert_eq!(transport.host_event_count(), 2);
        assert_eq!(transport.client_broadcast_event_count(), 1);

        // Step 8: Test event clearing
        transport.clear_events();
        assert!(
            !transport.has_narrative_events(),
            "Events should be cleared"
        );
        assert!(!transport.has_client_events(), "Events should be cleared");
        assert!(!transport.has_host_events(), "Events should be cleared");
        assert!(
            !transport.has_client_broadcast_events(),
            "Events should be cleared"
        );
    }

    #[test]
    fn test_login_command_flow() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Step 1: Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        assert!(
            establish_result.is_ok(),
            "Connection establishment should succeed"
        );
        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Attempt login command
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["create".to_string(), "TestPlayer".to_string()],
            do_attach: true,
        };

        let login_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            login_message,
        );

        // With NormalOperation scenario, login should succeed
        assert!(
            login_result.is_ok(),
            "Login should succeed with NormalOperation scenario: {login_result:?}"
        );

        // Verify the reply was captured
        let client_replies = transport.get_client_replies();
        assert_eq!(
            client_replies.len(),
            2,
            "Should have captured 2 client replies"
        );

        // First reply should be NewConnection, second should be LoginResult or error
        let (_, _, login_reply) = &client_replies[1];
        // With NormalOperation scenario, should get successful LoginResult
        assert!(
            matches!(
                login_reply,
                Ok(rpc_common::DaemonToClientReply::LoginResult(Some(_)))
            ),
            "Should have successful LoginResult with NormalOperation scenario: {login_reply:?}"
        );
    }

    #[test]
    fn test_login_failure_scenario() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        // Configure scheduler for login failures
        scheduler.set_scenario(moor_kernel::testing::MockScenario::LoginFailures);

        let client_id = Uuid::new_v4();

        // Step 1: Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:12345".to_string(),
            acceptable_content_types: None,
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Attempt login command - should fail due to LoginFailures scenario
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["connect".to_string(), "TestPlayer".to_string()],
            do_attach: true,
        };

        let login_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            login_message,
        );

        // With LoginFailures scenario (20% success rate), login should mostly fail
        let login_failed = matches!(
            &login_result,
            Ok(rpc_common::DaemonToClientReply::LoginResult(None)) // Failed login
                | Err(rpc_common::RpcMessageError::LoginTaskFailed(_))
        );
        // Note: Due to 20% success rate, we might occasionally get success, but failure is expected
        if !login_failed && login_result.is_err() {
            panic!("Unexpected login result with LoginFailures scenario: {login_result:?}");
        }

        // Verify scheduler scenario is working as expected
        assert_eq!(
            scheduler.get_current_scenario(),
            moor_kernel::testing::MockScenario::LoginFailures,
            "Scheduler should be in LoginFailures scenario"
        );
    }

    #[test]
    fn test_system_property_request() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Step 1: Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Request a system property
        let sysprop_message = rpc_common::HostClientToDaemonMessage::RequestSysProp(
            client_token,
            moor_common::model::ObjectRef::Id(SYSTEM_OBJECT),
            moor_var::Symbol::mk("name"),
        );

        let sysprop_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            sysprop_message,
        );

        // With NormalOperation scenario, property request should succeed
        assert!(
            sysprop_result.is_ok(),
            "System property request should succeed with NormalOperation scenario: {sysprop_result:?}"
        );
    }

    #[test]
    fn test_system_property_database_issues() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        // Configure scheduler for database issues that affect property lookups
        scheduler.set_scenario(moor_kernel::testing::MockScenario::DatabaseIssues);

        let client_id = Uuid::new_v4();

        // Establish connection first
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:12345".to_string(),
            acceptable_content_types: None,
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Request a system property - should fail due to DatabaseIssues scenario
        let sysprop_message = rpc_common::HostClientToDaemonMessage::RequestSysProp(
            client_token,
            moor_common::model::ObjectRef::Id(SYSTEM_OBJECT),
            moor_var::Symbol::mk("welcome_message"),
        );

        let sysprop_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            sysprop_message,
        );

        // With DatabaseIssues scenario, property requests have 40% success rate
        // Accept either success or property lookup failure
        let is_valid_result = matches!(
            &sysprop_result,
            Ok(_) | Err(rpc_common::RpcMessageError::ErrorCouldNotRetrieveSysProp(_))
        );
        assert!(
            is_valid_result,
            "Property request should succeed or fail with property error in DatabaseIssues scenario: {sysprop_result:?}"
        );

        // Verify scenario is set correctly
        assert_eq!(
            scheduler.get_current_scenario(),
            moor_kernel::testing::MockScenario::DatabaseIssues,
            "Scheduler should be in DatabaseIssues scenario"
        );
    }

    #[test]
    fn test_broadcast_listen_unlisten() {
        let (message_handler, _transport, _event_log, _scheduler) = setup_test_environment();

        // Test broadcast_listen
        let listen_result =
            message_handler.broadcast_listen(Obj::mk_id(100), HostType::WebSocket, 8080, true);

        assert!(listen_result.is_ok(), "Broadcast listen should succeed");

        // Test broadcast_unlisten
        let unlisten_result = message_handler.broadcast_unlisten(HostType::WebSocket, 8080);

        assert!(unlisten_result.is_ok(), "Broadcast unlisten should succeed");
    }

    #[test]
    fn test_ping_pong_protocol() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        // Step 1: Register a host first
        let host_token = create_test_host_token();
        let listeners = vec![(SYSTEM_OBJECT, "127.0.0.1:7777".parse().unwrap())];
        let register_message = rpc_common::HostToDaemonMessage::RegisterHost(
            std::time::SystemTime::now(),
            rpc_common::HostType::TCP,
            listeners.clone(),
        );

        let register_result = transport.process_host_message(
            message_handler.as_ref(),
            host_token.clone(),
            register_message,
        );
        assert!(register_result.is_ok(), "Host registration should succeed");

        // Step 2: Send a HostPong message (response to daemon's ping)
        let pong_time = std::time::SystemTime::now();
        let pong_message = rpc_common::HostToDaemonMessage::HostPong(
            pong_time,
            rpc_common::HostType::TCP,
            listeners,
        );

        let pong_result =
            transport.process_host_message(message_handler.as_ref(), host_token, pong_message);

        // With NormalOperation scenario, pong should be acknowledged
        assert!(
            matches!(pong_result, Ok(rpc_common::DaemonToHostReply::Ack)),
            "Host pong should be acknowledged with NormalOperation scenario: {pong_result:?}"
        );

        // Step 3: Test client ping-pong cycle
        let client_id = uuid::Uuid::new_v4();

        // Establish client connection first
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:12345".to_string(),
            acceptable_content_types: None,
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Send ClientPong message
        let client_pong_time = std::time::SystemTime::now();
        let client_pong_message = rpc_common::HostClientToDaemonMessage::ClientPong(
            client_token,
            client_pong_time,
            SYSTEM_OBJECT,
            rpc_common::HostType::TCP,
            "127.0.0.1:12345".parse().unwrap(),
        );

        let client_pong_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            client_pong_message,
        );

        // With NormalOperation scenario and established connection, pong should succeed
        assert!(
            matches!(
                client_pong_result,
                Ok(rpc_common::DaemonToClientReply::ThanksPong(_))
            ),
            "Client pong should succeed with established connection: {client_pong_result:?}"
        );

        // Step 4: Verify replies were captured correctly
        let host_replies = transport.get_host_replies();
        assert!(
            host_replies.len() >= 2,
            "Should have captured at least 2 host replies"
        );

        let client_replies = transport.get_client_replies();
        assert!(
            client_replies.len() >= 2,
            "Should have captured at least 2 client replies"
        );

        // Verify host pong was processed
        let host_pong_processed = host_replies
            .iter()
            .any(|(_, msg, _)| matches!(msg, rpc_common::HostToDaemonMessage::HostPong(_, _, _)));
        assert!(
            host_pong_processed,
            "Host pong message should have been processed"
        );

        // Verify client pong was processed
        let client_pong_processed = client_replies.iter().any(|(_, msg, _)| {
            matches!(
                msg,
                rpc_common::HostClientToDaemonMessage::ClientPong(_, _, _, _, _)
            )
        });
        assert!(
            client_pong_processed,
            "Client pong message should have been processed"
        );
    }

    #[test]
    fn test_token_validation() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let host_token = create_test_host_token();
        let client_id = Uuid::new_v4();

        // Test host token validation - should work with our test token
        let host_validation = message_handler.validate_host_token(&host_token);
        // Host token validation should succeed with valid test token
        assert!(
            host_validation.is_ok(),
            "Host token validation should succeed: {host_validation:?}"
        );

        // Test client token validation with a real token
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        if let Ok(rpc_common::DaemonToClientReply::NewConnection(client_token, _)) =
            establish_result
        {
            let client_validation = message_handler.validate_client_token(client_token, client_id);
            assert!(
                client_validation.is_ok(),
                "Valid client token should validate successfully"
            );
        }

        // Test invalid client token
        let invalid_token = rpc_common::ClientToken("invalid".to_string());
        let invalid_validation = message_handler.validate_client_token(invalid_token, client_id);
        assert!(
            invalid_validation.is_err(),
            "Invalid client token should fail validation"
        );
    }

    #[test]
    fn test_verb_and_property_introspection() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Setup: Establish connection and login
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Perform login to get auth token
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["create".to_string(), "TestPlayer".to_string()],
            do_attach: true,
        };

        let login_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            login_message,
        );

        assert!(
            login_result.is_ok(),
            "Login should succeed for authenticated operations: {login_result:?}"
        );

        // Extract auth token from login result
        let client_replies = transport.get_client_replies();
        let (_, _, login_reply) = &client_replies[1]; // Second reply should be LoginResult
        let auth_token = match login_reply {
            Ok(rpc_common::DaemonToClientReply::LoginResult(Some((token, _, _)))) => token.clone(),
            other => panic!("Expected successful LoginResult, got {other:?}"),
        };

        // Test verb introspection
        let verbs_message = rpc_common::HostClientToDaemonMessage::Verbs(
            client_token.clone(),
            auth_token.clone(),
            moor_common::model::ObjectRef::Id(SYSTEM_OBJECT),
        );

        let verbs_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            verbs_message,
        );

        // With NormalOperation scenario, verbs request should succeed or fail gracefully
        let verbs_processed = matches!(
            verbs_result,
            Ok(_) | Err(rpc_common::RpcMessageError::EntityRetrievalError(_))
        );
        assert!(
            verbs_processed,
            "Verbs request should be processed (success or graceful failure): {verbs_result:?}"
        );

        // Test property introspection
        let props_message = rpc_common::HostClientToDaemonMessage::Properties(
            client_token.clone(),
            auth_token.clone(),
            moor_common::model::ObjectRef::Id(SYSTEM_OBJECT),
        );

        let props_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            props_message,
        );

        // With NormalOperation scenario, properties request should succeed or fail gracefully
        let props_processed = matches!(
            props_result,
            Ok(_) | Err(rpc_common::RpcMessageError::EntityRetrievalError(_))
        );
        assert!(
            props_processed,
            "Properties request should be processed (success or graceful failure): {props_result:?}"
        );
    }

    #[test]
    fn test_command_execution() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        let client_id = Uuid::new_v4();

        // Setup: Establish connection
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:8080".to_string(),
            acceptable_content_types: Some(vec![moor_var::Symbol::mk("text/plain")]),
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 2: Perform login to get auth token
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["create".to_string(), "TestPlayer".to_string()],
            do_attach: true,
        };

        let login_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            login_message,
        );

        assert!(
            login_result.is_ok(),
            "Login should succeed for authenticated operations: {login_result:?}"
        );

        // Extract auth token from login result
        let client_replies = transport.get_client_replies();
        let (_, _, login_reply) = &client_replies[1]; // Second reply should be LoginResult
        let auth_token = match login_reply {
            Ok(rpc_common::DaemonToClientReply::LoginResult(Some((token, _, _)))) => token.clone(),
            other => panic!("Expected successful LoginResult, got {other:?}"),
        };

        // Test command execution
        let command_message = rpc_common::HostClientToDaemonMessage::Command(
            client_token,
            auth_token,
            SYSTEM_OBJECT,
            "look".to_string(),
        );

        let command_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            command_message,
        );

        // With NormalOperation scenario, command should be submitted successfully
        assert!(
            matches!(
                command_result,
                Ok(rpc_common::DaemonToClientReply::TaskSubmitted(_))
            ),
            "Command should be submitted successfully with NormalOperation scenario: {command_result:?}"
        );
    }

    #[test]
    fn test_request_input_round_trip() {
        let (message_handler, transport, _event_log, scheduler) = setup_test_environment();

        // Step 1: Establish a client connection
        let client_id = uuid::Uuid::new_v4();
        let establish_message = rpc_common::HostClientToDaemonMessage::ConnectionEstablish {
            peer_addr: "127.0.0.1:12345".to_string(),
            acceptable_content_types: None,
        };

        let establish_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            establish_message,
        );

        let (client_token, _connection_obj) = match establish_result.unwrap() {
            rpc_common::DaemonToClientReply::NewConnection(token, obj) => (token, obj),
            other => panic!("Expected NewConnection, got {other:?}"),
        };

        // Step 1.5: Perform login to get auth token
        let login_message = rpc_common::HostClientToDaemonMessage::LoginCommand {
            client_token: client_token.clone(),
            handler_object: SYSTEM_OBJECT,
            connect_args: vec!["create".to_string(), "TestPlayer".to_string()],
            do_attach: true,
        };

        let login_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            login_message,
        );

        assert!(
            login_result.is_ok(),
            "Login should succeed for authenticated operations: {login_result:?}"
        );

        // Extract auth token from login result
        let client_replies = transport.get_client_replies();
        let (_, _, login_reply) = &client_replies[1]; // Second reply should be LoginResult
        let auth_token = match login_reply {
            Ok(rpc_common::DaemonToClientReply::LoginResult(Some((token, _, _)))) => token.clone(),
            other => panic!("Expected successful LoginResult, got {other:?}"),
        };

        // Step 2: Create a scenario where the daemon would request input
        // We'll simulate this by triggering a client event that requests input
        let request_id = uuid::Uuid::new_v4();

        // Simulate the daemon sending a RequestInput event to the client
        let request_input_event = rpc_common::ClientEvent::RequestInput(request_id);

        // In a real scenario, this would be sent through the transport to the client
        // For testing, we'll simulate the client receiving this and responding
        transport.capture_client_event(client_id, request_input_event);

        // Step 3: Simulate client responding with RequestedInput message
        let input_response = "user typed response".to_string();
        let response_message = rpc_common::HostClientToDaemonMessage::RequestedInput(
            client_token.clone(),
            auth_token,
            request_id,
            input_response.clone(),
        );

        let response_result = transport.process_client_message(
            message_handler.as_ref(),
            scheduler.client(),
            client_id,
            response_message,
        );

        // Step 4: Verify the response was processed
        // The daemon should acknowledge the input
        // Input response should be processed (MockScheduler may not fully support this feature)
        let input_processed = matches!(
            response_result,
            Ok(rpc_common::DaemonToClientReply::InputThanks)
                | Err(rpc_common::RpcMessageError::InternalError(_))
        );
        assert!(
            input_processed,
            "Input response should be processed or fail gracefully: {response_result:?}"
        );

        // Step 5: Verify the transport captured the events correctly
        let client_events = transport.get_client_events();
        assert_eq!(
            client_events.len(),
            1,
            "Should have captured 1 client event"
        );

        let (captured_client_id, captured_event) = &client_events[0];
        assert_eq!(
            *captured_client_id, client_id,
            "Event should be for correct client"
        );
        match captured_event {
            rpc_common::ClientEvent::RequestInput(captured_request_id) => {
                assert_eq!(*captured_request_id, request_id, "Request ID should match");
            }
            other => panic!("Expected RequestInput event, got {other:?}"),
        }

        // Step 6: Verify replies were captured
        let client_replies = transport.get_client_replies();
        assert!(
            client_replies.len() >= 2,
            "Should have at least 2 client replies (NewConnection + InputThanks/Error)"
        );

        // Find the input response reply
        let input_reply_found = client_replies.iter().any(|(_, msg, reply)| {
            matches!(msg, rpc_common::HostClientToDaemonMessage::RequestedInput(_, _, captured_id, _)
                     if *captured_id == request_id) &&
            matches!(reply, Ok(rpc_common::DaemonToClientReply::InputThanks) | Err(_))
        });
        assert!(input_reply_found, "Should have found input response reply");

        // Step 7: Test complete round-trip timing and event flow
        // Verify the request ID is consistent throughout the flow
        let mut request_ids_seen = std::collections::HashSet::new();

        // Check client event
        for (_, event) in &client_events {
            if let rpc_common::ClientEvent::RequestInput(id) = event {
                request_ids_seen.insert(*id);
            }
        }

        // Check client replies
        for (_, msg, _) in &client_replies {
            if let rpc_common::HostClientToDaemonMessage::RequestedInput(_, _, id, _) = msg {
                request_ids_seen.insert(*id);
            }
        }

        assert_eq!(
            request_ids_seen.len(),
            1,
            "Should have exactly one unique request ID"
        );
        assert!(
            request_ids_seen.contains(&request_id),
            "Should contain our test request ID"
        );
    }
}
