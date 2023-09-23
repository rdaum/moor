use std::path::PathBuf;
/// The core of the server logic for the RPC daemon
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use anyhow::{Context, Error};
use futures_util::SinkExt;
use itertools::Itertools;
use metrics_macros::increment_counter;
use moor_kernel::tasks::command_parse::parse_into_words;
use tmq::publish::Publish;
use tmq::{publish, reply, Multipart};
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use crate::connections::Connections;
use crate::make_response;
use crate::rpc_session::RpcSession;
use moor_kernel::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_kernel::tasks::sessions::Session;
use moor_values::model::world_state::WorldStateSource;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::{v_objid, v_string};
use moor_values::SYSTEM_OBJECT;
use rpc_common::RpcResponse::{LoginResult, NewConnection};
use rpc_common::{
    BroadcastEvent, ConnectType, ConnectionEvent, RpcError, RpcRequest, RpcResponse,
    BROADCAST_TOPIC,
};

pub struct RpcServer {
    publish: Arc<Mutex<Publish>>,
    world_state_source: Arc<dyn WorldStateSource>,
    scheduler: Scheduler,
    connections: Connections,
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
            .bind(narrative_endpoint)
            .unwrap();
        let connections = Connections::new(connections_file).await;
        info!(
            "Created connections list, with {} initial known connections",
            connections.connections().await.len()
        );
        Self {
            world_state_source: wss,
            scheduler,
            connections: connections,
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
                    Ok(oid) => {
                        return make_response(Ok(NewConnection(oid)));
                    }
                    Err(e) => {
                        return make_response(Err(e));
                    }
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

                    return make_response(Err(RpcError::NoConnection));
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
                    return make_response(Err(RpcError::NoConnection));
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
                    return make_response(Err(RpcError::NoConnection));
                };

                make_response(
                    self.clone()
                        .perform_command(client_id, connection, command)
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
                    return make_response(Err(RpcError::NoConnection));
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
                    return make_response(Err(RpcError::NoConnection));
                };

                make_response(self.clone().eval(client_id, connection, evalstr).await)
            }
            RpcRequest::Detach => {
                increment_counter!("rpc_server.detach");
                info!("Detaching client: {}", client_id);

                // Detach this client id from the player/connection object.
                let Ok(_) = self.connections.remove_client_connection(client_id).await else {
                    return make_response(Err(RpcError::InternalError(
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
    ) -> Result<Arc<dyn Session>, Error> {
        debug!(?client_id, ?connection, "Started session",);
        increment_counter!("rpc_server.new_session");
        Ok(Arc::new(RpcSession::new(
            client_id,
            self.clone(),
            connection,
        )))
    }

    pub(crate) async fn connection_name_for(&self, player: Objid) -> Result<String, Error> {
        let connections = self.connections.connection_records_for(player).await?;
        // Grab the most recent connection record (they are sorted by last_activity, so last item).
        Ok(connections.last().unwrap().name.clone())
    }

    async fn last_activity_for(&self, player: Objid) -> Result<SystemTime, Error> {
        let connections = self.connections.connection_records_for(player).await?;
        // Grab the most recent connection record (they are sorted by last_activity, so last item).
        Ok(connections.last().unwrap().last_activity)
    }

    pub(crate) async fn idle_seconds_for(&self, player: Objid) -> Result<f64, Error> {
        let last_activity = self.last_activity_for(player).await?;
        Ok(last_activity.elapsed().unwrap().as_secs_f64())
    }

    pub(crate) async fn connected_seconds_for(&self, player: Objid) -> Result<f64, Error> {
        // Grab the highest of all connection times.
        let connections = self.connections.connection_records_for(player).await?;
        Ok(connections
            .iter()
            .map(|c| c.connect_time)
            .max()
            .unwrap()
            .elapsed()
            .unwrap()
            .as_secs_f64())
    }

    // TODO this will issue physical disconnects to *all* connections for this player.
    //   which probably isn't what you really want. This is just here to keep the existing behaviour
    //   of @quit and @boot-player working.
    //   in reality players using "@quit" will probably really want to just "sleep", and cores
    //   should be modified to reflect that.
    pub(crate) async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        warn!("Disconnecting player: {}", player);
        let connections = self.connections.connection_records_for(player).await?;
        let all_client_ids = connections.iter().map(|c| c.client_id).collect_vec();

        let mut publish = self.publish.lock().await;
        let event = ConnectionEvent::Disconnect();
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
        for client_id in all_client_ids {
            let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
            publish
                .send(payload)
                .await
                .expect("Unable to send system message");
        }

        Ok(())
    }

    pub(crate) async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        let connections = self.connections.connections().await;
        Ok(connections.iter().filter(|o| o.0 > 0).cloned().collect())
    }

    async fn request_sys_prop(
        self: Arc<Self>,
        object: String,
        property: String,
    ) -> Result<RpcResponse, RpcError> {
        let Ok(world_state) = self.world_state_source.new_world_state().await else {
            return Err(RpcError::CreateSessionFailed);
        };

        let Ok(sysprop) = world_state
            .retrieve_property(SYSTEM_OBJECT, SYSTEM_OBJECT, object.as_str())
            .await
        else {
            return Err(RpcError::ErrorCouldNotRetrieveSysProp(
                "could not access system object".to_string(),
            ));
        };

        let Variant::Obj(sysprop) = sysprop.variant() else {
            return Err(RpcError::ErrorCouldNotRetrieveSysProp(
                "system object invalid".to_string(),
            ));
        };

        let Ok(property_value) = world_state
            .retrieve_property(SYSTEM_OBJECT, *sysprop, property.as_str())
            .await
        else {
            return Err(RpcError::ErrorCouldNotRetrieveSysProp(
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
    ) -> Result<RpcResponse, RpcError> {
        debug!("Performing login for client: {}", client_id);
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_login.create_session_failed");
            return Err(RpcError::CreateSessionFailed);
        };
        let task_id = match self
            .clone()
            .scheduler
            .submit_verb_task(
                connection,
                SYSTEM_OBJECT,
                "do_login_command".to_string(),
                args.into_iter().map(v_string).collect(),
                SYSTEM_OBJECT,
                session,
            )
            .await
        {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting login task");
                increment_counter!("rpc_server.perform_login.submit_login_task_failed");
                return Err(RpcError::InternalError(e.to_string()));
            }
        };
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "Error subscribing to login task");
                increment_counter!("rpc_server.perform_login.subscribe_login_task_failed");
                return Err(RpcError::LoginTaskFailed);
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
                return Err(RpcError::LoginTaskFailed);
            }
            Err(e) => {
                error!(error = ?e, "Error waiting for login results");
                increment_counter!("rpc_server.perform_login.login_task_failed");
                return Err(RpcError::InternalError(e.to_string()));
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
            return Err(RpcError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        // Issue calls to user_connected/user_reconnected/user_created
        // TODO: Reconnected/created
        trace!(?player, "Submitting user_connected task");
        if let Err(e) = self
            .clone()
            .submit_connected_task(client_id, player, ConnectType::Connected)
            .await
        {
            error!(error = ?e, "Error submitting user_connected task");
            increment_counter!("rpc_server.perform_login.submit_connected_task_failed");
            // Note we still continue to return a successful login result here, hoping for the best
            // but we do log the error.
        }

        increment_counter!("rpc_server.perform_login.success");
        Ok(LoginResult(Some((ConnectType::Connected, player))))
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
    ) -> Result<RpcResponse, RpcError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_command.create_session_failed");
            return Err(RpcError::CreateSessionFailed);
        };

        // TODO: try to submit to do_command first and only parse_command after that fails.
        debug!(command, ?client_id, ?connection, "Submitting command task");
        let task_id = match self
            .clone()
            .scheduler
            .submit_command_task(connection, command.as_str(), session)
            .await
        {
            Ok(t) => t,
            Err(SchedulerError::CouldNotParseCommand(_)) => {
                increment_counter!("rpc_server.perform_command.could_not_parse_command");
                return Err(RpcError::CouldNotParseCommand);
            }
            Err(SchedulerError::NoCommandMatch(s, _)) => {
                increment_counter!("rpc_server.perform_command.no_command_match");
                return Err(RpcError::NoCommandMatch(s));
            }
            Err(e) => {
                increment_counter!("rpc_server.perform_command.submit_command_task_failed");
                error!(error = ?e, "Error submitting command task");
                return Err(RpcError::InternalError(e.to_string()));
            }
        };

        debug!(task_id, "Subscribed to command task results");
        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("rpc_server.perform_command.subscribe_command_task_failed");
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcError::InternalError(e.to_string()));
            }
        };

        match receiver.await {
            Ok(TaskWaiterResult::Success(_)) => {
                increment_counter!("rpc_server.perform_command.success");
                Ok(RpcResponse::CommandComplete)
            }
            Ok(TaskWaiterResult::Error(SchedulerError::PermissionDenied)) => {
                increment_counter!("rpc_server.perform_command.permission_denied");
                Err(RpcError::PermissionDenied)
            }
            Ok(TaskWaiterResult::Error(SchedulerError::CouldNotParseCommand(_))) => {
                increment_counter!("rpc_server.perform_command.could_not_parse_command");
                Err(RpcError::CouldNotParseCommand)
            }
            Ok(TaskWaiterResult::Error(SchedulerError::NoCommandMatch(s, _))) => {
                increment_counter!("rpc_server.perform_command.no_command_match");
                Err(RpcError::NoCommandMatch(s))
            }
            Ok(TaskWaiterResult::Error(SchedulerError::DatabaseError(e))) => {
                increment_counter!("rpc_server.perform_command.database_error");
                Err(RpcError::DatabaseError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => {
                warn!(error = ?e, "Error processing command");
                increment_counter!("rpc_server.perform_command.error");
                Err(RpcError::InternalError(e.to_string()))
            }
            Err(e) => {
                warn!(error = ?e, "Error processing command");
                increment_counter!("rpc_server.perform_command.error");
                Err(RpcError::InternalError(e.to_string()))
            }
        }
    }

    /// Call $do_out_of_band(command)
    async fn perform_out_of_band(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        command: String,
    ) -> Result<RpcResponse, RpcError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.perform_command.create_session_failed");
            return Err(RpcError::CreateSessionFailed);
        };
        let command_components = parse_into_words(command.as_str());
        let _ = match self
            .clone()
            .scheduler
            .submit_out_of_band_task(connection, command_components, command, session)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                increment_counter!("rpc_server.perform_out_of_band.do_out_of_band_failed");
                error!(error = ?e, "Error submitting command task");
                return Err(RpcError::InternalError(e.to_string()));
            }
        };

        increment_counter!("rpc_server.perform_out_of_band.submitted");

        // Just return immediately with success, we do not wait for the task to complete, we'll
        // let the session run to completion on its own and output back to the client.
        // Maybe we should be returning a value from this for the future, but the way clients are
        // written right now, there's little point.
        Ok(RpcResponse::CommandComplete)
    }

    async fn eval(
        self: Arc<Self>,
        client_id: Uuid,
        connection: Objid,
        expression: String,
    ) -> Result<RpcResponse, RpcError> {
        let Ok(session) = self.clone().new_session(client_id, connection).await else {
            increment_counter!("rpc_server.eval.create_session_failed");
            return Err(RpcError::CreateSessionFailed);
        };

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
                return Err(RpcError::InternalError(e.to_string()));
            }
        };

        let receiver = match self.clone().scheduler.subscribe_to_task(task_id).await {
            Ok(r) => r,
            Err(e) => {
                increment_counter!("rpc_server.eval.subscribe_eval_task_failed");
                error!(error = ?e, "Error subscribing to command task");
                return Err(RpcError::InternalError(e.to_string()));
            }
        };

        match receiver.await {
            Ok(TaskWaiterResult::Success(v)) => Ok(RpcResponse::EvalResult(v)),
            Ok(TaskWaiterResult::Error(SchedulerError::DatabaseError(e))) => {
                increment_counter!("rpc_server.eval.database_error");
                Err(RpcError::DatabaseError(e))
            }
            Ok(TaskWaiterResult::Error(e)) => {
                error!(error = ?e, "Error processing eval");
                increment_counter!("rpc_server.eval.task_error");
                Err(RpcError::InternalError(e.to_string()))
            }
            Err(e) => {
                error!(error = ?e, "Error processing eval");
                increment_counter!("rpc_server.eval.internal_error");
                Err(RpcError::InternalError(e.to_string()))
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
            let connections = self.connections.connection_records_for(*player).await?;
            let client_ids = connections.iter().map(|c| c.client_id).collect_vec();
            let event = ConnectionEvent::Narrative(*player, event.clone());
            let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
            for client_id in &client_ids {
                let payload = vec![client_id.as_bytes().to_vec(), event_bytes.clone()];
                publish
                    .send(payload)
                    .await
                    .expect("Unable to send narrative event");
            }
        }
        Ok(())
    }

    pub(crate) async fn send_system_message(
        &self,
        client_id: Uuid,
        player: Objid,
        message: String,
    ) -> Result<(), Error> {
        increment_counter!("rpc_server.send_system_message");
        let mut publish = self.publish.lock().await;
        let event = ConnectionEvent::SystemMessage(player, message);
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())?;
        let payload = vec![client_id.as_bytes().to_vec(), event_bytes];
        publish
            .send(payload)
            .await
            .expect("Unable to send system message");
        Ok(())
    }

    async fn ping_pong(&self) {
        let mut publish = self.publish.lock().await;
        let event = BroadcastEvent::PingPong(SystemTime::now());
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![BROADCAST_TOPIC.to_vec(), event_bytes];
        publish
            .send(payload)
            .await
            .expect("Unable to send system message");
        self.connections.ping_check().await;
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
                rpc_server.ping_pong().await;
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
                        .send(make_response(Err(RpcError::InvalidRequest)))
                        .await?;
                    continue;
                }

                let (Some(client_id), Some(request_body)) =
                    (request.pop_front(), request.pop_front())
                else {
                    increment_counter!("rpc_server.invalid_request");
                    rpc_socket = reply
                        .send(make_response(Err(RpcError::InvalidRequest)))
                        .await?;
                    continue;
                };

                let Ok(client_id) = Uuid::from_slice(&client_id) else {
                    increment_counter!("rpc_server.invalid_request");
                    rpc_socket = reply
                        .send(make_response(Err(RpcError::InvalidRequest)))
                        .await?;
                    continue;
                };

                // Decode 'request_body' as a bincode'd ClientEvent.
                let request =
                    match bincode::decode_from_slice(&request_body, bincode::config::standard()) {
                        Ok((request, _)) => request,
                        Err(_) => {
                            rpc_socket = reply
                                .send(make_response(Err(RpcError::InvalidRequest)))
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
