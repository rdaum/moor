// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! High-level async client for invoking verbs on a moor daemon.
//!
//! `TaskClient` wraps `RpcClient` + a PubSub subscription to provide a single
//! `invoke_verb(...).await` that submits a verb invocation and awaits the
//! task completion event. Designed for hundreds of concurrent in-flight
//! verb calls from a game host or similar.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use moor_common::model::ObjectRef;
use moor_common::tasks::{NarrativeEvent, SchedulerError};
use moor_schema::{
    convert::{narrative_event_from_ref, obj_from_ref, uuid_from_ref, var_from_flatbuffer_ref},
    rpc as moor_rpc,
};
use moor_var::{Obj, Symbol, Var};
use rpc_common::{
    AuthToken, ClientToken, RpcError, mk_attach_msg, mk_detach_msg, mk_invoke_verb_msg,
    read_reply_result, scheduler_error_from_ref,
};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

use crate::pubsub_client::events_recv;
use crate::rpc_client::{CurveKeys, RpcClient};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Result of a completed task.
#[derive(Debug)]
pub enum TaskResult {
    /// Task completed successfully with a return value.
    Success(Var),
    /// Task failed with a scheduler error.
    Error(SchedulerError),
    /// Task suspended (went to background). The task_id is included for reference.
    Suspended(u64),
}

/// Session-level events from the daemon that are not correlated to a specific task.
///
/// These arrive on the PubSub channel alongside task completion events but are
/// player/session-scoped rather than task-scoped.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Narrative output from `notify()`, `tell()`, etc.
    Narrative(Obj, NarrativeEvent),
    /// System message (server announcements, etc.)
    SystemMessage(Obj, String),
    /// The daemon is requesting input (MOO `read()` builtin).
    RequestInput(Uuid),
    /// Server is disconnecting this session.
    Disconnect,
    /// Player identity changed (e.g. after login verb).
    PlayerSwitched { player: Obj, auth_token: AuthToken },
    /// MOO code set a connection option.
    SetConnectionOption {
        connection_obj: Obj,
        option: Symbol,
        value: Var,
    },
    /// Credentials were refreshed (e.g. after reattach).
    CredentialsUpdated {
        client_id: Uuid,
        client_token: ClientToken,
    },
}

const SESSION_EVENT_CHANNEL_CAPACITY: usize = 256;

/// Errors from TaskClient operations (transport/protocol level, not task-level).
#[derive(Debug, thiserror::Error)]
pub enum TaskClientError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("failed to encode verb arguments")]
    ArgEncoding,
    #[error("unexpected reply from daemon: {0}")]
    UnexpectedReply(String),
    #[error("task {0} timed out after {1:?}")]
    Timeout(u64, Duration),
    #[error("event dispatcher stopped (connection lost)")]
    DispatcherGone,
    #[error("attach failed: {0}")]
    AttachFailed(String),
}

type WaiterMap = Mutex<HashMap<u64, oneshot::Sender<TaskResult>>>;

/// High-level async client for calling verbs on a moor daemon.
///
/// Manages a persistent PubSub subscription and correlates task completion
/// events back to waiting callers via a shared waiter map.
///
/// # Example
/// ```no_run
/// use rpc_async_client::task_client::TaskClient;
///
/// # async fn example(client: &TaskClient) {
/// use moor_common::model::ObjectRef;
/// use moor_var::Symbol;
///
/// let result = client.invoke_verb(
///     &ObjectRef::Id(42.into()),
///     &Symbol::mk("look"),
///     vec![],
/// ).await;
/// # }
/// ```
pub struct TaskClient {
    rpc: RpcClient,
    client_id: Uuid,
    client_token: ClientToken,
    auth_token: AuthToken,
    waiters: Arc<WaiterMap>,
    session_events_tx: broadcast::Sender<SessionEvent>,
    dispatcher_handle: tokio::task::JoinHandle<()>,
    default_timeout: Duration,
}

/// Configuration for creating a TaskClient session.
pub struct TaskClientConfig {
    /// ZMQ context (shared across clients).
    pub zmq_context: Arc<tmq::Context>,
    /// Address of the daemon RPC endpoint (e.g. `ipc:///tmp/moor.rpc`).
    pub rpc_addr: String,
    /// Address of the daemon PubSub endpoint (e.g. `ipc:///tmp/moor.events`).
    pub pubsub_addr: String,
    /// Auth token for the player session.
    pub auth_token: AuthToken,
    /// Handler object for this connection (e.g. `#0`).
    pub handler_object: Obj,
    /// Peer address string for connection metadata.
    pub peer_addr: String,
    /// Local port for connection metadata.
    pub local_port: u16,
    /// CURVE encryption keys (optional, for TCP connections).
    pub curve_keys: Option<CurveKeys>,
    /// Default timeout for verb invocations.
    pub default_timeout: Duration,
}

impl Default for TaskClientConfig {
    fn default() -> Self {
        Self {
            zmq_context: Arc::new(tmq::Context::new()),
            rpc_addr: String::new(),
            pubsub_addr: String::new(),
            auth_token: AuthToken(String::new()),
            handler_object: Obj::mk_id(0),
            peer_addr: "localhost".to_string(),
            local_port: 0,
            curve_keys: None,
            default_timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl TaskClient {
    /// Create a new TaskClient, establishing a daemon session.
    ///
    /// This performs the full attach flow: creates a client_id, sends an
    /// Attach message, sets up the PubSub subscription, and spawns the
    /// background event dispatcher.
    pub async fn connect(config: TaskClientConfig) -> Result<Self, TaskClientError> {
        let rpc = RpcClient::new_with_defaults(
            config.zmq_context.clone(),
            config.rpc_addr,
            config.curve_keys.clone(),
        );

        let client_id = Uuid::new_v4();

        // Attach to the daemon
        let attach_msg = mk_attach_msg(
            &config.auth_token,
            Some(moor_rpc::ConnectType::NoConnect),
            &config.handler_object,
            config.peer_addr,
            config.local_port,
            0, // remote_port not relevant for programmatic clients
            None,
        );

        let reply_bytes = rpc
            .make_client_rpc_call(client_id, attach_msg)
            .await
            .map_err(TaskClientError::Rpc)?;

        let client_token = decode_attach_reply(&reply_bytes)?;

        // Set up PubSub subscription for this client_id
        let subscribe = create_events_subscription(
            &config.zmq_context,
            &config.pubsub_addr,
            client_id,
            config.curve_keys.as_ref(),
        )?;

        // Brief pause for ZMQ slow-joiner (subscription propagation)
        tokio::time::sleep(Duration::from_millis(10)).await;

        let waiters = Arc::new(Mutex::new(HashMap::new()));
        let (session_events_tx, _) = broadcast::channel(SESSION_EVENT_CHANNEL_CAPACITY);

        let dispatcher_handle = tokio::spawn(dispatcher_loop(
            client_id,
            subscribe,
            waiters.clone(),
            session_events_tx.clone(),
        ));

        debug!("TaskClient connected: client_id={}", client_id);

        Ok(Self {
            rpc,
            client_id,
            client_token,
            auth_token: config.auth_token,
            waiters,
            session_events_tx,
            dispatcher_handle,
            default_timeout: config.default_timeout,
        })
    }

    /// Create a TaskClient from an already-attached session.
    ///
    /// Use this when the caller has already performed attach and has a
    /// client_id, client_token, and RpcClient. The PubSub subscription
    /// is created internally.
    pub async fn from_session(
        rpc: RpcClient,
        client_id: Uuid,
        client_token: ClientToken,
        auth_token: AuthToken,
        zmq_context: &tmq::Context,
        pubsub_addr: &str,
        curve_keys: Option<&CurveKeys>,
        default_timeout: Duration,
    ) -> Result<Self, TaskClientError> {
        let subscribe =
            create_events_subscription(zmq_context, pubsub_addr, client_id, curve_keys)?;

        // Brief pause for ZMQ slow-joiner
        tokio::time::sleep(Duration::from_millis(10)).await;

        let waiters = Arc::new(WaiterMap::new(HashMap::new()));
        let (session_events_tx, _) = broadcast::channel(SESSION_EVENT_CHANNEL_CAPACITY);

        let dispatcher_handle = tokio::spawn(dispatcher_loop(
            client_id,
            subscribe,
            waiters.clone(),
            session_events_tx.clone(),
        ));

        Ok(Self {
            rpc,
            client_id,
            client_token,
            auth_token,
            waiters,
            session_events_tx,
            dispatcher_handle,
            default_timeout,
        })
    }

    /// Invoke a verb on an object and await the result.
    ///
    /// Uses the default timeout configured at construction time.
    pub async fn invoke_verb(
        &self,
        object: &ObjectRef,
        verb_name: &Symbol,
        args: Vec<&Var>,
    ) -> Result<TaskResult, TaskClientError> {
        self.invoke_verb_with_timeout(object, verb_name, args, self.default_timeout)
            .await
    }

    /// Invoke a verb on an object and await the result with a custom timeout.
    pub async fn invoke_verb_with_timeout(
        &self,
        object: &ObjectRef,
        verb_name: &Symbol,
        args: Vec<&Var>,
        timeout_duration: Duration,
    ) -> Result<TaskResult, TaskClientError> {
        // Build the InvokeVerb message
        let msg = mk_invoke_verb_msg(
            &self.client_token,
            &self.auth_token,
            object,
            verb_name,
            args,
        )
        .ok_or(TaskClientError::ArgEncoding)?;

        // Send RPC, get TaskSubmitted reply
        let reply_bytes = self
            .rpc
            .make_client_rpc_call(self.client_id, msg)
            .await
            .map_err(TaskClientError::Rpc)?;

        let task_id = extract_task_id(&reply_bytes)?;

        trace!("Task {} submitted for {}:{}", task_id, object, verb_name);

        // Register waiter. This happens after we get the task_id but before the
        // task could complete — the daemon returns TaskSubmitted before starting
        // execution, and the PubSub event requires a network round-trip.
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().unwrap().insert(task_id, tx);

        // Await result with timeout
        match tokio::time::timeout(timeout_duration, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => {
                // Sender was dropped — dispatcher died
                Err(TaskClientError::DispatcherGone)
            }
            Err(_) => {
                // Timeout — clean up the waiter to prevent leak
                self.waiters.lock().unwrap().remove(&task_id);
                Err(TaskClientError::Timeout(task_id, timeout_duration))
            }
        }
    }

    /// Get the client_id for this session.
    pub fn client_id(&self) -> Uuid {
        self.client_id
    }

    /// Subscribe to session-level events (narrative, system messages, input
    /// requests, disconnect, etc.).
    ///
    /// Returns a broadcast receiver. Multiple subscribers are supported.
    /// Events that arrive before any subscriber is created are dropped.
    pub fn session_events(&self) -> broadcast::Receiver<SessionEvent> {
        self.session_events_tx.subscribe()
    }

    /// Get the number of currently in-flight (waiting) tasks.
    pub fn pending_tasks(&self) -> usize {
        self.waiters.lock().unwrap().len()
    }

    /// Gracefully shut down the client, detaching from the daemon.
    pub async fn shutdown(self) {
        // Detach from daemon
        let detach_msg = mk_detach_msg(&self.client_token, true);
        if let Err(e) = self
            .rpc
            .make_client_rpc_call(self.client_id, detach_msg)
            .await
        {
            warn!("Failed to send detach on shutdown: {}", e);
        }

        // Stop the dispatcher
        self.dispatcher_handle.abort();
        debug!("TaskClient shut down: client_id={}", self.client_id);
    }
}

impl Drop for TaskClient {
    fn drop(&mut self) {
        // Safety net: abort dispatcher if shutdown() wasn't called.
        // The detach message won't be sent (requires async), but at least
        // we stop the background task.
        self.dispatcher_handle.abort();
    }
}

// ---------------------------------------------------------------------------
// Background event dispatcher
// ---------------------------------------------------------------------------

async fn dispatcher_loop(
    client_id: Uuid,
    mut subscribe: tmq::subscribe::Subscribe,
    waiters: Arc<WaiterMap>,
    session_tx: broadcast::Sender<SessionEvent>,
) {
    loop {
        let event_msg = match events_recv(client_id, &mut subscribe).await {
            Ok(msg) => msg,
            Err(e) => {
                error!("TaskClient PubSub error, dispatcher exiting: {}", e);
                break;
            }
        };

        let Ok(event) = event_msg.event() else {
            continue;
        };
        let Ok(event_union) = event.event() else {
            continue;
        };

        match event_union {
            // ----- Task-correlated events → resolve waiters -----
            moor_rpc::ClientEventUnionRef::TaskSuccessEvent(success) => {
                let Ok(task_id) = success.task_id() else {
                    continue;
                };
                let sender = waiters.lock().unwrap().remove(&task_id);
                if let Some(sender) = sender {
                    let result = match success.result() {
                        Ok(result_ref) => match var_from_flatbuffer_ref(result_ref) {
                            Ok(var) => TaskResult::Success(var),
                            Err(e) => {
                                error!("Failed to decode task {} result: {}", task_id, e);
                                continue;
                            }
                        },
                        Err(e) => {
                            error!("Failed to read task {} result ref: {}", task_id, e);
                            continue;
                        }
                    };
                    let _ = sender.send(result);
                }
            }
            moor_rpc::ClientEventUnionRef::TaskErrorEvent(error_event) => {
                let Ok(task_id) = error_event.task_id() else {
                    continue;
                };
                let sender = waiters.lock().unwrap().remove(&task_id);
                if let Some(sender) = sender {
                    let result = match error_event.error() {
                        Ok(error_ref) => match scheduler_error_from_ref(error_ref) {
                            Ok(sched_err) => TaskResult::Error(sched_err),
                            Err(e) => {
                                error!("Failed to decode task {} error: {}", task_id, e);
                                continue;
                            }
                        },
                        Err(e) => {
                            error!("Failed to read task {} error ref: {}", task_id, e);
                            continue;
                        }
                    };
                    let _ = sender.send(result);
                }
            }
            moor_rpc::ClientEventUnionRef::TaskSuspendedEvent(suspended) => {
                let Ok(task_id) = suspended.task_id() else {
                    continue;
                };
                let sender = waiters.lock().unwrap().remove(&task_id);
                if let Some(sender) = sender {
                    let _ = sender.send(TaskResult::Suspended(task_id));
                }
            }

            // ----- Session-level events → broadcast channel -----
            moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative) => {
                let session_event = (|| {
                    let player_obj = obj_from_ref(narrative.player().ok()?).ok()?;
                    let event_ref = narrative.event().ok()?;
                    let event = narrative_event_from_ref(event_ref).ok()?;
                    Some(SessionEvent::Narrative(player_obj, event))
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
            moor_rpc::ClientEventUnionRef::SystemMessageEvent(sys_msg) => {
                let session_event = (|| {
                    let player_obj = obj_from_ref(sys_msg.player().ok()?).ok()?;
                    let message = sys_msg.message().ok()?.to_string();
                    Some(SessionEvent::SystemMessage(player_obj, message))
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
            moor_rpc::ClientEventUnionRef::RequestInputEvent(input_request) => {
                let session_event = (|| {
                    let request_id_ref = input_request.request_id().ok()?;
                    let request_id = uuid_from_ref(request_id_ref).ok()?;
                    Some(SessionEvent::RequestInput(request_id))
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
            moor_rpc::ClientEventUnionRef::DisconnectEvent(_) => {
                let _ = session_tx.send(SessionEvent::Disconnect);
            }
            moor_rpc::ClientEventUnionRef::PlayerSwitchedEvent(switch) => {
                let session_event = (|| {
                    let player_obj = obj_from_ref(switch.new_player().ok()?).ok()?;
                    let auth_ref = switch.new_auth_token().ok()?;
                    let token = auth_ref.token().ok()?.to_string();
                    Some(SessionEvent::PlayerSwitched {
                        player: player_obj,
                        auth_token: AuthToken(token),
                    })
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
            moor_rpc::ClientEventUnionRef::SetConnectionOptionEvent(opt) => {
                let session_event = (|| {
                    let conn_obj = obj_from_ref(opt.connection_obj().ok()?).ok()?;
                    let option_ref = opt.option_name().ok()?;
                    let option = Symbol::mk(option_ref.value().ok()?);
                    let value_ref = opt.value().ok()?;
                    let value = var_from_flatbuffer_ref(value_ref).ok()?;
                    Some(SessionEvent::SetConnectionOption {
                        connection_obj: conn_obj,
                        option,
                        value,
                    })
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
            moor_rpc::ClientEventUnionRef::CredentialsUpdatedEvent(creds) => {
                let session_event = (|| {
                    let client_id_ref = creds.client_id().ok()?;
                    let cid = uuid_from_ref(client_id_ref).ok()?;
                    let token_ref = creds.client_token().ok()?;
                    let token = token_ref.token().ok()?.to_string();
                    Some(SessionEvent::CredentialsUpdated {
                        client_id: cid,
                        client_token: ClientToken(token),
                    })
                })();
                if let Some(evt) = session_event {
                    let _ = session_tx.send(evt);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract task_id from a TaskSubmitted RPC reply.
fn extract_task_id(reply_bytes: &[u8]) -> Result<u64, TaskClientError> {
    let reply = read_reply_result(reply_bytes)
        .map_err(|e| TaskClientError::UnexpectedReply(format!("bad flatbuffer: {e}")))?;

    let result_union = reply
        .result()
        .map_err(|e| TaskClientError::UnexpectedReply(format!("missing result: {e}")))?;

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = result_union else {
        return Err(TaskClientError::UnexpectedReply(
            "expected ClientSuccess".into(),
        ));
    };

    let daemon_reply = client_success
        .reply()
        .map_err(|e| TaskClientError::UnexpectedReply(format!("missing reply: {e}")))?;

    let reply_union = daemon_reply
        .reply()
        .map_err(|e| TaskClientError::UnexpectedReply(format!("missing reply union: {e}")))?;

    let moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) = reply_union else {
        return Err(TaskClientError::UnexpectedReply(
            "expected TaskSubmitted".into(),
        ));
    };

    task_submitted
        .task_id()
        .map_err(|e| TaskClientError::UnexpectedReply(format!("missing task_id: {e}")))
}

/// Decode the attach reply, extracting the client token.
fn decode_attach_reply(reply_bytes: &[u8]) -> Result<ClientToken, TaskClientError> {
    let reply = read_reply_result(reply_bytes)
        .map_err(|e| TaskClientError::AttachFailed(format!("bad flatbuffer: {e}")))?;

    let result_union = reply
        .result()
        .map_err(|e| TaskClientError::AttachFailed(format!("missing result: {e}")))?;

    let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = result_union else {
        return Err(TaskClientError::AttachFailed(
            "expected ClientSuccess".into(),
        ));
    };

    let daemon_reply = client_success
        .reply()
        .map_err(|e| TaskClientError::AttachFailed(format!("missing reply: {e}")))?;

    let reply_union = daemon_reply
        .reply()
        .map_err(|e| TaskClientError::AttachFailed(format!("missing reply union: {e}")))?;

    let moor_rpc::DaemonToClientReplyUnionRef::AttachResult(attach_result) = reply_union else {
        return Err(TaskClientError::AttachFailed(
            "expected AttachResult".into(),
        ));
    };

    let success = attach_result
        .success()
        .map_err(|e| TaskClientError::AttachFailed(format!("missing success flag: {e}")))?;
    if !success {
        return Err(TaskClientError::AttachFailed(
            "daemon rejected attach".into(),
        ));
    }

    let client_token_ref = attach_result
        .client_token()
        .ok()
        .flatten()
        .ok_or_else(|| TaskClientError::AttachFailed("missing client_token".into()))?;

    let token_str = client_token_ref
        .token()
        .map_err(|e| TaskClientError::AttachFailed(format!("missing token string: {e}")))?;

    Ok(ClientToken(token_str.to_string()))
}

/// Create a PubSub subscription for receiving events for a specific client_id.
fn create_events_subscription(
    zmq_ctx: &tmq::Context,
    pubsub_addr: &str,
    client_id: Uuid,
    curve_keys: Option<&CurveKeys>,
) -> Result<tmq::subscribe::Subscribe, TaskClientError> {
    let mut socket_builder = tmq::subscribe(zmq_ctx);

    if let Some(keys) = curve_keys {
        let client_secret_bytes = zmq::z85_decode(&keys.client_secret)
            .map_err(|_| TaskClientError::AttachFailed("invalid CURVE client secret".into()))?;
        let client_public_bytes = zmq::z85_decode(&keys.client_public)
            .map_err(|_| TaskClientError::AttachFailed("invalid CURVE client public".into()))?;
        let server_public_bytes = zmq::z85_decode(&keys.server_public)
            .map_err(|_| TaskClientError::AttachFailed("invalid CURVE server public".into()))?;

        socket_builder = socket_builder
            .set_curve_secretkey(&client_secret_bytes)
            .set_curve_publickey(&client_public_bytes)
            .set_curve_serverkey(&server_public_bytes);
    }

    let sub = socket_builder
        .connect(pubsub_addr)
        .map_err(|e| TaskClientError::Rpc(RpcError::CouldNotInitiateSession(e.to_string())))?;

    sub.subscribe(&client_id.as_bytes()[..])
        .map_err(|e| TaskClientError::Rpc(RpcError::CouldNotInitiateSession(e.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_schema::{convert::var_to_flatbuffer, rpc as moor_rpc};
    use moor_var::v_int;
    use rpc_common::scheduler_error_to_flatbuffer_struct;

    /// Helper: build a ReplyResult containing a ClientSuccess with a DaemonToClientReply.
    fn build_reply_result(reply: moor_rpc::DaemonToClientReply) -> Vec<u8> {
        let reply_result = moor_rpc::ReplyResult {
            result: moor_rpc::ReplyResultUnion::ClientSuccess(Box::new(moor_rpc::ClientSuccess {
                reply: Box::new(reply),
            })),
        };
        let mut builder = planus::Builder::new();
        builder.finish(&reply_result, None).to_vec()
    }

    /// Helper: build a serialized ClientEvent.
    fn build_client_event(event: moor_rpc::ClientEvent) -> Vec<u8> {
        let mut builder = planus::Builder::new();
        builder.finish(&event, None).to_vec()
    }

    // -----------------------------------------------------------------------
    // Unit tests for extract_task_id
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_task_id_success() {
        let reply = moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::TaskSubmitted(Box::new(
                moor_rpc::TaskSubmitted { task_id: 42 },
            )),
        };
        let bytes = build_reply_result(reply);
        let task_id = extract_task_id(&bytes).unwrap();
        assert_eq!(task_id, 42);
    }

    #[test]
    fn test_extract_task_id_wrong_reply_type() {
        let reply = moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::Disconnected(Box::new(
                moor_rpc::Disconnected {},
            )),
        };
        let bytes = build_reply_result(reply);
        let err = extract_task_id(&bytes).unwrap_err();
        assert!(matches!(err, TaskClientError::UnexpectedReply(_)));
    }

    #[test]
    fn test_extract_task_id_garbage_bytes() {
        let err = extract_task_id(b"not a flatbuffer").unwrap_err();
        assert!(matches!(err, TaskClientError::UnexpectedReply(_)));
    }

    // -----------------------------------------------------------------------
    // Unit tests for decode_attach_reply
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_attach_reply_success() {
        let reply = moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::AttachResult(Box::new(
                moor_rpc::AttachResult {
                    success: true,
                    client_token: Some(Box::new(moor_rpc::ClientToken {
                        token: "test-token-123".to_string(),
                    })),
                    player: None,
                    player_flags: 0,
                },
            )),
        };
        let bytes = build_reply_result(reply);
        let token = decode_attach_reply(&bytes).unwrap();
        assert_eq!(token.0, "test-token-123");
    }

    #[test]
    fn test_decode_attach_reply_rejected() {
        let reply = moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::AttachResult(Box::new(
                moor_rpc::AttachResult {
                    success: false,
                    client_token: None,
                    player: None,
                    player_flags: 0,
                },
            )),
        };
        let bytes = build_reply_result(reply);
        let err = decode_attach_reply(&bytes).unwrap_err();
        assert!(matches!(err, TaskClientError::AttachFailed(_)));
    }

    #[test]
    fn test_decode_attach_reply_wrong_type() {
        let reply = moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::TaskSubmitted(Box::new(
                moor_rpc::TaskSubmitted { task_id: 1 },
            )),
        };
        let bytes = build_reply_result(reply);
        let err = decode_attach_reply(&bytes).unwrap_err();
        assert!(matches!(err, TaskClientError::AttachFailed(_)));
    }

    // -----------------------------------------------------------------------
    // Integration test: dispatcher_loop with real ZMQ PubSub
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_dispatcher_loop_success_event() {
        let client_id = Uuid::new_v4();
        let task_id: u64 = 99;

        // Bind a PUB socket
        let zmq_ctx = tmq::Context::new();
        let addr = format!("inproc://test-dispatcher-success-{}", Uuid::new_v4());
        let publisher = tmq::publish(&zmq_ctx).bind(&addr).expect("bind PUB");

        // Create SUB socket
        let sub = tmq::subscribe(&zmq_ctx)
            .connect(&addr)
            .expect("connect SUB");
        let sub = sub.subscribe(&client_id.as_bytes()[..]).expect("subscribe");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let waiters = Arc::new(WaiterMap::new(HashMap::new()));
        let (session_tx, _) = broadcast::channel(16);
        let (tx, rx) = oneshot::channel();
        waiters.lock().unwrap().insert(task_id, tx);

        let handle = tokio::spawn(dispatcher_loop(client_id, sub, waiters.clone(), session_tx));

        // Publish a TaskSuccessEvent
        let value_fb = var_to_flatbuffer(&v_int(42)).unwrap();
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::TaskSuccessEvent(Box::new(
                moor_rpc::TaskSuccessEvent {
                    task_id,
                    result: Box::new(value_fb),
                },
            )),
        };
        let event_bytes = build_client_event(event);

        use futures_util::SinkExt;
        let multipart = vec![client_id.as_bytes().to_vec(), event_bytes];
        let mut publisher = publisher;
        publisher
            .send(
                multipart
                    .into_iter()
                    .map(zmq::Message::from)
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("publish");

        let result = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("timeout")
            .expect("channel");

        match result {
            TaskResult::Success(v) => assert_eq!(v, v_int(42)),
            other => panic!("expected success, got: {:?}", other),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn test_dispatcher_loop_error_event() {
        let client_id = Uuid::new_v4();
        let task_id: u64 = 100;

        let zmq_ctx = tmq::Context::new();
        let addr = format!("inproc://test-dispatcher-error-{}", Uuid::new_v4());
        let publisher = tmq::publish(&zmq_ctx).bind(&addr).expect("bind PUB");

        let sub = tmq::subscribe(&zmq_ctx)
            .connect(&addr)
            .expect("connect SUB");
        let sub = sub.subscribe(&client_id.as_bytes()[..]).expect("subscribe");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let waiters = Arc::new(WaiterMap::new(HashMap::new()));
        let (session_tx, _) = broadcast::channel(16);
        let (tx, rx) = oneshot::channel();
        waiters.lock().unwrap().insert(task_id, tx);

        let handle = tokio::spawn(dispatcher_loop(client_id, sub, waiters.clone(), session_tx));

        // Publish a TaskErrorEvent
        let sched_err = SchedulerError::TaskAbortedError;
        let error_fb = scheduler_error_to_flatbuffer_struct(&sched_err).unwrap();
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::TaskErrorEvent(Box::new(moor_rpc::TaskErrorEvent {
                task_id,
                error: Box::new(error_fb),
            })),
        };
        let event_bytes = build_client_event(event);

        use futures_util::SinkExt;
        let multipart = vec![client_id.as_bytes().to_vec(), event_bytes];
        let mut publisher = publisher;
        publisher
            .send(
                multipart
                    .into_iter()
                    .map(zmq::Message::from)
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("publish");

        let result = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("timeout")
            .expect("channel");

        match result {
            TaskResult::Error(e) => {
                assert!(
                    matches!(e, SchedulerError::TaskAbortedError),
                    "expected TaskAbortedError, got: {:?}",
                    e
                );
            }
            other => panic!("expected error, got: {:?}", other),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn test_dispatcher_loop_unmatched_event_ignored() {
        let client_id = Uuid::new_v4();
        let task_id_registered: u64 = 200;
        let task_id_unmatched: u64 = 999;

        let zmq_ctx = tmq::Context::new();
        let addr = format!("inproc://test-dispatcher-unmatched-{}", Uuid::new_v4());
        let publisher = tmq::publish(&zmq_ctx).bind(&addr).expect("bind PUB");

        let sub = tmq::subscribe(&zmq_ctx)
            .connect(&addr)
            .expect("connect SUB");
        let sub = sub.subscribe(&client_id.as_bytes()[..]).expect("subscribe");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let waiters = Arc::new(WaiterMap::new(HashMap::new()));
        let (session_tx, _) = broadcast::channel(16);
        let (tx, rx) = oneshot::channel();
        waiters.lock().unwrap().insert(task_id_registered, tx);

        let handle = tokio::spawn(dispatcher_loop(client_id, sub, waiters.clone(), session_tx));

        // First: publish event for a different task_id — should be ignored
        let value_fb = var_to_flatbuffer(&v_int(0)).unwrap();
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::TaskSuccessEvent(Box::new(
                moor_rpc::TaskSuccessEvent {
                    task_id: task_id_unmatched,
                    result: Box::new(value_fb),
                },
            )),
        };
        let event_bytes = build_client_event(event);

        use futures_util::SinkExt;
        let mut publisher = publisher;
        publisher
            .send(
                vec![client_id.as_bytes().to_vec(), event_bytes]
                    .into_iter()
                    .map(zmq::Message::from)
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("publish unmatched");

        // Brief sleep to ensure the unmatched event is processed
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The registered waiter should still be pending
        assert_eq!(waiters.lock().unwrap().len(), 1);

        // Now: publish event for the registered task_id
        let value_fb = var_to_flatbuffer(&v_int(7)).unwrap();
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::TaskSuccessEvent(Box::new(
                moor_rpc::TaskSuccessEvent {
                    task_id: task_id_registered,
                    result: Box::new(value_fb),
                },
            )),
        };
        let event_bytes = build_client_event(event);

        publisher
            .send(
                vec![client_id.as_bytes().to_vec(), event_bytes]
                    .into_iter()
                    .map(zmq::Message::from)
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("publish matched");

        let result = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("timeout")
            .expect("channel");

        match result {
            TaskResult::Success(v) => assert_eq!(v, v_int(7)),
            other => panic!("expected success, got: {:?}", other),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn test_dispatcher_loop_session_events() {
        let client_id = Uuid::new_v4();

        let zmq_ctx = tmq::Context::new();
        let addr = format!("inproc://test-dispatcher-session-{}", Uuid::new_v4());
        let publisher = tmq::publish(&zmq_ctx).bind(&addr).expect("bind PUB");

        let sub = tmq::subscribe(&zmq_ctx)
            .connect(&addr)
            .expect("connect SUB");
        let sub = sub.subscribe(&client_id.as_bytes()[..]).expect("subscribe");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let waiters = Arc::new(WaiterMap::new(HashMap::new()));
        let (session_tx, mut session_rx) = broadcast::channel(16);

        let handle = tokio::spawn(dispatcher_loop(client_id, sub, waiters.clone(), session_tx));

        // Publish a DisconnectEvent (simplest session event to construct)
        let event = moor_rpc::ClientEvent {
            event: moor_rpc::ClientEventUnion::DisconnectEvent(Box::new(
                moor_rpc::DisconnectEvent {},
            )),
        };
        let event_bytes = build_client_event(event);

        use futures_util::SinkExt;
        let mut publisher = publisher;
        publisher
            .send(
                vec![client_id.as_bytes().to_vec(), event_bytes]
                    .into_iter()
                    .map(zmq::Message::from)
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("publish");

        let session_event = tokio::time::timeout(Duration::from_secs(5), session_rx.recv())
            .await
            .expect("timeout")
            .expect("recv");

        assert!(matches!(session_event, SessionEvent::Disconnect));

        handle.abort();
    }
}
