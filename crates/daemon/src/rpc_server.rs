/// The core of the server logic for the RPC daemon
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Error};
use async_trait::async_trait;
use futures_util::SinkExt;
use itertools::Itertools;
use metrics_macros::increment_counter;
use moor_kernel::tasks::command_parse::parse_into_words;
use tmq::publish::Publish;
use tmq::{publish, reply, Multipart};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use crate::make_response;
use moor_kernel::tasks::scheduler::{Scheduler, SchedulerError, TaskWaiterResult};
use moor_kernel::tasks::sessions::Session;
use moor_values::model::world_state::WorldStateSource;
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;
use moor_values::var::variant::Variant;
use moor_values::var::{v_objid, v_string};
use moor_values::SYSTEM_OBJECT;
use rpc_common::RpcResponse::{LoginResult, NewConnection};
use rpc_common::{ConnectType, ConnectionEvent, RpcError, RpcRequest, RpcResponse};

pub struct RpcServer {
    publish: Arc<Mutex<Publish>>,
    world_state_source: Arc<dyn WorldStateSource>,
    scheduler: Scheduler,
    // TODO: these should be backed by a database (eg. rocks or whatever) so that client connections
    //  are persistent between restarts.
    client_connections: RwLock<HashMap<Uuid, Objid>>,
    connections_client: RwLock<HashMap<Objid, Vec<ConnectionRecord>>>,
    next_connection_id: AtomicI64,
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
    pub fn new(
        zmq_context: tmq::Context,
        narrative_endpoint: &str,
        wss: Arc<dyn WorldStateSource>,
        scheduler: Scheduler,
    ) -> Self {
        let publish = publish(&zmq_context.clone())
            .bind(narrative_endpoint)
            .unwrap();
        Self {
            world_state_source: wss,
            scheduler,
            client_connections: Default::default(),
            connections_client: Default::default(),
            next_connection_id: AtomicI64::new(-4),
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
                make_response(
                    self.connection_establish(client_id, hostname.as_str())
                        .await,
                )
            }
            RpcRequest::RequestSysProp(object, property) => {
                increment_counter!("rpc_server.request_sys_prop");
                {
                    let client_connections = self.client_connections.read().await;
                    let Some(_) = client_connections.get(&client_id) else {
                        return make_response(Err(RpcError::InvalidRequest));
                    };
                }
                make_response(self.clone().request_sys_prop(object, property).await)
            }
            RpcRequest::LoginCommand(args) => {
                increment_counter!("rpc_server.login_command");
                let connection = {
                    let client_connections = self.client_connections.read().await;
                    let Some(connection) = client_connections.get(&client_id) else {
                        return make_response(Err(RpcError::NoConnection));
                    };
                    *connection
                };

                make_response(
                    self.clone()
                        .perform_login(client_id, connection, args)
                        .await,
                )
            }
            RpcRequest::Command(command) => {
                increment_counter!("rpc_server.command");
                let connection = {
                    let client_connections = self.client_connections.read().await;
                    let Some(connection) = client_connections.get(&client_id) else {
                        return make_response(Err(RpcError::NoConnection));
                    };
                    *connection
                };
                make_response(
                    self.clone()
                        .perform_command(client_id, connection, command)
                        .await,
                )
            }
            RpcRequest::OutOfBand(command) => {
                increment_counter!("rpc_server.out_of_band_received");
                let connection = {
                    let client_connections = self.client_connections.read().await;
                    let Some(connection) = client_connections.get(&client_id) else {
                        return make_response(Err(RpcError::NoConnection));
                    };
                    *connection
                };
                make_response(
                    self.clone()
                        .perform_out_of_band(client_id, connection, command)
                        .await,
                )
            }
            RpcRequest::Eval(evalstr) => {
                increment_counter!("rpc_server.eval");
                let connection = {
                    let client_connections = self.client_connections.read().await;
                    let Some(connection) = client_connections.get(&client_id) else {
                        return make_response(Err(RpcError::NoConnection));
                    };
                    *connection
                };

                make_response(self.clone().eval(client_id, connection, evalstr).await)
            }
            RpcRequest::Detach => {
                increment_counter!("rpc_server.detach");
                // Detach this client id from the player/connection object.
                let Ok(_) = self.remove_client_connection(client_id).await else {
                    return make_response(Err(RpcError::InternalError(
                        "Unable to remove client connection".to_string(),
                    )));
                };

                make_response(Ok(RpcResponse::Disconnected))
            }
        }
    }

    async fn new_session(
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

    async fn connection_records_for(&self, player: Objid) -> Result<Vec<ConnectionRecord>, Error> {
        let connections_client = self.connections_client.read().await;

        let Some(connections) = connections_client.get(&player) else {
            bail!("No connections for player: {}", player);
        };

        if connections.is_empty() {
            bail!("No connections for player: {}", player);
        }

        Ok(connections
            .iter()
            .sorted_by_key(|a| a.last_activity)
            .cloned()
            .collect())
    }

    async fn remove_client_connection(&self, client_id: Uuid) -> Result<(), anyhow::Error> {
        let (mut client_connections, mut connections_client) = (
            self.client_connections.write().await,
            self.connections_client.write().await,
        );

        let Some(connection) = client_connections.remove(&client_id) else {
            bail!("No (expected) connection for client: {}", client_id);
        };

        let Some(clients) = connections_client.get_mut(&connection) else {
            bail!("No (expected) connection record for player: {}", connection);
        };

        clients.retain(|c| c.client_id != client_id);

        Ok(())
    }

    async fn update_client_connection(
        &self,
        from_connection: Objid,
        to_player: Objid,
    ) -> Result<(), anyhow::Error> {
        let (mut client_connections, mut connections_client) = (
            self.client_connections.write().await,
            self.connections_client.write().await,
        );

        let mut connection_records = connections_client
            .remove(&from_connection)
            .expect("connection record missing");
        assert_eq!(
            connection_records.len(),
            1,
            "connection record for unlogged in connection has multiple entries"
        );
        let mut cr = connection_records.pop().unwrap();
        cr.player = to_player;
        cr.last_activity = Instant::now();

        client_connections.insert(cr.client_id, to_player);
        match connections_client.get_mut(&to_player) {
            None => {
                info!("insert new connection...");
                connections_client.insert(to_player, vec![cr]);
            }
            Some(ref mut crs) => {
                info!("append to existing connections...");
                crs.push(cr);
            }
        }
        connections_client.remove(&from_connection);
        Ok(())
    }

    async fn connection_name_for(&self, player: Objid) -> Result<String, Error> {
        let connections = self.connection_records_for(player).await?;
        // Grab the most recent connection record (they are sorted by last_activity, so last item).
        Ok(connections.last().unwrap().name.clone())
    }

    async fn last_activity_for(&self, player: Objid) -> Result<Instant, Error> {
        let connections = self.connection_records_for(player).await?;
        // Grab the most recent connection record (they are sorted by last_activity, so last item).
        Ok(connections.last().unwrap().last_activity)
    }

    async fn idle_seconds_for(&self, player: Objid) -> Result<f64, Error> {
        let last_activity = self.last_activity_for(player).await?;
        Ok(last_activity.elapsed().as_secs_f64())
    }

    async fn connected_seconds_for(&self, player: Objid) -> Result<f64, Error> {
        // Grab the highest of all connection times.
        let connections = self.connection_records_for(player).await?;
        Ok(connections
            .iter()
            .map(|c| c.connect_time)
            .max()
            .unwrap()
            .elapsed()
            .as_secs_f64())
    }

    // TODO this will issue physical disconnects to *all* connections for this player.
    //   which probably isn't what you really want. This is just here to keep the existing behaviour
    //   of @quit and @boot-player working.
    //   in reality players using "@quit" will probably really want to just "sleep", and cores
    //   should be modified to reflect that.
    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        warn!("Disconnecting player: {}", player);
        let connections = self.connection_records_for(player).await?;
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

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        let connections_client = self.connections_client.read().await;

        Ok(connections_client
            .iter()
            .map(|c| *c.0)
            .filter(|c| c.0 > 0)
            .collect())
    }

    async fn connection_establish(
        self: Arc<Self>,
        client_id: Uuid,
        hostname: &str,
    ) -> Result<RpcResponse, RpcError> {
        // We should not already have an object connection id for this client. If we do,
        // respond with an error.
        let (mut client_connections, mut connections_client) = (
            self.client_connections.write().await,
            self.connections_client.write().await,
        );

        if client_connections.contains_key(&client_id) {
            return Err(RpcError::AlreadyConnected);
        }

        // Get a new connection id, and create an entry for it.
        let connection_id = Objid(self.next_connection_id.fetch_sub(1, Ordering::SeqCst));
        client_connections.insert(client_id, connection_id);
        connections_client.insert(
            connection_id,
            vec![ConnectionRecord {
                client_id,
                player: connection_id,
                name: hostname.to_string(),
                last_activity: Instant::now(),
                connect_time: Instant::now(),
            }],
        );

        // And respond with the connection id via NewConnection;
        Ok(NewConnection(connection_id))
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
        info!(
            ?connection,
            ?player,
            "Transitioning connection record to logged in"
        );
        let Ok(_) = self.update_client_connection(connection, player).await else {
            increment_counter!("rpc_server.perform_login.update_client_connection_failed");
            return Err(RpcError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        // Issue calls to user_connected/user_reconnected/user_created
        // TODO: Reconnected/created
        info!(?player, "Submitting user_connected task");
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

    async fn publish_narrative_events(
        &self,
        events: &[(Objid, NarrativeEvent)],
    ) -> Result<(), Error> {
        increment_counter!("rpc_server.publish_narrative_events");
        let mut publish = self.publish.lock().await;
        let connections_client = self.connections_client.read().await;
        for (player, event) in events {
            let connections = match connections_client.get(player) {
                None => {
                    // No connections for this player, so we can't send them anything.
                    continue;
                }
                Some(connections) => connections,
            };
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

    async fn send_system_message(
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
}

pub(crate) async fn zmq_loop(
    wss: Arc<dyn WorldStateSource>,
    scheduler: Scheduler,
    rpc_endpoint: &str,
    narrative_endpoint: &str,
) -> anyhow::Result<()> {
    let zmq_ctx = tmq::Context::new();
    let rpc_server = Arc::new(RpcServer::new(
        zmq_ctx.clone(),
        narrative_endpoint,
        wss,
        scheduler,
    ));
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

/// A "session" that runs over the RPC system.
pub struct RpcSession {
    client_id: Uuid,
    rpc_server: Arc<RpcServer>,
    player: Objid,
    // TODO: manage this buffer better -- e.g. if it grows too big, for long-running tasks, etc. it
    //  should be mmap'd to disk or something.
    // TODO: We could also use Boxcar or other append-only lockless container for this, since we only
    //  ever append.
    session_buffer: Mutex<Vec<(Objid, NarrativeEvent)>>,
}

impl RpcSession {
    pub fn new(client_id: Uuid, rpc_server: Arc<RpcServer>, player: Objid) -> Self {
        Self {
            client_id,
            rpc_server,
            player,
            session_buffer: Default::default(),
        }
    }
}

#[async_trait]
impl Session for RpcSession {
    async fn commit(&self) -> Result<(), Error> {
        info!(player = ?self.player, client_id = ?self.client_id, "Committing session");
        let events: Vec<_> = {
            let mut session_buffer = self.session_buffer.lock().await;
            session_buffer.drain(..).collect()
        };

        self.rpc_server
            .publish_narrative_events(&events[..])
            .await
            .expect("Unable to publish narrative events");

        Ok(())
    }

    async fn rollback(&self) -> Result<(), Error> {
        let mut session_buffer = self.session_buffer.lock().await;
        session_buffer.clear();
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, Error> {
        // We ask the rpc server to create a new session, otherwise we'd need to have a copy of all
        // the info to create a Publish. The rpc server has that, though.
        let new_session = self
            .rpc_server
            .clone()
            .new_session(self.client_id, self.player)
            .await?;
        Ok(new_session)
    }

    async fn send_event(&self, player: Objid, event: NarrativeEvent) -> Result<(), Error> {
        self.session_buffer.lock().await.push((player, event));
        Ok(())
    }

    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), Error> {
        self.rpc_server
            .send_system_message(self.client_id, player, msg.to_string())
            .await?;
        Ok(())
    }

    async fn shutdown(&self, _msg: Option<String>) -> Result<(), Error> {
        todo!()
    }

    async fn connection_name(&self, player: Objid) -> Result<String, Error> {
        self.rpc_server.connection_name_for(player).await
    }

    async fn disconnect(&self, player: Objid) -> Result<(), Error> {
        self.rpc_server.disconnect(player).await
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
        self.rpc_server.connected_players().await
    }

    async fn connected_seconds(&self, player: Objid) -> Result<f64, Error> {
        self.rpc_server.connected_seconds_for(player).await
    }

    async fn idle_seconds(&self, player: Objid) -> Result<f64, Error> {
        self.rpc_server.idle_seconds_for(player).await
    }
}
