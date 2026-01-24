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

//! ZMQ transport layer for RPC, separated from business logic

use eyre::Context;
use planus::ReadAsRoot;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::{error, info, warn};
use uuid::Uuid;
use zmq::Socket;

use super::message_handler::MessageHandler;
use moor_common::tasks::{Event, NarrativeEvent};
use moor_kernel::SchedulerClient;
use moor_rpc::{HostToDaemonMessageRef, MessageTypeRef};
use moor_schema::{convert::narrative_event_to_flatbuffer_struct, rpc as moor_rpc};
use moor_var::Obj;
use rpc_common::{
    CLIENT_BROADCAST_TOPIC, HOST_BROADCAST_TOPIC, RpcMessageError, obj_fb,
    scheduler_error_to_flatbuffer_struct, var_to_flatbuffer_rpc,
};
/// Trait for the transport layer that handles communication between hosts and the daemon
pub trait Transport: Send + Sync {
    /// Start the request processing loop with ZMQ proxy architecture
    fn start_request_loop(
        &self,
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
        message_handler: Arc<dyn MessageHandler>,
    ) -> eyre::Result<()>;
    /// Publish narrative events to clients
    fn publish_narrative_events(
        &self,
        events: &[(Obj, Box<NarrativeEvent>)],
        connections: &dyn crate::connections::ConnectionRegistry,
    ) -> Result<(), eyre::Error>;
    /// Broadcast events to hosts
    fn broadcast_host_event(&self, event: moor_rpc::HostBroadcastEvent) -> Result<(), eyre::Error>;
    /// Publish event to specific client
    fn publish_client_event(
        &self,
        client_id: Uuid,
        event: moor_rpc::ClientEvent,
    ) -> Result<(), eyre::Error>;
    /// Broadcast events to all clients
    fn broadcast_client_event(
        &self,
        event: moor_rpc::ClientsBroadcastEvent,
    ) -> Result<(), eyre::Error>;
}

/// ZMQ + bincoded structs transport layer that handles socket management and message routing
pub struct RpcTransport {
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
    /// IPC PUB socket for local connections (no CURVE) - binds all IPC endpoints
    events_publish_ipc: Option<Arc<Mutex<Socket>>>,
    /// TCP PUB socket for remote connections (with CURVE) - binds all TCP endpoints
    events_publish_tcp: Option<Arc<Mutex<Socket>>>,
    curve_secret_key: Option<String>, // Z85-encoded CURVE secret key for server mode
    /// IPC endpoints for RPC (no CURVE)
    ipc_rpc_endpoints: Vec<String>,
    /// TCP endpoints for RPC (with CURVE)
    tcp_rpc_endpoints: Vec<String>,
}

impl RpcTransport {
    /// Parse comma-separated endpoints and group by type (IPC vs TCP)
    fn parse_endpoints(endpoints_str: &str) -> (Vec<String>, Vec<String>) {
        let mut ipc_endpoints = Vec::new();
        let mut tcp_endpoints = Vec::new();

        for endpoint in endpoints_str.split(',') {
            let endpoint = endpoint.trim();
            if endpoint.is_empty() {
                continue;
            }
            if endpoint.starts_with("tcp://") {
                tcp_endpoints.push(endpoint.to_string());
            } else {
                // IPC or other local transports
                ipc_endpoints.push(endpoint.to_string());
            }
        }

        (ipc_endpoints, tcp_endpoints)
    }

    pub fn new(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        events_endpoints: &str,
        rpc_endpoints: &str,
        curve_secret_key: Option<String>,
    ) -> Result<Self, eyre::Error> {
        // Parse endpoints into IPC and TCP groups
        let (ipc_events_endpoints, tcp_events_endpoints) = Self::parse_endpoints(events_endpoints);
        let (ipc_rpc_endpoints, tcp_rpc_endpoints) = Self::parse_endpoints(rpc_endpoints);

        // Create IPC PUB socket if we have any IPC endpoints
        let events_publish_ipc = if !ipc_events_endpoints.is_empty() {
            let ipc_publish = zmq_context
                .socket(zmq::SocketType::PUB)
                .context("Unable to create IPC PUB socket")?;

            for endpoint in &ipc_events_endpoints {
                ipc_publish
                    .bind(endpoint)
                    .with_context(|| format!("Unable to bind IPC PUB socket @ {endpoint}"))?;
                info!("IPC events endpoint bound at {}", endpoint);
            }

            Some(Arc::new(Mutex::new(ipc_publish)))
        } else {
            None
        };

        // Create TCP PUB socket with CURVE if we have any TCP endpoints
        let events_publish_tcp = if !tcp_events_endpoints.is_empty() {
            let tcp_publish = zmq_context
                .socket(zmq::SocketType::PUB)
                .context("Unable to create TCP PUB socket")?;

            // Configure CURVE encryption
            if let Some(ref secret_key) = curve_secret_key {
                tcp_publish
                    .set_zap_domain("moor")
                    .context("Failed to set ZAP domain on TCP PUB socket")?;
                tcp_publish
                    .set_curve_server(true)
                    .context("Failed to enable CURVE server on TCP PUB socket")?;
                let secret_key_bytes =
                    zmq::z85_decode(secret_key).context("Failed to decode Z85 secret key")?;
                tcp_publish
                    .set_curve_secretkey(&secret_key_bytes)
                    .context("Failed to set CURVE secret key on TCP PUB socket")?;
                info!("CURVE encryption enabled on TCP events publisher with ZAP authentication");
            }

            for endpoint in &tcp_events_endpoints {
                tcp_publish
                    .bind(endpoint)
                    .with_context(|| format!("Unable to bind TCP PUB socket @ {endpoint}"))?;
                info!("TCP events endpoint bound at {}", endpoint);
            }

            Some(Arc::new(Mutex::new(tcp_publish)))
        } else {
            None
        };

        Ok(Self {
            zmq_context,
            kill_switch,
            events_publish_ipc,
            events_publish_tcp,
            curve_secret_key,
            ipc_rpc_endpoints,
            tcp_rpc_endpoints,
        })
    }

    /// Individual RPC process loop that runs in worker threads
    fn rpc_process_loop(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        scheduler_client: SchedulerClient,
        message_handler: Arc<dyn MessageHandler>,
        connect_ipc: bool,
        connect_tcp: bool,
    ) -> eyre::Result<()> {
        let rpc_socket = zmq_context.socket(zmq::REP)?;
        // Connect to IPC workers if IPC endpoints are configured
        if connect_ipc {
            rpc_socket.connect("inproc://rpc-workers-ipc")?;
        }
        // Connect to TCP workers if TCP endpoints are configured
        if connect_tcp {
            rpc_socket.connect("inproc://rpc-workers-tcp")?;
        }

        loop {
            if kill_switch.load(Ordering::Relaxed) {
                return Ok(());
            }

            let poll_result = rpc_socket
                .poll(zmq::POLLIN, 1000)
                .with_context(|| "Error polling ZMQ socket. Bailing out.")?;
            if poll_result == 0 {
                continue;
            }

            match rpc_socket.recv_multipart(0) {
                Err(_) => {
                    info!("ZMQ socket closed, exiting");
                    return Ok(());
                }
                Ok(request) => {
                    if let Err(e) = Self::process_request(
                        &rpc_socket,
                        request,
                        &scheduler_client,
                        message_handler.as_ref(),
                    ) {
                        error!(error = ?e, "Error processing request");
                    }
                }
            }
        }
    }

    /// Process a single request message
    fn process_request(
        rpc_socket: &Socket,
        request: Vec<Vec<u8>>,
        scheduler_client: &SchedulerClient,
        message_handler: &dyn MessageHandler,
    ) -> eyre::Result<()> {
        // Components are: [discriminator, request_body]
        // discriminator is a MessageType FlatBuffer that tells us what's in request_body
        if request.len() != 2 {
            Self::reply_invalid_request(rpc_socket, "Incorrect message length")?;
            return Ok(());
        }

        let (discriminator, request_body) = (&request[0], &request[1]);

        // Decode the MessageType discriminator
        let message_type_ref = match MessageTypeRef::read_as_root(discriminator) {
            Ok(msg) => msg,
            Err(_) => {
                Self::reply_invalid_request(rpc_socket, "Could not decode message type")?;
                return Ok(());
            }
        };

        use moor_rpc::MessageTypeUnionRef;
        match message_type_ref
            .message()
            .map_err(|_| eyre::eyre!("Missing message union"))?
        {
            MessageTypeUnionRef::HostToDaemonMsg(host_msg) => {
                // Extract host_id from discriminator
                let host_id_ref = match host_msg.host_id() {
                    Ok(id) => id,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Missing host_id")?;
                        return Ok(());
                    }
                };
                let host_id_data = match host_id_ref.data() {
                    Ok(data) => data,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Invalid host_id")?;
                        return Ok(());
                    }
                };
                let Ok(host_id) = Uuid::from_slice(host_id_data) else {
                    Self::reply_invalid_request(rpc_socket, "Bad host_id")?;
                    return Ok(());
                };

                // Decode the actual HostToDaemonMessage from request_body
                let Ok(host_message_fb) = HostToDaemonMessageRef::read_as_root(request_body) else {
                    Self::reply_invalid_request(rpc_socket, "Could not decode host message")?;
                    return Ok(());
                };

                // Process
                let response = message_handler.handle_host_message(host_id, host_message_fb);
                match Self::pack_host_response(response) {
                    Ok(response) => {
                        rpc_socket.send_multipart(vec![response], 0)?;
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to encode host response");
                    }
                }
            }
            MessageTypeUnionRef::HostClientToDaemonMsg(client_msg) => {
                // Extract client_id from discriminator
                let Ok(client_data) = client_msg.client_data() else {
                    Self::reply_invalid_request(rpc_socket, "Missing client data")?;
                    return Ok(());
                };
                let Ok(client_id) = Uuid::from_slice(client_data) else {
                    Self::reply_invalid_request(rpc_socket, "Bad client id")?;
                    return Ok(());
                };

                // Decode the actual HostClientToDaemonMessage from request_body
                let Ok(request_fb) =
                    moor_rpc::HostClientToDaemonMessageRef::read_as_root(request_body)
                else {
                    Self::reply_invalid_request(rpc_socket, "Could not decode request body")?;
                    return Ok(());
                };

                // Process the request
                let response = message_handler.handle_client_message(
                    scheduler_client.clone(),
                    client_id,
                    request_fb,
                );
                match Self::pack_client_response(response) {
                    Ok(response) => {
                        rpc_socket.send_multipart(vec![response], 0)?;
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to encode client response");
                    }
                }
            }
        }

        Ok(())
    }

    fn reply_invalid_request(socket: &Socket, reason: &str) -> eyre::Result<()> {
        warn!("Invalid request received, replying with error: {reason}");
        let response =
            Self::pack_client_response(Err(RpcMessageError::InvalidRequest(reason.to_string())))?;
        socket.send_multipart(vec![response], 0)?;
        Ok(())
    }

    fn pack_client_response(
        result: Result<moor_rpc::DaemonToClientReply, RpcMessageError>,
    ) -> Result<Vec<u8>, eyre::Error> {
        match result {
            Ok(reply) => {
                let mut builder = planus::Builder::new();
                let reply_result = moor_rpc::ReplyResult {
                    result: moor_rpc::ReplyResultUnion::ClientSuccess(Box::new(
                        moor_rpc::ClientSuccess {
                            reply: Box::new(reply),
                        },
                    )),
                };
                let finished = builder.finish(&reply_result, None);
                Ok(finished.to_vec())
            }
            Err(e) => {
                let mut builder = planus::Builder::new();

                // Convert RpcMessageError to FlatBuffer format
                let (error_code, message, scheduler_error) = match &e {
                    RpcMessageError::AlreadyConnected => (
                        moor_rpc::RpcMessageErrorCode::AlreadyConnected,
                        Some("Already connected".to_string()),
                        None,
                    ),
                    RpcMessageError::InvalidRequest(msg) => (
                        moor_rpc::RpcMessageErrorCode::InvalidRequest,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::NoConnection => (
                        moor_rpc::RpcMessageErrorCode::NoConnection,
                        Some("No connection for client".to_string()),
                        None,
                    ),
                    RpcMessageError::ErrorCouldNotRetrieveSysProp(msg) => (
                        moor_rpc::RpcMessageErrorCode::ErrorCouldNotRetrieveSysProp,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::LoginTaskFailed(msg) => (
                        moor_rpc::RpcMessageErrorCode::LoginTaskFailed,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::CreateSessionFailed => (
                        moor_rpc::RpcMessageErrorCode::CreateSessionFailed,
                        Some("Could not create session".to_string()),
                        None,
                    ),
                    RpcMessageError::PermissionDenied => (
                        moor_rpc::RpcMessageErrorCode::PermissionDenied,
                        Some("Permission denied".to_string()),
                        None,
                    ),
                    RpcMessageError::TaskError(scheduler_error) => {
                        let scheduler_error_fb =
                            scheduler_error_to_flatbuffer_struct(scheduler_error)
                                .ok()
                                .map(Box::new);
                        (
                            moor_rpc::RpcMessageErrorCode::TaskError,
                            None,
                            scheduler_error_fb,
                        )
                    }
                    RpcMessageError::EntityRetrievalError(msg) => (
                        moor_rpc::RpcMessageErrorCode::EntityRetrievalError,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::InternalError(msg) => (
                        moor_rpc::RpcMessageErrorCode::InternalError,
                        Some(msg.clone()),
                        None,
                    ),
                };

                let error_fb = moor_rpc::RpcMessageError {
                    error_code,
                    message,
                    scheduler_error,
                };
                let reply_result = moor_rpc::ReplyResult {
                    result: moor_rpc::ReplyResultUnion::Failure(Box::new(moor_rpc::Failure {
                        error: Box::new(error_fb),
                    })),
                };
                let finished = builder.finish(&reply_result, None);
                Ok(finished.to_vec())
            }
        }
    }

    fn pack_host_response(
        result: Result<moor_rpc::DaemonToHostReply, RpcMessageError>,
    ) -> Result<Vec<u8>, eyre::Error> {
        match result {
            Ok(reply) => {
                let mut builder = planus::Builder::new();
                let reply_result = moor_rpc::ReplyResult {
                    result: moor_rpc::ReplyResultUnion::HostSuccess(Box::new(
                        moor_rpc::HostSuccess {
                            reply: Box::new(reply),
                        },
                    )),
                };
                let finished = builder.finish(&reply_result, None);
                Ok(finished.to_vec())
            }
            Err(e) => {
                let mut builder = planus::Builder::new();

                // Convert RpcMessageError to FlatBuffer format
                let (error_code, message, scheduler_error) = match &e {
                    RpcMessageError::AlreadyConnected => (
                        moor_rpc::RpcMessageErrorCode::AlreadyConnected,
                        Some("Already connected".to_string()),
                        None,
                    ),
                    RpcMessageError::InvalidRequest(msg) => (
                        moor_rpc::RpcMessageErrorCode::InvalidRequest,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::NoConnection => (
                        moor_rpc::RpcMessageErrorCode::NoConnection,
                        Some("No connection for client".to_string()),
                        None,
                    ),
                    RpcMessageError::ErrorCouldNotRetrieveSysProp(msg) => (
                        moor_rpc::RpcMessageErrorCode::ErrorCouldNotRetrieveSysProp,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::LoginTaskFailed(msg) => (
                        moor_rpc::RpcMessageErrorCode::LoginTaskFailed,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::CreateSessionFailed => (
                        moor_rpc::RpcMessageErrorCode::CreateSessionFailed,
                        Some("Could not create session".to_string()),
                        None,
                    ),
                    RpcMessageError::PermissionDenied => (
                        moor_rpc::RpcMessageErrorCode::PermissionDenied,
                        Some("Permission denied".to_string()),
                        None,
                    ),
                    RpcMessageError::TaskError(scheduler_error) => {
                        let scheduler_error_fb =
                            scheduler_error_to_flatbuffer_struct(scheduler_error)
                                .ok()
                                .map(Box::new);
                        (
                            moor_rpc::RpcMessageErrorCode::TaskError,
                            None,
                            scheduler_error_fb,
                        )
                    }
                    RpcMessageError::EntityRetrievalError(msg) => (
                        moor_rpc::RpcMessageErrorCode::EntityRetrievalError,
                        Some(msg.clone()),
                        None,
                    ),
                    RpcMessageError::InternalError(msg) => (
                        moor_rpc::RpcMessageErrorCode::InternalError,
                        Some(msg.clone()),
                        None,
                    ),
                };

                let error_fb = moor_rpc::RpcMessageError {
                    error_code,
                    message,
                    scheduler_error,
                };
                let reply_result = moor_rpc::ReplyResult {
                    result: moor_rpc::ReplyResultUnion::Failure(Box::new(moor_rpc::Failure {
                        error: Box::new(error_fb),
                    })),
                };
                let finished = builder.finish(&reply_result, None);
                Ok(finished.to_vec())
            }
        }
    }
}

impl Transport for RpcTransport {
    /// Start the request processing loop with ZMQ proxy architecture
    fn start_request_loop(
        &self,
        _rpc_endpoint: String, // Ignored - we use ipc_rpc_endpoints and tcp_rpc_endpoints
        scheduler_client: SchedulerClient,
        message_handler: Arc<dyn MessageHandler>,
    ) -> eyre::Result<()> {
        let num_io_threads = self.zmq_context.get_io_threads()?;
        let has_ipc = !self.ipc_rpc_endpoints.is_empty();
        let has_tcp = !self.tcp_rpc_endpoints.is_empty();

        info!(
            "Starting RPC server with {} IO threads, IPC endpoints: {:?}, TCP endpoints: {:?}",
            num_io_threads, self.ipc_rpc_endpoints, self.tcp_rpc_endpoints
        );

        // Set up IPC ROUTER/DEALER if we have IPC endpoints
        if has_ipc {
            let mut ipc_clients = self.zmq_context.socket(zmq::ROUTER)?;
            let mut ipc_workers = self.zmq_context.socket(zmq::DEALER)?;

            // Bind all IPC endpoints to the same socket (no CURVE)
            for endpoint in &self.ipc_rpc_endpoints {
                ipc_clients.bind(endpoint)?;
                info!("IPC RPC endpoint bound at {}", endpoint);
            }
            ipc_workers.bind("inproc://rpc-workers-ipc")?;

            // Start IPC proxy
            let mut ipc_control_socket = self.zmq_context.socket(zmq::REP)?;
            ipc_control_socket.bind("inproc://rpc-proxy-ipc-steer")?;
            std::thread::Builder::new()
                .name("moor-rpc-proxy-ipc".to_string())
                .spawn(move || {
                    zmq::proxy_steerable(
                        &mut ipc_clients,
                        &mut ipc_workers,
                        &mut ipc_control_socket,
                    )
                    .expect("Unable to start IPC proxy");
                })?;
        }

        // Set up TCP ROUTER/DEALER with CURVE if we have TCP endpoints
        if has_tcp {
            let mut tcp_clients = self.zmq_context.socket(zmq::ROUTER)?;
            let mut tcp_workers = self.zmq_context.socket(zmq::DEALER)?;

            // Configure CURVE encryption on TCP
            if let Some(ref secret_key) = self.curve_secret_key {
                tcp_clients
                    .set_zap_domain("moor")
                    .context("Failed to set ZAP domain on TCP ROUTER socket")?;
                tcp_clients
                    .set_curve_server(true)
                    .context("Failed to enable CURVE server on TCP ROUTER socket")?;
                let secret_key_bytes =
                    zmq::z85_decode(secret_key).context("Failed to decode Z85 secret key")?;
                tcp_clients
                    .set_curve_secretkey(&secret_key_bytes)
                    .context("Failed to set CURVE secret key on TCP ROUTER socket")?;
                info!("CURVE encryption enabled on TCP RPC server with ZAP authentication");
            }

            // Bind all TCP endpoints to the same socket
            for endpoint in &self.tcp_rpc_endpoints {
                tcp_clients.bind(endpoint)?;
                info!("TCP RPC endpoint bound at {}", endpoint);
            }
            tcp_workers.bind("inproc://rpc-workers-tcp")?;

            // Start TCP proxy
            let mut tcp_control_socket = self.zmq_context.socket(zmq::REP)?;
            tcp_control_socket.bind("inproc://rpc-proxy-tcp-steer")?;
            std::thread::Builder::new()
                .name("moor-rpc-proxy-tcp".to_string())
                .spawn(move || {
                    zmq::proxy_steerable(
                        &mut tcp_clients,
                        &mut tcp_workers,
                        &mut tcp_control_socket,
                    )
                    .expect("Unable to start TCP proxy");
                })?;
        }

        // Calculate number of workers (reserve threads for proxies)
        let num_proxies = (has_ipc as i32) + (has_tcp as i32);
        let num_workers = (num_io_threads - num_proxies).max(1);

        for i in 0..num_workers {
            let handler = message_handler.clone();
            let sched_client = scheduler_client.clone();
            let kill_switch = self.kill_switch.clone();
            let zmq_context = self.zmq_context.clone();
            let connect_ipc = has_ipc;
            let connect_tcp = has_tcp;

            std::thread::Builder::new()
                .name(format!("moor-rpc-srv{i}"))
                .spawn(move || {
                    if let Err(e) = Self::rpc_process_loop(
                        zmq_context,
                        kill_switch,
                        sched_client,
                        handler,
                        connect_ipc,
                        connect_tcp,
                    ) {
                        error!(error = ?e, "RPC process loop failed");
                    }
                })?;
        }

        // Set up control sockets for graceful shutdown
        let ipc_control = if has_ipc {
            let sock = self.zmq_context.socket(zmq::REQ)?;
            sock.connect("inproc://rpc-proxy-ipc-steer")?;
            Some(sock)
        } else {
            None
        };

        let tcp_control = if has_tcp {
            let sock = self.zmq_context.socket(zmq::REQ)?;
            sock.connect("inproc://rpc-proxy-tcp-steer")?;
            Some(sock)
        } else {
            None
        };

        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Kill switch activated, exiting");
                if let Some(ref sock) = ipc_control {
                    sock.send("TERMINATE", 0)?;
                }
                if let Some(ref sock) = tcp_control {
                    sock.send("TERMINATE", 0)?;
                }
                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(1000));
        }
    }
    /// Publish narrative events to clients
    fn publish_narrative_events(
        &self,
        events: &[(Obj, Box<NarrativeEvent>)],
        connections: &dyn crate::connections::ConnectionRegistry,
    ) -> Result<(), eyre::Error> {
        // Lock sockets if available
        let publish_ipc = self.events_publish_ipc.as_ref().map(|p| p.lock().unwrap());
        let publish_tcp = self.events_publish_tcp.as_ref().map(|p| p.lock().unwrap());

        for (player, event) in events {
            let client_ids = connections.client_ids_for(*player)?;

            let client_event = match &event.event {
                Event::SetConnectionOption {
                    connection,
                    option,
                    value,
                } => {
                    if let Some(&client_id) = client_ids.first() {
                        connections.set_client_attribute(
                            client_id,
                            *option,
                            Some(value.clone()),
                        )?;
                    }
                    let value_fb = var_to_flatbuffer_rpc(value)
                        .map_err(|e| eyre::eyre!("Failed to encode var: {}", e))?;
                    moor_rpc::ClientEvent {
                        event: moor_rpc::ClientEventUnion::SetConnectionOptionEvent(Box::new(
                            moor_rpc::SetConnectionOptionEvent {
                                connection_obj: obj_fb(connection),
                                option_name: Box::new(moor_rpc::Symbol {
                                    value: option.as_string(),
                                }),
                                value: Box::new(value_fb),
                            },
                        )),
                    }
                }
                _ => {
                    let narrative_fb = narrative_event_to_flatbuffer_struct(event.as_ref())
                        .map_err(|e| eyre::eyre!("Failed to convert narrative event: {}", e))?;
                    moor_rpc::ClientEvent {
                        event: moor_rpc::ClientEventUnion::NarrativeEventMessage(Box::new(
                            moor_rpc::NarrativeEventMessage {
                                player: obj_fb(player),
                                event: Box::new(narrative_fb),
                            },
                        )),
                    }
                }
            };

            let mut builder = planus::Builder::new();
            let event_bytes = builder.finish(&client_event, None).to_vec();

            for client_id in &client_ids {
                let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
                // Send to IPC socket if available
                if let Some(ref ipc_pub) = publish_ipc {
                    ipc_pub.send_multipart(payload.clone(), 0).map_err(|e| {
                        error!(error = ?e, "Unable to send event to IPC");
                        eyre::eyre!("Delivery error (IPC)")
                    })?;
                }
                // Send to TCP socket if available
                if let Some(ref tcp_pub) = publish_tcp {
                    tcp_pub.send_multipart(payload, 0).map_err(|e| {
                        error!(error = ?e, "Unable to send event to TCP");
                        eyre::eyre!("Delivery error (TCP)")
                    })?;
                }
            }
        }
        Ok(())
    }
    /// Broadcast events to hosts
    fn broadcast_host_event(&self, event: moor_rpc::HostBroadcastEvent) -> Result<(), eyre::Error> {
        let mut builder = planus::Builder::new();
        let event_bytes = builder.finish(&event, None).to_vec();
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];

        // Send to IPC socket if available
        if let Some(ref ipc_pub) = self.events_publish_ipc {
            let ipc_pub = ipc_pub.lock().unwrap();
            ipc_pub.send_multipart(payload.clone(), 0).map_err(|e| {
                error!(error = ?e, "Unable to send host broadcast event to IPC");
                eyre::eyre!("Delivery error (IPC)")
            })?;
        }

        // Send to TCP socket if available
        if let Some(ref tcp_pub) = self.events_publish_tcp {
            let tcp_pub = tcp_pub.lock().unwrap();
            tcp_pub.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send host broadcast event to TCP");
                eyre::eyre!("Delivery error (TCP)")
            })?;
        }

        Ok(())
    }
    /// Publish event to specific client
    fn publish_client_event(
        &self,
        client_id: Uuid,
        event: moor_rpc::ClientEvent,
    ) -> Result<(), eyre::Error> {
        let mut builder = planus::Builder::new();
        let event_bytes = builder.finish(&event, None).to_vec();
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];

        // Send to IPC socket if available
        if let Some(ref ipc_pub) = self.events_publish_ipc {
            let ipc_pub = ipc_pub.lock().unwrap();
            ipc_pub.send_multipart(payload.clone(), 0).map_err(|e| {
                error!(error = ?e, "Unable to send client event to IPC");
                eyre::eyre!("Delivery error (IPC)")
            })?;
        }

        // Send to TCP socket if available
        if let Some(ref tcp_pub) = self.events_publish_tcp {
            let tcp_pub = tcp_pub.lock().unwrap();
            tcp_pub.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send client event to TCP");
                eyre::eyre!("Delivery error (TCP)")
            })?;
        }

        Ok(())
    }
    /// Broadcast events to all clients
    fn broadcast_client_event(
        &self,
        event: moor_rpc::ClientsBroadcastEvent,
    ) -> Result<(), eyre::Error> {
        let mut builder = planus::Builder::new();
        let event_bytes = builder.finish(&event, None).to_vec();
        let payload = vec![CLIENT_BROADCAST_TOPIC.to_vec(), event_bytes];

        // Send to IPC socket if available
        if let Some(ref ipc_pub) = self.events_publish_ipc {
            let ipc_pub = ipc_pub.lock().unwrap();
            ipc_pub.send_multipart(payload.clone(), 0).map_err(|e| {
                error!(error = ?e, "Unable to send client broadcast event to IPC");
                eyre::eyre!("Delivery error (IPC)")
            })?;
        }

        // Send to TCP socket if available
        if let Some(ref tcp_pub) = self.events_publish_tcp {
            let tcp_pub = tcp_pub.lock().unwrap();
            tcp_pub.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send client broadcast event to TCP");
                eyre::eyre!("Delivery error (TCP)")
            })?;
        }

        Ok(())
    }
}
