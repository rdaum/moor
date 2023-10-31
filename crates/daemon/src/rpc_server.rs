use std::path::PathBuf;
/// The core of the server logic for the RPC daemon
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use anyhow::{Context, Error};
use futures_util::SinkExt;
use metrics_macros::increment_counter;
use tmq::publish::Publish;
use tmq::{publish, reply, Multipart};
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use moor_kernel::tasks::command_parse::parse_into_words;
use moor_kernel::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_kernel::tasks::sessions::SessionError::DeliveryError;
use moor_kernel::tasks::sessions::{Session, SessionError};
use moor_kernel::tasks::TaskId;
use moor_values::model::world_state::WorldStateSource;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::Var;
use moor_values::var::{v_bool, v_objid, v_str, v_string};
use moor_values::SYSTEM_OBJECT;
use rpc_common::RpcResponse::{LoginResult, NewConnection};
use rpc_common::{
    BroadcastEvent, ConnectType, ConnectionEvent, RpcRequest, RpcRequestError, RpcResponse,
    BROADCAST_TOPIC,
};

use crate::connections::ConnectionsDB;
use crate::connections_tb::ConnectionsTb;
use crate::make_response;
use crate::rpc_session::RpcSession;

pub struct RpcServer {
    publish: Arc<Mutex<Publish>>,
    world_state_source: Arc<dyn WorldStateSource>,
    scheduler: Scheduler,
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

impl RpcServer {
    pub async fn new(
        connections_file: PathBuf,
        zmq_context: tmq::Context,
        narrative_endpoint: &str,
        wss: Arc<dyn WorldStateSource>,
        scheduler: Scheduler,
    ) -> Self {
        info!(
            "Creating new RPC server; with {} ZMQ IO threads...",
            zmq_context.get_io_threads().unwrap()
        );
        let publish = publish(&zmq_context.clone())
            .set_sndtimeo(1)
            .bind(narrative_endpoint)
            .unwrap();
        let connections = Arc::new(ConnectionsTb::new(Some(connections_file)).await);
        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().await.len()
        );
        Self {
            world_state_source: wss,
            scheduler,
            connections,
            publish: Arc::new(Mutex::new(publish)),
        }
    }

    /// Process a request (originally ZMQ REQ) and produce a reply (becomes ZMQ REP)
    pub async fn process_request(
        self: Arc<Self>,
        client_id: Uuid,
        request: RpcRequest,
    ) -> Multipart {
        increment_counter!("rpc_server.process_request");
        match request {
            RpcRequest::ConnectionEstablish(hostname) => {
                increment_counter!("rpc_server.connection_establish");

                match self.connections.new_connection(client_id, hostname).await {
                    Ok(oid) => make_response(Ok(NewConnection(oid))),
                    Err(e) => make_response(Err(e)),
                }
            }
            RpcRequest::Pong(_client_sys_time) => {
                // Always respond with a ThanksPong, even if it's somebody we don't know.
                // Can easily be a connection that was in the middle of negotiation at the time the
                // ping was sent out, or dangling in some other way.
                let response = make_response(Ok(RpcResponse::ThanksPong(SystemTime::now())));
                increment_counter!("rpc_server.pong");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    warn!("Received Pong from invalid client: {}", client_id);
                    return response;
                };
                // Let 'connections' know that the connection is still alive.
                let Ok(_) = self
                    .connections
                    .notify_is_alive(client_id, connection)
                    .await
                else {
                    warn!("Unable to notify connection is alive: {}", client_id);
                    return response;
                };
                response
            }
            RpcRequest::RequestSysProp(object, property) => {
                increment_counter!("rpc_server.request_sys_prop");
                if !self.connections.is_valid_client(client_id).await {
                    warn!("Received RequestSysProp from invalid client: {}", client_id);

                    return make_response(Err(RpcRequestError::NoConnection));
                }
                make_response(self.clone().request_sys_prop(object, property).await)
            }
            RpcRequest::LoginCommand(args) => {
                increment_counter!("rpc_server.login_command");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                make_response(
                    self.clone()
                        .perform_login(client_id, connection, args)
                        .await,
                )
            }
            RpcRequest::Command(command) => {
                increment_counter!("rpc_server.command");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                make_response(
                    self.clone()
                        .perform_command(client_id, connection, command)
                        .await,
                )
            }
            RpcRequest::RequestedInput(request_id, input) => {
                increment_counter!("rpc_server.requested_input");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                let request_id = Uuid::from_u128(request_id);
                make_response(
                    self.clone()
                        .respond_input(client_id, connection, request_id, input)
                        .await,
                )
            }
            RpcRequest::OutOfBand(command) => {
                increment_counter!("rpc_server.out_of_band_received");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                make_response(
                    self.clone()
                        .perform_out_of_band(client_id, connection, command)
                        .await,
                )
            }

            RpcRequest::Eval(evalstr) => {
                increment_counter!("rpc_server.eval");
                let Some(connection) = self
                    .connections
                    .connection_object_for_client(client_id)
                    .await
                else {
                    return make_response(Err(RpcRequestError::NoConnection));
                };

                make_response(self.clone().eval(client_id, connection, evalstr).await)
            }
            RpcRequest::Detach => {
                increment_counter!("rpc_server.detach");
                info!("Detaching client: {}", client_id);

                // Detach this client id from the player/connection object.
                let Ok(_) = self.connections.remove_client_connection(client_id).await else {
                    return make_response(Err(RpcRequestError::InternalError(
                        "Unable to remove client connection".to_string(),
                    )));
                };

                make_response(Ok(RpcResponse::Disconnected))
            }
        }
    }

    pub(crate) async fn new_session(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
    ) -> Result<Arc<dyn Session>, SessionError> {
        debug!(?client_id, ?connection, "Started session",);
        increment_counter!("rpc_server.new_session");
        Ok(Arc::new(RpcSession::new(
            client_id,
            self.clone(),
            connection,
        )))
    }

    pub(crate) async fn connection_name_for(&self, player: Objid) -> Result<String, SessionError> {
        self.connections.connection_name_for(player).await
    }

    #[allow(dead_code)]
    async fn last_activity_for(&self, player: Objid) -> Result<SystemTime, SessionError> {
        self.connections.last_activity_for(player).await
    }

    pub(crate) async fn idle_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        let last_activity = self.connections.last_activity_for(player).await?;
        Ok(last_activity.elapsed().unwrap().as_secs_f64())
    }

    pub(crate) async fn connected_seconds_for(&self, player: Objid) -> Result<f64, SessionError> {
        self.connections.connected_seconds_for(player).await
    }

    // TODO this will issue physical disconnects to *all* connections for this player.
    //   which probably isn't what you really want. This is just here to keep the existing behaviour
    //   of @quit and @boot-player working.
    //   in reality players using "@quit" will probably really want to just "sleep", and cores
    //   should be modified to reflect that.
    pub(crate) async fn disconnect(&self, player: Objid) -> Result<(), SessionError> {
        warn!("Disconnecting player: {}", player);
        let all_client_ids = self.connections.client_ids_for(player).await?;

        let mut publish = self.publish.lock().await;
        let event = ConnectionEvent::Disconnect();
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())
            .expect("Unable to serialize disconnection event");
        for client_id in all_client_ids {
            let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
            publish.send(payload).await.map_err(|e| {
                error!(
                    "Unable to send disconnection event to narrative channel: {}",
                    e
                );
                DeliveryError
            })?
        }

        Ok(())
    }

    pub(crate) async fn connected_players(&self) -> Result<Vec<Objid>, SessionError> {
        let connections = self.connections.connections().await;
        Ok(connections.iter().filter(|o| o.0 > 0).cloned().collect())
    }

    async fn request_sys_prop(
        self: Arc<Self>,
        object: String,
        property: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(world_state) = self.world_state_source.new_world_state().await else {
            return Err(RpcRequestError::CreateSessionFailed);
        };

        let Ok(sysprop) = world_state
            .retrieve_property(SYSTEM_OBJECT, SYSTEM_OBJECT, object.as_str())
            .await
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

        let Ok(property_value) = world_state
            .retrieve_property(SYSTEM_OBJECT, *sysprop, property.as_str())
            .await
        else {
            return Err(RpcRequestError::ErrorCouldNotRetrieveSysProp(
                "could not sysprop".to_string(),
            ));
        };

        Ok(RpcResponse::SysPropValue(Some(property_value)))
    }

    async fn perform_login(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        args: Vec<String>,
    ) -> Result<RpcResponse, RpcRequestError> {
        increment_counter!("rpc_server.perform_login");

        // TODO: change result of login to return this information, rather than just Objid, so
        //   we're not dependent on this.
        let connect_type = if args.get(0) == Some(&"create".to_string()) {
            ConnectType::Created
        } else {
            ConnectType::Connected
        };

        debug!(
            "Performing {:?} login for client: {}",
            connect_type, client_id
        );
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_login.create_session_failed");
            return Err(RpcRequestError::CreateSessionFailed);
        };
        let task_id = match self
            .clone()
            .scheduler
            .submit_verb_task(
                connection,
                SYSTEM_OBJECT,
                "do_login_command".to_string(),
                args.iter().map(|s| v_string(s.clone())).collect(),
                args.join(" "),
                SYSTEM_OBJECT,
                session,
            )
            .await
        {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting login task");
                increment_counter!("rpc_server.perform_login.submit_login_task_failed");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "Error subscribing to login task");
                increment_counter!("rpc_server.perform_login.subscribe_login_task_failed");
                return Err(RpcRequestError::LoginTaskFailed);
            }
        };
        let player = match receiver.await {
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
                increment_counter!("rpc_server.perform_login.login_task_failed");
                return Err(RpcRequestError::LoginTaskFailed);
            }
            Err(e) => {
                error!(error = ?e, "Error waiting for login results");
                increment_counter!("rpc_server.perform_login.login_task_failed");
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
            .await
        else {
            increment_counter!("rpc_server.perform_login.update_client_connection_failed");
            return Err(RpcRequestError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        // Issue calls to user_connected/user_reconnected/user_created
        // TODO: Reconnected/created
        trace!(?player, "Submitting user_connected task");
        if let Err(e) = self
            .clone()
            .submit_connected_task(client_id, player, connect_type)
            .await
        {
            error!(error = ?e, "Error submitting user_connected task");
            increment_counter!("rpc_server.perform_login.submit_connected_task_failed");
            // Note we still continue to return a successful login result here, hoping for the best
            // but we do log the error.
        }

        increment_counter!("rpc_server.perform_login.success");
        Ok(LoginResult(Some((connect_type, player))))
    }

    async fn submit_connected_task(
        self: Arc<Self>,
        client_id: Uuid,
        player: Objid,
        initiation_type: ConnectType,
    ) -> Result<(), anyhow::Error> {
        let session = self
            .clone()
            .new_session(client_id, player)
            .await
            .with_context(|| "could not create 'connected' task session for player")?;

        increment_counter!("rpc_server.submit_connected_task");

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
            .await
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    async fn perform_command(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_command.create_session_failed");
            return Err(RpcRequestError::CreateSessionFailed);
        };

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
            .await
        {
            warn!("Unable to update client connection activity: {}", e);
        };
        increment_counter!("rpc_server.perform_command");

        // Try to submit to do_command as a verb call first and only parse_command after that fails.
        // TODO: fold this functionality into Task.
        increment_counter!("rpc_server.submit_sys_do_command_task");
        let arguments = parse_into_words(command.as_str());
        if let Ok(task_id) = self
            .clone()
            .scheduler
            .submit_verb_task(
                connection,
                SYSTEM_OBJECT,
                "do_command".to_string(),
                arguments.iter().map(|s| v_str(s)).collect(),
                command.clone(),
                SYSTEM_OBJECT,
                session.clone(),
            )
            .await
        {
            if let Ok(value) = self.clone().watch_command_task(task_id).await {
                if value != v_bool(false) {
                    return Ok(RpcResponse::CommandSubmitted(task_id));
                }
            }
        }

        // That having failed, we do the classic internal parse command cycle instead...
        increment_counter!("rpc_server.submit_command_task");
        debug!(
            command,
            ?client_id,
            ?connection,
            "Invoking submit_command_task"
        );
        let task_id = match self
            .clone()
            .scheduler
            .submit_command_task(connection, command.as_str(), session)
            .await
        {
            Ok(t) => t,
            Err(SchedulerError::CommandExecutionError(e)) => {
                increment_counter!("rpc_server.perform_command.could_not_parse_command");
                return Err(RpcRequestError::CommandError(e));
            }
            Err(e) => {
                increment_counter!("rpc_server.perform_command.submit_command_task_failed");
                error!(error = ?e, "Error submitting command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    async fn respond_input(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        input_request_id: Uuid,
        input: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
            .await
        {
            warn!("Unable to update client connection activity: {}", e);
        };
        increment_counter!("rpc_server.respond_input");

        // Pass this back over to the scheduler to handle.
        if let Err(e) = self
            .clone()
            .scheduler
            .submit_requested_input(connection, input_request_id, input)
            .await
        {
            increment_counter!("rpc_server.respond_input.submit_requested_input_failed");
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcRequestError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(RpcResponse::InputThanks)
    }

    async fn watch_command_task(self: Arc<Self>, task_id: TaskId) -> Result<Var, RpcRequestError> {
        debug!(task_id, "Subscribed to command task results");
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("rpc_server.perform_command.subscribe_command_task_failed");
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        match receiver.await {
            Ok(TaskWaiterResult::Success(value)) => Ok(value),
            Ok(TaskWaiterResult::Error(SchedulerError::CommandExecutionError(e))) => {
                Err(RpcRequestError::CommandError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => Err(RpcRequestError::InternalError(e.to_string())),
            Err(e) => {
                increment_counter!("rpc_server.perform_command.error");
                Err(RpcRequestError::InternalError(e.to_string()))
            }
        }
    }

    /// Call $do_out_of_band(command)
    async fn perform_out_of_band(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_command.create_session_failed");
            return Err(RpcRequestError::CreateSessionFailed);
        };
        increment_counter!("rpc_server.perform_out_of_band");

        let command_components = parse_into_words(command.as_str());
        let task_id = match self
            .clone()
            .scheduler
            .submit_out_of_band_task(connection, command_components, command, session)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                increment_counter!("rpc_server.perform_out_of_band.do_out_of_band_failed");
                error!(error = ?e, "Error submitting command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        increment_counter!("rpc_server.perform_out_of_band.submitted");

        // Just return immediately with success, we do not wait for the task to complete, we'll
        // let the session run to completion on its own and output back to the client.
        // Maybe we should be returning a value from this for the future, but the way clients are
        // written right now, there's little point.
        Ok(RpcResponse::CommandSubmitted(task_id))
    }

    async fn eval(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        expression: String,
    ) -> Result<RpcResponse, RpcRequestError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.eval.create_session_failed");
            return Err(RpcRequestError::CreateSessionFailed);
        };

        increment_counter!("rpc_server.eval");
        let task_id = match self
            .clone()
            .scheduler
            .submit_eval_task(connection, connection, expression, session)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                increment_counter!("rpc_server.eval.submit_eval_task_failed");
                error!(error = ?e, "Error submitting eval task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("rpc_server.eval.subscribe_eval_task_failed");
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcRequestError::InternalError(e.to_string()));
            }
        };

        match receiver.await {
            Ok(TaskWaiterResult::Success(v)) => Ok(RpcResponse::EvalResult(v)),
            Ok(TaskWaiterResult::Error(SchedulerError::CommandExecutionError(e))) => {
                increment_counter!("rpc_server.eval.database_error");
                Err(RpcRequestError::CommandError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => {
                error!(error = ?e, "Error processing eval");
                increment_counter!("rpc_server.eval.task_error");
                Err(RpcRequestError::InternalError(e.to_string()))
            }
            Err(e) => {
                error!(error = ?e, "Error processing eval");
                increment_counter!("rpc_server.eval.internal_error");
                Err(RpcRequestError::InternalError(e.to_string()))
            }
        }
    }

    pub(crate) async fn publish_narrative_events(
        &self,
        events: &[(Objid, NarrativeEvent)],
    ) -> Result<(), Error> {
        increment_counter!("rpc_server.publish_narrative_events");
        let mut publish = self.publish.lock().await;
        for (player, event) in events {
            let client_ids = self.connections.client_ids_for(*player).await?;
            let event = ConnectionEvent::Narrative(*player, event.clone());
            let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
            for client_id in &client_ids {
                let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
                publish.send(payload).await.map_err(|e| {
                    error!(error = ?e, "Unable to send narrative event");
                    DeliveryError
                })?;
            }
        }
        Ok(())
    }

    pub(crate) async fn send_system_message(
        &self,
        client_id: Uuid,
        player: Objid,
        message: String,
    ) -> Result<(), SessionError> {
        increment_counter!("rpc_server.send_system_message");
        let event = ConnectionEvent::SystemMessage(player, message);
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())
            .expect("Unable to serialize system message");
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        {
            let mut publish = self.publish.lock().await;
            publish.send(payload).await.map_err(|e| {
                error!(error = ?e, "Unable to send system message");
                DeliveryError
            })?;
        }
        Ok(())
    }

    /// Request that the client dispatch its next input event through as an input event into the
    /// scheduler submit_input, instead, with the attached input_request_id. So send a narrative
    /// event to this *specific* client id letting it know that it should issue a prompt.
    pub(crate) async fn request_client_input(
        &self,
        client_id: Uuid,
        player: Objid,
        input_request_id: Uuid,
    ) -> Result<(), SessionError> {
        // Mark this client as in `input mode`, which means that instead of dispatching its next
        // line to the scheduler as a command, it should instead dispatch it as an input event.

        // Validate first.
        let Some(connection) = self
            .connections
            .connection_object_for_client(client_id)
            .await
        else {
            return Err(SessionError::NoConnectionForPlayer(player));
        };
        if connection != player {
            return Err(SessionError::NoConnectionForPlayer(player));
        }

        let event = ConnectionEvent::RequestInput(input_request_id.as_u128());
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())
            .expect("Unable to serialize input request");
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        {
            let mut publish = self.publish.lock().await;
            publish.send(payload).await.map_err(|e| {
                error!(error = ?e, "Unable to send input request");
                DeliveryError
            })?;
        }
        Ok(())
    }

    async fn ping_pong(&self) -> Result<(), SessionError> {
        let event = BroadcastEvent::PingPong(SystemTime::now());
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let mut publish = self.publish.lock().await;
            publish.send(payload).await.map_err(|e| {
                error!(error = ?e, "Unable to send PingPong to client");
                DeliveryError
            })?;
        }
        self.connections.ping_check().await;
        Ok(())
    }
}

pub(crate) async fn zmq_loop(
    connections_file: PathBuf,
    wss: Arc<dyn WorldStateSource>,
    scheduler: Scheduler,
    rpc_endpoint: &str,
    narrative_endpoint: &str,
) -> anyhow::Result<()> {
    let zmq_ctx = tmq::Context::new();
    zmq_ctx
        .set_io_threads(8)
        .expect("Unable to set ZMQ IO threads");

    let rpc_server = Arc::new(
        RpcServer::new(
            connections_file,
            zmq_ctx.clone(),
            narrative_endpoint,
            wss,
            scheduler,
        )
        .await,
    );

    // Start up the ping-ponger timer in a background thread...
    tokio::spawn({
        let rpc_server = rpc_server.clone();
        async move {
            let mut ping_pong = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                ping_pong.tick().await;
                rpc_server
                    .ping_pong()
                    .await
                    .expect("Unable to play ping-pong");
            }
        }
    });

    // We need to bind a generic publisher to the narrative endpoint, so that subsequent sessions
    // are visible...
    let mut rpc_socket = reply(&zmq_ctx).bind(rpc_endpoint)?;
    loop {
        match rpc_socket.recv().await {
            Err(_) => {
                info!("ZMQ socket closed, exiting");
                return Ok(());
            }
            Ok((mut request, reply)) => {
                increment_counter!("rpc_server.request");
                trace!(num_parts = request.len(), "ZQM Request received");

                // Components are:
                if request.len() != 2 {
                    error!("Invalid request received, ignoring");
                    increment_counter!("rpc_server.invalid_request");
                    rpc_socket = reply
                        .send(make_response(Err(RpcRequestError::InvalidRequest)))
                        .await?;
                    continue;
                }

                let (Some(client_id), Some(request_body)) =
                    (request.pop_front(), request.pop_front())
                else {
                    increment_counter!("rpc_server.invalid_request");
                    rpc_socket = reply
                        .send(make_response(Err(RpcRequestError::InvalidRequest)))
                        .await?;
                    continue;
                };

                let Ok(client_id) = Uuid::from_slice(&client_id) else {
                    increment_counter!("rpc_server.invalid_request");
                    rpc_socket = reply
                        .send(make_response(Err(RpcRequestError::InvalidRequest)))
                        .await?;
                    continue;
                };

                // Decode 'request_body' as a bincode'd ClientEvent.
                let request =
                    match bincode::decode_from_slice(&request_body, bincode::config::standard()) {
                        Ok((request, _)) => request,
                        Err(_) => {
                            rpc_socket = reply
                                .send(make_response(Err(RpcRequestError::InvalidRequest)))
                                .await?;
                            continue;
                        }
                    };

                // The remainder of the payload are all the request arguments, which vary depending
                // on the type.
                let response = rpc_server.clone().process_request(client_id, request).await;
                rpc_socket = reply.send(response).await?;
                increment_counter!("rpc_server.processed_requests");
            }
        }
    }
}
