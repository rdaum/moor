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

use std::path::PathBuf;
/// The core of the server logic for the RPC daemon
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use eyre::{Context, Error};

use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use rusty_paseto::prelude::Key;
use serde_json::json;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;
use zmq::{Socket, SocketType};

use moor_kernel::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_kernel::tasks::sessions::SessionError::DeliveryError;
use moor_kernel::tasks::sessions::{Session, SessionError};
use moor_kernel::tasks::TaskId;
use moor_values::model::NarrativeEvent;
use moor_values::model::WorldStateSource;
use moor_values::util::parse_into_words;
use moor_values::var::Objid;
use moor_values::var::Var;
use moor_values::var::Variant;
use moor_values::var::{v_bool, v_objid, v_str, v_string};
use moor_values::SYSTEM_OBJECT;
use rpc_common::RpcResponse::{LoginResult, NewConnection};
use rpc_common::{
    AuthToken, BroadcastEvent, ClientToken, ConnectType, ConnectionEvent, RpcRequest,
    RpcRequestError, RpcResponse, RpcResult, BROADCAST_TOPIC, MOOR_AUTH_TOKEN_FOOTER,
    MOOR_SESSION_TOKEN_FOOTER,
};

use crate::connections::ConnectionsDB;
use crate::connections_tb::ConnectionsTb;
use crate::rpc_session::RpcSession;

pub struct RpcServer {
    keypair: Key<64>,
    publish: Arc<Mutex<Socket>>,
    world_state_source: Arc<dyn WorldStateSource>,
    scheduler: Arc<Scheduler>,
    connections: Arc<dyn ConnectionsDB + Send + Sync>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConnectionRecord {
    client_id: Uuid,
    player: Objid,
    name: String,
    last_activity: Instant,
    connect_time: Instant,
}

pub(crate) fn make_response(result: Result<RpcResponse, RpcRequestError>) -> Vec<u8> {
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
        wss: Arc<dyn WorldStateSource>,
        scheduler: Arc<Scheduler>,
    ) -> Self {
        info!(
            "Creating new RPC server; with {} ZMQ IO threads...",
            zmq_context.get_io_threads().unwrap()
        );
        let publish = zmq_context
            .socket(SocketType::PUB)
            .expect("Unable to create ZMQ PUB socket");
        publish
            .bind(narrative_endpoint)
            .expect("Unable to bind ZMQ PUB socket");
        let connections = Arc::new(ConnectionsTb::new(Some(connections_db_path)));
        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().len()
        );
        Self {
            keypair,
            world_state_source: wss,
            scheduler,
            connections,
            publish: Arc::new(Mutex::new(publish)),
        }
    }

    /// Process a request (originally ZMQ REQ) and produce a reply (becomes ZMQ REP)
    pub fn process_request(self: Arc<Self>, client_id: Uuid, request: RpcRequest) -> Vec<u8> {
        match request {
            RpcRequest::ConnectionEstablish(hostname) => {
                match self.connections.new_connection(client_id, hostname, None) {
                    Ok(oid) => {
                        let token = self.make_client_token(client_id);
                        make_response(Ok(NewConnection(token, oid)))
                    }
                    Err(e) => make_response(Err(e)),
                }
            }
            RpcRequest::Attach(auth_token, connect_type, hostname) => {
                // Validate the auth token, and get the player.
                let Ok(player) = self.validate_auth_token(auth_token, None) else {
                    warn!("Invalid auth token for attach request");
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };
                let client_token =
                    match self
                        .connections
                        .new_connection(client_id, hostname, Some(player))
                    {
                        Ok(_) => self.make_client_token(client_id),
                        Err(e) => return make_response(Err(e)),
                    };

                if let Some(connect_type) = connect_type {
                    trace!(?player, "Submitting user_connected task");
                    if let Err(e) =
                        self.clone()
                            .submit_connected_task(client_id, player, connect_type)
                    {
                        error!(error = ?e, "Error submitting user_connected task");

                        // Note we still continue to return a successful login result here, hoping for the best
                        // but we do log the error.
                    }
                }
                make_response(Ok(RpcResponse::AttachResult(Some((client_token, player)))))
            }
            // Bodacious Totally Awesome Hey Dudes Have Mr Pong's Chinese Food
            RpcRequest::Pong(token, _client_sys_time) => {
                // Always respond with a ThanksPong, even if it's somebody we don't know.
                // Can easily be a connection that was in the middle of negotiation at the time the
                // ping was sent out, or dangling in some other way.
                let response = make_response(Ok(RpcResponse::ThanksPong(SystemTime::now())));

                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    warn!("Received Pong from invalid client: {}", client_id);
                    return response;
                };
                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                // Let 'connections' know that the connection is still alive.
                let Ok(_) = self.connections.notify_is_alive(client_id, connection) else {
                    warn!("Unable to notify connection is alive: {}", client_id);
                    return response;
                };
                response
            }
            RpcRequest::RequestSysProp(token, object, property) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };
                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                make_response(self.clone().request_sys_prop(object, property))
            }
            RpcRequest::LoginCommand(token, args, attach) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };
                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                make_response(
                    self.clone()
                        .perform_login(client_id, connection, args, attach),
                )
            }
            RpcRequest::Command(token, auth_token, command) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                let Ok(_) = self.validate_auth_token(auth_token, Some(connection)) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Auth token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };
                make_response(self.clone().perform_command(client_id, connection, command))
            }
            RpcRequest::RequestedInput(token, auth_token, request_id, input) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                let Ok(_) = self.validate_auth_token(auth_token, Some(connection)) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Auth token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };
                let request_id = Uuid::from_u128(request_id);
                make_response(
                    self.clone()
                        .respond_input(client_id, connection, request_id, input),
                )
            }
            RpcRequest::OutOfBand(token, auth_token, command) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };
                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                let Ok(_) = self.validate_auth_token(auth_token, Some(connection)) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Auth token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                make_response(
                    self.clone()
                        .perform_out_of_band(client_id, connection, command),
                )
            }

            RpcRequest::Eval(token, auth_token, evalstr) => {
                let Some(connection) = self.connections.connection_object_for_client(client_id)
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Client token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                let Ok(_) = self.validate_auth_token(auth_token, Some(connection)) else {
                    warn!(
                        ?client_id,
                        ?connection,
                        "Auth token validation failed for request"
                    );
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };
                make_response(self.clone().eval(client_id, connection, evalstr))
            }
            RpcRequest::Detach(token) => {
                let Ok(_) = self.validate_client_token(token, client_id) else {
                    warn!(?client_id, "Client token validation failed for request");
                    return make_response(Err(RpcRequestError::PermissionDenied));
                };

                info!("Detaching client: {}", client_id);

                // Detach this client id from the player/connection object.
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return make_response(Err(RpcRequestError::InternalError(
                        "Unable to remove client connection".to_string(),
                    )));
                };

                make_response(Ok(RpcResponse::Disconnected))
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

        let publish = self.publish.lock().unwrap();
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
        object: String,
        property: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(world_state) = self.world_state_source.new_world_state() else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let Ok(sysprop) =
            world_state.retrieve_property(SYSTEM_OBJECT, SYSTEM_OBJECT, object.as_str())
        else {
            return Err(RpcRequestError::ErrorCouldNotRetrieveSysProp(
                "could not access system object".to_string(),
            ));
        };

        let Variant::Obj(sysprop) = sysprop.variant() else {
            return Err(RpcRequestError::ErrorCouldNotRetrieveSysProp(
                "system object invalid".to_string(),
            ));
        };

        let Ok(property_value) =
            world_state.retrieve_property(SYSTEM_OBJECT, *sysprop, property.as_str())
        else {
            return Err(RpcRequestError::ErrorCouldNotRetrieveSysProp(
                "could not sysprop".to_string(),
            ));
        };

        Ok(RpcResponse::SysPropValue(Some(property_value)))
    }

    fn perform_login(
        self: Arc<Self>,
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
        let task_id = match self.clone().scheduler.submit_verb_task(
            connection,
            SYSTEM_OBJECT,
            "do_login_command".to_string(),
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
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id) {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "Error subscribing to login task");

                return Err(RpcRequestError::LoginTaskFailed);
            }
        };
        let player = match receiver.recv() {
            Ok(TaskWaiterResult::Success(v)) => {
                // If v is an objid, we have a successful login and we need to rewrite this
                // client id to use the player objid and then return a result to the client.
                // with its new player objid and login result.
                // If it's not an objid, that's considered an auth failure.
                match v.variant() {
                    Variant::Obj(o) => *o,
                    _ => {
                        return Ok(LoginResult(None));
                    }
                }
            }
            Ok(TaskWaiterResult::Error(e)) => {
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
            if let Err(e) = self
                .clone()
                .submit_connected_task(client_id, player, connect_type)
            {
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
        client_id: Uuid,
        player: Objid,
        initiation_type: ConnectType,
    ) -> Result<(), eyre::Error> {
        let session = self
            .clone()
            .new_session(client_id, player)
            .with_context(|| "could not create 'connected' task session for player")?;

        let connected_verb = match initiation_type {
            ConnectType::Connected => "user_connected".to_string(),
            ConnectType::Reconnected => "user_reconnected".to_string(),
            ConnectType::Created => "user_created".to_string(),
        };
        self.scheduler
            .submit_verb_task(
                player,
                SYSTEM_OBJECT,
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

        // Try to submit to do_command as a verb call first and only parse_command after that fails.
        // TODO: fold this functionality into Task.

        let arguments = parse_into_words(command.as_str());
        if let Ok(task_id) = self.clone().scheduler.submit_verb_task(
            connection,
            SYSTEM_OBJECT,
            "do_command".to_string(),
            arguments.iter().map(|s| v_str(s)).collect(),
            command.clone(),
            SYSTEM_OBJECT,
            session.clone(),
        ) {
            if let Ok(value) = self.clone().watch_command_task(task_id) {
                if value != v_bool(false) {
                    return Ok(RpcResponse::CommandSubmitted(task_id));
                }
            }
        }

        // That having failed, we do the classic internal parse command cycle instead...

        debug!(
            command,
            ?client_id,
            ?connection,
            "Invoking submit_command_task"
        );
        let task_id =
            match self
                .clone()
                .scheduler
                .submit_command_task(connection, command.as_str(), session)
            {
                Ok(t) => t,
                Err(SchedulerError::CommandExecutionError(e)) => {
                    return Err(RpcRequestError::CommandError(e));
                }
                Err(e) => {
                    error!(error = ?e, "Error submitting command task");
                    return Err(RpcRequestError::InternalError(e.to_string()));
                }
            };

        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    fn respond_input(
        self: Arc<Self>,
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
        if let Err(e) =
            self.clone()
                .scheduler
                .submit_requested_input(connection, input_request_id, input)
        {
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcRequestError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(RpcResponse::InputThanks)
    }

    fn watch_command_task(self: Arc<Self>, task_id: TaskId) -> Result<Var, RpcRequestError> {
        debug!(task_id, "Subscribed to command task results");
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id) {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        match receiver.recv() {
            Ok(TaskWaiterResult::Success(value)) => Ok(value),
            Ok(TaskWaiterResult::Error(SchedulerError::CommandExecutionError(e))) => {
                Err(RpcRequestError::CommandError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => Err(RpcRequestError::InternalError(e.to_string())),
            Err(e) => Err(RpcRequestError::InternalError(e.to_string())),
        }
    }

    /// Call $do_out_of_band(command)
    fn perform_out_of_band(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let command_components = parse_into_words(command.as_str());
        let task_id = match self.clone().scheduler.submit_out_of_band_task(
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
        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    fn eval(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        expression: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection) else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let task_id = match self
            .clone()
            .scheduler
            .submit_eval_task(connection, connection, expression, session)
        {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting eval task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        let receiver = match self.clone().scheduler.subscribe_to_task(task_id) {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        match receiver.recv() {
            Ok(TaskWaiterResult::Success(v)) => Ok(RpcResponse::EvalResult(v)),
            Ok(TaskWaiterResult::Error(SchedulerError::CommandExecutionError(e))) => {
                Err(RpcRequestError::CommandError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => {
                error!(error = ?e, "Error processing increment");

                Err(RpcRequestError::InternalError(e.to_string()))
            }
            Err(e) => {
                error!(error = ?e, "Error processing eval");

                Err(RpcRequestError::InternalError(e.to_string()))
            }
        }
    }

    pub(crate) fn publish_narrative_events(
        &self,
        events: &[(Objid, NarrativeEvent)],
    ) -> Result<(), Error> {
        let publish = self.publish.lock().unwrap();
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
            let publish = self.publish.lock().unwrap();
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
            let publish = self.publish.lock().unwrap();
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
            let publish = self.publish.lock().unwrap();
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
    ) -> Result<(), SessionError> {
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
            SessionError::InvalidToken
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                SessionError::InvalidToken
            })?;

        // Does the token match the client it came from? If not, reject it.
        let Some(token_client_id) = verified_token.get("client_id") else {
            debug!("Token does not contain client_id");
            return Err(SessionError::InvalidToken);
        };
        let Some(token_client_id) = token_client_id.as_str() else {
            debug!("Token client_id is null");
            return Err(SessionError::InvalidToken);
        };
        let Ok(token_client_id) = Uuid::parse_str(token_client_id) else {
            debug!("Token client_id is not a valid UUID");
            return Err(SessionError::InvalidToken);
        };
        if client_id != token_client_id {
            debug!(
                ?client_id,
                ?token_client_id,
                "Token client_id does not match client_id"
            );
            return Err(SessionError::InvalidToken);
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
    ) -> Result<Objid, SessionError> {
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
            SessionError::InvalidToken
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                SessionError::InvalidToken
            })
            .unwrap();

        let Some(token_player) = verified_token.get("player") else {
            debug!("Token does not contain player");
            return Err(SessionError::InvalidToken);
        };
        let Some(token_player) = token_player.as_i64() else {
            debug!("Token player is not valid");
            return Err(SessionError::InvalidToken);
        };
        let token_player = Objid(token_player);
        if let Some(objid) = objid {
            // Does the 'player' match objid? If not, reject it.
            if objid != token_player {
                debug!(?objid, ?token_player, "Token player does not match objid");
                return Err(SessionError::InvalidToken);
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

pub(crate) fn zmq_loop(
    keypair: Key<64>,
    connections_db_path: PathBuf,
    wss: Arc<dyn WorldStateSource>,
    scheduler: Arc<Scheduler>,
    rpc_endpoint: &str,
    narrative_endpoint: &str,
    num_threads: Option<i32>,
) -> eyre::Result<()> {
    let zmq_ctx = zmq::Context::new();
    if let Some(num_threads) = num_threads {
        zmq_ctx.set_io_threads(num_threads)?;
    }

    let rpc_server = Arc::new(RpcServer::new(
        keypair,
        connections_db_path,
        zmq_ctx.clone(),
        narrative_endpoint,
        wss,
        scheduler,
    ));

    // Start up the ping-ponger timer in a background thread...
    let t_rpc_server = rpc_server.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        t_rpc_server.ping_pong().expect("Unable to play ping-pong");
    });

    // We need to bind a generic publisher to the narrative endpoint, so that subsequent sessions
    // are visible...
    let rpc_socket = zmq_ctx.socket(zmq::REP)?;
    rpc_socket.bind(rpc_endpoint)?;

    info!(
        "0mq server listening on {} with {} IO threads",
        rpc_endpoint,
        zmq_ctx.get_io_threads().unwrap()
    );

    loop {
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
                        vec![make_response(Err(RpcRequestError::InvalidRequest))],
                        0,
                    )?;
                    continue;
                }

                if request.len() != 2 {
                    rpc_socket.send_multipart(
                        vec![make_response(Err(RpcRequestError::InvalidRequest))],
                        0,
                    )?;
                    continue;
                }

                let (client_id, request_body) = (&request[0], &request[1]);

                let Ok(client_id) = Uuid::from_slice(client_id) else {
                    rpc_socket.send_multipart(
                        vec![make_response(Err(RpcRequestError::InvalidRequest))],
                        0,
                    )?;
                    continue;
                };

                // Decode 'request_body' as a bincode'd ClientEvent.
                let request =
                    match bincode::decode_from_slice(request_body, bincode::config::standard()) {
                        Ok((request, _)) => request,
                        Err(_) => {
                            rpc_socket.send_multipart(
                                vec![make_response(Err(RpcRequestError::InvalidRequest))],
                                0,
                            )?;

                            continue;
                        }
                    };

                // The remainder of the payload are all the request arguments, which vary depending
                // on the type.
                let response = rpc_server.clone().process_request(client_id, request);
                rpc_socket.send_multipart(vec![response], 0)?;
            }
        }
    }
}
