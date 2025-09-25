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

//! Message handler for RPC business logic, separated from transport concerns

use ahash::AHasher;
use eyre::{Context, Error};
use flume::Sender;
use lazy_static::lazy_static;
use papaya::HashMap as PapayaHashMap;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;

use super::hosts::Hosts;
use super::session::{RpcSession, SessionActions};
use super::transport::Transport;
use crate::connections::{ConnectionRegistry, NewConnectionParams};
use crate::event_log::EventLogOps;
use crate::tasks::task_monitor::TaskMonitor;
use moor_common::model::{Named, ObjectRef, PropFlag, ValSet, VerbFlag, preposition_to_string};
use moor_common::tasks::NarrativeEvent;
use moor_common::tasks::SchedulerError::CommandExecutionError;
use moor_common::tasks::{CommandError, ConnectionDetails};
use moor_common::util::parse_into_words;
use moor_db::db_counters;
use moor_kernel::SchedulerClient;
use moor_kernel::config::Config;
use moor_kernel::tasks::{TaskResult, sched_counters};
use moor_kernel::vm::builtins::bf_perf_counters;
use moor_var::SYSTEM_OBJECT;
use moor_var::{List, Variant};
use moor_var::{Obj, Var};
use moor_var::{Symbol, v_obj, v_str};
use rpc_common::ClientEvent;
use rpc_common::DaemonToClientReply::{LoginResult, NewConnection};
use rpc_common::{
    AuthToken, ClientToken, ClientsBroadcastEvent, ConnectType, DaemonToClientReply,
    DaemonToHostReply, EntityType, HistoricalNarrativeEvent, HistoryRecall, HistoryResponse,
    HostBroadcastEvent, HostClientToDaemonMessage, HostToDaemonMessage, HostToken, HostType,
    MOOR_AUTH_TOKEN_FOOTER, MOOR_HOST_TOKEN_FOOTER, MOOR_SESSION_TOKEN_FOOTER, PropInfo,
    RpcMessageError, VerbInfo, VerbProgramResponse,
};
use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use rusty_paseto::prelude::Key;
use serde_json::json;
use tracing::{debug, error, info, warn};

lazy_static! {
    static ref USER_CONNECTED_SYM: Symbol = Symbol::mk("user_connected");
    static ref USER_DISCONNECTED_SYM: Symbol = Symbol::mk("user_disconnected");
    static ref USER_RECONNECTED_SYM: Symbol = Symbol::mk("user_reconnected");
    static ref USER_CREATED_SYM: Symbol = Symbol::mk("user_created");
    static ref DO_LOGIN_COMMAND: Symbol = Symbol::mk("do_login_command");
    static ref SCHED_SYM: Symbol = Symbol::mk("sched");
    static ref DB_SYM: Symbol = Symbol::mk("db");
    static ref BF_SYM: Symbol = Symbol::mk("bf");
}

/// If we don't hear from a host in this time, we consider it dead and its listeners gone.
pub const HOST_TIMEOUT: Duration = Duration::from_secs(10);

/// Type alias for connection attributes result to reduce complexity
type ConnectionAttributesResult =
    Result<Vec<(Obj, std::collections::HashMap<Symbol, Var>)>, moor_common::tasks::SessionError>;

/// Trait for handling RPC message business logic
pub trait MessageHandler: Send + Sync {
    /// Process a host-to-daemon message
    fn handle_host_message(
        &self,
        host_token: HostToken,
        message: HostToDaemonMessage,
    ) -> Result<DaemonToHostReply, RpcMessageError>;

    /// Process a client-to-daemon message
    fn handle_client_message(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessage,
    ) -> Result<DaemonToClientReply, RpcMessageError>;

    /// Validate a host token
    fn validate_host_token(&self, token: &HostToken) -> Result<HostType, RpcMessageError>;

    /// Validate a client token
    fn validate_client_token(
        &self,
        token: ClientToken,
        client_id: Uuid,
    ) -> Result<(), RpcMessageError>;

    /// Broadcast a listen event to hosts
    fn broadcast_listen(
        &self,
        handler_object: Obj,
        host_type: HostType,
        port: u16,
        print_messages: bool,
    ) -> Result<(), moor_common::tasks::SessionError>;

    /// Broadcast an unlisten event to hosts
    fn broadcast_unlisten(
        &self,
        host_type: HostType,
        port: u16,
    ) -> Result<(), moor_common::tasks::SessionError>;

    /// Get current listeners
    fn get_listeners(&self) -> Vec<(Obj, HostType, u16)>;

    /// Get current connections
    #[allow(dead_code)]
    fn get_connections(&self) -> Vec<Obj>;

    fn ping_pong(&self) -> Result<(), moor_common::tasks::SessionError>;

    fn handle_session_event(&self, session_event: SessionActions) -> Result<(), Error>;

    /// Switch the player for the given connection object to the new player.
    fn switch_player(
        &self,
        connection_obj: Obj,
        new_player: Obj,
    ) -> Result<(), moor_common::tasks::SessionError>;
}

/// Implementation of message handler that contains the actual business logic
pub struct RpcMessageHandler {
    config: Arc<Config>,
    public_key: Key<32>,
    private_key: Key<64>,

    connections: Box<dyn ConnectionRegistry + Send + Sync>,
    task_monitor: Arc<TaskMonitor>,

    hosts: Arc<RwLock<Hosts>>,

    host_token_cache: PapayaHashMap<HostToken, (Instant, HostType), BuildHasherDefault<AHasher>>,
    auth_token_cache: PapayaHashMap<AuthToken, (Instant, Obj), BuildHasherDefault<AHasher>>,
    client_token_cache: PapayaHashMap<ClientToken, Instant, BuildHasherDefault<AHasher>>,

    mailbox_sender: Sender<SessionActions>,
    event_log: Arc<dyn EventLogOps>,
    transport: Arc<dyn Transport>,
}

impl RpcMessageHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<Config>,
        public_key: Key<32>,
        private_key: Key<64>,
        connections: Box<dyn ConnectionRegistry + Send + Sync>,
        hosts: Arc<RwLock<Hosts>>,
        mailbox_sender: Sender<SessionActions>,
        event_log: Arc<dyn EventLogOps>,
        task_monitor: Arc<TaskMonitor>,
        transport: Arc<dyn Transport>,
    ) -> Self {
        Self {
            config,
            public_key,
            private_key,
            connections,
            task_monitor,
            hosts,
            host_token_cache: Default::default(),
            auth_token_cache: Default::default(),
            client_token_cache: Default::default(),
            mailbox_sender,
            event_log,
            transport,
        }
    }
}

impl MessageHandler for RpcMessageHandler {
    fn handle_host_message(
        &self,
        host_token: HostToken,
        message: HostToDaemonMessage,
    ) -> Result<DaemonToHostReply, RpcMessageError> {
        Ok(self.process_host_request(host_token, message))
    }

    fn handle_client_message(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessage,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        self.process_request(scheduler_client, client_id, message)
    }

    fn validate_host_token(&self, token: &HostToken) -> Result<HostType, RpcMessageError> {
        // Check cache first.
        {
            let guard = self.host_token_cache.pin();
            if let Some((t, host_type)) = guard.get(token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(*host_type);
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
        let guard = self.host_token_cache.pin();
        guard.insert(token.clone(), (Instant::now(), host_type));

        Ok(host_type)
    }

    fn validate_client_token(
        &self,
        token: ClientToken,
        client_id: Uuid,
    ) -> Result<(), RpcMessageError> {
        {
            let guard = self.client_token_cache.pin();
            if let Some(t) = guard.get(&token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(());
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

        let guard = self.client_token_cache.pin();
        guard.insert(token.clone(), Instant::now());

        Ok(())
    }

    fn broadcast_listen(
        &self,
        handler_object: Obj,
        host_type: HostType,
        port: u16,
        print_messages: bool,
    ) -> Result<(), moor_common::tasks::SessionError> {
        let event = HostBroadcastEvent::Listen {
            handler_object,
            host_type,
            port,
            print_messages,
        };

        self.transport
            .broadcast_host_event(event)
            .map_err(|_| moor_common::tasks::SessionError::DeliveryError)
    }

    fn broadcast_unlisten(
        &self,
        host_type: HostType,
        port: u16,
    ) -> Result<(), moor_common::tasks::SessionError> {
        let event = HostBroadcastEvent::Unlisten { host_type, port };

        self.transport
            .broadcast_host_event(event)
            .map_err(|_| moor_common::tasks::SessionError::DeliveryError)
    }

    fn get_listeners(&self) -> Vec<(Obj, HostType, u16)> {
        let hosts = self.hosts.read().unwrap();
        hosts
            .listeners()
            .iter()
            .map(|(o, t, h)| (*o, *t, h.port()))
            .collect()
    }

    fn get_connections(&self) -> Vec<Obj> {
        self.connections.connections()
    }

    fn ping_pong(&self) -> Result<(), moor_common::tasks::SessionError> {
        // Send ping to all clients
        let client_event = ClientsBroadcastEvent::PingPong(SystemTime::now());
        self.transport
            .broadcast_client_event(client_event)
            .map_err(|_| moor_common::tasks::SessionError::DeliveryError)?;
        self.connections.ping_check();

        // Send ping to all hosts
        let host_event = HostBroadcastEvent::PingPong(SystemTime::now());
        self.transport
            .broadcast_host_event(host_event)
            .map_err(|_| moor_common::tasks::SessionError::DeliveryError)?;

        let mut hosts = self.hosts.write().unwrap();
        hosts.ping_check(HOST_TIMEOUT);
        Ok(())
    }

    fn handle_session_event(&self, session_event: SessionActions) -> Result<(), Error> {
        match session_event {
            SessionActions::PublishNarrativeEvents(events) => {
                if let Err(e) = self.publish_narrative_events(&events) {
                    error!(error = ?e, "Unable to publish narrative events");
                }
            }
            SessionActions::RequestClientInput {
                client_id,
                connection,
                request_id: input_request_id,
            } => {
                if let Err(e) = self.request_client_input(client_id, connection, input_request_id) {
                    error!(error = ?e, "Unable to request client input");
                }
            }
            SessionActions::SendSystemMessage {
                client_id,
                connection,
                system_message: message,
            } => {
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
                let connected_seconds_send_result = match self.connected_seconds_for(connection) {
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
            SessionActions::RequestConnections(client_id, player, reply) => {
                let connections_send_result = match self.connections_for(client_id, player) {
                    Ok(c) => reply.send(Ok(c)),
                    Err(e) => {
                        error!(error = ?e, "Unable to get connections");
                        reply.send(Err(e))
                    }
                };
                if let Err(e) = connections_send_result {
                    error!(error = ?e, "Unable to send connections");
                }
            }
            SessionActions::RequestConnectionDetails(client_id, player, reply) => {
                let connection_details_send_result =
                    match self.connection_details_for(client_id, player) {
                        Ok(details) => reply.send(Ok(details)),
                        Err(e) => {
                            error!(error = ?e, "Unable to get connection details");
                            reply.send(Err(e))
                        }
                    };
                if let Err(e) = connection_details_send_result {
                    error!(error = ?e, "Unable to send connection details");
                }
            }
            SessionActions::RequestClientAttributes(_client_id, obj, reply) => {
                use moor_var::{v_list, v_map, v_obj, v_sym};

                let handle_result = || -> Result<Var, moor_common::tasks::SessionError> {
                    if !obj.is_positive() {
                        // This is a connection object - return just its attributes
                        let attributes =
                            self.get_connection_attributes_for_single_connection(obj)?;
                        let attr_pairs: Vec<_> =
                            attributes.into_iter().map(|(k, v)| (v_sym(k), v)).collect();
                        Ok(v_map(&attr_pairs))
                    } else {
                        // This is a player object - return list of [connection_obj, attributes] pairs
                        let connection_attrs_list =
                            self.get_connection_attributes_for_player(obj)?;
                        let items: Vec<_> = connection_attrs_list
                            .into_iter()
                            .map(|(conn_obj, attributes)| {
                                let attr_pairs: Vec<_> =
                                    attributes.into_iter().map(|(k, v)| (v_sym(k), v)).collect();
                                v_list(&[v_obj(conn_obj), v_map(&attr_pairs)])
                            })
                            .collect();
                        Ok(v_list(&items))
                    }
                };

                let result = handle_result();
                if let Err(e) = reply.send(result) {
                    error!(error = ?e, "Unable to send client attributes");
                }
            }
            SessionActions::SetClientAttribute(client_id, connection_obj, key, value) => {
                if let Err(e) = self.set_client_attribute(client_id, connection_obj, key, value) {
                    error!(error = ?e, client_id = ?client_id, "Unable to set client attribute");
                }
            }
            SessionActions::PublishTaskCompletion(client_id, task_event) => {
                if let Err(e) = self.publish_task_completion(client_id, task_event) {
                    error!(error = ?e, client_id = ?client_id, "Unable to publish task completion");
                }
            }
        }
        Ok(())
    }

    fn switch_player(
        &self,
        connection_obj: Obj,
        new_player: Obj,
    ) -> Result<(), moor_common::tasks::SessionError> {
        // Get the client IDs for this connection object
        let client_ids = self
            .connections
            .client_ids_for(connection_obj)
            .map_err(|_| moor_common::tasks::SessionError::DeliveryError)?;

        // Generate a new auth token for the new player
        let new_auth_token = self.make_auth_token(&new_player);

        // Prepare events for all clients before making any changes
        let mut events_to_send = Vec::new();
        for client_id in &client_ids {
            let event = ClientEvent::PlayerSwitched {
                new_player,
                new_auth_token: new_auth_token.clone(),
            };
            events_to_send.push((*client_id, event));
        }

        // Switch the player for each client ID associated with this connection
        // Do this in one batch to minimize the window for inconsistency
        for client_id in &client_ids {
            self.connections
                .switch_player_for_client(*client_id, new_player)
                .map_err(|_| moor_common::tasks::SessionError::DeliveryError)?;
        }

        // Send events after all connection updates are complete
        // If any event fails to send, log it but don't fail the entire operation
        // since the connection registry has already been updated
        for (client_id, event) in events_to_send {
            if let Err(e) = self.transport.publish_client_event(client_id, event) {
                error!(
                    client_id = ?client_id,
                    new_player = ?new_player,
                    connection_obj = ?connection_obj,
                    error = ?e,
                    "Failed to send PlayerSwitched event to client after successful connection switch"
                );
            }
        }

        Ok(())
    }
}

impl RpcMessageHandler {
    fn publish_narrative_events(&self, events: &[(Obj, Box<NarrativeEvent>)]) -> Result<(), Error> {
        self.transport
            .publish_narrative_events(events, self.connections.as_ref())
    }

    // Helper methods that delegate to connections
    pub fn connection_name_for(
        &self,
        connection: Obj,
    ) -> Result<String, moor_common::tasks::SessionError> {
        self.connections.connection_name_for(connection)
    }

    pub fn connected_seconds_for(
        &self,
        connection: Obj,
    ) -> Result<f64, moor_common::tasks::SessionError> {
        self.connections.connected_seconds_for(connection)
    }

    pub fn disconnect(&self, player: Obj) -> Result<(), moor_common::tasks::SessionError> {
        warn!("Disconnecting player: {}", player);
        let all_client_ids = self.connections.client_ids_for(player)?;

        // Send disconnect event to all client connections for this player
        let event = ClientEvent::Disconnect();

        for client_id in &all_client_ids {
            // First send the disconnect event to the client
            if let Err(e) = self
                .transport
                .publish_client_event(*client_id, event.clone())
            {
                error!(error = ?e, client_id = ?client_id, "Unable to send disconnect event to client");
            }

            // Then remove the client connection
            if let Err(e) = self.connections.remove_client_connection(*client_id) {
                error!(error = ?e, "Unable to remove client connection for disconnect");
            }
        }

        Ok(())
    }

    pub fn request_client_input(
        &self,
        client_id: Uuid,
        player: Obj,
        input_request_id: Uuid,
    ) -> Result<(), Error> {
        // Validate first - check that the player matches the logged-in player for this client
        let Some(logged_in_player) = self.connections.player_object_for_client(client_id) else {
            return Err(eyre::eyre!("No connection for player"));
        };
        if logged_in_player != player {
            return Err(eyre::eyre!("Player mismatch"));
        }

        let event = ClientEvent::RequestInput(input_request_id);
        self.transport.publish_client_event(client_id, event)
    }

    pub fn send_system_message(
        &self,
        client_id: Uuid,
        player: Obj,
        message: String,
    ) -> Result<(), Error> {
        let event = ClientEvent::SystemMessage(player, message);
        self.transport.publish_client_event(client_id, event)
    }

    pub fn connected_players(&self) -> Result<Vec<Obj>, moor_common::tasks::SessionError> {
        let connections = self.connections.connections();
        Ok(connections
            .iter()
            .filter(|o| o > &&SYSTEM_OBJECT)
            .cloned()
            .collect())
    }

    pub fn idle_seconds_for(&self, player: Obj) -> Result<f64, moor_common::tasks::SessionError> {
        let last_activity = self.connections.last_activity_for(player)?;
        Ok(last_activity
            .elapsed()
            .map(|e| e.as_secs_f64())
            .unwrap_or(0.0))
    }

    pub fn connections_for(
        &self,
        client_id: Uuid,
        player: Option<Obj>,
    ) -> Result<Vec<Obj>, moor_common::tasks::SessionError> {
        if let Some(target_player) = player {
            // First find the client IDs for the player
            let client_ids = self.connections.client_ids_for(target_player)?;
            // Then return the connections for those client IDs
            let mut connections = vec![];
            for id in client_ids {
                if let Some(connection) = self.connections.connection_object_for_client(id) {
                    connections.push(connection);
                }
            }
            Ok(connections)
        } else {
            // We want all connections for the player associated with this client_id, but we'll
            // put the connection associated with the client_id first.  So let's get that first.
            let mut connections = vec![];
            if let Some(connection) = self.connections.connection_object_for_client(client_id) {
                connections.push(connection);
            }
            // Now get all connections for the player associated with this client_id
            let player_obj = self.connections.player_object_for_client(client_id);
            if let Some(player_obj) = player_obj {
                let client_ids = self.connections.client_ids_for(player_obj)?;
                for id in client_ids {
                    if let Some(connection) = self.connections.connection_object_for_client(id) {
                        // Avoid adding the same connection again
                        if !connections.contains(&connection) {
                            connections.push(connection);
                        }
                    }
                }
            }
            Ok(connections)
        }
    }

    pub fn connection_details_for(
        &self,
        client_id: Uuid,
        player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, moor_common::tasks::SessionError> {
        if let Some(target_player) = player {
            // Get connection details for the specified player
            let client_ids = self.connections.client_ids_for(target_player)?;
            let mut details = vec![];
            for id in client_ids {
                if let Some(connection_obj) = self.connections.connection_object_for_client(id) {
                    let hostname = self.connections.connection_name_for(connection_obj)?;
                    let idle_seconds = self.idle_seconds_for(connection_obj)?;
                    let acceptable_content_types = self
                        .connections
                        .acceptable_content_types_for(connection_obj)?;
                    details.push(ConnectionDetails {
                        connection_obj,
                        peer_addr: hostname,
                        idle_seconds,
                        acceptable_content_types,
                    });
                }
            }
            Ok(details)
        } else {
            // Get connection details for the player associated with this client_id
            let mut details = vec![];

            // Start with the connection for this specific client_id
            if let Some(connection_obj) = self.connections.connection_object_for_client(client_id) {
                let hostname = self.connections.connection_name_for(connection_obj)?;
                let idle_seconds = self.idle_seconds_for(connection_obj)?;
                let acceptable_content_types = self
                    .connections
                    .acceptable_content_types_for(connection_obj)?;
                details.push(ConnectionDetails {
                    connection_obj,
                    peer_addr: hostname,
                    idle_seconds,
                    acceptable_content_types,
                });
            }

            // Now get all other connections for the same player
            if let Some(player_obj) = self.connections.player_object_for_client(client_id) {
                let client_ids = self.connections.client_ids_for(player_obj)?;
                for id in client_ids {
                    if id != client_id {
                        // Skip the one we already added
                        if let Some(connection_obj) =
                            self.connections.connection_object_for_client(id)
                        {
                            // Check if we already have this connection to avoid duplicates
                            if !details.iter().any(|d| d.connection_obj == connection_obj) {
                                let hostname =
                                    self.connections.connection_name_for(connection_obj)?;
                                let idle_seconds = self.idle_seconds_for(connection_obj)?;
                                let acceptable_content_types = self
                                    .connections
                                    .acceptable_content_types_for(connection_obj)?;
                                details.push(ConnectionDetails {
                                    connection_obj,
                                    peer_addr: hostname,
                                    idle_seconds,
                                    acceptable_content_types,
                                });
                            }
                        }
                    }
                }
            }
            Ok(details)
        }
    }

    fn set_client_attribute(
        &self,
        client_id: Uuid,
        connection_obj: Obj,
        key: Symbol,
        value: Var,
    ) -> Result<(), Error> {
        // Store the attribute in the connection registry
        self.connections
            .set_client_attribute(client_id, key, Some(value.clone()))?;

        // Send SetConnectionOption event to the host
        self.transport.publish_client_event(
            client_id,
            ClientEvent::SetConnectionOption {
                connection_obj,
                option_name: key,
                value,
            },
        )
    }

    fn publish_task_completion(
        &self,
        client_id: Uuid,
        task_event: ClientEvent,
    ) -> Result<(), Error> {
        self.transport.publish_client_event(client_id, task_event)
    }

    pub fn client_auth(&self, token: ClientToken, client_id: Uuid) -> Result<Obj, RpcMessageError> {
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(RpcMessageError::NoConnection);
        };

        self.validate_client_token(token, client_id)?;
        Ok(connection)
    }

    fn process_host_request(
        &self,
        host_token: HostToken,
        message: HostToDaemonMessage,
    ) -> DaemonToHostReply {
        match message {
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
                DaemonToHostReply::Ack
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
                DaemonToHostReply::Ack
            }
            HostToDaemonMessage::RequestPerformanceCounters => {
                let mut all_counters = vec![];
                let mut sch = vec![];
                for c in sched_counters().all_counters() {
                    sch.push((
                        c.operation,
                        c.invocations().sum(),
                        c.cumulative_duration_nanos().sum(),
                    ));
                }
                all_counters.push((*SCHED_SYM, sch));

                let mut db = vec![];
                for c in db_counters().all_counters() {
                    db.push((
                        c.operation,
                        c.invocations().sum(),
                        c.cumulative_duration_nanos().sum(),
                    ));
                }
                all_counters.push((*DB_SYM, db));

                let mut bf = vec![];
                for c in bf_perf_counters().all_counters() {
                    bf.push((
                        c.operation,
                        c.invocations().sum(),
                        c.cumulative_duration_nanos().sum(),
                    ));
                }
                all_counters.push((*BF_SYM, bf));

                DaemonToHostReply::PerfCounters(SystemTime::now(), all_counters)
            }
            HostToDaemonMessage::DetachHost => {
                let mut hosts = self.hosts.write().unwrap();
                hosts.unregister_host(&host_token);
                DaemonToHostReply::Ack
            }
        }
    }

    fn process_request(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessage,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        match message {
            HostClientToDaemonMessage::ConnectionEstablish {
                peer_addr: hostname,
                local_port,
                remote_port,
                acceptable_content_types,
                connection_attributes,
            } => {
                let oid = self.connections.new_connection(NewConnectionParams {
                    client_id,
                    hostname,
                    local_port,
                    remote_port,
                    player: None,
                    acceptable_content_types,
                    connection_attributes,
                })?;
                let token = self.make_client_token(client_id);
                Ok(NewConnection(token, oid))
            }
            HostClientToDaemonMessage::Attach {
                auth_token,
                connect_type,
                handler_object,
                peer_addr: hostname,
                local_port,
                remote_port,
                acceptable_content_types,
            } => {
                // Validate the auth token, and get the player.
                let player = self.validate_auth_token(auth_token, None)?;

                self.connections.new_connection(NewConnectionParams {
                    client_id,
                    hostname,
                    local_port,
                    remote_port,
                    player: Some(player),
                    acceptable_content_types,
                    connection_attributes: None,
                })?;
                let client_token = self.make_client_token(client_id);

                if let Some(connect_type) = connect_type {
                    let connection = self
                        .connections
                        .connection_object_for_client(client_id)
                        .ok_or(RpcMessageError::InternalError(
                            "Connection not found".to_string(),
                        ))?;

                    if let Err(e) = self.submit_connected_task(
                        &handler_object,
                        scheduler_client,
                        client_id,
                        &player,
                        &connection,
                        connect_type,
                    ) {
                        error!(error = ?e, "Error submitting user_connected task");

                        // Note we still continue to return a successful login result here, hoping for the best
                        // but we do log the error.
                    }
                }
                Ok(DaemonToClientReply::AttachResult(Some((
                    client_token,
                    player,
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
            HostClientToDaemonMessage::LoginCommand {
                client_token: token,
                handler_object,
                connect_args: args,
                do_attach: attach,
            } => {
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
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.perform_command(
                    scheduler_client,
                    client_id,
                    &handler_object,
                    &player,
                    command,
                )
            }
            HostClientToDaemonMessage::RequestedInput(token, auth_token, request_id, input) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.respond_input(scheduler_client, client_id, &player, request_id, input)
            }
            HostClientToDaemonMessage::OutOfBand(token, auth_token, handler_object, command) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.perform_out_of_band(
                    scheduler_client,
                    &handler_object,
                    client_id,
                    &player,
                    command,
                )
            }

            HostClientToDaemonMessage::Eval(token, auth_token, evalstr) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.eval(scheduler_client, client_id, &player, evalstr)
            }

            HostClientToDaemonMessage::InvokeVerb(token, auth_token, object, verb, args) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.invoke_verb(scheduler_client, client_id, &player, &object, verb, args)
            }

            HostClientToDaemonMessage::Retrieve(token, auth_token, who, retr_type, what) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                match retr_type {
                    EntityType::Property => {
                        let (propdef, propperms, value) = scheduler_client
                            .request_property(&player, &player, &who, what)
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
                                name: propdef.name(),
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
                            .request_verb(&player, &player, &who, what)
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
                                names: verbdef.names().to_vec(),
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
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                let resolved = scheduler_client
                    .resolve_object(player, objref)
                    .map_err(|e| {
                        error!(error = ?e, "Error resolving object");
                        RpcMessageError::EntityRetrievalError("error resolving object".to_string())
                    })?;

                Ok(DaemonToClientReply::ResolveResult(resolved))
            }
            HostClientToDaemonMessage::Properties(token, auth_token, obj, inherited) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                let props = scheduler_client
                    .request_properties(&player, &player, &obj, inherited)
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
                        name: propdef.name(),
                        owner: propperms.owner(),
                        r: propperms.flags().contains(PropFlag::Read),
                        w: propperms.flags().contains(PropFlag::Write),
                        chown: propperms.flags().contains(PropFlag::Chown),
                    })
                    .collect();

                Ok(DaemonToClientReply::Properties(props))
            }
            HostClientToDaemonMessage::Verbs(token, auth_token, obj, inherited) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                let verbs = scheduler_client
                    .request_verbs(&player, &player, &obj, inherited)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting verbs");
                        RpcMessageError::EntityRetrievalError("error requesting verbs".to_string())
                    })?;

                let verbs = verbs
                    .iter()
                    .map(|v| VerbInfo {
                        location: v.location(),
                        owner: v.owner(),
                        names: v.names().to_vec(),
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
            HostClientToDaemonMessage::RequestHistory(_token, auth_token, history_recall) => {
                // Validate the auth token to get the player
                let player = self.validate_auth_token(auth_token, None)?;

                // Build history response based on history recall option
                let history_response = self.build_history_response(player, history_recall);

                Ok(DaemonToClientReply::HistoryResponse(history_response))
            }
            HostClientToDaemonMessage::RequestCurrentPresentations(_token, auth_token) => {
                // Validate the auth token to get the player
                let player = self.validate_auth_token(auth_token, None)?;

                // Get current presentations from event log
                let presentations = self.event_log.current_presentations(player);
                let presentation_list: Vec<_> = presentations.into_values().collect();

                Ok(DaemonToClientReply::CurrentPresentations(presentation_list))
            }
            HostClientToDaemonMessage::DismissPresentation(_token, auth_token, presentation_id) => {
                // Validate the auth token to get the player
                let player = self.validate_auth_token(auth_token, None)?;

                // Remove the presentation from the event log state
                self.event_log.dismiss_presentation(player, presentation_id);

                Ok(DaemonToClientReply::PresentationDismissed)
            }
            HostClientToDaemonMessage::SetClientAttribute(token, auth_token, key, value) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                // Store the attribute in the connection registry
                self.connections
                    .set_client_attribute(client_id, key, value)?;

                Ok(DaemonToClientReply::ClientAttributeSet)
            }
            HostClientToDaemonMessage::Detach(token, disconnected) => {
                let connection = self.client_auth(token, client_id)?;

                // Submit disconnected only if there's a logged-in player, and if the intent is
                // to actually disconnect.
                if disconnected
                    && let Some(player) = self.connections.player_object_for_client(client_id)
                    && let Err(e) = self.submit_disconnected_task(
                        &SYSTEM_OBJECT,
                        scheduler_client,
                        client_id,
                        &player,
                        &connection,
                    )
                {
                    error!(error = ?e, "Error submitting user_disconnected task");
                }
                // Detach this client id from the connection DB and any connection object
                // associations it may have.
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcMessageError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };

                Ok(DaemonToClientReply::Disconnected)
            }
            HostClientToDaemonMessage::Program(token, auth_token, object, verb, code) => {
                let _connection = self.client_auth(token, client_id)?;
                let player = self.validate_auth_token(auth_token, None)?;

                // Verify the player matches the logged-in player for this connection
                let Some(logged_in_player) = self.connections.player_object_for_client(client_id)
                else {
                    return Err(RpcMessageError::PermissionDenied);
                };
                if player != logged_in_player {
                    return Err(RpcMessageError::PermissionDenied);
                }

                self.program_verb(scheduler_client, client_id, &player, &object, verb, code)
            }
        }
    }

    fn validate_auth_token(
        &self,
        token: AuthToken,
        objid: Option<&Obj>,
    ) -> Result<Obj, RpcMessageError> {
        {
            let guard = self.auth_token_cache.pin();
            if let Some((t, o)) = guard.get(&token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(*o);
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
        let Some(token_player) = token_player.as_str() else {
            debug!("Token player is not valid (expected string, found: {token_player:?})");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Ok(token_player) = Obj::try_from(token_player) else {
            debug!("Token player is not valid");
            return Err(RpcMessageError::PermissionDenied);
        };
        if !token_player.is_positive() {
            debug!("Token player is not a valid objid");
            return Err(RpcMessageError::PermissionDenied);
        }
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

        let guard = self.auth_token_cache.pin();
        guard.insert(token.clone(), (Instant::now(), token_player));
        Ok(token_player)
    }

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

    fn make_auth_token(&self, oid: &Obj) -> AuthToken {
        let privkey = PasetoAsymmetricPrivateKey::from(self.private_key.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_AUTH_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "player": oid.to_string(),
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");
        AuthToken(token)
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
            "Performing {:?} login for client: {}",
            connect_type, client_id
        );
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));
        let mut task_handle = match scheduler_client.submit_verb_task(
            connection,
            &ObjectRef::Id(*handler_object),
            *DO_LOGIN_COMMAND,
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
                        Variant::Obj(o) => break *o,
                        _ => {
                            return Ok(LoginResult(None));
                        }
                    }
                }
                Ok((_, Err(e))) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::LoginTaskFailed(e.to_string()));
                }
                Err(e) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        };

        let Ok(_) = self
            .connections
            .associate_player_object(*connection, player)
        else {
            return Err(RpcMessageError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        if attach
            && let Err(e) = self.submit_connected_task(
                handler_object,
                scheduler_client,
                client_id,
                &player,
                connection,
                connect_type,
            )
        {
            error!(error = ?e, "Error submitting user_connected task");

            // Note we still continue to return a successful login result here, hoping for the best
            // but we do log the error.
        }

        let auth_token = self.make_auth_token(&player);

        Ok(LoginResult(Some((auth_token, connect_type, player))))
    }

    fn submit_connected_task(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        connection: &Obj,
        initiation_type: ConnectType,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let connected_verb = match initiation_type {
            ConnectType::Connected => *USER_CONNECTED_SYM,
            ConnectType::Reconnected => *USER_RECONNECTED_SYM,
            ConnectType::Created => *USER_CREATED_SYM,
        };
        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(*handler_object),
                connected_verb,
                List::mk_list(&[v_obj(*player)]),
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
        connection: &Obj,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(*handler_object),
                *USER_DISCONNECTED_SYM,
                List::mk_list(&[v_obj(*player)]),
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
        player: &Obj,
        command: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Get the connection object for activity tracking and session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        debug!(command, ?client_id, ?player, "Invoking submit_command_task");
        let parse_command_task_handle = match scheduler_client.submit_command_task(
            handler_object,
            player,
            command.as_str(),
            session,
        ) {
            Ok(t) => t,
            Err(e) => return Err(RpcMessageError::TaskError(e)),
        };

        let task_id = parse_command_task_handle.task_id();
        if let Err(e) = self
            .task_monitor
            .add_task(task_id, client_id, parse_command_task_handle)
        {
            error!(error = ?e, "Error adding task to monitor");
        }
        Ok(DaemonToClientReply::TaskSubmitted(task_id))
    }

    fn respond_input(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        input_request_id: Uuid,
        input: Var,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Get the connection object for activity tracking
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        // Pass this back over to the scheduler to handle using the player object.
        if let Err(e) = scheduler_client.submit_requested_input(player, input_request_id, input) {
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcMessageError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(DaemonToClientReply::InputThanks)
    }

    fn perform_out_of_band(
        &self,
        scheduler_client: SchedulerClient,
        handler_object: &Obj,
        client_id: Uuid,
        player: &Obj,
        command: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let command_components = parse_into_words(command.as_str());
        let task_handle = match scheduler_client.submit_out_of_band_task(
            handler_object,
            player,
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
        player: &Obj,
        expression: String,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let mut task_handle = match scheduler_client.submit_eval_task(
            player,
            player,
            expression,
            session,
            self.config.features.clone(),
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
        player: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let task_handle = match scheduler_client.submit_verb_task(
            player,
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
        if let Err(e) = self.task_monitor.add_task(task_id, client_id, task_handle) {
            error!(error = ?e, "Error adding task to monitor");
            return Err(RpcMessageError::InternalError(e.to_string()));
        }
        Ok(DaemonToClientReply::TaskSubmitted(task_id))
    }

    fn program_verb(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        player: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        code: Vec<String>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        match scheduler_client.submit_verb_program(player, player, object, verb, code) {
            Ok((obj, verb)) => Ok(DaemonToClientReply::ProgramResponse(
                VerbProgramResponse::Success(obj, verb.to_string()),
            )),
            Err(moor_common::tasks::SchedulerError::VerbProgramFailed(f)) => Ok(
                DaemonToClientReply::ProgramResponse(VerbProgramResponse::Failure(f)),
            ),
            Err(e) => Err(RpcMessageError::TaskError(e)),
        }
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

    fn build_history_response(
        &self,
        player: Obj,
        history_recall: HistoryRecall,
    ) -> HistoryResponse {
        let (events, total_events_available, has_more_before) = match history_recall {
            HistoryRecall::SinceEvent(since_id, limit) => {
                let all_events = self
                    .event_log
                    .events_for_player_since(player, Some(since_id));
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    all_events.into_iter().take(limit).collect()
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            HistoryRecall::UntilEvent(until_id, limit) => {
                let all_events = self
                    .event_log
                    .events_for_player_until(player, Some(until_id));
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    // For UntilEvent, we want the MOST RECENT events before the boundary, not the oldest
                    // So take from the end of the chronologically sorted list
                    let len = all_events.len();
                    if len > limit {
                        all_events.into_iter().skip(len - limit).collect()
                    } else {
                        all_events
                    }
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            HistoryRecall::SinceSeconds(seconds_ago, limit) => {
                let all_events = self
                    .event_log
                    .events_for_player_since_seconds(player, seconds_ago);
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    // For SinceSeconds, we want the MOST RECENT events, not the oldest
                    // So take from the end of the chronologically sorted list
                    let len = all_events.len();
                    if len > limit {
                        all_events.into_iter().skip(len - limit).collect()
                    } else {
                        all_events
                    }
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            HistoryRecall::None => (Vec::new(), 0, false),
        };

        let historical_events = events
            .into_iter()
            .map(|logged_event| HistoricalNarrativeEvent {
                event: (*logged_event.event).clone(),
                is_historical: true,
                player: logged_event.player,
            })
            .collect::<Vec<_>>();

        // Calculate metadata
        let (earliest_time, latest_time) = if historical_events.is_empty() {
            (SystemTime::now(), SystemTime::now())
        } else {
            (
                historical_events.first().unwrap().event.timestamp(),
                historical_events.last().unwrap().event.timestamp(),
            )
        };

        debug!(
            "Built history response with {} events for player {} (total available: {}, has more: {}, time range: {:?} to {:?})",
            historical_events.len(),
            player,
            total_events_available,
            has_more_before,
            earliest_time,
            latest_time
        );

        // Find actual earliest and latest event IDs from the returned events
        let (earliest_event_id, latest_event_id) = if historical_events.is_empty() {
            (None, None)
        } else {
            let mut event_ids: Vec<_> = historical_events
                .iter()
                .map(|e| e.event.event_id())
                .collect();
            event_ids.sort(); // UUIDs sort chronologically
            (Some(event_ids[0]), Some(event_ids[event_ids.len() - 1]))
        };

        HistoryResponse {
            total_events: total_events_available,
            earliest_event_id,
            latest_event_id,
            time_range: (earliest_time, latest_time),
            has_more_before,
            events: historical_events,
        }
    }

    /// Get attributes for a single connection object
    fn get_connection_attributes_for_single_connection(
        &self,
        connection_obj: Obj,
    ) -> Result<std::collections::HashMap<Symbol, Var>, moor_common::tasks::SessionError> {
        // Get attributes directly from the connection registry
        // The connection registry now handles both player and connection objects
        self.connections.get_client_attributes(connection_obj)
    }

    /// Get attributes for all connections of a player
    fn get_connection_attributes_for_player(&self, player: Obj) -> ConnectionAttributesResult {
        // Get all client IDs for this player
        let client_ids = self.connections.client_ids_for(player)?;

        let mut result = Vec::new();
        for client_id in client_ids {
            // Get the connection object for this client
            let Some(connection_obj) = self.connections.connection_object_for_client(client_id)
            else {
                continue;
            };

            // Get attributes for this specific connection
            let attributes = self
                .get_connection_attributes_for_single_connection(connection_obj)
                .unwrap_or_else(|_| std::collections::HashMap::new());
            result.push((connection_obj, attributes));
        }

        Ok(result)
    }
}
