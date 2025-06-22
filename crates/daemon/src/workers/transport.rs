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

//! ZMQ transport layer for workers, separated from business logic

use eyre::Context;
use rpc_common::{DaemonToWorkerReply, WorkerToDaemonMessage};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};
use zmq::Socket;

use super::message_handler::WorkersMessageHandler;

pub struct WorkersTransport {
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
}

impl WorkersTransport {
    pub fn new(zmq_context: zmq::Context, kill_switch: Arc<AtomicBool>) -> Self {
        Self {
            zmq_context,
            kill_switch,
        }
    }

    /// Start listening for worker messages
    pub fn start_listen_loop<H: WorkersMessageHandler + 'static>(
        self,
        workers_endpoint: &str,
        message_handler: Arc<H>,
    ) -> eyre::Result<()> {
        let rpc_socket = self.zmq_context.socket(zmq::REP)?;
        rpc_socket.bind(workers_endpoint)?;

        info!(
            "Workers 0mq server listening on {} with {} IO threads",
            workers_endpoint,
            self.zmq_context.get_io_threads().unwrap()
        );

        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Workers transport kill switch activated, exiting");
                return Ok(());
            }

            let poll_result = rpc_socket
                .poll(zmq::POLLIN, 100)
                .with_context(|| "Error polling ZMQ socket. Bailing out.")?;
            if poll_result == 0 {
                continue;
            }

            let msg = rpc_socket
                .recv_multipart(0)
                .with_context(|| "Error receiving message from ZMQ socket. Bailing out.")?;

            if msg.len() != 3 {
                warn!(
                    "Received message with {} parts, expected 3; rejecting",
                    msg.len()
                );
                Self::reject(&rpc_socket);
                continue;
            }

            // First argument should be a WorkerToken
            // Second argument is a Uuid
            // Third argument is a bincoded WorkerToDaemonMessage
            let (worker_token, worker_id, request) = (&msg[0], &msg[1], &msg[2]);

            let Ok((msg, _)) = bincode::decode_from_slice::<WorkerToDaemonMessage, _>(
                request,
                bincode::config::standard(),
            ) else {
                error!("Unable to decode WorkerToDaemonMessage message");
                Self::reject(&rpc_socket);
                continue;
            };

            // Delegate to message handler
            let reply = message_handler.handle_worker_message(worker_token, worker_id, msg);

            // Send the reply
            Self::send_reply(&rpc_socket, reply)?;
        }
    }

    fn reject(rpc_socket: &Socket) {
        Self::send_reply(rpc_socket, DaemonToWorkerReply::Rejected).ok();
    }

    fn send_reply(rpc_socket: &Socket, reply: DaemonToWorkerReply) -> eyre::Result<()> {
        let response = bincode::encode_to_vec(&reply, bincode::config::standard())
            .context("Unable to encode response")?;

        rpc_socket
            .send_multipart([&response], 0)
            .context("Unable to send response")?;

        Ok(())
    }
}
