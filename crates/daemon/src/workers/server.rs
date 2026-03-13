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

//! Thin coordinator for workers server that delegates business logic to message handler

use moor_common::threading::spawn_efficient;
use moor_common::util::Deadline;
use moor_kernel::tasks::workers::WorkerRequest;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::{error, info};

use super::{
    message_handler::{PING_FREQUENCY, WorkersMessageHandler, WorkersMessageHandlerImpl},
    transport::WorkersTransport,
};

/// Coordinator for workers server that delegates business logic to message handler
pub struct WorkersServer {
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,

    // Core business logic handler
    message_handler: Arc<WorkersMessageHandlerImpl>,

    // CURVE encryption key
    curve_secret_key: Option<String>,

    // Thread handles
    request_processor_jh: Option<std::thread::JoinHandle<()>>,
    transport_jh: Option<std::thread::JoinHandle<()>>,
}

impl WorkersServer {
    /// Create a new workers server coordinator with a pre-built message handler.
    pub fn new(
        kill_switch: Arc<AtomicBool>,
        zmq_context: zmq::Context,
        message_handler: Arc<WorkersMessageHandlerImpl>,
        curve_secret_key: Option<String>,
    ) -> Self {
        Self {
            zmq_context,
            kill_switch,
            message_handler,
            curve_secret_key,
            request_processor_jh: None,
            transport_jh: None,
        }
    }

    /// Start the request processor thread that handles scheduler requests
    pub fn start(&mut self) -> eyre::Result<flume::Sender<WorkerRequest>> {
        let (send, recv) = flume::unbounded::<WorkerRequest>();

        let message_handler = self.message_handler.clone();
        let kill_switch = self.kill_switch.clone();

        let jh = spawn_efficient("moor-workers-proc", move || {
            Self::process_requests(kill_switch, recv, message_handler)
        })?;

        self.request_processor_jh = Some(jh);
        Ok(send)
    }

    /// Start the transport layer to listen for worker connections
    pub fn listen(&mut self, workers_endpoint: &str) -> eyre::Result<()> {
        let transport = WorkersTransport::new(
            self.zmq_context.clone(),
            self.kill_switch.clone(),
            self.curve_secret_key.clone(),
        );
        let message_handler = self.message_handler.clone();
        let workers_endpoint = workers_endpoint.to_string();

        let jh = spawn_efficient("moor-workers-transport", move || {
            if let Err(e) = transport.start_listen_loop(&workers_endpoint, message_handler) {
                error!(error = ?e, "Workers transport failed");
            }
        })?;

        self.transport_jh = Some(jh);
        Ok(())
    }

    /// Process requests from the scheduler
    fn process_requests(
        kill_switch: Arc<AtomicBool>,
        recv: flume::Receiver<WorkerRequest>,
        message_handler: Arc<WorkersMessageHandlerImpl>,
    ) {
        let mut next_ping_due = Deadline::from_now(PING_FREQUENCY);
        loop {
            if kill_switch.load(Ordering::Relaxed) {
                info!("Workers server thread exiting.");
                break;
            }

            // Check for expired workers and remove them
            message_handler.check_expired_workers();

            // If it's been a while since we sent a ping, send one out to all workers
            if next_ping_due.is_expired() {
                if let Err(e) = message_handler.ping_workers() {
                    error!(error = ?e, "Unable to ping workers");
                }
                next_ping_due = Deadline::from_now(PING_FREQUENCY);
            }

            // Process incoming requests with timeout
            match recv.recv_timeout(Duration::from_millis(200)) {
                Ok(request) => {
                    if let Err(e) = message_handler.process_worker_request(request) {
                        error!(error = ?e, "Unable to process worker request");
                    }
                }
                Err(_) => continue,
            }
        }
    }
}
