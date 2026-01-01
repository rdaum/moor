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

#![allow(clippy::too_many_arguments)]

pub use host::{process_hosts_events, start_host_session};
pub use listeners::{ListenerInfo, ListenersClient, ListenersError, ListenersMessage};
pub use worker::attach_worker;
pub use worker_loop::{WorkerRpcError, worker_loop};
pub use worker_rpc_client::WorkerRpcSendClient;
pub use zmq;

pub mod curve_keys;
pub mod enrollment_client;
mod host;
mod listeners;
pub mod pubsub_client;
pub mod rpc_client;
mod worker;
mod worker_loop;
mod worker_rpc_client;

/// Helper function to configure CURVE encryption on a tmq socket builder
///
/// # Arguments
/// * `socket_builder` - The tmq socket builder to configure
/// * `client_secret` - Z85-encoded client secret key
/// * `client_public` - Z85-encoded client public key
/// * `server_public` - Z85-encoded server public key
///
/// # Returns
/// The socket builder with CURVE encryption configured
///
/// # Example
/// ```no_run
/// use tmq::request;
/// use rpc_async_client::configure_curve_client;
///
/// let ctx = tmq::Context::new();
/// let socket_builder = request(&ctx);
/// let secure_socket = configure_curve_client(
///     socket_builder,
///     "client_secret_z85",
///     "client_public_z85",
///     "server_public_z85"
/// ).unwrap().connect("tcp://localhost:7899").unwrap();
/// ```
pub fn configure_curve_client(
    socket_builder: tmq::SocketBuilder<tmq::request_reply::RequestSender>,
    client_secret: &str,
    client_public: &str,
    server_public: &str,
) -> Result<tmq::SocketBuilder<tmq::request_reply::RequestSender>, String> {
    // Decode Z85 keys to bytes
    let client_secret_bytes =
        zmq::z85_decode(client_secret).map_err(|_| "Invalid client secret key")?;
    let client_public_bytes =
        zmq::z85_decode(client_public).map_err(|_| "Invalid client public key")?;
    let server_public_bytes =
        zmq::z85_decode(server_public).map_err(|_| "Invalid server public key")?;

    Ok(socket_builder
        .set_curve_secretkey(&client_secret_bytes)
        .set_curve_publickey(&client_public_bytes)
        .set_curve_serverkey(&server_public_bytes))
}
