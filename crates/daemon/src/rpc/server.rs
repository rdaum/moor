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

//! Thin coordinator for RPC server that delegates business logic to message handler

use flume::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::connections::ConnectionRegistry;
use crate::event_log::EventLog;
use super::message_handler::RpcMessageHandler;
use super::session::SessionActions;
use super::transport::RpcTransport;
use crate::system_control::SystemControlHandle;
use crate::task_monitor::TaskMonitor;
use moor_kernel::SchedulerClient;
use moor_kernel::config::Config;
use rusty_paseto::prelude::Key;
use tracing::{error, info};

/// RPCcoordinator that delegates business logic to message handler
pub struct RpcServer {
    zmq_context: zmq::Context,
    pub(crate) kill_switch: Arc<AtomicBool>,

    // Core business logic handler
    message_handler: Arc<RpcMessageHandler>,

    mailbox_receive: Receiver<SessionActions>,

    pub(crate) mailbox_sender: Sender<SessionActions>,

    // Core server resources
    event_log: Arc<EventLog>,
}

impl RpcServer {
    pub fn new(
        public_key: Key<32>,
        private_key: Key<64>,
        connections: Box<dyn ConnectionRegistry + Send + Sync>,
        zmq_context: zmq::Context,
        narrative_endpoint: &str,
        config: Arc<Config>,
        events_db_path: &std::path::Path,
    ) -> (Self, TaskMonitor, SystemControlHandle) {
        info!(
            "Creating new RPC server; with {} ZMQ IO threads...",
            zmq_context.get_io_threads().unwrap()
        );

        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().len()
        );
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (mailbox_sender, mailbox_receive) = flume::unbounded();

        // Create the event log
        let event_log = Arc::new(EventLog::with_config(
            crate::event_log::EventLogConfig::default(),
            Some(events_db_path),
        ));

        // Create the task monitor with mailbox sender
        let (task_monitor, task_monitor_handle) = TaskMonitor::new(mailbox_sender.clone());

        // Create hosts as Arc<RwLock> so it can be shared
        let hosts = Arc::new(RwLock::new(Default::default()));

        // Create the business logic handler
        let message_handler = Arc::new(RpcMessageHandler::new(
            zmq_context.clone(),
            narrative_endpoint,
            config,
            public_key,
            private_key,
            connections,
            hosts.clone(),
            mailbox_sender.clone(),
            event_log.clone(),
            task_monitor_handle,
        ).expect("Failed to create RpcMessageHandler"));

        // Create the system control handle for the scheduler
        let system_control =
            SystemControlHandle::new(kill_switch.clone(), message_handler.clone());

        let server = Self {
            zmq_context,
            kill_switch,
            message_handler,
            mailbox_sender,
            mailbox_receive,
            event_log,
        };

        (server, task_monitor, system_control)
    }

    pub fn request_loop(
        self: Arc<Self>,
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
        mut task_monitor: TaskMonitor,
    ) -> eyre::Result<()> {
        // Move out parts we need for threads before consuming self
        let ping_pong_handler = self.message_handler.clone();

        // Start up the ping-ponger timer in a background thread...
        std::thread::Builder::new()
            .name("rpc-ping-pong".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(5));
                    if let Err(e) = ping_pong_handler.ping_pong() {
                        error!(error = ?e, "Unable to ping-pong");
                    }
                }
            })?;

        // Clone what we need before consuming self
        let transport_zmq_context = self.zmq_context.clone();
        let transport_kill_switch = self.kill_switch.clone();
        let transport_message_handler = self.message_handler.clone();
        let task_completion_kill_switch = self.kill_switch.clone();

        // Use the separated transport layer for message handling
        let transport = RpcTransport::new(transport_zmq_context, transport_kill_switch.clone());

        // Start the transport in a background thread
        let transport_scheduler = scheduler_client.clone();
        std::thread::Builder::new()
            .name("moor-rpc-transport".to_string())
            .spawn(move || {
                if let Err(e) = transport.start_request_loop(
                    rpc_endpoint,
                    transport_scheduler,
                    transport_message_handler,
                ) {
                    error!(error = ?e, "Transport layer failed");
                }
            })?;

        // Process task completions
        std::thread::Builder::new()
            .name("moor-tc".to_string())
            .spawn(move || {
                loop {
                    if task_completion_kill_switch.load(Ordering::Relaxed) {
                        return;
                    }
                    // Process commands and check for task completions
                    task_monitor.run_loop(Duration::from_millis(5));
                }
            })?;

        // Main loop processes session events and monitors kill switch
        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Kill switch activated, exiting");
                return Ok(());
            }

            // Check the mailbox - just process session events
            if let Ok(session_event) = self.mailbox_receive.recv_timeout(Duration::from_millis(5)) {
                // Delegate all session event handling to message handler
                if let Err(e) = self.message_handler.handle_session_event(session_event) {
                    error!(error = ?e, "Error handling session event");
                }
            }
        }
    }

    // Clean interface methods for external modules that need limited access
    pub fn event_log(&self) -> &Arc<EventLog> {
        &self.event_log
    }
}
