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

//! Thin coordinator for workers server that delegates business logic to message handler

use moor_kernel::tasks::workers::{WorkerRequest, WorkerResponse};
use rusty_paseto::core::Key;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
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

    // Thread handles
    request_processor_jh: Option<std::thread::JoinHandle<()>>,
    transport_jh: Option<std::thread::JoinHandle<()>>,
}

impl WorkersServer {
    /// Create a new workers server coordinator
    pub fn new(
        kill_switch: Arc<AtomicBool>,
        public_key: Key<32>,
        private_key: Key<64>,
        zmq_context: zmq::Context,
        workers_broadcast: &str,
        scheduler_send: flume::Sender<WorkerResponse>,
    ) -> eyre::Result<Self> {
        // Create the message handler
        let message_handler = Arc::new(WorkersMessageHandlerImpl::new(
            zmq_context.clone(),
            workers_broadcast,
            public_key,
            private_key,
            scheduler_send,
        )?);

        Ok(Self {
            zmq_context,
            kill_switch,
            message_handler,
            request_processor_jh: None,
            transport_jh: None,
        })
    }

    /// Start the request processor thread that handles scheduler requests
    pub fn start(&mut self) -> eyre::Result<flume::Sender<WorkerRequest>> {
        let (send, recv) = flume::unbounded::<WorkerRequest>();

        let message_handler = self.message_handler.clone();
        let kill_switch = self.kill_switch.clone();

        let jh = std::thread::Builder::new()
            .name("moor-workers-proc".into())
            .spawn(move || Self::process_requests(kill_switch, recv, message_handler))?;

        self.request_processor_jh = Some(jh);
        Ok(send)
    }

    /// Start the transport layer to listen for worker connections
    pub fn listen(&mut self, workers_endpoint: &str) -> eyre::Result<()> {
        let transport = WorkersTransport::new(self.zmq_context.clone(), self.kill_switch.clone());
        let message_handler = self.message_handler.clone();
        let workers_endpoint = workers_endpoint.to_string();

        let jh = std::thread::Builder::new()
            .name("moor-workers-transport".into())
            .spawn(move || {
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
        let mut last_ping_out = Instant::now();
        loop {
            if kill_switch.load(Ordering::Relaxed) {
                info!("Workers server thread exiting.");
                break;
            }

            // Check for expired workers and remove them
            message_handler.check_expired_workers();

            // If it's been a while since we sent a ping, send one out to all workers
            if last_ping_out.elapsed() > PING_FREQUENCY {
                if let Err(e) = message_handler.ping_workers() {
                    error!(error = ?e, "Unable to ping workers");
                }
                last_ping_out = Instant::now();
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
