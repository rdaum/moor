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

//! The core of the server logic for the RPC daemon

use crossbeam_channel::{Receiver, Select, Sender};
use eyre::{Context, Error};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use crate::connections::ConnectionsDB;
use crate::connections_fjall::ConnectionsFjall;
use crate::rpc_hosts::Hosts;
use crate::rpc_session::{RpcSession, SessionActions};
use moor_common::model::{Named, ObjectRef, PropFlag, ValSet, VerbFlag, preposition_to_string};
use moor_common::tasks::SchedulerError::CommandExecutionError;
use moor_common::tasks::SessionError;
use moor_common::tasks::SessionError::DeliveryError;
use moor_common::tasks::{CommandError, NarrativeEvent, SchedulerError, TaskId};
use moor_common::util::parse_into_words;
use moor_db::db_counters;
use moor_kernel::SchedulerClient;
use moor_kernel::config::Config;
use moor_kernel::tasks::{TaskHandle, TaskResult, sched_counters};
use moor_kernel::vm::builtins::bf_perf_counters;
use moor_var::SYSTEM_OBJECT;
use moor_var::{List, Variant};
use moor_var::{Obj, Var};
use moor_var::{Symbol, v_obj, v_str};
use rpc_common::DaemonToClientReply::{LoginResult, NewConnection};
use rpc_common::{
    AuthToken, CLIENT_BROADCAST_TOPIC, ClientEvent, ClientToken, ClientsBroadcastEvent,
    ConnectType, DaemonToClientReply, DaemonToHostReply, EntityType, HOST_BROADCAST_TOPIC,
    HostBroadcastEvent, HostClientToDaemonMessage, HostToDaemonMessage, HostToken, HostType,
    MOOR_AUTH_TOKEN_FOOTER, MOOR_HOST_TOKEN_FOOTER, MOOR_SESSION_TOKEN_FOOTER, MessageType,
    PropInfo, ReplyResult, RpcMessageError, VerbInfo, VerbProgramResponse,
};
use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use rusty_paseto::prelude::Key;
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use zmq::{Socket, SocketType};

// TODO: split up the transport/rpc layer from the session handling / events logic better, and get
//  rid of the last vestiges of Arc<Self>

pub struct RpcServer {
    zmq_context: zmq::Context,
    config: Arc<Config>,
    public_key: Key<32>,
    private_key: Key<64>,

    pub(crate) kill_switch: Arc<AtomicBool>,

    connections: Box<dyn ConnectionsDB + Send + Sync>,
    task_handles: Mutex<HashMap<TaskId, (Uuid, TaskHandle)>>,
    mailbox_receive: Receiver<SessionActions>,

    pub(crate) hosts: RwLock<Hosts>,

    host_token_cache: RwLock<HashMap<HostToken, (Instant, HostType)>>,
    auth_token_cache: RwLock<HashMap<AuthToken, (Instant, Obj)>>,
    client_token_cache: RwLock<HashMap<ClientToken, Instant>>,

    pub(crate) mailbox_sender: Sender<SessionActions>,
    pub(crate) events_publish: Mutex<Socket>,
}

/// If we don't hear from a host in this time, we consider it dead and its listeners gone.
pub const HOST_TIMEOUT: Duration = Duration::from_secs(10);

fn pack_client_response(result: Result<DaemonToClientReply, RpcMessageError>) -> Vec<u8> {
    let rpc_result = match result {
        Ok(r) => ReplyResult::ClientSuccess(r),
        Err(e) => ReplyResult::Failure(e),
    };
    bincode::encode_to_vec(&rpc_result, bincode::config::standard()).unwrap()
}

fn pack_host_response(result: Result<DaemonToHostReply, RpcMessageError>) -> Vec<u8> {
    let rpc_result = match result {
        Ok(r) => ReplyResult::HostSuccess(r),
        Err(e) => ReplyResult::Failure(e),
    };
    bincode::encode_to_vec(&rpc_result, bincode::config::standard()).unwrap()
}

impl RpcServer {
    pub fn new(
        public_key: Key<32>,
        private_key: Key<64>,
        connections_db_path: PathBuf,
        zmq_context: zmq::Context,
        narrative_endpoint: &str,
        // For determining the flavor for the connections database.
        config: Arc<Config>,
    ) -> Self {
        info!(
            "Creating new RPC server; with {} ZMQ IO threads...",
            zmq_context.get_io_threads().unwrap()
        );

        // The socket for publishing narrative events.
        let publish = zmq_context
            .socket(SocketType::PUB)
            .expect("Unable to create ZMQ PUB socket");
        publish
            .bind(narrative_endpoint)
            .expect("Unable to bind ZMQ PUB socket");
        let connections = Box::new(ConnectionsFjall::open(Some(&connections_db_path)));
        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().len()
        );
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (mailbox_sender, mailbox_receive) = crossbeam_channel::unbounded();
        Self {
            public_key,
            private_key,
            connections,
            events_publish: Mutex::new(publish),
            zmq_context,
            task_handles: Default::default(),
            config,
            kill_switch,
            hosts: Default::default(),
            mailbox_sender,
            mailbox_receive,
            host_token_cache: RwLock::new(Default::default()),
            auth_token_cache: RwLock::new(Default::default()),
            client_token_cache: RwLock::new(Default::default()),
        }
    }

    pub fn request_loop(
        self: Arc<Self>,
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
    ) -> eyre::Result<()> {
        // Start up the ping-ponger timer in a background thread...
        let t_rpc_server = self.clone();
        std::thread::Builder::new()
            .name("rpc-ping-pong".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(5));
                    t_rpc_server.ping_pong().expect("Unable to play ping-pong");
                }
            })?;

        let num_io_threads = self.zmq_context.get_io_threads()?;
        info!("0mq server listening on {rpc_endpoint} with {num_io_threads} IO threads",);

        let mut clients = self.zmq_context.socket(zmq::ROUTER)?;
        let mut workers = self.zmq_context.socket(zmq::DEALER)?;

        clients.bind(&rpc_endpoint)?;
        workers.bind("inproc://rpc-workers")?;

        // Start N  RPC servers in a background thread. We match the # of IO threads, minus 1
        // which we use for the proxy.
        for i in 0..num_io_threads - 1 {
            let rpc_this = self.clone();
            let sched_client = scheduler_client.clone();
            std::thread::Builder::new()
                .name(format!("moor-rpc-srv{i}"))
                .spawn(move || {
                    rpc_this
                        .clone()
                        .rpc_process_loop(sched_client)
                        .expect("Unable to process RPC requests");
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

        // Final piece processes the session events and task completions, and the kill switch.
        let control_socket = self.zmq_context.socket(zmq::REQ)?;
        control_socket.connect("inproc://rpc-proxy-steer")?;
        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Kill switch activated, exiting");
                control_socket.send("TERMINATE", 0)?;
                return Ok(());
            }

            // Check the mailbox
            if let Ok(session_event) = self.mailbox_receive.recv() {
                match session_event {
                    SessionActions::PublishNarrativeEvents(events) => {
                        if let Err(e) = self.publish_narrative_events(&events) {
                            error!(error = ?e, "Unable to publish narrative events");
                        }
                    }
                    SessionActions::RequestClientInput(client_id, connection, input_request_id) => {
                        if let Err(e) =
                            self.request_client_input(client_id, connection, input_request_id)
                        {
                            error!(error = ?e, "Unable to request client input");
                        }
                    }
                    SessionActions::SendSystemMessage(client_id, connection, message) => {
                        if let Err(e) = self.send_system_message(client_id, connection, message) {
                            error!(error = ?e, "Unable to send system message");
                        }
                    }
                    SessionActions::RequestConnectionName(_client_id, connection, reply) => {
                        let connection_send_result = match self.connection_name_for(connection) {
                            Ok(c) => reply.send(Ok(c)),
                            Err(e) => {
                                error!(error = ?e, "Unable to get connection name");
                                reply.send(Err(e))
                            }
                        };
                        if let Err(e) = connection_send_result {
                            error!(error = ?e, "Unable to send connection name");
                        }
                    }
                    SessionActions::Disconnect(_client_id, connection) => {
                        if let Err(e) = self.disconnect(connection) {
                            error!(error = ?e, "Unable to disconnect client");
                        }
                    }
                    SessionActions::RequestConnectedPlayers(_client_id, reply) => {
                        let connected_players_send_result = match self.connected_players() {
                            Ok(c) => reply.send(Ok(c)),
                            Err(e) => {
                                error!(error = ?e, "Unable to get connected players");
                                reply.send(Err(e))
                            }
                        };
                        if let Err(e) = connected_players_send_result {
                            error!(error = ?e, "Unable to send connected players");
                        }
                    }
                    SessionActions::RequestConnectedSeconds(_client_id, connection, reply) => {
                        let connected_seconds_send_result =
                            match self.connected_seconds_for(connection) {
                                Ok(c) => reply.send(Ok(c)),
                                Err(e) => {
                                    error!(error = ?e, "Unable to get connected seconds");
                                    reply.send(Err(e))
                                }
                            };
                        if let Err(e) = connected_seconds_send_result {
                            error!(error = ?e, "Unable to send connected seconds");
                        }
                    }
                    SessionActions::RequestIdleSeconds(_client_id, connection, reply) => {
                        let idle_seconds_send_result = match self.idle_seconds_for(connection) {
                            Ok(c) => reply.send(Ok(c)),
                            Err(e) => {
                                error!(error = ?e, "Unable to get idle seconds");
                                reply.send(Err(e))
                            }
                        };
                        if let Err(e) = idle_seconds_send_result {
                            error!(error = ?e, "Unable to send idle seconds");
                        }
                    }
                }
            }
            // Check any task handles for completion.
            // TODO: rewrite the task completion checking here to not poll and use a lock.
            //  we can probably do something much nicer with channels, etc.
            self.clone()
                .process_task_completions(Duration::from_millis(10));
        }
    }

    fn rpc_process_loop(self: Arc<Self>, scheduler_client: SchedulerClient) -> eyre::Result<()> {
        let this = self.clone();
        let rpc_socket = this.zmq_context.clone().socket(zmq::REP)?;
        rpc_socket.connect("inproc://rpc-workers")?;
        loop {
            if this.kill_switch.load(Ordering::Relaxed) {
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
                    // Components are: [msg_type,  request_body]

                    if request.len() != 2 {
                        Self::reply_invalid_request(&rpc_socket, "Incorrect message length")?;
                        continue;
                    }

                    let (msg_type, request_body) = (&request[0], &request[1]);

                    // Decode the msg_type
                    let msg_type: MessageType =
                        match bincode::decode_from_slice(msg_type, bincode::config::standard()) {
                            Ok((msg_type, _)) => msg_type,
                            Err(_) => {
                                Self::reply_invalid_request(
                                    &rpc_socket,
                                    "Could not decode message type",
                                )?;
                                continue;
                            }
                        };

                    match msg_type {
                        MessageType::HostToDaemon(host_token) => {
                            // Validate host token, and process host message...
                            // The host token is a Paseto Token signed with our same keypair.
                            if let Err(e) = self.validate_host_token(&host_token) {
                                Self::reply_invalid_request(
                                    &rpc_socket,
                                    &format!("Invalid host token received: {}", e),
                                )?;
                                continue;
                            }

                            // Decode.
                            let host_message: HostToDaemonMessage = match bincode::decode_from_slice(
                                request_body,
                                bincode::config::standard(),
                            ) {
                                Ok((host_message, _)) => host_message,
                                Err(_) => {
                                    Self::reply_invalid_request(
                                        &rpc_socket,
                                        "Could not decode host message",
                                    )?;
                                    continue;
                                }
                            };

                            // Process
                            let response =
                                this.clone().process_host_request(host_token, host_message);

                            // Reply with Ack.
                            rpc_socket.send_multipart(vec![response], 0)?;
                        }
                        MessageType::HostClientToDaemon(client_id) => {
                            // Parse the client_id as a uuid
                            let client_id = match Uuid::from_slice(&client_id) {
                                Ok(client_id) => client_id,
                                Err(_) => {
                                    Self::reply_invalid_request(&rpc_socket, "Bad client id")?;
                                    continue;
                                }
                            };

                            // Decode 'request_body' as a bincode'd ClientEvent.
                            let request = match bincode::decode_from_slice(
                                request_body,
                                bincode::config::standard(),
                            ) {
                                Ok((request, _)) => request,
                                Err(_) => {
                                    Self::reply_invalid_request(
                                        &rpc_socket,
                                        "Could not decode request body",
                                    )?;
                                    continue;
                                }
                            };

                            // The remainder of the payload are all the request arguments, which vary depending
                            // on the type.
                            let response = this.clone().process_request(
                                scheduler_client.clone(),
                                client_id,
                                request,
                            );
                            let response = pack_client_response(response);
                            rpc_socket.send_multipart(vec![response], 0)?;
                        }
                    }
                }
            }
        }
    }

    fn reply_invalid_request(socket: &Socket, reason: &str) -> eyre::Result<()> {
        warn!("Invalid request received, replying with error: {reason}");
        socket.send_multipart(
            vec![pack_client_response(Err(RpcMessageError::InvalidRequest(
                reason.to_string(),
            )))],
            0,
        )?;
        Ok(())
    }

    fn client_auth(&self, token: ClientToken, client_id: Uuid) -> Result<Obj, RpcMessageError> {
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(RpcMessageError::NoConnection);
        };

        self.validate_client_token(token, client_id)?;
        Ok(connection)
    }

    fn process_host_request(
        &self,
        host_token: HostToken,
        host_message: HostToDaemonMessage,
    ) -> Vec<u8> {
        match host_message {
            HostToDaemonMessage::RegisterHost(_, host_type, listeners) => {
                info!(
                    "Host {} registered with {} listeners",
                    host_token.0,
                    listeners.len()
                );
                let mut hosts = self.hosts.write().unwrap();
                // Record this as a ping. If it's a new host, log that.
                hosts.receive_ping(host_token, host_type, listeners);

                // Reply with an ack.
                pack_host_response(Ok(DaemonToHostReply::Ack))
            }
            HostToDaemonMessage::HostPong(_, host_type, listeners) => {
                // Record this to our hosts DB.
                // This will update the last-seen time for the host and its listeners-set.
                let num_listeners = listeners.len();
                let mut hosts = self.hosts.write().unwrap();
                if hosts.receive_ping(host_token.clone(), host_type, listeners) {
                    info!(
                        "Host {} registered with {} listeners",
                        host_token.0, num_listeners
                    );
                }

                // Reply with an ack.
                pack_host_response(Ok(DaemonToHostReply::Ack))
            }
            HostToDaemonMessage::RequestPerformanceCounters => {
                let mut all_counters = vec![];
                let mut sch = vec![];
                for c in sched_counters().all_counters() {
                    sch.push((
                        c.operation,
                        c.invocations.sum(),
                        c.cumulative_duration_nanos.sum(),
                    ));
                }
                all_counters.push((Symbol::mk("sched"), sch));

                let mut db = vec![];
                for c in db_counters().all_counters() {
                    db.push((
                        c.operation,
                        c.invocations.sum(),
                        c.cumulative_duration_nanos.sum(),
                    ));
                }
                all_counters.push((Symbol::mk("db"), db));

                let mut bf = vec![];
                for c in bf_perf_counters().all_counters() {
                    bf.push((
                        c.operation,
                        c.invocations.sum(),
                        c.cumulative_duration_nanos.sum(),
                    ));
                }
                all_counters.push((Symbol::mk("bf"), bf));

                pack_host_response(Ok(DaemonToHostReply::PerfCounters(
                    SystemTime::now(),
                    all_counters,
                )))
            }
            HostToDaemonMessage::DetachHost => {
                let mut hosts = self.hosts.write().unwrap();
                hosts.unregister_host(&host_token);
                pack_host_response(Ok(DaemonToHostReply::Ack))
            }
        }
    }

    /// Process a request (originally ZMQ REQ) and produce a reply (becomes ZMQ REP)
    fn process_request(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        request: HostClientToDaemonMessage,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        match request {
            HostClientToDaemonMessage::ConnectionEstablish(hostname) => {
                let oid = self.connections.new_connection(client_id, hostname, None)?;
                let token = self.make_client_token(client_id);
                Ok(NewConnection(token, oid))
            }
            HostClientToDaemonMessage::Attach(
                auth_token,
                connect_type,
                handler_object,
                hostname,
            ) => {
                // Validate the auth token, and get the player.
                let player = self.validate_auth_token(auth_token, None)?;

                self.connections
                    .new_connection(client_id, hostname, Some(player.clone()))?;
                let client_token = self.make_client_token(client_id);

                if let Some(connect_type) = connect_type {
                    if let Err(e) = self.submit_connected_task(
                        &handler_object,
                        scheduler_client,
                        client_id,
                        &player,
                        connect_type,
                    ) {
                        error!(error = ?e, "Error submitting user_connected task");

                        // Note we still continue to return a successful login result here, hoping for the best
                        // but we do log the error.
                    }
                }
                Ok(DaemonToClientReply::AttachResult(Some((
                    client_token,
                    player.clone(),
                ))))
            }
            // Bodacious Totally Awesome Hey Dudes Have Mr Pong's Chinese Food
            HostClientToDaemonMessage::ClientPong(token, _client_sys_time, _, _, _) => {
                // Always respond with a ThanksPong, even if it's somebody we don't know.
                // Can easily be a connection that was in the middle of negotiation at the time the
                // ping was sent out, or dangling in some other way.
                let response = Ok(DaemonToClientReply::ThanksPong(SystemTime::now()));

                let connection = self.client_auth(token, client_id)?;
                // Let 'connections' know that the connection is still alive.
                let Ok(_) = self.connections.notify_is_alive(client_id, connection) else {
                    warn!("Unable to notify connection is alive: {}", client_id);
                    return response;
                };
                response
            }
            HostClientToDaemonMessage::RequestSysProp(token, object, property) => {
                let connection = self.client_auth(token, client_id)?;

                self.request_sys_prop(scheduler_client, connection, object, property)
            }
            HostClientToDaemonMessage::LoginCommand(token, handler_object, args, attach) => {
                let connection = self.client_auth(token, client_id)?;

                self.perform_login(
                    &handler_object,
                    scheduler_client,
                    client_id,
                    &connection,
                    args,
                    attach,
                )
            }
            HostClientToDaemonMessage::Command(token, auth_token, handler_object, command) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                self.perform_command(
                    scheduler_client,
                    client_id,
                    &handler_object,
                    &connection,
                    command,
                )
            }
            HostClientToDaemonMessage::RequestedInput(token, auth_token, request_id, input) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                self.respond_input(scheduler_client, client_id, &connection, request_id, input)
            }
            HostClientToDaemonMessage::OutOfBand(token, auth_token, handler_object, command) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                self.perform_out_of_band(
                    scheduler_client,
                    &handler_object,
                    client_id,
                    &connection,
                    command,
                )
            }

            HostClientToDaemonMessage::Eval(token, auth_token, evalstr) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;
                self.eval(scheduler_client, client_id, &connection, evalstr)
            }

            HostClientToDaemonMessage::InvokeVerb(token, auth_token, object, verb, args) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                self.invoke_verb(
                    scheduler_client,
                    client_id,
                    &connection,
                    &object,
                    verb,
                    args,
                )
            }

            HostClientToDaemonMessage::Retrieve(token, auth_token, who, retr_type, what) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                match retr_type {
                    EntityType::Property => {
                        let (propdef, propperms, value) = scheduler_client
                            .request_property(&connection, &connection, &who, what)
                            .map_err(|e| {
                                error!(error = ?e, "Error requesting property");
                                RpcMessageError::EntityRetrievalError(
                                    "error requesting property".to_string(),
                                )
                            })?;
                        Ok(DaemonToClientReply::PropertyValue(
                            PropInfo {
                                definer: propdef.definer(),
                                location: propdef.location(),
                                name: Symbol::mk(propdef.name()),
                                owner: propperms.owner(),
                                r: propperms.flags().contains(PropFlag::Read),
                                w: propperms.flags().contains(PropFlag::Write),
                                chown: propperms.flags().contains(PropFlag::Chown),
                            },
                            value,
                        ))
                    }
                    EntityType::Verb => {
                        let (verbdef, code) = scheduler_client
                            .request_verb(&connection, &connection, &who, what)
                            .map_err(|e| {
                                error!(error = ?e, "Error requesting verb");
                                RpcMessageError::EntityRetrievalError(
                                    "error requesting verb".to_string(),
                                )
                            })?;
                        let argspec = verbdef.args();
                        let arg_spec = vec![
                            Symbol::mk(argspec.dobj.to_string()),
                            Symbol::mk(preposition_to_string(&argspec.prep)),
                            Symbol::mk(argspec.iobj.to_string()),
                        ];
                        Ok(DaemonToClientReply::VerbValue(
                            VerbInfo {
                                location: verbdef.location(),
                                owner: verbdef.owner(),
                                names: verbdef.names().iter().map(|s| Symbol::mk(s)).collect(),
                                r: verbdef.flags().contains(VerbFlag::Read),
                                w: verbdef.flags().contains(VerbFlag::Write),
                                x: verbdef.flags().contains(VerbFlag::Exec),
                                d: verbdef.flags().contains(VerbFlag::Debug),
                                arg_spec,
                            },
                            code,
                        ))
                    }
                }
            }
            HostClientToDaemonMessage::Resolve(token, auth_token, objref) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                let resolved = scheduler_client
                    .resolve_object(connection, objref)
                    .map_err(|e| {
                        error!(error = ?e, "Error resolving object");
                        RpcMessageError::EntityRetrievalError("error resolving object".to_string())
                    })?;

                Ok(DaemonToClientReply::ResolveResult(resolved))
            }
            HostClientToDaemonMessage::Properties(token, auth_token, obj) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                let props = scheduler_client
                    .request_properties(&connection, &connection, &obj)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting properties");
                        RpcMessageError::EntityRetrievalError(
                            "error requesting properties".to_string(),
                        )
                    })?;

                let props = props
                    .iter()
                    .map(|(propdef, propperms)| PropInfo {
                        definer: propdef.definer(),
                        location: propdef.location(),
                        name: Symbol::mk(propdef.name()),
                        owner: propperms.owner(),
                        r: propperms.flags().contains(PropFlag::Read),
                        w: propperms.flags().contains(PropFlag::Write),
                        chown: propperms.flags().contains(PropFlag::Chown),
                    })
                    .collect();

                Ok(DaemonToClientReply::Properties(props))
            }
            HostClientToDaemonMessage::Verbs(token, auth_token, obj) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                let verbs = scheduler_client
                    .request_verbs(&connection, &connection, &obj)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting verbs");
                        RpcMessageError::EntityRetrievalError("error requesting verbs".to_string())
                    })?;

                let verbs = verbs
                    .iter()
                    .map(|v| VerbInfo {
                        location: v.location(),
                        owner: v.owner(),
                        names: v.names().iter().map(|s| Symbol::mk(s)).collect(),
                        r: v.flags().contains(VerbFlag::Read),
                        w: v.flags().contains(VerbFlag::Write),
                        x: v.flags().contains(VerbFlag::Exec),
                        d: v.flags().contains(VerbFlag::Debug),
                        arg_spec: vec![
                            Symbol::mk(v.args().dobj.to_string()),
                            Symbol::mk(preposition_to_string(&v.args().prep)),
                            Symbol::mk(v.args().iobj.to_string()),
                        ],
                    })
                    .collect();

                Ok(DaemonToClientReply::Verbs(verbs))
            }
            HostClientToDaemonMessage::Detach(token) => {
                info!(?client_id, "Detaching client");
                let connection = self.client_auth(token, client_id)?;

                // Submit disconnected only if this is an authenticated user... that is,
                // the connection oid >= 0
                if connection.is_positive() {
                    if let Err(e) = self.submit_disconnected_task(
                        &SYSTEM_OBJECT,
                        scheduler_client,
                        client_id,
                        &connection,
                    ) {
                        error!(error = ?e, "Error submitting user_disconnected task");
                    }
                }
                // Detach this client id from the player/connection object.
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcMessageError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };

                Ok(DaemonToClientReply::Disconnected)
            }
            HostClientToDaemonMessage::Program(token, auth_token, object, verb, code) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(&connection))?;

                self.program_verb(
                    scheduler_client,
                    client_id,
                    &connection,
                    &object,
                    verb,
                    code,
                )
            }
        }
    }

    fn perform_login(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        args: Vec<String>,
        attach: bool,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // TODO: change result of login to return this information, rather than just Objid, so
        //   we're not dependent on this.
        let connect_type = if args.first() == Some(&"create".to_string()) {
            ConnectType::Created
        } else {
            ConnectType::Connected
        };

        info!(
            "Performing {:?} login for client: {}, with args: {:?}",
            connect_type, client_id, args
        );
        let session = Arc::new(RpcSession::new(
            client_id,
            connection.clone(),
            self.mailbox_sender.clone(),
        ));
        let mut task_handle = match scheduler_client.submit_verb_task(
            connection,
            &ObjectRef::Id(handler_object.clone()),
            Symbol::mk("do_login_command"),
            args.iter().map(|s| v_str(s)).collect(),
            args.join(" "),
            &SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting login task");

                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };
        let player = loop {
            let receiver = task_handle.into_receiver();
            match receiver.recv() {
                Ok((_, Ok(TaskResult::Replaced(th)))) => {
                    task_handle = th;
                    continue;
                }
                Ok((_, Ok(TaskResult::Result(v)))) => {
                    // If v is an objid, we have a successful login and we need to rewrite this
                    // client id to use the player objid and then return a result to the client.
                    // with its new player objid and login result.
                    // If it's not an objid, that's considered an auth failure.
                    match v.variant() {
                        Variant::Obj(o) => break o.clone(),
                        _ => {
                            return Ok(LoginResult(None));
                        }
                    }
                }
                Ok((_, Err(e))) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::LoginTaskFailed);
                }
                Err(e) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        };

        let Ok(_) = self
            .connections
            .update_client_connection(connection.clone(), player.clone())
        else {
            return Err(RpcMessageError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        if attach {
            if let Err(e) = self.submit_connected_task(
                handler_object,
                scheduler_client,
                client_id,
                &player,
                connect_type,
            ) {
                error!(error = ?e, "Error submitting user_connected task");

                // Note we still continue to return a successful login result here, hoping for the best
                // but we do log the error.
            }
        }

        let auth_token = self.make_auth_token(&player);

        Ok(LoginResult(Some((
            auth_token,
            connect_type,
            player.clone(),
        ))))
    }

    fn submit_connected_task(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        initiation_type: ConnectType,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            player.clone(),
            self.mailbox_sender.clone(),
        ));

        let connected_verb = match initiation_type {
            ConnectType::Connected => Symbol::mk("user_connected"),
            ConnectType::Reconnected => Symbol::mk("user_reconnected"),
            ConnectType::Created => Symbol::mk("user_created"),
        };
        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(handler_object.clone()),
                connected_verb,
                List::mk_list(&[v_obj(player.clone())]),
                "".to_string(),
                &SYSTEM_OBJECT,
                session,
            )
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    fn submit_disconnected_task(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            player.clone(),
            self.mailbox_sender.clone(),
        ));

        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(handler_object.clone()),
                Symbol::mk("user_disconnected"),
                List::mk_list(&[v_obj(player.clone())]),
                "".to_string(),
                &SYSTEM_OBJECT,
                session,
            )
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    fn perform_command(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        handler_object: &Obj,
        connection: &Obj,
        command: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let session = Arc::new(RpcSession::new(
            client_id,
            connection.clone(),
            self.mailbox_sender.clone(),
        ));

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection.clone())
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        debug!(
            command,
            ?client_id,
            ?connection,
            "Invoking submit_command_task"
        );
        let parse_command_task_handle = match scheduler_client.submit_command_task(
            handler_object,
            connection,
            command.as_str(),
            session,
        ) {
            Ok(t) => t,
            Err(e) => return Err(RpcMessageError::TaskError(e)),
        };

        let task_id = parse_command_task_handle.task_id();
        let mut th_q = self.task_handles.lock().unwrap();
        th_q.insert(task_id, (client_id, parse_command_task_handle));
        Ok(DaemonToClientReply::TaskSubmitted(task_id))
    }

    fn respond_input(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        input_request_id: Uuid,
        input: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection.clone())
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        // Pass this back over to the scheduler to handle.
        if let Err(e) = scheduler_client.submit_requested_input(connection, input_request_id, input)
        {
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcMessageError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(DaemonToClientReply::InputThanks)
    }

    /// Call $do_out_of_band(command)
    fn perform_out_of_band(
        &self,
        scheduler_client: SchedulerClient,
        handler_object: &Obj,
        client_id: Uuid,
        connection: &Obj,
        command: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let session = Arc::new(RpcSession::new(
            client_id,
            connection.clone(),
            self.mailbox_sender.clone(),
        ));

        let command_components = parse_into_words(command.as_str());
        let task_handle = match scheduler_client.submit_out_of_band_task(
            handler_object,
            connection,
            command_components,
            command,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting command task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };

        // Just return immediately with success, we do not wait for the task to complete, we'll
        // let the session run to completion on its own and output back to the client.
        // Maybe we should be returning a value from this for the future, but the way clients are
        // written right now, there's little point.
        Ok(DaemonToClientReply::TaskSubmitted(task_handle.task_id()))
    }

    fn eval(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        expression: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let session = Arc::new(RpcSession::new(
            client_id,
            connection.clone(),
            self.mailbox_sender.clone(),
        ));

        let mut task_handle = match scheduler_client.submit_eval_task(
            connection,
            connection,
            expression,
            session,
            self.config.features_config.clone(),
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting eval task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };
        loop {
            match task_handle.into_receiver().recv() {
                Ok((_, Ok(TaskResult::Replaced(th)))) => {
                    task_handle = th;
                    continue;
                }
                Ok((_, Ok(TaskResult::Result(v)))) => break Ok(DaemonToClientReply::EvalResult(v)),
                Ok((_, Err(e))) => break Err(RpcMessageError::TaskError(e)),
                Err(e) => {
                    error!(error = ?e, "Error processing eval");

                    break Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        }
    }

    fn invoke_verb(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let session = Arc::new(RpcSession::new(
            client_id,
            connection.clone(),
            self.mailbox_sender.clone(),
        ));

        let task_handle = match scheduler_client.submit_verb_task(
            connection,
            object,
            verb,
            List::mk_list(&args),
            "".to_string(),
            &SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting verb task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };

        let task_id = task_handle.task_id();
        let mut th_q = self.task_handles.lock().unwrap();
        th_q.insert(task_id, (client_id, task_handle));
        Ok(DaemonToClientReply::TaskSubmitted(task_id))
    }

    fn program_verb(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        connection: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        code: Vec<String>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let verb = Symbol::mk_case_insensitive(verb.as_str());
        match scheduler_client.submit_verb_program(connection, connection, object, verb, code) {
            Ok((obj, verb)) => Ok(DaemonToClientReply::ProgramResponse(
                VerbProgramResponse::Success(obj, verb.to_string()),
            )),
            Err(SchedulerError::VerbProgramFailed(f)) => Ok(DaemonToClientReply::ProgramResponse(
                VerbProgramResponse::Failure(f),
            )),
            Err(e) => Err(RpcMessageError::TaskError(e)),
        }
    }

    fn ping_pong(&self) -> Result<(), SessionError> {
        let event = ClientsBroadcastEvent::PingPong(SystemTime::now());
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![CLIENT_BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send PingPong to client");
                DeliveryError
            })?;
        }
        self.connections.ping_check();

        // while we're here we're also sending HostPings, requesting their list of listeners,
        // and their liveness.
        let event = HostBroadcastEvent::PingPong(SystemTime::now());
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send PingPong to host");
                DeliveryError
            })?;
        }

        let mut hosts = self.hosts.write().unwrap();
        hosts.ping_check(HOST_TIMEOUT);
        Ok(())
    }

    /// Construct a PASETO token for this client_id and player combination. This token is used to
    /// validate the client connection to the daemon for future requests.
    fn make_client_token(&self, client_id: Uuid) -> ClientToken {
        let privkey: PasetoAsymmetricPrivateKey<V4, Public> =
            PasetoAsymmetricPrivateKey::from(self.private_key.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_SESSION_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "client_id": client_id.to_string(),
                    "iss": "moor",
                    "aud": "moor_connection",
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");

        ClientToken(token)
    }

    /// Construct a PASETO token for this player login. This token is used to provide credentials
    /// for requests, to allow reconnection with a different client_id.
    fn make_auth_token(&self, oid: &Obj) -> AuthToken {
        let privkey = PasetoAsymmetricPrivateKey::from(self.private_key.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_AUTH_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "player": oid.id().0,
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");
        AuthToken(token)
    }

    /// Validate a provided PASTEO host token.  Just verifying that it is a valid token signed
    /// with our same keypair.
    fn validate_host_token(&self, token: &HostToken) -> Result<HostType, RpcMessageError> {
        // Check cache first.
        {
            let host_tokens = self.host_token_cache.read().unwrap();

            if let Some((t, host_type)) = host_tokens.get(token) {
                if t.elapsed().as_secs() <= 60 {
                    return Ok(*host_type);
                }
            }
        }
        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let host_type = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_HOST_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let Some(host_type) = HostType::parse_id_str(host_type.as_str()) else {
            warn!("Unable to parse/validate host type in token");
            return Err(RpcMessageError::PermissionDenied);
        };

        // Cache the result.
        let mut host_tokens = self.host_token_cache.write().unwrap();
        host_tokens.insert(token.clone(), (Instant::now(), host_type));

        Ok(host_type)
    }

    /// Validate the provided PASETO client token against the provided client id
    /// If they do not match, the request is rejected, permissions denied.
    fn validate_client_token(
        &self,
        token: ClientToken,
        client_id: Uuid,
    ) -> Result<(), RpcMessageError> {
        {
            let client_tokens = self.client_token_cache.read().unwrap();
            if let Some(t) = client_tokens.get(&token) {
                if t.elapsed().as_secs() <= 60 {
                    return Ok(());
                }
            }
        }

        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_SESSION_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcMessageError::PermissionDenied
            })?;

        // Does the token match the client it came from? If not, reject it.
        let Some(token_client_id) = verified_token.get("client_id") else {
            debug!("Token does not contain client_id");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Some(token_client_id) = token_client_id.as_str() else {
            debug!("Token client_id is null");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Ok(token_client_id) = Uuid::parse_str(token_client_id) else {
            debug!("Token client_id is not a valid UUID");
            return Err(RpcMessageError::PermissionDenied);
        };
        if client_id != token_client_id {
            debug!(
                ?client_id,
                ?token_client_id,
                "Token client_id does not match client_id"
            );
            return Err(RpcMessageError::PermissionDenied);
        }

        let mut client_tokens = self.client_token_cache.write().unwrap();
        client_tokens.insert(token.clone(), Instant::now());

        Ok(())
    }

    /// Validate that the provided PASETO token is valid.
    /// If a player id is provided, validate it matches the player id.
    /// Return the player id if it is valid.
    /// Note that this is merely validating that the token is valid, not that the actual player
    /// inside the token is valid and has the capabilities it thinks it has. That must be done in
    /// the runtime itself.
    fn validate_auth_token(
        &self,
        token: AuthToken,
        objid: Option<&Obj>,
    ) -> Result<Obj, RpcMessageError> {
        {
            let auth_tokens = self.auth_token_cache.read().unwrap();
            if let Some((t, o)) = auth_tokens.get(&token) {
                if t.elapsed().as_secs() <= 60 {
                    return Ok(o.clone());
                }
            }
        }
        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_AUTH_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcMessageError::PermissionDenied
            })
            .unwrap();

        let Some(token_player) = verified_token.get("player") else {
            debug!("Token does not contain player");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Some(token_player) = token_player.as_i64() else {
            debug!("Token player is not valid");
            return Err(RpcMessageError::PermissionDenied);
        };
        if token_player < i32::MIN as i64 || token_player > i32::MAX as i64 {
            debug!("Token player is not a valid objid");
            return Err(RpcMessageError::PermissionDenied);
        }
        let token_player = Obj::mk_id(token_player as i32);
        if let Some(objid) = objid {
            // Does the 'player' match objid? If not, reject it.
            if objid.ne(&token_player) {
                debug!(?objid, ?token_player, "Token player does not match objid");
                return Err(RpcMessageError::PermissionDenied);
            }
        }

        // TODO: we will need to verify that the player object id inside the token is valid inside
        //   moor itself. And really only something with a WorldState can do that. So it's not
        //   enough to have validated the auth token here, we will need to pepper the scheduler/task
        //   code with checks to make sure that the player objid is valid before letting it go
        //   forwards.

        let mut auth_tokens = self.auth_token_cache.write().unwrap();
        auth_tokens.insert(token.clone(), (Instant::now(), token_player.clone()));
        Ok(token_player)
    }

    // Session stuff below

    fn publish_narrative_events(&self, events: &[(Obj, Box<NarrativeEvent>)]) -> Result<(), Error> {
        let publish = self.events_publish.lock().unwrap();
        for (player, event) in events {
            let client_ids = self.connections.client_ids_for(player.clone())?;
            let event = ClientEvent::Narrative(player.clone(), event.as_ref().clone());
            let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
            for client_id in &client_ids {
                let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
                publish.send_multipart(payload, 0).map_err(|e| {
                    error!(error = ?e, "Unable to send narrative event");
                    DeliveryError
                })?;
            }
        }
        Ok(())
    }

    fn send_system_message(
        &self,
        client_id: Uuid,
        player: Obj,
        message: String,
    ) -> Result<(), SessionError> {
        let event = ClientEvent::SystemMessage(player, message);
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard())
            .expect("Unable to serialize system message");
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send system message");
                DeliveryError
            })?;
        }
        Ok(())
    }

    /// Request that the client dispatch its next input event through as an input event into the
    /// scheduler submit_input, instead, with the attached input_request_id. So send a narrative
    /// event to this *specific* client id letting it know that it should issue a prompt.
    fn request_client_input(
        &self,
        client_id: Uuid,
        player: Obj,
        input_request_id: Uuid,
    ) -> Result<(), SessionError> {
        // Mark this client as in `input mode`, which means that instead of dispatching its next
        // line to the scheduler as a command, it should instead dispatch it as an input event.

        // Validate first.
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(SessionError::NoConnectionForPlayer(player));
        };
        if connection != player {
            return Err(SessionError::NoConnectionForPlayer(player));
        }

        let event = ClientEvent::RequestInput(input_request_id);
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard())
            .expect("Unable to serialize input request");
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send input request");
                DeliveryError
            })?;
        }
        Ok(())
    }

    fn connection_name_for(&self, player: Obj) -> Result<String, SessionError> {
        self.connections.connection_name_for(player)
    }

    #[allow(dead_code)]
    fn last_activity_for(&self, player: Obj) -> Result<SystemTime, SessionError> {
        self.connections.last_activity_for(player)
    }

    fn idle_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
        let last_activity = self.connections.last_activity_for(player)?;
        Ok(last_activity
            .elapsed()
            .map(|e| e.as_secs_f64())
            .unwrap_or(0.0))
    }

    fn connected_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
        self.connections.connected_seconds_for(player)
    }

    // TODO this will issue physical disconnects to *all* connections for this player.
    //   which probably isn't what you really want. This is just here to keep the existing behaviour
    //   of @quit and @boot-player working.
    //   in reality players using "@quit" will probably really want to just "sleep", and cores
    //   should be modified to reflect that.
    fn disconnect(&self, player: Obj) -> Result<(), SessionError> {
        warn!("Disconnecting player: {}", player);
        let all_client_ids = self.connections.client_ids_for(player)?;

        let publish = self.events_publish.lock().unwrap();
        let event = ClientEvent::Disconnect();
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard())
            .expect("Unable to serialize disconnection event");
        for client_id in all_client_ids {
            let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(
                    "Unable to send disconnection event to narrative channel: {}",
                    e
                );
                DeliveryError
            })?
        }

        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        let connections = self.connections.connections();
        Ok(connections
            .iter()
            .filter(|o| o > &&SYSTEM_OBJECT)
            .cloned()
            .collect())
    }

    fn request_sys_prop(
        &self,
        scheduler_client: SchedulerClient,
        player: Obj,
        object: ObjectRef,
        property: Symbol,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let pv = match scheduler_client.request_system_property(&player, &object, property) {
            Ok(pv) => pv,
            Err(CommandExecutionError(CommandError::NoObjectMatch)) => {
                return Ok(DaemonToClientReply::SysPropValue(None));
            }
            Err(e) => {
                error!(error = ?e, "Error requesting system property");
                return Err(RpcMessageError::ErrorCouldNotRetrieveSysProp(
                    "error requesting system property".to_string(),
                ));
            }
        };

        Ok(DaemonToClientReply::SysPropValue(Some(pv)))
    }

    // Task Q

    fn process_task_completions(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        // Collect all the receives into one crossbeam select and see if any of them are ready.
        // If so, process the first one we hit. We'll loop around and do this until we hit
        // the deadline.
        let mut receives = vec![];
        let mut task_client_ids = vec![];
        {
            let th_q = self.task_handles.lock().unwrap();
            for (task_id, (client_id, task_handle)) in th_q.iter() {
                receives.push(task_handle.receiver().clone());
                task_client_ids.push((*task_id, *client_id));
            }
        }
        let mut select = Select::new();
        for recv in &receives {
            select.recv(recv);
        }
        if let Ok(index) = select.ready_deadline(deadline) {
            let recv = &receives[index];
            let client_id = task_client_ids[index].1;
            match recv.recv_deadline(deadline) {
                Ok((task_id, r)) => {
                    let result = match r {
                        Ok(TaskResult::Result(v)) => ClientEvent::TaskSuccess(task_id, v),
                        Ok(TaskResult::Replaced(th)) => {
                            info!(?client_id, ?task_id, "Task restarted");
                            let mut th_q = self.task_handles.lock().unwrap();
                            th_q.insert(task_id, (client_id, th));
                            return;
                        }
                        Err(e) => ClientEvent::TaskError(task_id, e),
                    };
                    let payload = bincode::encode_to_vec(&result, bincode::config::standard())
                        .expect("Unable to serialize task result");
                    let payload = vec![client_id.as_bytes().to_vec(), payload];
                    {
                        let publish = self.events_publish.lock().unwrap();
                        if let Err(e) = publish.send_multipart(payload, 0) {
                            error!(error = ?e, "Unable to send task result");
                        }
                    }
                    let mut th_q = self.task_handles.lock().unwrap();
                    th_q.remove(&task_id);
                }
                Err(e) => {
                    if e.is_disconnected() {
                        // The client disconnected, so we need to remove the task handle from the
                        // queue, and break out of this loop to avoid polling this channel again.
                        let mut th_q = self.task_handles.lock().unwrap();
                        th_q.remove(&task_client_ids[index].0);
                    }
                }
            }
        }
    }
}
