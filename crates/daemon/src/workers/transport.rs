// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use moor_schema::rpc as moor_rpc;
use planus::{Builder, ReadAsRoot};
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
    curve_secret_key: Option<String>, // Z85-encoded CURVE secret key
}

impl WorkersTransport {
    pub fn new(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        curve_secret_key: Option<String>,
    ) -> Self {
        Self {
            zmq_context,
            kill_switch,
            curve_secret_key,
        }
    }

    /// Start listening for worker messages
    pub fn start_listen_loop<H: WorkersMessageHandler + 'static>(
        self,
        workers_endpoint: &str,
        message_handler: Arc<H>,
    ) -> eyre::Result<()> {
        let rpc_socket = self.zmq_context.socket(zmq::REP)?;

        // Configure CURVE encryption if key provided
        if let Some(ref secret_key) = self.curve_secret_key {
            // Set ZAP domain for authentication
            rpc_socket
                .set_zap_domain("moor")
                .context("Failed to set ZAP domain on workers REP socket")?;

            rpc_socket
                .set_curve_server(true)
                .context("Failed to enable CURVE server on workers REP socket")?;

            // Decode Z85-encoded secret key to bytes
            let secret_key_bytes =
                zmq::z85_decode(secret_key).context("Failed to decode Z85 secret key")?;
            rpc_socket
                .set_curve_secretkey(&secret_key_bytes)
                .context("Failed to set CURVE secret key on workers REP socket")?;

            info!("CURVE encryption enabled on workers REP socket with ZAP authentication");
        }

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

            if msg.len() != 2 {
                warn!(
                    "Received message with {} parts, expected 2; rejecting",
                    msg.len()
                );
                Self::reject(&rpc_socket);
                continue;
            }

            // First argument is a Uuid
            // Second argument is a flatbuffer RPC message
            let (worker_id, request) = (&msg[0], &msg[1]);

            let Ok(fb_msg) = moor_rpc::WorkerToDaemonMessageRef::read_as_root(request) else {
                error!("Unable to decode flatbuffer WorkerToDaemonMessage");
                Self::reject(&rpc_socket);
                continue;
            };

            // Use flatbuffer handler
            let fb_reply = message_handler.handle_worker_message(worker_id, &fb_msg);
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
