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
use planus::{Builder, ReadAsRoot};
use rpc_common::flatbuffers_generated::moor_rpc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
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
            // Third argument is either a flatbuffer or bincoded WorkerToDaemonMessage
            let (worker_token, worker_id, request) = (&msg[0], &msg[1], &msg[2]);

            let Ok(fb_msg) = moor_rpc::WorkerToDaemonMessageRef::read_as_root(request) else {
                error!("Unable to decode flatbuffer WorkerToDaemonMessage");
                Self::reject(&rpc_socket);
                continue;
            };

            // Use flatbuffer handler
            let fb_reply = message_handler.handle_worker_message(worker_token, worker_id, &fb_msg);
            Self::send_flatbuffer_reply(&rpc_socket, fb_reply)?;
        }
    }

    fn reject(rpc_socket: &Socket) {
        let reject_reply = moor_rpc::DaemonToWorkerReply {
            reply: moor_rpc::DaemonToWorkerReplyUnion::WorkerRejected(Box::new(
                moor_rpc::WorkerRejected {
                    reason: Some("Message rejected".to_string()),
                },
            )),
        };
        Self::send_flatbuffer_reply(rpc_socket, reject_reply).ok();
    }

    fn send_flatbuffer_reply(
        rpc_socket: &Socket,
        reply: moor_rpc::DaemonToWorkerReply,
    ) -> eyre::Result<()> {
        let mut builder = Builder::new();
        let response = builder.finish(&reply, None);

        rpc_socket
            .send_multipart([response.to_vec()], 0)
            .context("Unable to send flatbuffer response")?;

        Ok(())
    }
}
