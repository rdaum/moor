// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! The core of the server logic for the RPC daemon

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use eyre::{Context, Error};

use crate::connections::ConnectionsDB;
#[cfg(feature = "relbox")]
use crate::connections_rb::ConnectionsRb;
use crate::connections_wt::ConnectionsWT;
use crate::rpc_session::RpcSession;
use moor_db::DatabaseFlavour;
use moor_kernel::config::Config;
use moor_kernel::tasks::command_parse::preposition_to_string;
use moor_kernel::tasks::sessions::SessionError::DeliveryError;
use moor_kernel::tasks::sessions::{Session, SessionError, SessionFactory};
use moor_kernel::tasks::TaskHandle;
use moor_kernel::SchedulerClient;
use moor_values::model::{Named, ObjectRef, PropFlag, ValSet, VerbFlag};
use moor_values::tasks::SchedulerError::CommandExecutionError;
use moor_values::tasks::{CommandError, NarrativeEvent, SchedulerError, TaskId};
use moor_values::util::parse_into_words;
use moor_values::Symbol;
use moor_values::Variant;
use moor_values::SYSTEM_OBJECT;
use moor_values::{v_objid, v_string};
use moor_values::{Objid, Var};
use rpc_common::RpcResponse::{LoginResult, NewConnection};
use rpc_common::{
    AuthToken, BroadcastEvent, ClientToken, ConnectType, ConnectionEvent, EntityType, PropInfo,
    RpcRequest, RpcRequestError, RpcResponse, RpcResult, VerbInfo, VerbProgramResponse,
    BROADCAST_TOPIC, MOOR_AUTH_TOKEN_FOOTER, MOOR_SESSION_TOKEN_FOOTER,
};
use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use rusty_paseto::prelude::Key;
use serde_json::json;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;
use zmq::{Socket, SocketType};

pub struct RpcServer {
    zmq_context: zmq::Context,
    keypair: Key<64>,
    events_publish: Arc<Mutex<Socket>>,
    connections: Arc<dyn ConnectionsDB + Send + Sync>,
    task_handles: Mutex<HashMap<TaskId, (Uuid, TaskHandle)>>,
    config: Arc<Config>,
}

pub(crate) fn pack_response(result: Result<RpcResponse, RpcRequestError>) -> Vec<u8> {
    let rpc_result = match result {
        Ok(r) => RpcResult::Success(r),
        Err(e) => RpcResult::Failure(e),
    };
    bincode::encode_to_vec(&rpc_result, bincode::config::standard()).unwrap()
}

impl RpcServer {
    pub fn new(
        keypair: Key<64>,
        connections_db_path: PathBuf,
        zmq_context: zmq::Context,
        narrative_endpoint: &str,
        // For determining the flavor for the connections database.
        db_flavor: DatabaseFlavour,
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
        let connections: Arc<dyn ConnectionsDB + Send + Sync> = match db_flavor {
            DatabaseFlavour::WiredTiger => Arc::new(ConnectionsWT::new(Some(connections_db_path))),
            #[cfg(feature = "relbox")]
            DatabaseFlavour::RelBox => Arc::new(ConnectionsRb::new(Some(connections_db_path))),
        };
        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().len()
        );
        Self {
            keypair,
            connections,
            events_publish: Arc::new(Mutex::new(publish)),
            zmq_context,
            task_handles: Default::default(),
            config,
        }
    }

    pub(crate) fn zmq_loop(
        self: Arc<Self>,
        rpc_endpoint: String,
        scheduler_client: SchedulerClient,
        kill_switch: Arc<AtomicBool>,
    ) -> eyre::Result<()> {
        // Start up the ping-ponger timer in a background thread...
        let t_rpc_server = self.clone();
        std::thread::Builder::new()
            .name("rpc-ping-pong".to_string())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                t_rpc_server.ping_pong().expect("Unable to play ping-pong");
            })?;

        // We need to bind a generic publisher to the narrative endpoint, so that subsequent sessions
        // are visible...
        let rpc_socket = self.zmq_context.socket(zmq::REP)?;
        rpc_socket.bind(&rpc_endpoint)?;

        info!(
            "0mq server listening on {} with {} IO threads",
            rpc_endpoint,
            self.zmq_context.get_io_threads().unwrap()
        );

        let this = self.clone();
        loop {
            if kill_switch.load(Ordering::Relaxed) {
                info!("Kill switch activated, exiting");
                return Ok(());
            }
            // Check any task handles for completion.
            self.clone().process_task_completions();

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
                    trace!(num_parts = request.len(), "ZQM Request received");

                    // Components are:
                    if request.len() != 2 {
                        error!("Invalid request received, ignoring");

                        rpc_socket.send_multipart(
                            vec![pack_response(Err(RpcRequestError::InvalidRequest))],
                            0,
                        )?;
                        continue;
                    }

                    if request.len() != 2 {
                        rpc_socket.send_multipart(
                            vec![pack_response(Err(RpcRequestError::InvalidRequest))],
                            0,
                        )?;
                        continue;
                    }

                    let (client_id, request_body) = (&request[0], &request[1]);

                    let Ok(client_id) = Uuid::from_slice(client_id) else {
                        rpc_socket.send_multipart(
                            vec![pack_response(Err(RpcRequestError::InvalidRequest))],
                            0,
                        )?;
                        continue;
                    };

                    // Decode 'request_body' as a bincode'd ClientEvent.
                    let request =
                        match bincode::decode_from_slice(request_body, bincode::config::standard())
                        {
                            Ok((request, _)) => request,
                            Err(_) => {
                                rpc_socket.send_multipart(
                                    vec![pack_response(Err(RpcRequestError::InvalidRequest))],
                                    0,
                                )?;

                                continue;
                            }
                        };

                    // The remainder of the payload are all the request arguments, which vary depending
                    // on the type.
                    let response =
                        this.clone()
                            .process_request(scheduler_client.clone(), client_id, request);
                    let response = pack_response(response);
                    rpc_socket.send_multipart(vec![response], 0)?;
                }
            }
        }
    }

    fn client_auth(&self, token: ClientToken, client_id: Uuid) -> Result<Objid, RpcRequestError> {
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(RpcRequestError::NoConnection);
        };

        self.validate_client_token(token, client_id)?;
        Ok(connection)
    }

    /// Process a request (originally ZMQ REQ) and produce a reply (becomes ZMQ REP)
    pub fn process_request(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        request: RpcRequest,
    ) -> Result<RpcResponse, RpcRequestError> {
        match request {
            RpcRequest::ConnectionEstablish(hostname) => {
                let oid = self.connections.new_connection(client_id, hostname, None)?;
                let token = self.make_client_token(client_id);
                Ok(NewConnection(token, oid))
            }
            RpcRequest::Attach(auth_token, connect_type, hostname) => {
                // Validate the auth token, and get the player.
                let player = self.validate_auth_token(auth_token, None)?;

                self.connections
                    .new_connection(client_id, hostname, Some(player))?;
                let client_token = self.make_client_token(client_id);

                if let Some(connect_type) = connect_type {
                    trace!(?player, "Submitting user_connected task");
                    if let Err(e) = self.clone().submit_connected_task(
                        scheduler_client,
                        client_id,
                        player,
                        connect_type,
                    ) {
                        error!(error = ?e, "Error submitting user_connected task");

                        // Note we still continue to return a successful login result here, hoping for the best
                        // but we do log the error.
                    }
                }
                Ok(RpcResponse::AttachResult(Some((client_token, player))))
            }
            // Bodacious Totally Awesome Hey Dudes Have Mr Pong's Chinese Food
            RpcRequest::Pong(token, _client_sys_time) => {
                // Always respond with a ThanksPong, even if it's somebody we don't know.
                // Can easily be a connection that was in the middle of negotiation at the time the
                // ping was sent out, or dangling in some other way.
                let response = Ok(RpcResponse::ThanksPong(SystemTime::now()));

                let connection = self.client_auth(token, client_id)?;
                // Let 'connections' know that the connection is still alive.
                let Ok(_) = self.connections.notify_is_alive(client_id, connection) else {
                    warn!("Unable to notify connection is alive: {}", client_id);
                    return response;
                };
                response
            }
            RpcRequest::RequestSysProp(token, object, property) => {
                let connection = self.client_auth(token, client_id)?;

                self.clone()
                    .request_sys_prop(scheduler_client, connection, object, property)
            }
            RpcRequest::LoginCommand(token, args, attach) => {
                let connection = self.client_auth(token, client_id)?;

                self.clone()
                    .perform_login(scheduler_client, client_id, connection, args, attach)
            }
            RpcRequest::Command(token, auth_token, command) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                self.clone()
                    .perform_command(scheduler_client, client_id, connection, command)
            }
            RpcRequest::RequestedInput(token, auth_token, request_id, input) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                let request_id = Uuid::from_u128(request_id);
                self.clone().respond_input(
                    scheduler_client,
                    client_id,
                    connection,
                    request_id,
                    input,
                )
            }
            RpcRequest::OutOfBand(token, auth_token, command) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                self.clone()
                    .perform_out_of_band(scheduler_client, client_id, connection, command)
            }

            RpcRequest::Eval(token, auth_token, evalstr) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;
                self.clone()
                    .eval(scheduler_client, client_id, connection, evalstr)
            }

            RpcRequest::InvokeVerb(token, auth_token, object, verb, args) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                self.clone().invoke_verb(
                    scheduler_client,
                    client_id,
                    connection,
                    object,
                    verb,
                    args,
                )
            }

            RpcRequest::Retrieve(token, auth_token, who, retr_type, what) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                match retr_type {
                    EntityType::Property => {
                        let (propdef, propperms, value) = scheduler_client
                            .request_property(connection, connection, who, what)
                            .map_err(|e| {
                                error!(error = ?e, "Error requesting property");
                                RpcRequestError::EntityRetrievalError(
                                    "error requesting property".to_string(),
                                )
                            })?;
                        Ok(RpcResponse::PropertyValue(
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
                            .request_verb(connection, connection, who, what)
                            .map_err(|e| {
                                error!(error = ?e, "Error requesting verb");
                                RpcRequestError::EntityRetrievalError(
                                    "error requesting verb".to_string(),
                                )
                            })?;
                        let argspec = verbdef.args();
                        let arg_spec = vec![
                            Symbol::mk(argspec.dobj.to_string()),
                            Symbol::mk(preposition_to_string(&argspec.prep)),
                            Symbol::mk(argspec.iobj.to_string()),
                        ];
                        Ok(RpcResponse::VerbValue(
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
            RpcRequest::Properties(token, auth_token, obj) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                let props = scheduler_client
                    .request_properties(connection, connection, obj)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting properties");
                        RpcRequestError::EntityRetrievalError(
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

                Ok(RpcResponse::Properties(props))
            }
            RpcRequest::Verbs(token, auth_token, obj) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                let verbs = scheduler_client
                    .request_verbs(connection, connection, obj)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting verbs");
                        RpcRequestError::EntityRetrievalError("error requesting verbs".to_string())
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

                Ok(RpcResponse::Verbs(verbs))
            }
            RpcRequest::Detach(token) => {
                self.validate_client_token(token, client_id)?;

                debug!(?client_id, "Detaching client");

                // Detach this client id from the player/connection object.
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcRequestError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };

                Ok(RpcResponse::Disconnected)
            }
            RpcRequest::Program(token, auth_token, object, verb, code) => {
                let connection = self.client_auth(token, client_id)?;
                self.validate_auth_token(auth_token, Some(connection))?;

                self.clone().program_verb(
                    scheduler_client,
                    client_id,
                    connection,
                    object,
                    verb,
                    code,
                )
            }
        }
    }

    pub(crate) fn new_session(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
    ) -> Result<Arc<dyn Session>, SessionError> {
        debug!(?client_id, ?connection, "Started session",);

        Ok(Arc::new(RpcSession::new(
            client_id,
            self.clone(),
            connection,
        )))
    }

    pub(crate) fn connection_name_for(&self, player: Objid) -> Result<String, SessionError> {
        self.connections.connection_name_for(player)
    }

    #[allow(dead_code)]
    fn last_activity_for(&self, player: Objid) -> Result<SystemTime, SessionError> {
        self.connections.last_activity_for(player)
    }

    pub(crate) fn idle_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        let last_activity = self.connections.last_activity_for(player)?;
        Ok(last_activity.elapsed().unwrap().as_secs_f64())
    }

    pub(crate) fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        self.connections.connected_seconds_for(player)
    }

    // TODO this will issue physical disconnects to *all* connections for this player.
    //   which probably isn't what you really want. This is just here to keep the existing behaviour
    //   of @quit and @boot-player working.
    //   in reality players using "@quit" will probably really want to just "sleep", and cores
    //   should be modified to reflect that.
    pub(crate) fn disconnect(&self, player: Objid) -> Result<(), SessionError> {
        warn!("Disconnecting player: {}", player);
        let all_client_ids = self.connections.client_ids_for(player)?;

        let publish = self.events_publish.lock().unwrap();
        let event = ConnectionEvent::Disconnect();
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

    pub(crate) fn connected_players(&self) -> Result<Vec<Objid>, SessionError> {
        let connections = self.connections.connections();
        Ok(connections.iter().filter(|o| o.0 > 0).cloned().collect())
    }

    fn request_sys_prop(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        player: Objid,
        object: ObjectRef,
        property: Symbol,
    ) -> Result<RpcResponse, RpcRequestError> {
        let pv = match scheduler_client.request_system_property(player, object, property) {
            Ok(pv) => pv,
            Err(CommandExecutionError(CommandError::NoObjectMatch)) => {
                return Ok(RpcResponse::SysPropValue(None));
            }
            Err(e) => {
                error!(error = ?e, "Error requesting system property");
                return Err(RpcRequestError::ErrorCouldNotRetrieveSysProp(
                    "error requesting system property".to_string(),
                ));
            }
        };

        Ok(RpcResponse::SysPropValue(Some(pv)))
    }

    fn perform_login(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        args: Vec<String>,
        attach: bool,
    ) -> Result<RpcResponse, RpcRequestError> {
        // TODO: change result of login to return this information, rather than just Objid, so
        //   we're not dependent on this.
        let connect_type = if args.first() == Some(&"create".to_string()) {
            ConnectType::Created
        } else {
            ConnectType::Connected
        };

        debug!(
            "Performing {:?} login for client: {}",
            connect_type, client_id
        );
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };
        let task_handle = match scheduler_client.submit_verb_task(
            connection,
            ObjectRef::Id(SYSTEM_OBJECT),
            Symbol::mk("do_login_command"),
            args.iter().map(|s| v_string(s.clone())).collect(),
            args.join(" "),
            SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting login task");

                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };
        let receiver = task_handle.into_receiver();
        let player = match receiver.recv() {
            Ok(Ok(v)) => {
                // If v is an objid, we have a successful login and we need to rewrite this
                // client id to use the player objid and then return a result to the client.
                // with its new player objid and login result.
                // If it's not an objid, that's considered an auth failure.
                match v.variant() {
                    Variant::Obj(o) => o,
                    _ => {
                        return Ok(LoginResult(None));
                    }
                }
            }
            Ok(Err(e)) => {
                error!(error = ?e, "Error waiting for login results");

                return Err(RpcRequestError::LoginTaskFailed);
            }
            Err(e) => {
                error!(error = ?e, "Error waiting for login results");

                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        // Update the connection records.
        trace!(
            ?connection,
            ?player,
            "Transitioning connection record to logged in"
        );
        let Ok(_) = self
            .connections
            .update_client_connection(connection, player)
        else {
            return Err(RpcRequestError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        if attach {
            trace!(?player, "Submitting user_connected task");
            if let Err(e) = self.clone().submit_connected_task(
                scheduler_client,
                client_id,
                player,
                connect_type,
            ) {
                error!(error = ?e, "Error submitting user_connected task");

                // Note we still continue to return a successful login result here, hoping for the best
                // but we do log the error.
            }
        }

        let auth_token = self.make_auth_token(player);

        Ok(LoginResult(Some((auth_token, connect_type, player))))
    }

    fn submit_connected_task(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: Objid,
        initiation_type: ConnectType,
    ) -> Result<(), eyre::Error> {
        let session = self
            .clone()
            .new_session(client_id, player)
            .with_context(|| "could not create 'connected' task session for player")?;

        let connected_verb = match initiation_type {
            ConnectType::Connected => Symbol::mk("user_connected"),
            ConnectType::Reconnected => Symbol::mk("user_reconnected"),
            ConnectType::Created => Symbol::mk("user_created"),
        };
        scheduler_client
            .submit_verb_task(
                player,
                ObjectRef::Id(SYSTEM_OBJECT),
                connected_verb,
                vec![v_objid(player)],
                "".to_string(),
                SYSTEM_OBJECT,
                session,
            )
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    fn perform_command(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        debug!(
            command,
            ?client_id,
            ?connection,
            "Invoking submit_command_task"
        );
        let parse_command_task_handle =
            match scheduler_client.submit_command_task(connection, command.as_str(), session) {
                Ok(t) => t,
                Err(e) => return Err(RpcRequestError::TaskError(e)),
            };

        let task_id = parse_command_task_handle.task_id();
        let mut th_q = self.task_handles.lock().unwrap();
        th_q.insert(task_id, (client_id, parse_command_task_handle));
        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    fn respond_input(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        input_request_id: Uuid,
        input: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        // Pass this back over to the scheduler to handle.
        if let Err(e) = scheduler_client.submit_requested_input(connection, input_request_id, input)
        {
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcRequestError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(RpcResponse::InputThanks)
    }

    /// Call $do_out_of_band(command)
    fn perform_out_of_band(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let command_components = parse_into_words(command.as_str());
        let task_handle = match scheduler_client.submit_out_of_band_task(
            connection,
            command_components,
            command,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        // Just return immediately with success, we do not wait for the task to complete, we'll
        // let the session run to completion on its own and output back to the client.
        // Maybe we should be returning a value from this for the future, but the way clients are
        // written right now, there's little point.
        Ok(RpcResponse::CommandSubmitted(task_handle.task_id()))
    }

    fn eval(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        expression: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let task_handle = match scheduler_client.submit_eval_task(
            connection,
            connection,
            expression,
            session,
            self.config.clone(),
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting eval task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };
        match task_handle.into_receiver().recv() {
            Ok(Ok(v)) => Ok(RpcResponse::EvalResult(v)),
            Ok(Err(e)) => Err(RpcRequestError::TaskError(e)),
            Err(e) => {
                error!(error = ?e, "Error processing eval");

                Err(RpcRequestError::InternalError(e.to_string()))
            }
        }
    }

    fn invoke_verb(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        object: ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let task_handle = match scheduler_client.submit_verb_task(
            connection,
            object,
            verb,
            args,
            "".to_string(),
            SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting verb task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        let task_id = task_handle.task_id();
        let mut th_q = self.task_handles.lock().unwrap();
        th_q.insert(task_id, (client_id, task_handle));
        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    fn program_verb(
        self: Arc<Self>,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: Objid,
        object: ObjectRef,
        verb: Symbol,
        code: Vec<String>,
    ) -> Result<RpcResponse, RpcRequestError> {
        if self.clone().new_session(client_id, connection).is_err() {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let verb = Symbol::mk_case_insensitive(verb.as_str());
        match scheduler_client.submit_verb_program(connection, connection, object, verb, code) {
            Ok((obj, verb)) => Ok(RpcResponse::ProgramResponse(VerbProgramResponse::Success(
                obj,
                verb.to_string(),
            ))),
            Err(SchedulerError::VerbProgramFailed(f)) => Ok(RpcResponse::ProgramResponse(
                VerbProgramResponse::Failure(f),
            )),
            Err(e) => Err(RpcRequestError::TaskError(e)),
        }
    }

    fn process_task_completions(self: Arc<Self>) {
        let mut th_q = self.task_handles.lock().unwrap();

        let mut completed = vec![];
        let mut gone = vec![];

        for (task_id, (client_id, task_handle)) in th_q.iter() {
            match task_handle.receiver().try_recv() {
                Ok(result) => completed.push((*task_id, *client_id, result)),
                Err(oneshot::TryRecvError::Disconnected) => gone.push(*task_id),
                Err(oneshot::TryRecvError::Empty) => {
                    continue;
                }
            }
        }
        for task_id in gone {
            th_q.remove(&task_id);
        }
        if !completed.is_empty() {
            let publish = self.events_publish.lock().unwrap();
            for (task_id, client_id, result) in completed {
                let result = match result {
                    Ok(v) => ConnectionEvent::TaskSuccess(v),
                    Err(e) => ConnectionEvent::TaskError(e),
                };
                debug!(?client_id, ?task_id, ?result, "Task completed");
                let payload = bincode::encode_to_vec(&result, bincode::config::standard())
                    .expect("Unable to serialize task result");
                let payload = vec![client_id.as_bytes().to_vec(), payload];
                if let Err(e) = publish.send_multipart(payload, 0) {
                    error!(error = ?e, "Unable to send task result");
                }
                th_q.remove(&task_id);
            }
        }
    }

    pub(crate) fn publish_narrative_events(
        &self,
        events: &[(Objid, NarrativeEvent)],
    ) -> Result<(), Error> {
        let publish = self.events_publish.lock().unwrap();
        for (player, event) in events {
            let client_ids = self.connections.client_ids_for(*player)?;
            let event = ConnectionEvent::Narrative(*player, event.clone());
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

    pub(crate) fn send_system_message(
        &self,
        client_id: Uuid,
        player: Objid,
        message: String,
    ) -> Result<(), SessionError> {
        let event = ConnectionEvent::SystemMessage(player, message);
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
    pub(crate) fn request_client_input(
        &self,
        client_id: Uuid,
        player: Objid,
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

        let event = ConnectionEvent::RequestInput(input_request_id.as_u128());
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

    fn ping_pong(&self) -> Result<(), SessionError> {
        let event = BroadcastEvent::PingPong(SystemTime::now());
        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish.send_multipart(payload, 0).map_err(|e| {
                error!(error = ?e, "Unable to send PingPong to client");
                DeliveryError
            })?;
        }
        self.connections.ping_check();
        Ok(())
    }

    /// Construct a PASETO token for this client_id and player combination. This token is used to
    /// validate the client connection to the daemon for future requests.
    fn make_client_token(&self, client_id: Uuid) -> ClientToken {
        let privkey: PasetoAsymmetricPrivateKey<V4, Public> =
            PasetoAsymmetricPrivateKey::from(self.keypair.as_ref());
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
    fn make_auth_token(&self, oid: Objid) -> AuthToken {
        let privkey = PasetoAsymmetricPrivateKey::from(self.keypair.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_AUTH_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "player": oid.0,
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");
        AuthToken(token)
    }

    /// Validate the provided PASETO token against the provided client id
    /// If they do not match, the request is rejected, permissions denied.
    fn validate_client_token(
        &self,
        token: ClientToken,
        client_id: Uuid,
    ) -> Result<(), RpcRequestError> {
        let key: Key<32> = Key::from(&self.keypair[32..]);
        let pk: PasetoAsymmetricPublicKey<V4, Public> = PasetoAsymmetricPublicKey::from(&key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_SESSION_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcRequestError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcRequestError::PermissionDenied
            })?;

        // Does the token match the client it came from? If not, reject it.
        let Some(token_client_id) = verified_token.get("client_id") else {
            debug!("Token does not contain client_id");
            return Err(RpcRequestError::PermissionDenied);
        };
        let Some(token_client_id) = token_client_id.as_str() else {
            debug!("Token client_id is null");
            return Err(RpcRequestError::PermissionDenied);
        };
        let Ok(token_client_id) = Uuid::parse_str(token_client_id) else {
            debug!("Token client_id is not a valid UUID");
            return Err(RpcRequestError::PermissionDenied);
        };
        if client_id != token_client_id {
            debug!(
                ?client_id,
                ?token_client_id,
                "Token client_id does not match client_id"
            );
            return Err(RpcRequestError::PermissionDenied);
        }

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
        objid: Option<Objid>,
    ) -> Result<Objid, RpcRequestError> {
        let key: Key<32> = Key::from(&self.keypair[32..]);
        let pk: PasetoAsymmetricPublicKey<V4, Public> = PasetoAsymmetricPublicKey::from(&key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_AUTH_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcRequestError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcRequestError::PermissionDenied
            })
            .unwrap();

        let Some(token_player) = verified_token.get("player") else {
            debug!("Token does not contain player");
            return Err(RpcRequestError::PermissionDenied);
        };
        let Some(token_player) = token_player.as_i64() else {
            debug!("Token player is not valid");
            return Err(RpcRequestError::PermissionDenied);
        };
        let token_player = Objid(token_player);
        if let Some(objid) = objid {
            // Does the 'player' match objid? If not, reject it.
            if objid != token_player {
                debug!(?objid, ?token_player, "Token player does not match objid");
                return Err(RpcRequestError::PermissionDenied);
            }
        }

        // TODO: we will need to verify that the player object id inside the token is valid inside
        //   moor itself. And really only something with a WorldState can do that. So it's not
        //   enough to have validated the auth token here, we will need to pepper the scheduler/task
        //   code with checks to make sure that the player objid is valid before letting it go
        //   forwards.

        Ok(token_player)
    }
}

impl SessionFactory for RpcServer {
    fn mk_background_session(
        self: Arc<Self>,
        player: Objid,
    ) -> Result<Arc<dyn Session>, SessionError> {
        self.clone().new_session(Uuid::new_v4(), player)
    }
}
