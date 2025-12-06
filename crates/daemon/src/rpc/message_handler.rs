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
use eyre::Error;
use flume::Sender;
use lazy_static::lazy_static;
use moor_rpc::{
    ClientEvent, ClientEventUnion, DaemonToClientReply, DaemonToClientReplyUnion,
    DaemonToHostReply, DaemonToHostReplyUnion, HistoryResponseReply, HostClientToDaemonMessageRef,
    VerbProgramResponseReply, VerbProgramResponseUnion,
};
use moor_schema::{
    common, rpc as moor_rpc,
    rpc::{HostClientToDaemonMessageUnionRef, ListenerRef},
};
use papaya::HashMap as PapayaHashMap;
use std::{
    hash::BuildHasherDefault,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};
use uuid::Uuid;

use super::{
    hosts::Hosts, output_capture_session::OutputCaptureSession, session::SessionActions,
    transport::Transport,
};
use crate::{
    connections::{ConnectionRegistry, NewConnectionParams},
    event_log::EventLogOps,
    tasks::task_monitor::TaskMonitor,
};
use moor_common::{
    model::{Named, ObjectRef, PropFlag, ValSet, VerbFlag, preposition_to_string},
    tasks::{
        CommandError, ConnectionDetails, NarrativeEvent, SchedulerError,
        SchedulerError::CommandExecutionError, SessionError,
    },
};
use moor_db::db_counters;
use moor_kernel::{
    SchedulerClient,
    config::Config,
    tasks::{TaskNotification, sched_counters},
    vm::builtins::bf_perf_counters,
};

use moor_schema::convert::{
    narrative_event_to_flatbuffer_struct, obj_from_ref, presentation_to_flatbuffer_struct,
    var_from_ref, var_to_flatbuffer,
};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Var};
use rpc_common::{
    AuthToken, ClientToken, HostType, RpcErr, RpcMessageError, auth_token_from_ref,
    client_token_from_ref, extract_field_rpc, extract_host_type, extract_obj_rpc,
    extract_object_ref_rpc, extract_string_list_rpc, extract_string_rpc, extract_symbol_rpc,
    extract_uuid_rpc, extract_var_rpc, mk_client_attribute_set_reply, mk_daemon_to_host_ack,
    mk_disconnected_reply, mk_new_connection_reply, mk_presentation_dismissed_reply,
    mk_thanks_pong_reply, obj_fb, scheduler_error_to_flatbuffer_struct, symbol_fb, uuid_fb,
    var_to_flatbuffer_rpc, verb_program_error_to_flatbuffer_struct,
};
use rusty_paseto::prelude::Key;
use tracing::{debug, error, info, warn};

lazy_static! {
    pub(crate) static ref USER_CONNECTED_SYM: Symbol = Symbol::mk("user_connected");
    pub(crate) static ref USER_DISCONNECTED_SYM: Symbol = Symbol::mk("user_disconnected");
    pub(crate) static ref USER_RECONNECTED_SYM: Symbol = Symbol::mk("user_reconnected");
    pub(crate) static ref USER_CREATED_SYM: Symbol = Symbol::mk("user_created");
    pub(crate) static ref DO_LOGIN_COMMAND: Symbol = Symbol::mk("do_login_command");
    pub(crate) static ref SCHED_SYM: Symbol = Symbol::mk("sched");
    pub(crate) static ref DB_SYM: Symbol = Symbol::mk("db");
    pub(crate) static ref BF_SYM: Symbol = Symbol::mk("bf");
}

/// If we don't hear from a host in this time, we consider it dead and its listeners gone.
pub const HOST_TIMEOUT: Duration = Duration::from_secs(10);

/// Type alias for connection attributes result to reduce complexity
type ConnectionAttributesResult =
    Result<Vec<(Obj, std::collections::HashMap<Symbol, Var>)>, SessionError>;

/// Trait for handling RPC message business logic
pub trait MessageHandler: Send + Sync {
    /// Process a host-to-daemon message (FlatBuffer refs)
    fn handle_host_message(
        &self,
        host_id: Uuid,
        message: moor_rpc::HostToDaemonMessageRef<'_>,
    ) -> Result<DaemonToHostReply, RpcMessageError>;

    /// Process a client-to-daemon message (FlatBuffer refs)
    fn handle_client_message(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessageRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError>;

    /// Broadcast a listen event to hosts
    fn broadcast_listen(
        &self,
        handler_object: Obj,
        host_type: HostType,
        port: u16,
        print_messages: bool,
    ) -> Result<(), SessionError>;

    /// Broadcast an unlisten event to hosts
    fn broadcast_unlisten(&self, host_type: HostType, port: u16) -> Result<(), SessionError>;

    /// Get current listeners
    fn get_listeners(&self) -> Vec<(Obj, HostType, u16)>;

    /// Get current connections
    #[allow(dead_code)]
    fn get_connections(&self) -> Vec<Obj>;

    fn ping_pong(&self) -> Result<(), SessionError>;

    fn handle_session_event(&self, session_event: SessionActions) -> Result<(), Error>;

    /// Switch the player for the given connection object to the new player.
    fn switch_player(&self, connection_obj: Obj, new_player: Obj) -> Result<(), SessionError>;
}

/// Implementation of message handler that contains the actual business logic
pub struct RpcMessageHandler {
    pub(crate) config: Arc<Config>,
    pub(crate) public_key: Key<32>,
    pub(crate) private_key: Key<64>,

    pub(crate) connections: Box<dyn ConnectionRegistry + Send + Sync>,
    pub(crate) task_monitor: Arc<TaskMonitor>,

    pub(crate) hosts: Arc<RwLock<Hosts>>,

    pub(crate) auth_token_cache:
        PapayaHashMap<AuthToken, (Instant, Obj), BuildHasherDefault<AHasher>>,
    pub(crate) client_token_cache: PapayaHashMap<ClientToken, Instant, BuildHasherDefault<AHasher>>,

    pub(crate) mailbox_sender: Sender<SessionActions>,
    pub(crate) event_log: Arc<dyn EventLogOps>,
    pub(crate) transport: Arc<dyn Transport>,
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
        host_id: Uuid,
        message: moor_rpc::HostToDaemonMessageRef<'_>,
    ) -> Result<DaemonToHostReply, RpcMessageError> {
        let response = match message.message().map_err(|e| {
            RpcMessageError::InvalidRequest(format!("missing host message union: {e:?}"))
        })? {
            moor_rpc::HostToDaemonMessageUnionRef::RegisterHost(reg) => {
                let host_type = extract_host_type(&reg, "host_type", |r| r.host_type())?;

                // Convert listeners from FlatBuffer to Vec<(Obj, SocketAddr)>
                let listeners: Vec<(Obj, SocketAddr)> = convert_listeners(reg.listeners().ok());

                info!(
                    "Host {} registered with {} listeners",
                    host_id,
                    listeners.len()
                );
                let mut hosts = self.hosts.write().unwrap();
                hosts.receive_ping(host_id, host_type, listeners);

                mk_daemon_to_host_ack()
            }
            moor_rpc::HostToDaemonMessageUnionRef::HostPong(pong) => {
                let host_type = extract_host_type(&pong, "host_type", |p| p.host_type())?;

                let listeners: Vec<(Obj, SocketAddr)> = convert_listeners(pong.listeners().ok());

                let num_listeners = listeners.len();
                let mut hosts = self.hosts.write().unwrap();
                if hosts.receive_ping(host_id, host_type, listeners) {
                    info!(
                        "Host {} registered with {} listeners",
                        host_id, num_listeners
                    );
                }

                mk_daemon_to_host_ack()
            }
            moor_rpc::HostToDaemonMessageUnionRef::RequestPerformanceCounters(_) => {
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

                let timestamp = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;

                let counters_fb: Vec<moor_rpc::CounterCategory> = all_counters
                    .iter()
                    .map(|(category_sym, counters_list)| {
                        let counters: Vec<moor_rpc::Counter> = counters_list
                            .iter()
                            .map(|(name_sym, count, cumulative_ns)| moor_rpc::Counter {
                                name: Box::new(moor_rpc::Symbol {
                                    value: name_sym.as_string(),
                                }),
                                count: *count as i64,
                                total_cumulative_ns: *cumulative_ns as i64,
                            })
                            .collect();

                        moor_rpc::CounterCategory {
                            category: Box::new(moor_rpc::Symbol {
                                value: category_sym.as_string(),
                            }),
                            counters,
                        }
                    })
                    .collect();

                DaemonToHostReply {
                    reply: DaemonToHostReplyUnion::DaemonToHostPerfCounters(Box::new(
                        moor_rpc::DaemonToHostPerfCounters {
                            timestamp,
                            counters: counters_fb,
                        },
                    )),
                }
            }
            moor_rpc::HostToDaemonMessageUnionRef::GetServerFeatures(_) => {
                let features = self.config.features.as_ref();
                DaemonToHostReply {
                    reply: DaemonToHostReplyUnion::ServerFeatures(Box::new(
                        moor_rpc::ServerFeatures {
                            persistent_tasks: features.persistent_tasks,
                            rich_notify: features.rich_notify,
                            lexical_scopes: features.lexical_scopes,
                            type_dispatch: features.type_dispatch,
                            flyweight_type: features.flyweight_type,
                            list_comprehensions: features.list_comprehensions,
                            bool_type: features.bool_type,
                            use_boolean_returns: features.use_boolean_returns,
                            symbol_type: features.symbol_type,
                            use_symbols_in_builtins: features.use_symbols_in_builtins,
                            custom_errors: features.custom_errors,
                            use_uuobjids: features.use_uuobjids,
                            enable_eventlog: features.enable_eventlog,
                            anonymous_objects: features.anonymous_objects,
                        },
                    )),
                }
            }
            moor_rpc::HostToDaemonMessageUnionRef::DetachHost(_) => {
                let mut hosts = self.hosts.write().unwrap();
                hosts.unregister_host(&host_id);
                mk_daemon_to_host_ack()
            }
        };

        Ok(response)
    }

    fn handle_client_message(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessageRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        match message
            .message()
            .map_err(|_| RpcMessageError::InvalidRequest("Missing message union".to_string()))?
        {
            HostClientToDaemonMessageUnionRef::ConnectionEstablish(conn_est) => {
                self.handle_connection_establish(client_id, conn_est)
            }
            HostClientToDaemonMessageUnionRef::Reattach(reattach) => {
                self.handle_reattach(scheduler_client, client_id, reattach)
            }
            HostClientToDaemonMessageUnionRef::ClientPong(pong) => {
                self.handle_client_pong(client_id, pong)
            }
            HostClientToDaemonMessageUnionRef::RequestSysProp(req) => {
                self.handle_request_sys_prop(scheduler_client, req)
            }
            HostClientToDaemonMessageUnionRef::LoginCommand(login) => {
                self.handle_login_command(scheduler_client, client_id, login)
            }
            HostClientToDaemonMessageUnionRef::Attach(attach_msg) => {
                self.handle_attach(scheduler_client, client_id, attach_msg)
            }
            HostClientToDaemonMessageUnionRef::Command(cmd) => {
                self.handle_command(scheduler_client, client_id, cmd)
            }
            HostClientToDaemonMessageUnionRef::Detach(detach) => {
                self.handle_detach(scheduler_client, client_id, detach)
            }
            HostClientToDaemonMessageUnionRef::RequestedInput(input) => {
                self.handle_requested_input(scheduler_client, client_id, input)
            }
            HostClientToDaemonMessageUnionRef::OutOfBand(oob) => {
                self.handle_out_of_band(scheduler_client, client_id, oob)
            }
            HostClientToDaemonMessageUnionRef::Eval(eval) => {
                self.handle_eval(scheduler_client, client_id, eval)
            }
            HostClientToDaemonMessageUnionRef::InvokeVerb(invoke) => {
                self.handle_invoke_verb(scheduler_client, client_id, invoke)
            }
            HostClientToDaemonMessageUnionRef::Retrieve(retr) => {
                self.handle_retrieve(scheduler_client, client_id, retr)
            }
            HostClientToDaemonMessageUnionRef::Resolve(resolve) => {
                self.handle_resolve(scheduler_client, client_id, resolve)
            }
            HostClientToDaemonMessageUnionRef::Properties(props) => {
                self.handle_properties(scheduler_client, client_id, props)
            }
            HostClientToDaemonMessageUnionRef::Verbs(verbs) => {
                self.handle_verbs(scheduler_client, client_id, verbs)
            }
            HostClientToDaemonMessageUnionRef::RequestHistory(hist) => {
                self.handle_request_history(hist)
            }
            HostClientToDaemonMessageUnionRef::RequestCurrentPresentations(req) => {
                self.handle_request_current_presentations(req)
            }
            HostClientToDaemonMessageUnionRef::DismissPresentation(dismiss) => {
                self.handle_dismiss_presentation(dismiss)
            }
            HostClientToDaemonMessageUnionRef::SetClientAttribute(set_attr) => {
                self.handle_set_client_attribute(client_id, set_attr)
            }
            HostClientToDaemonMessageUnionRef::Program(prog) => {
                self.handle_program(scheduler_client, client_id, prog)
            }
            HostClientToDaemonMessageUnionRef::GetEventLogPublicKey(req) => {
                self.handle_get_event_log_pubkey(client_id, req)
            }
            HostClientToDaemonMessageUnionRef::SetEventLogPublicKey(req) => {
                self.handle_set_event_log_pubkey(client_id, req)
            }
            HostClientToDaemonMessageUnionRef::DeleteEventLogHistory(req) => {
                self.handle_delete_event_log_history(client_id, req)
            }
            HostClientToDaemonMessageUnionRef::ListObjects(req) => {
                self.handle_list_objects(scheduler_client, client_id, req)
            }
            HostClientToDaemonMessageUnionRef::UpdateProperty(req) => {
                self.handle_update_property(scheduler_client, client_id, req)
            }
            HostClientToDaemonMessageUnionRef::InvokeSystemHandler(invoke) => {
                self.handle_invoke_system_handler(scheduler_client, client_id, invoke)
            }
            HostClientToDaemonMessageUnionRef::CallSystemVerb(call) => {
                self.handle_call_system_verb(scheduler_client, client_id, call)
            }
        }
    }

    fn broadcast_listen(
        &self,
        handler_object: Obj,
        host_type: HostType,
        port: u16,
        print_messages: bool,
    ) -> Result<(), SessionError> {
        let host_type_enum = match host_type {
            HostType::TCP => moor_rpc::HostType::Tcp,
            HostType::WebSocket => moor_rpc::HostType::WebSocket,
        };
        let event = moor_rpc::HostBroadcastEvent {
            event: moor_rpc::HostBroadcastEventUnion::HostBroadcastListen(Box::new(
                moor_rpc::HostBroadcastListen {
                    handler_object: obj_fb(&handler_object),
                    host_type: host_type_enum,
                    port,
                    print_messages,
                },
            )),
        };

        self.transport
            .broadcast_host_event(event)
            .map_err(|_| SessionError::DeliveryError)
    }

    fn broadcast_unlisten(&self, host_type: HostType, port: u16) -> Result<(), SessionError> {
        let host_type_enum = match host_type {
            HostType::TCP => moor_rpc::HostType::Tcp,
            HostType::WebSocket => moor_rpc::HostType::WebSocket,
        };
        let event = moor_rpc::HostBroadcastEvent {
            event: moor_rpc::HostBroadcastEventUnion::HostBroadcastUnlisten(Box::new(
                moor_rpc::HostBroadcastUnlisten {
                    host_type: host_type_enum,
                    port,
                },
            )),
        };

        self.transport
            .broadcast_host_event(event)
            .map_err(|_| SessionError::DeliveryError)
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

    fn ping_pong(&self) -> Result<(), SessionError> {
        // Send ping to all clients
        let timestamp_nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let client_event = moor_rpc::ClientsBroadcastEvent {
            event: moor_rpc::ClientsBroadcastEventUnion::ClientsBroadcastPingPong(Box::new(
                moor_rpc::ClientsBroadcastPingPong {
                    timestamp: timestamp_nanos,
                },
            )),
        };
        self.transport
            .broadcast_client_event(client_event)
            .map_err(|_| SessionError::DeliveryError)?;
        self.connections.ping_check();

        // Send ping to all hosts
        let timestamp_nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let host_event = moor_rpc::HostBroadcastEvent {
            event: moor_rpc::HostBroadcastEventUnion::HostBroadcastPingPong(Box::new(
                moor_rpc::HostBroadcastPingPong {
                    timestamp: timestamp_nanos,
                },
            )),
        };
        self.transport
            .broadcast_host_event(host_event)
            .map_err(|_| SessionError::DeliveryError)?;

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
                metadata,
            } => {
                if let Err(e) =
                    self.request_client_input(client_id, connection, input_request_id, metadata)
                {
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

                let handle_result = || -> Result<Var, SessionError> {
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

    fn switch_player(&self, connection_obj: Obj, new_player: Obj) -> Result<(), SessionError> {
        // Get the client IDs for this connection object
        let client_ids = self
            .connections
            .client_ids_for(connection_obj)
            .map_err(|_| SessionError::DeliveryError)?;

        // Generate a new auth token for the new player
        let new_auth_token = self.make_auth_token(&new_player);

        // Prepare events for all clients before making any changes
        let mut events_to_send = Vec::new();
        for client_id in &client_ids {
            let event = ClientEvent {
                event: ClientEventUnion::PlayerSwitchedEvent(Box::new(
                    moor_rpc::PlayerSwitchedEvent {
                        new_player: obj_fb(&new_player),
                        new_auth_token: Box::new(moor_rpc::AuthToken {
                            token: new_auth_token.0.clone(),
                        }),
                    },
                )),
            };
            events_to_send.push((*client_id, event));
        }

        // Switch the player for each client ID associated with this connection
        // Do this in one batch to minimize the window for inconsistency
        for client_id in &client_ids {
            self.connections
                .switch_player_for_client(*client_id, new_player)
                .map_err(|_| SessionError::DeliveryError)?;
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
    // Client message handlers - extracted from handle_client_message for clarity

    fn handle_connection_establish(
        &self,
        client_id: Uuid,
        conn_est: moor_rpc::ConnectionEstablishRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (hostname, local_port, remote_port) = self.extract_connection_params(
            &conn_est,
            |c| c.peer_addr(),
            |c| c.local_port(),
            |c| c.remote_port(),
        )?;

        let acceptable_content_types =
            self.extract_acceptable_content_types(&conn_est, |c| c.acceptable_content_types());

        let connection_attributes =
            self.extract_connection_attributes(&conn_est, |c| c.connection_attributes());

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
        Ok(mk_new_connection_reply(token, &oid))
    }

    fn handle_reattach(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        reattach: moor_rpc::ReattachRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let client_token = reattach
            .client_token()
            .rpc_err()
            .and_then(|r| client_token_from_ref(r).rpc_err())?;
        let connection = self.client_auth(client_token.clone(), client_id)?;

        let auth_token = reattach
            .auth_token()
            .rpc_err()
            .and_then(|r| auth_token_from_ref(r).rpc_err())?;
        let player = self.validate_auth_token(auth_token, None)?;

        let Some(current_player) = self.connections.player_object_for_client(client_id) else {
            return Err(RpcMessageError::NoConnection);
        };

        if current_player != player {
            return Err(RpcMessageError::PermissionDenied);
        }

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!(error = ?e, client_id = ?client_id, "Failed to refresh client activity during reattach");
        }

        let player_flags = scheduler_client.get_object_flags(&player).unwrap_or(0);

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::AttachResult(Box::new(moor_rpc::AttachResult {
                success: true,
                client_token: Some(Box::new(moor_rpc::ClientToken {
                    token: client_token.0,
                })),
                player: Some(obj_fb(&player)),
                player_flags,
            })),
        })
    }

    fn handle_client_pong(
        &self,
        client_id: Uuid,
        pong: moor_rpc::ClientPongRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let token = pong
            .client_token()
            .rpc_err()
            .and_then(|r| client_token_from_ref(r).rpc_err())?;

        let connection = self.client_auth(token, client_id)?;

        if self
            .connections
            .notify_is_alive(client_id, connection)
            .is_err()
        {
            warn!("Unable to notify connection is alive: {}", client_id);
        }

        Ok(mk_thanks_pong_reply(timestamp))
    }

    fn handle_request_sys_prop(
        &self,
        scheduler_client: SchedulerClient,
        req: moor_rpc::RequestSysPropRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = match req.auth_token() {
            Ok(Some(auth_ref)) => {
                let auth_token = auth_token_from_ref(auth_ref).rpc_err()?;
                self.validate_auth_token(auth_token, None)?
            }
            _ => SYSTEM_OBJECT,
        };
        let object = extract_object_ref_rpc(&req, "object", |r| r.object())?;
        let property = extract_symbol_rpc(&req, "property", |r| r.property())?;

        self.request_sys_prop(scheduler_client, player, object, property)
    }

    fn handle_login_command(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        login: moor_rpc::LoginCommandRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let connection = self.extract_client_token(&login, client_id, |l| l.client_token())?;

        let handler_object = extract_obj_rpc(&login, "handler_object", |l| l.handler_object())?;
        let args = extract_string_list_rpc(&login, "connect_args", |l| l.connect_args())?;
        let attach = extract_field_rpc(&login, "do_attach", |l| l.do_attach())?;

        self.perform_login(
            &handler_object,
            scheduler_client,
            client_id,
            &connection,
            args,
            attach,
        )
    }

    fn handle_attach(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        attach_msg: moor_rpc::AttachRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let auth_token = attach_msg
            .auth_token()
            .rpc_err()
            .and_then(|r| auth_token_from_ref(r).rpc_err())?;
        let player = self.validate_auth_token(auth_token, None)?;

        let handler_object =
            extract_obj_rpc(&attach_msg, "handler_object", |a| a.handler_object())?;

        let (hostname, local_port, remote_port) = self.extract_connection_params(
            &attach_msg,
            |a| a.peer_addr(),
            |a| a.local_port(),
            |a| a.remote_port(),
        )?;

        let acceptable_content_types =
            self.extract_acceptable_content_types(&attach_msg, |a| a.acceptable_content_types());

        let connect_type = attach_msg.connect_type().rpc_err()?;

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

        // Only submit user_connected task for actual connection types, not transient sessions
        if connect_type != moor_rpc::ConnectType::NoConnect {
            let connection = self
                .connections
                .connection_object_for_client(client_id)
                .ok_or(RpcMessageError::InternalError(
                    "Connection not found".to_string(),
                ))?;

            if let Err(e) = self.submit_connected_task(
                &handler_object,
                scheduler_client.clone(),
                client_id,
                &player,
                &connection,
                connect_type,
            ) {
                error!(error = ?e, "Error submitting user_connected task");
            }
        }

        // Get player flags for client-side permission checks
        let player_flags = scheduler_client.get_object_flags(&player).unwrap_or(0);

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::AttachResult(Box::new(moor_rpc::AttachResult {
                success: true,
                client_token: Some(Box::new(moor_rpc::ClientToken {
                    token: client_token.0.clone(),
                })),
                player: Some(obj_fb(&player)),
                player_flags,
            })),
        })
    }

    fn handle_command(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        cmd: moor_rpc::CommandRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &cmd,
            client_id,
            |c| c.client_token(),
            |c| c.auth_token(),
        )?;

        let handler_object = extract_obj_rpc(&cmd, "handler_object", |c| c.handler_object())?;
        let command = extract_string_rpc(&cmd, "command", |c| c.command())?;

        self.submit_command_task(
            scheduler_client,
            client_id,
            &handler_object,
            &player,
            command,
        )
    }

    fn handle_detach(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        detach: moor_rpc::DetachRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let connection = self.extract_client_token(&detach, client_id, |d| d.client_token())?;

        let disconnected = detach.disconnected().rpc_err()?;

        if disconnected {
            // Get player before removing connection
            if let Some(player) = self.connections.player_object_for_client(client_id) {
                // Remove this connection
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcMessageError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };

                // Only trigger user_disconnected if this was the last connection for the player
                match self.connections.client_ids_for(player) {
                    Ok(remaining_clients) if remaining_clients.is_empty() => {
                        // Last connection for this player - trigger user_disconnected
                        if let Err(e) = self.submit_disconnected_task(
                            &SYSTEM_OBJECT,
                            scheduler_client,
                            client_id,
                            &player,
                            &connection,
                        ) {
                            error!(error = ?e, "Error submitting user_disconnected task");
                        }
                    }
                    Ok(remaining_clients) => {
                        debug!(
                            player = ?player,
                            remaining_connections = remaining_clients.len(),
                            "Player still has active connections after detach"
                        );
                    }
                    Err(e) => {
                        error!(error = ?e, "Error checking remaining connections for player");
                    }
                }
            } else {
                // No player associated - just remove the connection
                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcMessageError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };
            }
        } else if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!(error = ?e, "Unable to refresh client activity on soft detach");
        }

        Ok(mk_disconnected_reply())
    }

    fn handle_requested_input(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        input: moor_rpc::RequestedInputRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &input,
            client_id,
            |i| i.client_token(),
            |i| i.auth_token(),
        )?;

        let request_id = extract_uuid_rpc(&input, "request_id", |i| i.request_id())?;
        let input_var = extract_var_rpc(&input, "input", |i| i.input())?;

        self.respond_input(scheduler_client, client_id, &player, request_id, input_var)
    }

    fn handle_out_of_band(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        oob: moor_rpc::OutOfBandRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &oob,
            client_id,
            |o| o.client_token(),
            |o| o.auth_token(),
        )?;

        let handler_object = extract_obj_rpc(&oob, "handler_object", |o| o.handler_object())?;
        let command = extract_string_rpc(&oob, "command", |o| o.command())?;

        self.submit_out_of_bound_task(
            scheduler_client,
            &handler_object,
            client_id,
            &player,
            command,
        )
    }

    fn handle_eval(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        eval: moor_rpc::EvalRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &eval,
            client_id,
            |e| e.client_token(),
            |e| e.auth_token(),
        )?;

        let evalstr = extract_string_rpc(&eval, "expression", |e| e.expression())?;

        self.submit_eval_task(scheduler_client, client_id, &player, evalstr)
    }

    fn handle_invoke_verb(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        invoke: moor_rpc::InvokeVerbRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &invoke,
            client_id,
            |i| i.client_token(),
            |i| i.auth_token(),
        )?;

        let object = extract_object_ref_rpc(&invoke, "object", |i| i.object())?;
        let verb = extract_symbol_rpc(&invoke, "verb", |i| i.verb())?;

        let args_vec = invoke.args().rpc_err()?;
        let args: Vec<Var> = args_vec
            .iter()
            .filter_map(|v| v.ok().and_then(|v| var_from_ref(v).ok()))
            .collect();

        self.submit_invoke_verb_task(scheduler_client, client_id, &player, &object, verb, args)
    }

    fn handle_retrieve(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        retr: moor_rpc::RetrieveRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&retr, |r| r.auth_token())?;

        let who = extract_object_ref_rpc(&retr, "object", |r| r.object())?;
        let retr_type = extract_field_rpc(&retr, "entity_type", |r| r.entity_type())?;
        let what = extract_symbol_rpc(&retr, "name", |r| r.name())?;

        match retr_type {
            moor_rpc::EntityType::Property => {
                let (propdef, propperms, value) = scheduler_client
                    .request_property(&player, &player, &who, what)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting property");
                        RpcMessageError::EntityRetrievalError(
                            "error requesting property".to_string(),
                        )
                    })?;
                let value_fb = var_to_flatbuffer_rpc(&value)?;
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::PropertyValue(Box::new(
                        moor_rpc::PropertyValue {
                            prop_info: Box::new(common::PropInfo {
                                definer: obj_fb(&propdef.definer()),
                                location: obj_fb(&propdef.location()),
                                name: Box::new(moor_rpc::Symbol {
                                    value: propdef.name().as_string(),
                                }),
                                owner: obj_fb(&propperms.owner()),
                                r: propperms.flags().contains(PropFlag::Read),
                                w: propperms.flags().contains(PropFlag::Write),
                                chown: propperms.flags().contains(PropFlag::Chown),
                            }),
                            value: Box::new(value_fb),
                        },
                    )),
                })
            }
            moor_rpc::EntityType::Verb => {
                let (verbdef, code) = scheduler_client
                    .request_verb(&player, &player, &who, what)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting verb");
                        RpcMessageError::EntityRetrievalError("error requesting verb".to_string())
                    })?;
                let argspec = verbdef.args();
                let arg_spec = vec![
                    moor_rpc::Symbol {
                        value: argspec.dobj.to_string().to_string(),
                    },
                    moor_rpc::Symbol {
                        value: preposition_to_string(&argspec.prep).to_string(),
                    },
                    moor_rpc::Symbol {
                        value: argspec.iobj.to_string().to_string(),
                    },
                ];
                let names = verbdef
                    .names()
                    .iter()
                    .map(|n| moor_rpc::Symbol {
                        value: n.as_string(),
                    })
                    .collect();
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::VerbValue(Box::new(moor_rpc::VerbValue {
                        verb_info: Box::new(common::VerbInfo {
                            location: obj_fb(&verbdef.location()),
                            owner: obj_fb(&verbdef.owner()),
                            names,
                            r: verbdef.flags().contains(VerbFlag::Read),
                            w: verbdef.flags().contains(VerbFlag::Write),
                            x: verbdef.flags().contains(VerbFlag::Exec),
                            d: verbdef.flags().contains(VerbFlag::Debug),
                            arg_spec,
                        }),
                        code,
                    })),
                })
            }
        }
    }

    fn handle_resolve(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        resolve: moor_rpc::ResolveRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&resolve, |r| r.auth_token())?;

        let objref = extract_object_ref_rpc(&resolve, "objref", |r| r.objref())?;

        let resolved = scheduler_client
            .resolve_object(player, objref)
            .map_err(|e| {
                error!(error = ?e, "Error resolving object");
                RpcMessageError::EntityRetrievalError("error resolving object".to_string())
            })?;

        let result_fb = var_to_flatbuffer_rpc(&resolved)?;
        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::ResolveResult(Box::new(moor_rpc::ResolveResult {
                result: Box::new(result_fb),
            })),
        })
    }

    fn handle_properties(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        props: moor_rpc::PropertiesRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&props, |p| p.auth_token())?;

        let obj = extract_object_ref_rpc(&props, "object", |p| p.object())?;

        let inherited = props.inherited().rpc_err()?;

        let prop_list = scheduler_client
            .request_properties(&player, &player, &obj, inherited)
            .map_err(|e| {
                error!(error = ?e, "Error requesting properties");
                RpcMessageError::EntityRetrievalError("error requesting properties".to_string())
            })?;

        let props = prop_list
            .iter()
            .map(|(propdef, propperms)| common::PropInfo {
                definer: obj_fb(&propdef.definer()),
                location: obj_fb(&propdef.location()),
                name: Box::new(moor_rpc::Symbol {
                    value: propdef.name().as_string(),
                }),
                owner: obj_fb(&propperms.owner()),
                r: propperms.flags().contains(PropFlag::Read),
                w: propperms.flags().contains(PropFlag::Write),
                chown: propperms.flags().contains(PropFlag::Chown),
            })
            .collect();

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::PropertiesReply(Box::new(moor_rpc::PropertiesReply {
                properties: props,
            })),
        })
    }

    fn handle_verbs(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        verbs: moor_rpc::VerbsRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&verbs, |v| v.auth_token())?;

        let obj = extract_object_ref_rpc(&verbs, "object", |v| v.object())?;

        let inherited = verbs.inherited().rpc_err()?;

        let verb_list = scheduler_client
            .request_verbs(&player, &player, &obj, inherited)
            .map_err(|e| {
                error!(error = ?e, "Error requesting verbs");
                RpcMessageError::EntityRetrievalError("error requesting verbs".to_string())
            })?;

        let verbs = verb_list
            .iter()
            .map(|v| {
                let names = v
                    .names()
                    .iter()
                    .map(|n| moor_rpc::Symbol {
                        value: n.as_string(),
                    })
                    .collect();
                let arg_spec = vec![
                    moor_rpc::Symbol {
                        value: v.args().dobj.to_string().to_string(),
                    },
                    moor_rpc::Symbol {
                        value: preposition_to_string(&v.args().prep).to_string(),
                    },
                    moor_rpc::Symbol {
                        value: v.args().iobj.to_string().to_string(),
                    },
                ];
                common::VerbInfo {
                    location: obj_fb(&v.location()),
                    owner: obj_fb(&v.owner()),
                    names,
                    r: v.flags().contains(VerbFlag::Read),
                    w: v.flags().contains(VerbFlag::Write),
                    x: v.flags().contains(VerbFlag::Exec),
                    d: v.flags().contains(VerbFlag::Debug),
                    arg_spec,
                }
            })
            .collect();

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::VerbsReply(Box::new(moor_rpc::VerbsReply { verbs })),
        })
    }

    fn handle_request_history(
        &self,
        hist: moor_rpc::RequestHistoryRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&hist, |h| h.auth_token())?;

        let history_recall = hist.history_recall().rpc_err()?;
        let history_response = self.build_history_response(player, history_recall)?;

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::HistoryResponseReply(Box::new(HistoryResponseReply {
                response: Box::new(history_response),
            })),
        })
    }

    fn handle_request_current_presentations(
        &self,
        req: moor_rpc::RequestCurrentPresentationsRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        let presentations = self.event_log.current_presentations(player);
        let presentation_list: Result<Vec<_>, _> = presentations
            .iter()
            .map(presentation_to_flatbuffer_struct)
            .collect();

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::CurrentPresentations(Box::new(
                moor_rpc::CurrentPresentations {
                    presentations: presentation_list.map_err(|e| {
                        RpcMessageError::InternalError(format!(
                            "Failed to convert presentation: {e}"
                        ))
                    })?,
                },
            )),
        })
    }

    fn handle_dismiss_presentation(
        &self,
        dismiss: moor_rpc::DismissPresentationRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&dismiss, |d| d.auth_token())?;

        let presentation_id = dismiss.presentation_id().rpc_err()?.to_string();

        self.event_log.dismiss_presentation(player, presentation_id);

        Ok(mk_presentation_dismissed_reply())
    }

    fn handle_set_client_attribute(
        &self,
        client_id: Uuid,
        set_attr: moor_rpc::SetClientAttributeRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, _player) = self.extract_and_verify_tokens(
            &set_attr,
            client_id,
            |s| s.client_token(),
            |s| s.auth_token(),
        )?;

        let key = extract_symbol_rpc(&set_attr, "key", |s| s.key())?;

        let value = set_attr
            .value()
            .ok()
            .and_then(|v_opt| v_opt.and_then(|v_ref| var_from_ref(v_ref).ok()));

        self.connections
            .set_client_attribute(client_id, key, value)?;

        Ok(mk_client_attribute_set_reply())
    }

    fn handle_program(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        prog: moor_rpc::ProgramRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let (_connection, player) = self.extract_and_verify_tokens(
            &prog,
            client_id,
            |p| p.client_token(),
            |p| p.auth_token(),
        )?;

        let object = extract_object_ref_rpc(&prog, "object", |p| p.object())?;

        let verb = extract_symbol_rpc(&prog, "verb", |p| p.verb())?;

        let code_vec = prog.code().rpc_err()?;
        let code: Vec<String> = code_vec
            .iter()
            .filter_map(|s| s.ok().map(|s| s.to_string()))
            .collect();

        self.program_verb(scheduler_client, client_id, &player, &object, verb, code)
    }

    fn publish_narrative_events(&self, events: &[(Obj, Box<NarrativeEvent>)]) -> Result<(), Error> {
        self.transport
            .publish_narrative_events(events, self.connections.as_ref())
    }

    // Helper methods that delegate to connections
    pub fn connection_name_for(&self, connection: Obj) -> Result<String, SessionError> {
        self.connections.connection_name_for(connection)
    }

    pub fn connected_seconds_for(&self, connection: Obj) -> Result<f64, SessionError> {
        self.connections.connected_seconds_for(connection)
    }

    pub fn disconnect(&self, player: Obj) -> Result<(), SessionError> {
        warn!("Disconnecting player: {}", player);
        let all_client_ids = self.connections.client_ids_for(player)?;

        // Send disconnect event to all client connections for this player
        let event = ClientEvent {
            event: ClientEventUnion::DisconnectEvent(Box::new(moor_rpc::DisconnectEvent {})),
        };

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
        metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), Error> {
        // Validate first - check that the player matches the logged-in player for this client
        let Some(logged_in_player) = self.connections.player_object_for_client(client_id) else {
            return Err(eyre::eyre!("No connection for player"));
        };
        if logged_in_player != player {
            return Err(eyre::eyre!("Player mismatch"));
        }

        // Serialize metadata to FlatBuffer format
        let metadata_fb = metadata.map(|meta| {
            meta.iter()
                .filter_map(|(key, value)| match var_to_flatbuffer(value) {
                    Ok(value_fb) => Some(moor_rpc::MetadataPair {
                        key: symbol_fb(key),
                        value: Box::new(value_fb),
                    }),
                    Err(e) => {
                        warn!(error = ?e, key = ?key, "Failed to serialize metadata value");
                        None
                    }
                })
                .collect::<Vec<_>>()
        });

        let event = ClientEvent {
            event: ClientEventUnion::RequestInputEvent(Box::new(moor_rpc::RequestInputEvent {
                request_id: uuid_fb(input_request_id),
                metadata: metadata_fb,
            })),
        };
        self.transport.publish_client_event(client_id, event)
    }

    pub fn send_system_message(
        &self,
        client_id: Uuid,
        player: Obj,
        message: String,
    ) -> Result<(), Error> {
        let event = ClientEvent {
            event: ClientEventUnion::SystemMessageEvent(Box::new(moor_rpc::SystemMessageEvent {
                player: obj_fb(&player),
                message,
            })),
        };
        self.transport.publish_client_event(client_id, event)
    }

    pub fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        let connections = self.connections.connections();
        Ok(connections
            .iter()
            .filter(|o| o > &&SYSTEM_OBJECT)
            .cloned()
            .collect())
    }

    pub fn idle_seconds_for(&self, player: Obj) -> Result<f64, SessionError> {
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
    ) -> Result<Vec<Obj>, SessionError> {
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
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
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
        let value_fb = var_to_flatbuffer_rpc(&value)
            .map_err(|e| eyre::eyre!("Failed to encode var: {}", e))?;
        self.transport.publish_client_event(
            client_id,
            ClientEvent {
                event: ClientEventUnion::SetConnectionOptionEvent(Box::new(
                    moor_rpc::SetConnectionOptionEvent {
                        connection_obj: obj_fb(&connection_obj),
                        option_name: Box::new(moor_rpc::Symbol {
                            value: key.as_string(),
                        }),
                        value: Box::new(value_fb),
                    },
                )),
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
            Ok((obj, verb)) => Ok(DaemonToClientReply {
                reply: DaemonToClientReplyUnion::VerbProgramResponseReply(Box::new(
                    VerbProgramResponseReply {
                        response: Box::new(moor_rpc::VerbProgramResponse {
                            response: VerbProgramResponseUnion::VerbProgramSuccess(Box::new(
                                moor_rpc::VerbProgramSuccess {
                                    obj: obj_fb(&obj),
                                    verb_name: verb.to_string(),
                                },
                            )),
                        }),
                    },
                )),
            }),
            Err(SchedulerError::VerbProgramFailed(f)) => {
                let verb_error = verb_program_error_to_flatbuffer_struct(&f).map_err(|e| {
                    RpcMessageError::InternalError(format!(
                        "Failed to convert VerbProgramError: {e}"
                    ))
                })?;
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::VerbProgramResponseReply(Box::new(
                        VerbProgramResponseReply {
                            response: Box::new(moor_rpc::VerbProgramResponse {
                                response: VerbProgramResponseUnion::VerbProgramFailure(Box::new(
                                    moor_rpc::VerbProgramFailure {
                                        error: Box::new(verb_error),
                                    },
                                )),
                            }),
                        },
                    )),
                })
            }
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
                return Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::SysPropValue(Box::new(
                        moor_rpc::SysPropValue { value: None },
                    )),
                });
            }
            Err(e) => {
                error!(error = ?e, "Error requesting system property");
                return Err(RpcMessageError::ErrorCouldNotRetrieveSysProp(
                    "error requesting system property".to_string(),
                ));
            }
        };

        let pv_fb = var_to_flatbuffer_rpc(&pv)?;
        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::SysPropValue(Box::new(moor_rpc::SysPropValue {
                value: Some(Box::new(pv_fb)),
            })),
        })
    }

    /// Get attributes for a single connection object
    fn get_connection_attributes_for_single_connection(
        &self,
        connection_obj: Obj,
    ) -> Result<std::collections::HashMap<Symbol, Var>, SessionError> {
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

    fn handle_get_event_log_pubkey(
        &self,
        _client_id: Uuid,
        req: moor_rpc::GetEventLogPublicKeyRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        let public_key = self.event_log.get_pubkey(player);

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::EventLogPublicKey(Box::new(
                moor_rpc::EventLogPublicKey { public_key },
            )),
        })
    }

    fn handle_set_event_log_pubkey(
        &self,
        _client_id: Uuid,
        req: moor_rpc::SetEventLogPublicKeyRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        let public_key = extract_string_rpc(&req, "public_key", |r| r.public_key())?;

        self.event_log.set_pubkey(player, public_key);

        // Return the key that was set
        let public_key = self.event_log.get_pubkey(player);

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::EventLogPublicKey(Box::new(
                moor_rpc::EventLogPublicKey { public_key },
            )),
        })
    }

    fn handle_delete_event_log_history(
        &self,
        _client_id: Uuid,
        req: moor_rpc::DeleteEventLogHistoryRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        let success = match self.event_log.delete_all_events(player) {
            Ok(_) => true,
            Err(e) => {
                error!(
                    "Failed to delete event history for player {:?}: {}",
                    player, e
                );
                false
            }
        };

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::EventLogHistoryDeleted(Box::new(
                moor_rpc::EventLogHistoryDeleted { success },
            )),
        })
    }

    fn handle_list_objects(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        req: moor_rpc::ListObjectsRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        // Get list of all objects the player can see
        let objects = scheduler_client.list_objects(&player).map_err(|e| {
            error!(error = ?e, "Error listing objects");
            RpcMessageError::EntityRetrievalError("error listing objects".to_string())
        })?;

        // Convert to ObjectInfo FlatBuffer structures
        let object_infos: Result<Vec<_>, _> = objects
            .iter()
            .map(|(obj, attrs, verbs_count, props_count)| {
                // Get contents count - for MVP we'll skip this expensive operation
                let contents_count = 0;

                Ok(moor_rpc::ObjectInfo {
                    obj: obj_fb(obj),
                    name: attrs
                        .name()
                        .map(|n| Box::new(moor_rpc::Symbol { value: n })),
                    parent: attrs.parent().map(|p| obj_fb(&p)),
                    owner: obj_fb(&attrs.owner().unwrap_or(*obj)),
                    flags: attrs.flags().to_u16(),
                    location: attrs.location().map(|l| obj_fb(&l)),
                    contents_count,
                    verbs_count: *verbs_count as u32,
                    properties_count: *props_count as u32,
                })
            })
            .collect();

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::ListObjectsReply(Box::new(
                moor_rpc::ListObjectsReply {
                    objects: object_infos.map_err(|e: &str| {
                        RpcMessageError::InternalError(format!("Failed to convert object: {e}"))
                    })?,
                },
            )),
        })
    }

    fn handle_invoke_system_handler(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        invoke: moor_rpc::InvokeSystemHandlerRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Extract host_id for accountability (currently unused but available for logging/auditing)
        let _host_id = extract_uuid_rpc(&invoke, "host_id", |i| i.host_id())?;

        // Extract handler_type for routing to #0:invoke_<handler_type>_handler
        let handler_type = extract_string_rpc(&invoke, "handler_type", |i| i.handler_type())?;

        // Extract args for passing to the handler
        let args_vec = invoke.args().rpc_err()?;
        let args: Vec<Var> = args_vec
            .iter()
            .filter_map(|v| v.ok().and_then(|v| var_from_ref(v).ok()))
            .collect();

        // Handle authentication - auth_token is optional for system handlers
        let player = match invoke.auth_token() {
            Ok(Some(auth_token_ref)) => {
                // If auth token is provided, validate it and use that player
                let auth_token = auth_token_from_ref(auth_token_ref).rpc_err()?;
                self.validate_auth_token(auth_token, None)?
            }
            Ok(None) | Err(_) => {
                // If no auth token provided or error, use the system object (#0)
                SYSTEM_OBJECT
            }
        };

        // Submit the system handler task - will call #0:invoke_<handler_type>_handler
        self.submit_invoke_system_handler_task(
            scheduler_client,
            client_id,
            &player,
            handler_type,
            args,
        )
    }

    fn handle_update_property(
        &self,
        scheduler_client: SchedulerClient,
        _client_id: Uuid,
        req: moor_rpc::UpdatePropertyRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = self.extract_auth_token(&req, |r| r.auth_token())?;

        let object = extract_object_ref_rpc(&req, "object", |r| r.object())?;
        let property = extract_symbol_rpc(&req, "property", |r| r.property())?;
        let value = extract_var_rpc(&req, "value", |r| r.value())?;

        // Update the property
        scheduler_client
            .update_property(&player, &player, &object, property, value)
            .map_err(|e| {
                error!(error = ?e, "Error updating property");
                RpcMessageError::EntityRetrievalError("error updating property".to_string())
            })?;

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::PropertyUpdated(Box::new(
                moor_rpc::PropertyUpdated {},
            )),
        })
    }

    fn handle_call_system_verb(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        call: moor_rpc::CallSystemVerbRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        let player = match call.auth_token() {
            Ok(Some(auth_token_ref)) => {
                let auth_token = auth_token_from_ref(auth_token_ref).rpc_err()?;
                self.validate_auth_token(auth_token, None)?
            }
            _ => SYSTEM_OBJECT,
        };
        let verb = extract_symbol_rpc(&call, "verb", |c| c.verb())?;

        let args_vec = call.args().rpc_err()?;
        let args: Vec<Var> = args_vec
            .iter()
            .filter_map(|v| v.ok().and_then(|v| var_from_ref(v).ok()))
            .collect();

        // Use output capture session for system verb calls
        self.submit_system_verb_task(
            scheduler_client,
            client_id,
            &player,
            &ObjectRef::Id(SYSTEM_OBJECT), // Target system object
            verb,
            args,
        )
    }

    fn submit_system_verb_task(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // Create output capture session instead of regular RpcSession
        let session = Arc::new(OutputCaptureSession::new(client_id, *player));

        let task_handle = match scheduler_client.submit_verb_task(
            player,
            object,
            verb,
            List::mk_list(&args),
            "".to_string(),
            &SYSTEM_OBJECT,
            session.clone(),
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting system verb task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };

        // Wait for task completion like eval and system handler tasks do
        let receiver = task_handle.into_receiver();
        loop {
            match receiver.recv() {
                Ok((_, Ok(TaskNotification::Result(v)))) => {
                    // Get captured output from the session
                    let captured_events = session.take_captured_events();

                    let result_fb = var_to_flatbuffer(&v).map_err(|e| {
                        RpcMessageError::InternalError(format!("Failed to encode result: {e}"))
                    })?;

                    // Convert captured events to FlatBuffer format
                    debug!(
                        "System verb task completed, captured {} events",
                        captured_events.len()
                    );
                    let output_fb: Vec<moor_rpc::NarrativeEvent> = captured_events
                        .into_iter()
                        .filter_map(|(_player, event)| {
                            match narrative_event_to_flatbuffer_struct(&event) {
                                Ok(fb_event) => Some(fb_event),
                                Err(e) => {
                                    error!("Failed to convert narrative event to FlatBuffer: {e}");
                                    None
                                }
                            }
                        })
                        .collect();
                    debug!(
                        "Successfully converted {} events to FlatBuffer",
                        output_fb.len()
                    );

                    break Ok(moor_rpc::DaemonToClientReply {
                        reply: moor_rpc::DaemonToClientReplyUnion::SystemVerbResponseReply(
                            Box::new(moor_rpc::SystemVerbResponseReply {
                                response: moor_rpc::SystemVerbResponseUnion::SystemVerbSuccess(
                                    Box::new(moor_rpc::SystemVerbSuccess {
                                        result: Box::new(result_fb),
                                        output: output_fb,
                                    }),
                                ),
                            }),
                        ),
                    });
                }
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Err(e))) => {
                    // Convert scheduler error to FlatBuffer and return as SystemVerbError
                    let scheduler_error_fb = match scheduler_error_to_flatbuffer_struct(&e) {
                        Ok(fb) => fb,
                        Err(encode_err) => {
                            break Err(RpcMessageError::InternalError(format!(
                                "Failed to encode scheduler error: {encode_err}"
                            )));
                        }
                    };
                    break Ok(moor_rpc::DaemonToClientReply {
                        reply: moor_rpc::DaemonToClientReplyUnion::SystemVerbResponseReply(
                            Box::new(moor_rpc::SystemVerbResponseReply {
                                response: moor_rpc::SystemVerbResponseUnion::SystemVerbError(
                                    Box::new(moor_rpc::SystemVerbError {
                                        error: Box::new(scheduler_error_fb),
                                    }),
                                ),
                            }),
                        ),
                    });
                }
                Err(e) => {
                    break Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        }
    }
}

fn convert_listeners<'a>(
    listeners: Option<planus::Vector<'a, ::planus::Result<ListenerRef<'a>>>>,
) -> Vec<(Obj, SocketAddr)> {
    let listeners: Vec<(Obj, SocketAddr)> = listeners
        .map(|ls| {
            ls.iter()
                .filter_map(|l| {
                    let l = l.ok()?;
                    let obj = obj_from_ref(l.handler_object().ok()?).ok()?;
                    let addr_str = l.socket_addr().ok()?;
                    let socket_addr: SocketAddr = addr_str.parse().ok()?;
                    Some((obj, socket_addr))
                })
                .collect()
        })
        .unwrap_or_default();

    listeners
}
