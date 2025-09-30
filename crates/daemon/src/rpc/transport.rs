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
use moor_common::{schema::rpc as moor_rpc, tasks::NarrativeEvent};
use moor_kernel::SchedulerClient;
use moor_rpc::{HostToDaemonMessageRef, MessageTypeRef};
use moor_var::Obj;
use rpc_common::{CLIENT_BROADCAST_TOPIC, HOST_BROADCAST_TOPIC, RpcMessageError};
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
    events_publish: Arc<Mutex<Socket>>,
}

impl RpcTransport {
    pub fn new(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        narrative_endpoint: &str,
    ) -> Result<Self, eyre::Error> {
        // Create the socket for publishing narrative events
        let publish = zmq_context
            .socket(zmq::SocketType::PUB)
            .context("Unable to create ZMQ PUB socket")?;
        publish
            .bind(narrative_endpoint)
            .context("Unable to bind ZMQ PUB socket")?;

        let events_publish = Arc::new(Mutex::new(publish));

        Ok(Self {
            zmq_context,
            kill_switch,
            events_publish,
        })
    }

    /// Individual RPC process loop that runs in worker threads
    fn rpc_process_loop(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        scheduler_client: SchedulerClient,
        message_handler: Arc<dyn MessageHandler>,
    ) -> eyre::Result<()> {
        let rpc_socket = zmq_context.socket(zmq::REP)?;
        rpc_socket.connect("inproc://rpc-workers")?;

        loop {
            if kill_switch.load(Ordering::Relaxed) {
                return Ok(());
            }

            let poll_result = rpc_socket
                .poll(zmq::POLLIN, 100)
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
                // Extract host token from discriminator
                let host_token_ref = match host_msg.host_token() {
                    Ok(t) => t,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Missing host token")?;
                        return Ok(());
                    }
                };
                let host_token_string = match host_token_ref.token() {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Invalid host token")?;
                        return Ok(());
                    }
                };
                let host_token = rpc_common::HostToken(host_token_string);

                // Validate host token
                if let Err(e) = message_handler.validate_host_token(&host_token) {
                    Self::reply_invalid_request(
                        rpc_socket,
                        &format!("Invalid host token received: {e}"),
                    )?;
                    return Ok(());
                }

                // Decode the actual HostToDaemonMessage from request_body
                let Ok(host_message_fb) = HostToDaemonMessageRef::read_as_root(request_body) else {
                    Self::reply_invalid_request(rpc_socket, "Could not decode host message")?;
                    return Ok(());
                };

                // Process
                let response = message_handler.handle_host_message(host_token, host_message_fb);
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
                // TODO: Convert RpcMessageError to FlatBuffer properly
                let mut builder = planus::Builder::new();
                let error_fb = moor_rpc::RpcMessageError {
                    error_code: moor_rpc::RpcMessageErrorCode::InternalError,
                    message: Some(format!("{:?}", e)),
                    scheduler_error: None,
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
                // TODO: Convert RpcMessageError to FlatBuffer properly
                let mut builder = planus::Builder::new();
                let error_fb = moor_rpc::RpcMessageError {
                    error_code: moor_rpc::RpcMessageErrorCode::InternalError,
                    message: Some(format!("{:?}", e)),
                    scheduler_error: None,
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
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
        message_handler: Arc<dyn MessageHandler>,
    ) -> eyre::Result<()> {
        let num_io_threads = self.zmq_context.get_io_threads()?;
        info!("0mq server listening on {rpc_endpoint} with {num_io_threads} IO threads");

        let mut clients = self.zmq_context.socket(zmq::ROUTER)?;
        let mut workers = self.zmq_context.socket(zmq::DEALER)?;

        clients.bind(&rpc_endpoint)?;
        workers.bind("inproc://rpc-workers")?;

        // Start N RPC servers in a background thread. We match the # of IO threads, minus 1
        // which we use for the proxy.
        for i in 0..num_io_threads - 1 {
            let handler = message_handler.clone();
            let sched_client = scheduler_client.clone();
            let kill_switch = self.kill_switch.clone();
            let zmq_context = self.zmq_context.clone();

            std::thread::Builder::new()
                .name(format!("moor-rpc-srv{i}"))
                .spawn(move || {
                    if let Err(e) =
                        Self::rpc_process_loop(zmq_context, kill_switch, sched_client, handler)
                    {
                        error!(error = ?e, "RPC process loop failed");
                    }
                })?;
        }

        // Start the proxy in a background thread, which will route messages between
        // clients and workers.
        let mut control_socket = self.zmq_context.socket(zmq::REP)?;
        control_socket.bind("inproc://rpc-proxy-steer")?;
        std::thread::Builder::new()
            .name("moor-rpc-proxy".to_string())
            .spawn(move || {
                zmq::proxy_steerable(&mut clients, &mut workers, &mut control_socket)
                    .expect("Unable to start proxy");
            })?;

        // Control the proxy
        let control_socket = self.zmq_context.socket(zmq::REQ)?;
        control_socket.connect("inproc://rpc-proxy-steer")?;
        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Kill switch activated, exiting");
                control_socket.send("TERMINATE", 0)?;
                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
    /// Publish narrative events to clients
    fn publish_narrative_events(
        &self,
        events: &[(Obj, Box<NarrativeEvent>)],
        connections: &dyn crate::connections::ConnectionRegistry,
    ) -> Result<(), eyre::Error> {
        let publish = self.events_publish.lock().unwrap();
        for (player, event) in events {
            let client_ids = connections.client_ids_for(*player)?;

            // Build FlatBuffer ClientEvent directly
            let narrative_fb = rpc_common::narrative_event_to_flatbuffer_struct(event.as_ref())
                .map_err(|e| eyre::eyre!("Failed to convert narrative event: {}", e))?;
            let client_event = moor_rpc::ClientEvent {
                event: moor_rpc::ClientEventUnion::NarrativeEventMessage(Box::new(
                    moor_rpc::NarrativeEventMessage {
                        player: Box::new(rpc_common::obj_to_flatbuffer_struct(player)),
                        event: Box::new(narrative_fb),
                    },
                )),
            };

            // Serialize to bytes
            let mut builder = planus::Builder::new();
            let event_bytes = builder.finish(&client_event, None).to_vec();

            for client_id in &client_ids {
                let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
                publish.send_multipart(payload, 0).map_err(|e| {
                    error!(error = ?e, "Unable to send narrative event");
                    eyre::eyre!("Delivery error")
                })?;
            }
        }
        Ok(())
    }
    /// Broadcast events to hosts
    fn broadcast_host_event(&self, event: moor_rpc::HostBroadcastEvent) -> Result<(), eyre::Error> {
        let mut builder = planus::Builder::new();
        let event_bytes = builder.finish(&event, None).to_vec();
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];

        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send host broadcast event");
            eyre::eyre!("Delivery error")
        })?;

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

        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send client event");
            eyre::eyre!("Delivery error")
        })?;

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

        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send client broadcast event");
            eyre::eyre!("Delivery error")
        })?;

        Ok(())
    }
}
