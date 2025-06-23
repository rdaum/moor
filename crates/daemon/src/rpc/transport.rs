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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;
use zmq::Socket;

use super::message_handler::MessageHandler;
use moor_kernel::SchedulerClient;
use moor_common::tasks::NarrativeEvent;
use moor_var::Obj;
use rpc_common::{
    ClientEvent, ClientsBroadcastEvent, DaemonToClientReply, DaemonToHostReply, 
    HostBroadcastEvent, HostToDaemonMessage, MessageType, ReplyResult, 
    RpcMessageError, CLIENT_BROADCAST_TOPIC, HOST_BROADCAST_TOPIC,
};

/// ZMQ transport layer that handles socket management and message routing
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


    /// Start the request processing loop with ZMQ proxy architecture
    pub fn start_request_loop<H: MessageHandler + 'static>(
        &self,
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
        message_handler: Arc<H>,
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
                    if let Err(e) = Self::rpc_process_loop(
                        zmq_context,
                        kill_switch,
                        sched_client,
                        handler,
                    ) {
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

    /// Individual RPC process loop that runs in worker threads
    fn rpc_process_loop<H: MessageHandler>(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        scheduler_client: SchedulerClient,
        message_handler: Arc<H>,
    ) -> eyre::Result<()> {
        let rpc_socket = zmq_context.socket(zmq::REP)?;
        rpc_socket.connect("inproc://rpc-workers")?;
        
        loop {
            if kill_switch.load(Ordering::Relaxed) {
                return Ok(());
            }

            let poll_result = rpc_socket
                .poll(zmq::POLLIN, 10)
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
                        &message_handler,
                    ) {
                        error!(error = ?e, "Error processing request");
                    }
                }
            }
        }
    }

    /// Process a single request message
    fn process_request<H: MessageHandler>(
        rpc_socket: &Socket,
        request: Vec<Vec<u8>>,
        scheduler_client: &SchedulerClient,
        message_handler: &Arc<H>,
    ) -> eyre::Result<()> {
        // Components are: [msg_type, request_body]
        if request.len() != 2 {
            Self::reply_invalid_request(rpc_socket, "Incorrect message length")?;
            return Ok(());
        }

        let (msg_type, request_body) = (&request[0], &request[1]);

        // Decode the msg_type
        let msg_type: MessageType =
            match bincode::decode_from_slice(msg_type, bincode::config::standard()) {
                Ok((msg_type, _)) => msg_type,
                Err(_) => {
                    Self::reply_invalid_request(rpc_socket, "Could not decode message type")?;
                    return Ok(());
                }
            };

        match msg_type {
            MessageType::HostToDaemon(host_token) => {
                // Validate host token, and process host message
                if let Err(e) = message_handler.validate_host_token(&host_token) {
                    Self::reply_invalid_request(
                        rpc_socket,
                        &format!("Invalid host token received: {}", e),
                    )?;
                    return Ok(());
                }

                // Decode
                let host_message: HostToDaemonMessage = match bincode::decode_from_slice(
                    request_body,
                    bincode::config::standard(),
                ) {
                    Ok((host_message, _)) => host_message,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Could not decode host message")?;
                        return Ok(());
                    }
                };

                // Process
                let response = message_handler.handle_host_message(host_token, host_message);
                match Self::pack_host_response(response) {
                    Ok(response) => {
                        // Reply
                        rpc_socket.send_multipart(vec![response], 0)?;
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to encode host response");
                        // Skip sending response if encoding failed
                    }
                }
            }
            MessageType::HostClientToDaemon(client_id) => {
                // Parse the client_id as a uuid
                let client_id = match Uuid::from_slice(&client_id) {
                    Ok(client_id) => client_id,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Bad client id")?;
                        return Ok(());
                    }
                };

                // Decode 'request_body' as a bincode'd ClientEvent
                let request = match bincode::decode_from_slice(
                    request_body,
                    bincode::config::standard(),
                ) {
                    Ok((request, _)) => request,
                    Err(_) => {
                        Self::reply_invalid_request(rpc_socket, "Could not decode request body")?;
                        return Ok(());
                    }
                };

                // Process the request
                let response = message_handler.handle_client_message(
                    scheduler_client.clone(),
                    client_id,
                    request,
                );
                match Self::pack_client_response(response) {
                    Ok(response) => {
                        rpc_socket.send_multipart(vec![response], 0)?;
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to encode client response");
                        // Skip sending response if encoding failed
                    }
                }
            }
        }

        Ok(())
    }

    fn reply_invalid_request(socket: &Socket, reason: &str) -> eyre::Result<()> {
        warn!("Invalid request received, replying with error: {reason}");
        let response = Self::pack_client_response(Err(RpcMessageError::InvalidRequest(
            reason.to_string(),
        )))?;
        socket.send_multipart(vec![response], 0)?;
        Ok(())
    }

    fn pack_client_response(result: Result<DaemonToClientReply, RpcMessageError>) -> Result<Vec<u8>, eyre::Error> {
        let rpc_result = match result {
            Ok(r) => ReplyResult::ClientSuccess(r),
            Err(e) => ReplyResult::Failure(e),
        };
        bincode::encode_to_vec(&rpc_result, bincode::config::standard())
            .context("Failed to encode client response")
    }

    fn pack_host_response(result: Result<DaemonToHostReply, RpcMessageError>) -> Result<Vec<u8>, eyre::Error> {
        let rpc_result = match result {
            Ok(r) => ReplyResult::HostSuccess(r),
            Err(e) => ReplyResult::Failure(e),
        };
        bincode::encode_to_vec(&rpc_result, bincode::config::standard())
            .context("Failed to encode host response")
    }

    /// Publish narrative events to clients
    pub fn publish_narrative_events(
        &self,
        events: &[(Obj, Box<NarrativeEvent>)],
        connections: &dyn crate::connections::ConnectionRegistry,
    ) -> Result<(), eyre::Error> {
        let publish = self.events_publish.lock().unwrap();
        for (player, event) in events {
            let client_ids = connections.client_ids_for(*player)?;
            let event = ClientEvent::Narrative(*player, event.as_ref().clone());
            let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
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
    pub fn broadcast_host_event(&self, event: HostBroadcastEvent) -> Result<(), eyre::Error> {
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];

        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send host broadcast event");
            eyre::eyre!("Delivery error")
        })?;

        Ok(())
    }

    /// Publish event to specific client
    pub fn publish_client_event(&self, client_id: Uuid, event: ClientEvent) -> Result<(), eyre::Error> {
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard())
            .context("Unable to serialize client event")?;
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        
        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send client event");
            eyre::eyre!("Delivery error")
        })?;

        Ok(())
    }

    /// Broadcast events to all clients
    pub fn broadcast_client_event(&self, event: ClientsBroadcastEvent) -> Result<(), eyre::Error> {
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();
        let payload = vec![CLIENT_BROADCAST_TOPIC.to_vec(), event_bytes];

        let publish = self.events_publish.lock().unwrap();
        publish.send_multipart(payload, 0).map_err(|e| {
            error!(error = ?e, "Unable to send client broadcast event");
            eyre::eyre!("Delivery error")
        })?;

        Ok(())
    }
}