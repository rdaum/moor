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
use papaya::HashMap as PapayaHashMap;
use rpc_common::flatbuffers_generated::moor_rpc::{HostClientToDaemonMessageUnionRef, ListenerRef};
use std::{
    hash::BuildHasherDefault,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};
use uuid::Uuid;

use super::{hosts::Hosts, session::SessionActions, transport::Transport};
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
    SchedulerClient, config::Config, tasks::sched_counters, vm::builtins::bf_perf_counters,
};
use moor_rpc::{
    ClientEvent, ClientEventUnion, DaemonToClientReply, DaemonToClientReplyUnion, DaemonToHostAck,
    DaemonToHostReply, DaemonToHostReplyUnion, HostClientToDaemonMessageRef,
    VerbProgramResponseReply, VerbProgramResponseUnion,
};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var};
use rpc_common::{
    AuthToken, ClientToken, HostToken, HostType, RpcMessageError, auth_token_from_ref,
    extract_field, extract_obj, extract_object_ref, extract_string, extract_string_list,
    extract_symbol, extract_uuid, extract_var,
    flatbuffers_generated::{moor_rpc, moor_rpc::HostToDaemonMessageUnionRef},
    obj_from_ref, obj_to_flatbuffer_struct, objectref_from_ref, presentation_to_flatbuffer_struct,
    symbol_from_ref, uuid_to_flatbuffer_struct, var_from_ref, var_to_flatbuffer_bytes,
    verb_program_error_to_flatbuffer_struct,
};
use rusty_paseto::prelude::Key;
use tracing::{error, info, warn};

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
        host_token: HostToken,
        message: moor_rpc::HostToDaemonMessageRef<'_>,
    ) -> Result<DaemonToHostReply, RpcMessageError>;

    /// Process a client-to-daemon message (FlatBuffer refs)
    fn handle_client_message(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        message: HostClientToDaemonMessageRef<'_>,
    ) -> Result<DaemonToClientReply, RpcMessageError>;

    /// Validate a host token
    fn validate_host_token(&self, token: &HostToken) -> Result<HostType, RpcMessageError>;

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

    pub(crate) host_token_cache:
        PapayaHashMap<HostToken, (Instant, HostType), BuildHasherDefault<AHasher>>,
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
        message: moor_rpc::HostToDaemonMessageRef<'_>,
    ) -> Result<DaemonToHostReply, RpcMessageError> {
        let response = match message.message().map_err(|e| {
            RpcMessageError::InvalidRequest(format!("missing host message union: {e:?}"))
        })? {
            HostToDaemonMessageUnionRef::RegisterHost(reg) => {
                let host_type = match reg.host_type().unwrap() {
                    moor_rpc::HostType::Tcp => HostType::TCP,
                    moor_rpc::HostType::WebSocket => HostType::WebSocket,
                };

                // Convert listeners from FlatBuffer to Vec<(Obj, SocketAddr)>
                let listeners: Vec<(Obj, SocketAddr)> = convert_listeners(reg.listeners().ok());

                info!(
                    "Host {} registered with {} listeners",
                    host_token.0,
                    listeners.len()
                );
                let mut hosts = self.hosts.write().unwrap();
                hosts.receive_ping(host_token, host_type, listeners);

                DaemonToHostReply {
                    reply: DaemonToHostReplyUnion::DaemonToHostAck(Box::new(DaemonToHostAck {})),
                }
            }
            HostToDaemonMessageUnionRef::HostPong(pong) => {
                let host_type = match pong.host_type().unwrap() {
                    moor_rpc::HostType::Tcp => HostType::TCP,
                    moor_rpc::HostType::WebSocket => HostType::WebSocket,
                };

                let listeners: Vec<(Obj, SocketAddr)> = convert_listeners(pong.listeners().ok());

                let num_listeners = listeners.len();
                let mut hosts = self.hosts.write().unwrap();
                if hosts.receive_ping(host_token.clone(), host_type, listeners) {
                    info!(
                        "Host {} registered with {} listeners",
                        host_token.0, num_listeners
                    );
                }

                DaemonToHostReply {
                    reply: DaemonToHostReplyUnion::DaemonToHostAck(Box::new(DaemonToHostAck {})),
                }
            }
            HostToDaemonMessageUnionRef::RequestPerformanceCounters(_) => {
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
            HostToDaemonMessageUnionRef::DetachHost(_) => {
                let mut hosts = self.hosts.write().unwrap();
                hosts.unregister_host(&host_token);
                DaemonToHostReply {
                    reply: DaemonToHostReplyUnion::DaemonToHostAck(Box::new(DaemonToHostAck {})),
                }
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
                let hostname = conn_est
                    .peer_addr()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing peer_addr".to_string()))?
                    .to_string();
                let local_port = conn_est.local_port().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing local_port".to_string())
                })?;
                let remote_port = conn_est.remote_port().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing remote_port".to_string())
                })?;

                // Convert acceptable_content_types
                let acceptable_content_types =
                    conn_est
                        .acceptable_content_types()
                        .ok()
                        .and_then(|types_opt| {
                            types_opt.map(|types| {
                                types
                                    .iter()
                                    .filter_map(|s| {
                                        let s = s.ok()?;
                                        symbol_from_ref(s).ok()
                                    })
                                    .collect()
                            })
                        });

                // Convert connection_attributes
                let connection_attributes =
                    conn_est.connection_attributes().ok().and_then(|attrs_opt| {
                        attrs_opt.map(|attrs| {
                            attrs
                                .iter()
                                .filter_map(|attr| {
                                    let attr = attr.ok()?;
                                    let key = symbol_from_ref(attr.key().ok()?).ok()?;
                                    let value = var_from_ref(attr.value().ok()?).ok()?;
                                    Some((key, value))
                                })
                                .collect()
                        })
                    });

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
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::NewConnection(Box::new(
                        moor_rpc::NewConnection {
                            client_token: Box::new(moor_rpc::ClientToken {
                                token: token.0.clone(),
                            }),
                            connection_obj: Box::new(obj_to_flatbuffer_struct(&oid)),
                        },
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::ClientPong(pong) => {
                // Always respond with a ThanksPong
                let timestamp = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                let response = Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::ThanksPong(Box::new(moor_rpc::ThanksPong {
                        timestamp,
                    })),
                });

                // Extract and validate client token
                let token_ref = pong.client_token().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing client_token".to_string())
                })?;
                let token_string = token_ref
                    .token()
                    .map_err(|_| {
                        RpcMessageError::InvalidRequest("Missing token string".to_string())
                    })?
                    .to_string();
                let token = ClientToken(token_string);

                let connection = self.client_auth(token, client_id)?;
                // Let 'connections' know that the connection is still alive.
                let Ok(_) = self.connections.notify_is_alive(client_id, connection) else {
                    warn!("Unable to notify connection is alive: {}", client_id);
                    return response;
                };
                response
            }
            HostClientToDaemonMessageUnionRef::RequestSysProp(req) => {
                let connection =
                    self.extract_client_token(&req, client_id, |r| r.client_token())?;

                let object = extract_object_ref(&req, "object", |r| r.object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let property = extract_symbol(&req, "property", |r| r.property())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.request_sys_prop(scheduler_client, connection, object, property)
            }
            HostClientToDaemonMessageUnionRef::LoginCommand(login) => {
                let connection =
                    self.extract_client_token(&login, client_id, |l| l.client_token())?;

                let handler_object = extract_obj(&login, "handler_object", |l| l.handler_object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let args = extract_string_list(&login, "connect_args", |l| l.connect_args())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let attach = extract_field(&login, "do_attach", |l| l.do_attach())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.perform_login(
                    &handler_object,
                    scheduler_client,
                    client_id,
                    &connection,
                    args,
                    attach,
                )
            }
            HostClientToDaemonMessageUnionRef::Attach(attach_msg) => {
                let auth_token_ref = attach_msg.auth_token().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing auth_token".to_string())
                })?;
                let auth_token = auth_token_from_ref(auth_token_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
                let player = self.validate_auth_token(auth_token, None)?;

                let handler_object_ref = attach_msg.handler_object().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing handler_object".to_string())
                })?;
                let handler_object = obj_from_ref(handler_object_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;

                let hostname = attach_msg
                    .peer_addr()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing peer_addr".to_string()))?
                    .to_string();
                let local_port = attach_msg.local_port().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing local_port".to_string())
                })?;
                let remote_port = attach_msg.remote_port().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing remote_port".to_string())
                })?;

                let acceptable_content_types =
                    attach_msg
                        .acceptable_content_types()
                        .ok()
                        .and_then(|types_opt| {
                            types_opt.map(|types| {
                                types
                                    .iter()
                                    .filter_map(|s| s.ok().and_then(|s| symbol_from_ref(s).ok()))
                                    .collect()
                            })
                        });

                let connect_type_val = attach_msg.connect_type().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing connect_type".to_string())
                })?;
                let connect_type = Some(connect_type_val);

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
                    }
                }
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::AttachResult(Box::new(
                        moor_rpc::AttachResult {
                            success: true,
                            client_token: Some(Box::new(moor_rpc::ClientToken {
                                token: client_token.0.clone(),
                            })),
                            player: Some(Box::new(obj_to_flatbuffer_struct(&player))),
                        },
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::Command(cmd) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &cmd,
                    client_id,
                    |c| c.client_token(),
                    |c| c.auth_token(),
                )?;

                let handler_object = extract_obj(&cmd, "handler_object", |c| c.handler_object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let command = extract_string(&cmd, "command", |c| c.command())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.submit_command_task(
                    scheduler_client,
                    client_id,
                    &handler_object,
                    &player,
                    command,
                )
            }
            HostClientToDaemonMessageUnionRef::Detach(detach) => {
                let connection =
                    self.extract_client_token(&detach, client_id, |d| d.client_token())?;

                let disconnected = detach.disconnected().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing disconnected".to_string())
                })?;

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

                let Ok(_) = self.connections.remove_client_connection(client_id) else {
                    return Err(RpcMessageError::InternalError(
                        "Unable to remove client connection".to_string(),
                    ));
                };

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::Disconnected(Box::new(
                        moor_rpc::Disconnected {},
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::RequestedInput(input) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &input,
                    client_id,
                    |i| i.client_token(),
                    |i| i.auth_token(),
                )?;

                let request_id = extract_uuid(&input, "request_id", |i| i.request_id())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let input_var = extract_var(&input, "input", |i| i.input())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.respond_input(scheduler_client, client_id, &player, request_id, input_var)
            }
            HostClientToDaemonMessageUnionRef::OutOfBand(oob) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &oob,
                    client_id,
                    |o| o.client_token(),
                    |o| o.auth_token(),
                )?;

                let handler_object = extract_obj(&oob, "handler_object", |o| o.handler_object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let command = extract_string(&oob, "command", |o| o.command())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.submit_out_of_bound_task(
                    scheduler_client,
                    &handler_object,
                    client_id,
                    &player,
                    command,
                )
            }
            HostClientToDaemonMessageUnionRef::Eval(eval) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &eval,
                    client_id,
                    |e| e.client_token(),
                    |e| e.auth_token(),
                )?;

                let evalstr = extract_string(&eval, "expression", |e| e.expression())
                    .map_err(RpcMessageError::InvalidRequest)?;

                self.submit_eval_task(scheduler_client, client_id, &player, evalstr)
            }
            HostClientToDaemonMessageUnionRef::InvokeVerb(invoke) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &invoke,
                    client_id,
                    |i| i.client_token(),
                    |i| i.auth_token(),
                )?;

                let object = extract_object_ref(&invoke, "object", |i| i.object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let verb = extract_symbol(&invoke, "verb", |i| i.verb())
                    .map_err(RpcMessageError::InvalidRequest)?;

                let args_vec = invoke
                    .args()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing args".to_string()))?;
                let args: Vec<Var> = args_vec
                    .iter()
                    .filter_map(|v| v.ok().and_then(|v| var_from_ref(v).ok()))
                    .collect();

                self.submit_invoke_verb_task(
                    scheduler_client,
                    client_id,
                    &player,
                    &object,
                    verb,
                    args,
                )
            }
            HostClientToDaemonMessageUnionRef::Retrieve(retr) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &retr,
                    client_id,
                    |r| r.client_token(),
                    |r| r.auth_token(),
                )?;

                let who = extract_object_ref(&retr, "object", |r| r.object())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let retr_type = extract_field(&retr, "entity_type", |r| r.entity_type())
                    .map_err(RpcMessageError::InvalidRequest)?;
                let what = extract_symbol(&retr, "name", |r| r.name())
                    .map_err(RpcMessageError::InvalidRequest)?;

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
                        let value_bytes = var_to_flatbuffer_bytes(&value).map_err(|e| {
                            RpcMessageError::InternalError(format!("Failed to encode var: {}", e))
                        })?;
                        Ok(DaemonToClientReply {
                            reply: DaemonToClientReplyUnion::PropertyValue(Box::new(
                                moor_rpc::PropertyValue {
                                    prop_info: Box::new(moor_rpc::PropInfo {
                                        definer: Box::new(obj_to_flatbuffer_struct(
                                            &propdef.definer(),
                                        )),
                                        location: Box::new(obj_to_flatbuffer_struct(
                                            &propdef.location(),
                                        )),
                                        name: Box::new(moor_rpc::Symbol {
                                            value: propdef.name().as_string(),
                                        }),
                                        owner: Box::new(obj_to_flatbuffer_struct(
                                            &propperms.owner(),
                                        )),
                                        r: propperms.flags().contains(PropFlag::Read),
                                        w: propperms.flags().contains(PropFlag::Write),
                                        chown: propperms.flags().contains(PropFlag::Chown),
                                    }),
                                    value: Box::new(moor_rpc::VarBytes { data: value_bytes }),
                                },
                            )),
                        })
                    }
                    moor_rpc::EntityType::Verb => {
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
                            reply: DaemonToClientReplyUnion::VerbValue(Box::new(
                                moor_rpc::VerbValue {
                                    verb_info: Box::new(moor_rpc::VerbInfo {
                                        location: Box::new(obj_to_flatbuffer_struct(
                                            &verbdef.location(),
                                        )),
                                        owner: Box::new(obj_to_flatbuffer_struct(&verbdef.owner())),
                                        names,
                                        r: verbdef.flags().contains(VerbFlag::Read),
                                        w: verbdef.flags().contains(VerbFlag::Write),
                                        x: verbdef.flags().contains(VerbFlag::Exec),
                                        d: verbdef.flags().contains(VerbFlag::Debug),
                                        arg_spec,
                                    }),
                                    code,
                                },
                            )),
                        })
                    }
                }
            }
            HostClientToDaemonMessageUnionRef::Resolve(resolve) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &resolve,
                    client_id,
                    |r| r.client_token(),
                    |r| r.auth_token(),
                )?;

                let objref_ref = resolve
                    .objref()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing objref".to_string()))?;
                let objref = objectref_from_ref(objref_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;

                let resolved = scheduler_client
                    .resolve_object(player, objref)
                    .map_err(|e| {
                        error!(error = ?e, "Error resolving object");
                        RpcMessageError::EntityRetrievalError("error resolving object".to_string())
                    })?;

                let result_bytes = var_to_flatbuffer_bytes(&resolved).map_err(|e| {
                    RpcMessageError::InternalError(format!("Failed to encode result: {}", e))
                })?;
                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::ResolveResult(Box::new(
                        moor_rpc::ResolveResult {
                            result: Box::new(moor_rpc::VarBytes { data: result_bytes }),
                        },
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::Properties(props) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &props,
                    client_id,
                    |p| p.client_token(),
                    |p| p.auth_token(),
                )?;

                let obj_ref = props
                    .object()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing object".to_string()))?;
                let obj = objectref_from_ref(obj_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;

                let inherited = props.inherited().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing inherited".to_string())
                })?;

                let prop_list = scheduler_client
                    .request_properties(&player, &player, &obj, inherited)
                    .map_err(|e| {
                        error!(error = ?e, "Error requesting properties");
                        RpcMessageError::EntityRetrievalError(
                            "error requesting properties".to_string(),
                        )
                    })?;

                let props = prop_list
                    .iter()
                    .map(|(propdef, propperms)| moor_rpc::PropInfo {
                        definer: Box::new(obj_to_flatbuffer_struct(&propdef.definer())),
                        location: Box::new(obj_to_flatbuffer_struct(&propdef.location())),
                        name: Box::new(moor_rpc::Symbol {
                            value: propdef.name().as_string(),
                        }),
                        owner: Box::new(obj_to_flatbuffer_struct(&propperms.owner())),
                        r: propperms.flags().contains(PropFlag::Read),
                        w: propperms.flags().contains(PropFlag::Write),
                        chown: propperms.flags().contains(PropFlag::Chown),
                    })
                    .collect();

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::PropertiesReply(Box::new(
                        moor_rpc::PropertiesReply { properties: props },
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::Verbs(verbs) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &verbs,
                    client_id,
                    |v| v.client_token(),
                    |v| v.auth_token(),
                )?;

                let obj_ref = verbs
                    .object()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing object".to_string()))?;
                let obj = objectref_from_ref(obj_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;

                let inherited = verbs.inherited().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing inherited".to_string())
                })?;

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
                        moor_rpc::VerbInfo {
                            location: Box::new(obj_to_flatbuffer_struct(&v.location())),
                            owner: Box::new(obj_to_flatbuffer_struct(&v.owner())),
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
                    reply: DaemonToClientReplyUnion::VerbsReply(Box::new(moor_rpc::VerbsReply {
                        verbs,
                    })),
                })
            }
            HostClientToDaemonMessageUnionRef::RequestHistory(hist) => {
                let player = self.extract_auth_token(&hist, |h| h.auth_token())?;

                let history_recall_ref = hist.history_recall().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing history_recall".to_string())
                })?;
                let history_response = self.build_history_response(player, history_recall_ref)?;

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::HistoryResponse(Box::new(history_response)),
                })
            }
            HostClientToDaemonMessageUnionRef::RequestCurrentPresentations(req) => {
                let player = self.extract_auth_token(&req, |r| r.auth_token())?;

                let presentations = self.event_log.current_presentations(player);
                let presentation_list: Result<Vec<_>, _> = presentations
                    .into_values()
                    .map(|p| presentation_to_flatbuffer_struct(&p))
                    .collect();

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::CurrentPresentations(Box::new(
                        moor_rpc::CurrentPresentations {
                            presentations: presentation_list.map_err(|e| {
                                RpcMessageError::InternalError(format!(
                                    "Failed to convert presentation: {}",
                                    e
                                ))
                            })?,
                        },
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::DismissPresentation(dismiss) => {
                let player = self.extract_auth_token(&dismiss, |d| d.auth_token())?;

                let presentation_id = dismiss
                    .presentation_id()
                    .map_err(|_| {
                        RpcMessageError::InvalidRequest("Missing presentation_id".to_string())
                    })?
                    .to_string();

                self.event_log.dismiss_presentation(player, presentation_id);

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::PresentationDismissed(Box::new(
                        moor_rpc::PresentationDismissed {},
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::SetClientAttribute(set_attr) => {
                let (_connection, _player) = self.extract_and_verify_tokens(
                    &set_attr,
                    client_id,
                    |s| s.client_token(),
                    |s| s.auth_token(),
                )?;

                let key_ref = set_attr
                    .key()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing key".to_string()))?;
                let key = symbol_from_ref(key_ref).map_err(RpcMessageError::InvalidRequest)?;

                let value = set_attr
                    .value()
                    .ok()
                    .and_then(|v_opt| v_opt.and_then(|v_ref| var_from_ref(v_ref).ok()));

                self.connections
                    .set_client_attribute(client_id, key, value)?;

                Ok(DaemonToClientReply {
                    reply: DaemonToClientReplyUnion::ClientAttributeSet(Box::new(
                        moor_rpc::ClientAttributeSet {},
                    )),
                })
            }
            HostClientToDaemonMessageUnionRef::Program(prog) => {
                let (_connection, player) = self.extract_and_verify_tokens(
                    &prog,
                    client_id,
                    |p| p.client_token(),
                    |p| p.auth_token(),
                )?;

                let object_ref = prog
                    .object()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing object".to_string()))?;
                let object = objectref_from_ref(object_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;

                let verb_ref = prog
                    .verb()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing verb".to_string()))?;
                let verb = symbol_from_ref(verb_ref).map_err(RpcMessageError::InvalidRequest)?;

                let code_vec = prog
                    .code()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing code".to_string()))?;
                let code: Vec<String> = code_vec
                    .iter()
                    .filter_map(|s| s.ok().map(|s| s.to_string()))
                    .collect();

                self.program_verb(scheduler_client, client_id, &player, &object, verb, code)
            }
        }
    }

    fn validate_host_token(&self, token: &HostToken) -> Result<HostType, RpcMessageError> {
        self.validate_host_token_impl(token)
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
                    handler_object: Box::new(obj_to_flatbuffer_struct(&handler_object)),
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
                        new_player: Box::new(obj_to_flatbuffer_struct(&new_player)),
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
    ) -> Result<(), Error> {
        // Validate first - check that the player matches the logged-in player for this client
        let Some(logged_in_player) = self.connections.player_object_for_client(client_id) else {
            return Err(eyre::eyre!("No connection for player"));
        };
        if logged_in_player != player {
            return Err(eyre::eyre!("Player mismatch"));
        }

        let event = ClientEvent {
            event: ClientEventUnion::RequestInputEvent(Box::new(moor_rpc::RequestInputEvent {
                request_id: Box::new(uuid_to_flatbuffer_struct(&input_request_id)),
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
                player: Box::new(obj_to_flatbuffer_struct(&player)),
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
        let value_bytes = var_to_flatbuffer_bytes(&value)
            .map_err(|e| eyre::eyre!("Failed to encode var: {}", e))?;
        self.transport.publish_client_event(
            client_id,
            ClientEvent {
                event: ClientEventUnion::SetConnectionOptionEvent(Box::new(
                    moor_rpc::SetConnectionOptionEvent {
                        connection_obj: Box::new(obj_to_flatbuffer_struct(&connection_obj)),
                        option_name: Box::new(moor_rpc::Symbol {
                            value: key.as_string(),
                        }),
                        value: Box::new(moor_rpc::VarBytes { data: value_bytes }),
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
                                    obj: Box::new(obj_to_flatbuffer_struct(&obj)),
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
                        "Failed to convert VerbProgramError: {}",
                        e
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

        let pv_bytes = var_to_flatbuffer_bytes(&pv)
            .map_err(|e| RpcMessageError::InternalError(format!("Failed to encode var: {}", e)))?;
        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::SysPropValue(Box::new(moor_rpc::SysPropValue {
                value: Some(Box::new(moor_rpc::VarBytes { data: pv_bytes })),
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
