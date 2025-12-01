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

//! mooR RPC client wrapper for MCP host
//!
//! This module provides a high-level interface to communicate with the mooR daemon,
//! handling authentication, connection management, and RPC call translation.

use eyre::{Result, eyre};
use moor_common::model::ObjectRef;
use moor_common::tasks::Event;
use moor_schema::convert::{narrative_event_from_ref, var_from_flatbuffer};
use moor_schema::rpc as moor_rpc;
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var};
use planus::ReadAsRoot;
use rpc_async_client::pubsub_client::events_recv;
use rpc_async_client::rpc_client::{CurveKeys, RpcClient};
use rpc_async_client::zmq;
use rpc_common::{
    AuthToken, ClientToken, mk_command_msg, mk_connection_establish_msg, mk_detach_msg,
    mk_eval_msg, mk_invoke_verb_msg, mk_list_objects_msg, mk_program_msg, mk_properties_msg,
    mk_resolve_msg, mk_retrieve_msg, mk_update_property_msg, mk_verbs_msg,
};
use std::sync::Arc;
use std::time::Duration;
use tmq::subscribe;
use tokio::time::timeout;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

/// Configuration for connecting to the mooR daemon
#[derive(Debug, Clone)]
pub struct MoorClientConfig {
    pub rpc_address: String,
    pub events_address: String,
    pub curve_keys: Option<(String, String, String)>,
}

/// mooR client for MCP host
///
/// Manages the connection to the mooR daemon and provides high-level
/// methods for interacting with the MOO world.
pub struct MoorClient {
    zmq_context: tmq::Context,
    rpc_client: RpcClient,
    config: MoorClientConfig,
    #[allow(dead_code)]
    host_id: Uuid,
    client_id: Uuid,
    client_token: Option<ClientToken>,
    auth_token: Option<AuthToken>,
    handler_object: Obj,
    player: Option<Obj>,
    /// Stored credentials for reconnection
    stored_credentials: Option<(String, String)>,
}

/// Result of a MOO task operation (command or verb invoke)
#[derive(Debug)]
pub struct TaskResult {
    /// Whether the task succeeded
    pub success: bool,
    /// Return value (for successful tasks) or error info
    pub result: Var,
    /// Narrative output collected during task execution
    pub narrative: Vec<String>,
}

/// Result of a MOO operation
#[derive(Debug)]
pub enum MoorResult {
    /// Successful result with a value
    Success(Var),
    /// Error with message
    Error(String),
}

impl MoorClient {
    /// Create a new mooR client
    pub fn new(config: MoorClientConfig) -> Result<Self> {
        let zmq_context = tmq::Context::new();
        let host_id = Uuid::new_v4();
        let client_id = Uuid::new_v4();

        let curve_keys = config
            .curve_keys
            .as_ref()
            .map(|(secret, public, server)| CurveKeys {
                client_secret: secret.clone(),
                client_public: public.clone(),
                server_public: server.clone(),
            });

        let rpc_client = RpcClient::new_with_defaults(
            Arc::new(zmq_context.clone()),
            config.rpc_address.clone(),
            curve_keys,
        );

        Ok(Self {
            zmq_context,
            rpc_client,
            config,
            host_id,
            client_id,
            client_token: None,
            auth_token: None,
            handler_object: SYSTEM_OBJECT,
            player: None,
            stored_credentials: None,
        })
    }

    /// Establish a connection to the mooR daemon
    pub async fn connect(&mut self) -> Result<()> {
        info!("Establishing connection to mooR daemon...");

        // Create a connection establish message
        let content_types = vec![moor_rpc::Symbol {
            value: "text_plain".to_string(),
        }];

        let establish_msg = mk_connection_establish_msg(
            "mcp-host".to_string(), // peer_addr
            0,                      // local_port
            0,                      // remote_port
            Some(content_types),
            Some(vec![]),
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, establish_msg)
            .await
            .map_err(|e| eyre!("Failed to establish connection: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::NewConnection(new_conn) => {
                        let client_token_ref = new_conn
                            .client_token()
                            .map_err(|e| eyre!("Missing client_token: {}", e))?;
                        self.client_token = Some(ClientToken(
                            client_token_ref
                                .token()
                                .map_err(|e| eyre!("Missing token: {}", e))?
                                .to_string(),
                        ));
                        info!("Connection established with mooR daemon");
                        Ok(())
                    }
                    other => Err(eyre!("Unexpected response: {:?}", other)),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Connection failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Authenticate as a player
    ///
    /// Stores credentials for automatic re-authentication after reconnection.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        // Store credentials for reconnection
        self.stored_credentials = Some((username.to_string(), password.to_string()));

        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected - call connect() first"))?;

        // Use login_command to authenticate
        let login_msg = rpc_common::mk_login_command_msg(
            client_token,
            &self.handler_object,
            vec![
                "connect".to_string(),
                username.to_string(),
                password.to_string(),
            ],
            true,
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, login_msg)
            .await
            .map_err(|e| eyre!("Login failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) => {
                        if login_result.success().unwrap_or(false) {
                            if let Ok(Some(auth_token_ref)) = login_result.auth_token() {
                                self.auth_token = Some(AuthToken(
                                    auth_token_ref
                                        .token()
                                        .map_err(|e| eyre!("Missing auth token: {}", e))?
                                        .to_string(),
                                ));
                            }
                            if let Ok(Some(player_ref)) = login_result.player() {
                                let player_struct = moor_rpc::Obj::try_from(player_ref)
                                    .map_err(|e| eyre!("Failed to convert player: {}", e))?;
                                self.player = Some(
                                    moor_schema::convert::obj_from_flatbuffer_struct(
                                        &player_struct,
                                    )
                                    .map_err(|e| eyre!("Failed to decode player: {}", e))?,
                                );
                            }
                            info!("Logged in as {:?}", self.player);
                            Ok(())
                        } else {
                            Err(eyre!("Login failed"))
                        }
                    }
                    _ => Err(eyre!("Unexpected login response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Login failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Check if we're authenticated
    pub fn is_authenticated(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Get the current player object
    pub fn player(&self) -> Option<&Obj> {
        self.player.as_ref()
    }

    /// Clear connection state without sending detach message
    fn clear_connection_state(&mut self) {
        self.client_token = None;
        self.auth_token = None;
        self.player = None;
    }

    /// Reconnect to the mooR daemon
    ///
    /// Clears stale connection state, re-establishes connection, and
    /// re-authenticates using stored credentials if available.
    pub async fn reconnect(&mut self) -> Result<()> {
        info!("Attempting to reconnect to mooR daemon...");

        // Clear the RPC connection pool to discard stale sockets
        self.rpc_client.clear_pool().await;

        // Clear connection state
        self.clear_connection_state();

        // Re-establish connection
        self.connect().await?;

        // Re-authenticate if we have stored credentials
        if let Some((username, password)) = self.stored_credentials.clone() {
            info!("Re-authenticating as {}...", username);
            self.login_internal(&username, &password).await?;
            info!("Successfully re-authenticated as {}", username);
        }

        Ok(())
    }

    /// Reconnect with exponential backoff
    ///
    /// Attempts to reconnect with increasing delays between attempts.
    pub async fn reconnect_with_backoff(&mut self, max_attempts: u32) -> Result<()> {
        let base_delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);

        for attempt in 1..=max_attempts {
            match self.reconnect().await {
                Ok(()) => {
                    info!("Reconnected successfully on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    if attempt == max_attempts {
                        return Err(eyre!(
                            "Failed to reconnect after {} attempts: {}",
                            max_attempts,
                            e
                        ));
                    }

                    // Calculate delay with exponential backoff
                    let delay = std::cmp::min(base_delay * 2u32.pow(attempt - 1), max_delay);
                    warn!(
                        "Reconnect attempt {} failed: {}. Retrying in {:?}...",
                        attempt, e, delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        unreachable!()
    }

    /// Internal login that doesn't store credentials (for reconnection)
    async fn login_internal(&mut self, username: &str, password: &str) -> Result<()> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected - call connect() first"))?;

        let login_msg = rpc_common::mk_login_command_msg(
            client_token,
            &self.handler_object,
            vec![
                "connect".to_string(),
                username.to_string(),
                password.to_string(),
            ],
            true,
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, login_msg)
            .await
            .map_err(|e| eyre!("Login failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) => {
                        if login_result.success().unwrap_or(false) {
                            if let Ok(Some(auth_token_ref)) = login_result.auth_token() {
                                self.auth_token = Some(AuthToken(
                                    auth_token_ref
                                        .token()
                                        .map_err(|e| eyre!("Missing auth token: {}", e))?
                                        .to_string(),
                                ));
                            }
                            if let Ok(Some(player_ref)) = login_result.player() {
                                let player_struct = moor_rpc::Obj::try_from(player_ref)
                                    .map_err(|e| eyre!("Failed to convert player: {}", e))?;
                                self.player = Some(
                                    moor_schema::convert::obj_from_flatbuffer_struct(
                                        &player_struct,
                                    )
                                    .map_err(|e| eyre!("Failed to decode player: {}", e))?,
                                );
                            }
                            Ok(())
                        } else {
                            Err(eyre!("Login failed"))
                        }
                    }
                    _ => Err(eyre!("Unexpected login response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Login failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Evaluate a MOO expression
    pub async fn eval(&mut self, expression: &str) -> Result<MoorResult> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?;
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let eval_msg = mk_eval_msg(client_token, auth_token, expression.to_string());

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, eval_msg)
            .await
            .map_err(|e| eyre!("Eval failed: {}", e))?;

        self.parse_eval_result(&reply_bytes)
    }

    /// Execute a command as the player and wait for completion with narrative output
    pub async fn command(&mut self, command: &str) -> Result<TaskResult> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?
            .clone();
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?
            .clone();

        // Subscribe to events BEFORE submitting the task
        let mut events_sub = self.create_events_subscriber().await?;

        // Give ZMQ subscription time to establish (slow joiner problem)
        tokio::time::sleep(Duration::from_millis(10)).await;

        let cmd_msg = mk_command_msg(
            &client_token,
            &auth_token,
            &self.handler_object,
            command.to_string(),
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, cmd_msg)
            .await
            .map_err(|e| eyre!("Command failed: {}", e))?;

        // Extract task_id from TaskSubmitted response
        let task_id = self.extract_task_id(&reply_bytes)?;
        debug!("Command submitted as task {}", task_id);

        // Wait for task completion with 60 second timeout
        match timeout(
            Duration::from_secs(60),
            self.wait_for_task_completion(&mut events_sub, task_id),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(eyre!("Task {} timed out after 60 seconds", task_id)),
        }
    }

    /// Invoke a verb on an object and wait for completion with narrative output
    pub async fn invoke_verb(
        &mut self,
        object: &ObjectRef,
        verb: &str,
        args: Vec<Var>,
    ) -> Result<TaskResult> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?
            .clone();
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?
            .clone();

        // Subscribe to events BEFORE submitting the task
        let mut events_sub = self.create_events_subscriber().await?;

        // Give ZMQ subscription time to establish (slow joiner problem)
        tokio::time::sleep(Duration::from_millis(10)).await;

        let verb_sym = Symbol::mk(verb);
        let args_refs: Vec<&Var> = args.iter().collect();

        let invoke_msg =
            mk_invoke_verb_msg(&client_token, &auth_token, object, &verb_sym, args_refs)
                .ok_or_else(|| eyre!("Failed to create invoke message"))?;

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, invoke_msg)
            .await
            .map_err(|e| eyre!("Invoke failed: {}", e))?;

        // Extract task_id from TaskSubmitted response
        let task_id = self.extract_task_id(&reply_bytes)?;
        debug!("Verb invoke submitted as task {}", task_id);

        // Wait for task completion with 60 second timeout
        match timeout(
            Duration::from_secs(60),
            self.wait_for_task_completion(&mut events_sub, task_id),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(eyre!("Task {} timed out after 60 seconds", task_id)),
        }
    }

    /// Create an events subscriber for this client
    async fn create_events_subscriber(&self) -> Result<tmq::subscribe::Subscribe> {
        let mut socket_builder = subscribe(&self.zmq_context);

        // Configure CURVE encryption if keys provided
        if let Some((client_secret, client_public, server_public)) = &self.config.curve_keys {
            let client_secret_bytes =
                zmq::z85_decode(client_secret).map_err(|_| eyre!("Invalid client secret key"))?;
            let client_public_bytes =
                zmq::z85_decode(client_public).map_err(|_| eyre!("Invalid client public key"))?;
            let server_public_bytes =
                zmq::z85_decode(server_public).map_err(|_| eyre!("Invalid server public key"))?;

            socket_builder = socket_builder
                .set_curve_secretkey(&client_secret_bytes)
                .set_curve_publickey(&client_public_bytes)
                .set_curve_serverkey(&server_public_bytes);
        }

        let events_sub = socket_builder
            .connect(&self.config.events_address)
            .map_err(|e| eyre!("Unable to connect events subscriber: {}", e))?;

        let events_sub = events_sub
            .subscribe(&self.client_id.as_bytes()[..])
            .map_err(|e| eyre!("Unable to subscribe to client events: {}", e))?;

        Ok(events_sub)
    }

    /// Extract task_id from TaskSubmitted response
    fn extract_task_id(&self, reply_bytes: &[u8]) -> Result<u64> {
        let reply = moor_rpc::ReplyResultRef::read_as_root(reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task) => {
                        Ok(task.task_id().unwrap_or(0))
                    }
                    other => Err(eyre!("Expected TaskSubmitted, got {:?}", other)),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Task submission failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Wait for task completion, collecting narrative messages along the way
    async fn wait_for_task_completion(
        &self,
        events_sub: &mut tmq::subscribe::Subscribe,
        task_id: u64,
    ) -> Result<TaskResult> {
        let mut narrative = Vec::new();

        loop {
            let event_msg = events_recv(self.client_id, events_sub)
                .await
                .map_err(|e| eyre!("Error receiving event: {}", e))?;

            let event = event_msg
                .event()
                .map_err(|e| eyre!("Failed to parse event: {}", e))?;

            let event_ref = event
                .event()
                .map_err(|e| eyre!("Failed to parse event union: {}", e))?;

            match event_ref {
                moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative_msg) => {
                    if let Ok(event_ref) = narrative_msg.event()
                        && let Ok(narrative_event) = narrative_event_from_ref(event_ref)
                        && let Some(text) = extract_narrative_text(&narrative_event.event)
                    {
                        trace!("Narrative: {}", text);
                        narrative.push(text);
                    }
                }
                moor_rpc::ClientEventUnionRef::SystemMessageEvent(sys_msg) => {
                    if let Ok(msg) = sys_msg.message() {
                        trace!("System message: {}", msg);
                        narrative.push(msg.to_string());
                    }
                }
                moor_rpc::ClientEventUnionRef::TaskSuccessEvent(success) => {
                    if let Ok(event_task_id) = success.task_id()
                        && event_task_id == task_id
                    {
                        debug!("Task {} completed successfully", task_id);
                        // Extract result value
                        let result = success
                            .result()
                            .ok()
                            .and_then(|result_ref| moor_schema::var::Var::try_from(result_ref).ok())
                            .and_then(|result_struct| var_from_flatbuffer(&result_struct).ok())
                            .unwrap_or(moor_var::v_none());
                        return Ok(TaskResult {
                            success: true,
                            result,
                            narrative,
                        });
                    }
                }
                moor_rpc::ClientEventUnionRef::TaskErrorEvent(error_event) => {
                    if let Ok(event_task_id) = error_event.task_id()
                        && event_task_id == task_id
                    {
                        let error_msg = error_event
                            .error()
                            .ok()
                            .and_then(|error_ref| {
                                rpc_common::scheduler_error_from_ref(error_ref).ok()
                            })
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "Unknown error".to_string());
                        warn!("Task {} failed: {}", task_id, error_msg);
                        return Ok(TaskResult {
                            success: false,
                            result: moor_var::v_str(&error_msg),
                            narrative,
                        });
                    }
                }
                _ => {
                    // Ignore other event types
                    trace!("Ignoring event type");
                }
            }
        }
    }

    /// List verbs on an object
    pub async fn list_verbs(
        &mut self,
        object: &ObjectRef,
        inherited: bool,
    ) -> Result<Vec<VerbInfo>> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let verbs_msg = mk_verbs_msg(auth_token, object, inherited);

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, verbs_msg)
            .await
            .map_err(|e| eyre!("List verbs failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::VerbsReply(verbs_reply) => {
                        let verbs = verbs_reply
                            .verbs()
                            .map_err(|e| eyre!("Missing verbs: {}", e))?;
                        let mut result = Vec::new();
                        for verb in verbs.iter().flatten() {
                            // Get names as concatenated string
                            let name = verb
                                .names()
                                .ok()
                                .map(|names| {
                                    names
                                        .iter()
                                        .flatten()
                                        .filter_map(|s| s.value().ok())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                                .unwrap_or_default();
                            // Build flags string from booleans (rwxd format)
                            let r = verb.r().ok().unwrap_or(false);
                            let w = verb.w().ok().unwrap_or(false);
                            let x = verb.x().ok().unwrap_or(false);
                            let d = verb.d().ok().unwrap_or(false);
                            let mut flags = String::new();
                            if r {
                                flags.push('r');
                            }
                            if w {
                                flags.push('w');
                            }
                            if x {
                                flags.push('x');
                            }
                            if d {
                                flags.push('d');
                            }
                            if flags.is_empty() {
                                flags.push_str("none");
                            }
                            // Get arg_spec as human-readable string (e.g., "this none this")
                            let args = verb
                                .arg_spec()
                                .ok()
                                .map(|spec| {
                                    spec.iter()
                                        .flatten()
                                        .filter_map(|sym| sym.value().ok().map(|v| v.to_string()))
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                                .unwrap_or_else(|| "none none none".to_string());
                            result.push(VerbInfo {
                                name,
                                owner: verb
                                    .owner()
                                    .ok()
                                    .map(|o| format!("{:?}", o))
                                    .unwrap_or_default(),
                                flags,
                                args,
                            });
                        }
                        Ok(result)
                    }
                    _ => Err(eyre!("Unexpected verbs response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("List verbs failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Get verb code
    pub async fn get_verb(&mut self, object: &ObjectRef, verb_name: &str) -> Result<VerbCode> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let retrieve_msg = mk_retrieve_msg(
            auth_token,
            object,
            moor_rpc::EntityType::Verb,
            &Symbol::mk(verb_name),
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, retrieve_msg)
            .await
            .map_err(|e| eyre!("Get verb failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::VerbValue(verb_value) => {
                        let code = verb_value
                            .code()
                            .map_err(|e| eyre!("Missing code: {}", e))?
                            .iter()
                            .filter_map(|s| s.ok().map(|s| s.to_string()))
                            .collect();
                        Ok(VerbCode {
                            name: verb_name.to_string(),
                            code,
                        })
                    }
                    _ => Err(eyre!("Unexpected verb response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Get verb failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Program (compile/save) a verb
    pub async fn program_verb(
        &mut self,
        object: &ObjectRef,
        verb_name: &str,
        code: Vec<String>,
    ) -> Result<()> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?;
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let program_msg = mk_program_msg(
            client_token,
            auth_token,
            object,
            &Symbol::mk(verb_name),
            code,
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, program_msg)
            .await
            .map_err(|e| eyre!("Program verb failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::VerbProgramResponseReply(response) => {
                        let resp = response
                            .response()
                            .map_err(|e| eyre!("Missing response: {}", e))?;
                        match resp
                            .response()
                            .map_err(|e| eyre!("Missing response union: {}", e))?
                        {
                            moor_rpc::VerbProgramResponseUnionRef::VerbProgramSuccess(_) => Ok(()),
                            moor_rpc::VerbProgramResponseUnionRef::VerbProgramFailure(failure) => {
                                Err(eyre!("Verb program failed: {:?}", failure))
                            }
                        }
                    }
                    _ => Err(eyre!("Unexpected program response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Program verb failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// List properties on an object
    pub async fn list_properties(
        &mut self,
        object: &ObjectRef,
        inherited: bool,
    ) -> Result<Vec<PropertyInfo>> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let props_msg = mk_properties_msg(auth_token, object, inherited);

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, props_msg)
            .await
            .map_err(|e| eyre!("List properties failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::PropertiesReply(props_reply) => {
                        let props = props_reply
                            .properties()
                            .map_err(|e| eyre!("Missing properties: {}", e))?;
                        let mut result = Vec::new();
                        for prop in props.iter().flatten() {
                            // Build flags string from booleans (rwc format)
                            let r = prop.r().ok().unwrap_or(false);
                            let w = prop.w().ok().unwrap_or(false);
                            let chown = prop.chown().ok().unwrap_or(false);
                            let mut flags = String::new();
                            if r {
                                flags.push('r');
                            }
                            if w {
                                flags.push('w');
                            }
                            if chown {
                                flags.push('c');
                            }
                            if flags.is_empty() {
                                flags.push_str("none");
                            }
                            result.push(PropertyInfo {
                                name: prop
                                    .name()
                                    .ok()
                                    .and_then(|s| s.value().ok())
                                    .unwrap_or_default()
                                    .to_string(),
                                owner: prop
                                    .owner()
                                    .ok()
                                    .map(|o| format!("{:?}", o))
                                    .unwrap_or_default(),
                                flags,
                            });
                        }
                        Ok(result)
                    }
                    _ => Err(eyre!("Unexpected properties response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("List properties failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Get a property value
    pub async fn get_property(&mut self, object: &ObjectRef, prop_name: &str) -> Result<Var> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let retrieve_msg = mk_retrieve_msg(
            auth_token,
            object,
            moor_rpc::EntityType::Property,
            &Symbol::mk(prop_name),
        );

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, retrieve_msg)
            .await
            .map_err(|e| eyre!("Get property failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::PropertyValue(prop_value) => {
                        let value_ref = prop_value
                            .value()
                            .map_err(|e| eyre!("Missing value: {}", e))?;
                        let value_struct = moor_schema::var::Var::try_from(value_ref)
                            .map_err(|e| eyre!("Failed to convert value: {}", e))?;
                        var_from_flatbuffer(&value_struct)
                            .map_err(|e| eyre!("Failed to decode value: {}", e))
                    }
                    _ => Err(eyre!("Unexpected property response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Get property failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Update a property value
    pub async fn set_property(
        &mut self,
        object: &ObjectRef,
        prop_name: &str,
        value: &Var,
    ) -> Result<()> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let update_msg = mk_update_property_msg(auth_token, object, &Symbol::mk(prop_name), value)
            .ok_or_else(|| eyre!("Failed to create update message"))?;

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, update_msg)
            .await
            .map_err(|e| eyre!("Set property failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(_) => Ok(()),
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Set property failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// List all objects in the database
    pub async fn list_objects(&mut self) -> Result<Vec<ObjectInfo>> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let list_msg = mk_list_objects_msg(auth_token);

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, list_msg)
            .await
            .map_err(|e| eyre!("List objects failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::ListObjectsReply(list_reply) => {
                        let objects = list_reply
                            .objects()
                            .map_err(|e| eyre!("Missing objects: {}", e))?;
                        let mut result = Vec::new();
                        for obj in objects.iter().flatten() {
                            // Format object flags using MOO builtin property names
                            let flag_bits = obj.flags().ok().unwrap_or(0);
                            let mut flag_parts = Vec::new();
                            if flag_bits & 0x01 != 0 {
                                flag_parts.push("player");
                            }
                            if flag_bits & 0x02 != 0 {
                                flag_parts.push("programmer");
                            }
                            if flag_bits & 0x04 != 0 {
                                flag_parts.push("wizard");
                            }
                            if flag_bits & 0x10 != 0 {
                                flag_parts.push("r");
                            }
                            if flag_bits & 0x20 != 0 {
                                flag_parts.push("w");
                            }
                            if flag_bits & 0x80 != 0 {
                                flag_parts.push("f");
                            }
                            let flags = if flag_parts.is_empty() {
                                "none".to_string()
                            } else {
                                flag_parts.join(" ")
                            };
                            result.push(ObjectInfo {
                                obj: obj.obj().map(|o| format!("{:?}", o)).unwrap_or_default(),
                                name: obj
                                    .name()
                                    .ok()
                                    .flatten()
                                    .and_then(|s| s.value().ok())
                                    .unwrap_or_default()
                                    .to_string(),
                                flags,
                            });
                        }
                        Ok(result)
                    }
                    _ => Err(eyre!("Unexpected list objects response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("List objects failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Resolve an object reference
    #[allow(dead_code)]
    pub async fn resolve(&mut self, objref: &ObjectRef) -> Result<Var> {
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let resolve_msg = mk_resolve_msg(auth_token, objref);

        let reply_bytes = self
            .rpc_client
            .make_client_rpc_call(self.client_id, resolve_msg)
            .await
            .map_err(|e| eyre!("Resolve failed: {}", e))?;

        let reply = moor_rpc::ReplyResultRef::read_as_root(&reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::ResolveResult(resolve_result) => {
                        let result_ref = resolve_result
                            .result()
                            .map_err(|e| eyre!("Missing result: {}", e))?;
                        let result_struct = moor_schema::var::Var::try_from(result_ref)
                            .map_err(|e| eyre!("Failed to convert result: {}", e))?;
                        var_from_flatbuffer(&result_struct)
                            .map_err(|e| eyre!("Failed to decode result: {}", e))
                    }
                    _ => Err(eyre!("Unexpected resolve response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                Err(eyre!("Resolve failed: {}", error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Disconnect from the daemon
    #[allow(dead_code)]
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(client_token) = &self.client_token {
            let detach_msg = mk_detach_msg(client_token, true);
            let _ = self
                .rpc_client
                .make_client_rpc_call(self.client_id, detach_msg)
                .await;
        }
        self.client_token = None;
        self.auth_token = None;
        self.player = None;
        Ok(())
    }

    /// Parse eval result from reply bytes
    fn parse_eval_result(&self, reply_bytes: &[u8]) -> Result<MoorResult> {
        let reply = moor_rpc::ReplyResultRef::read_as_root(reply_bytes)
            .map_err(|e| eyre!("Failed to parse reply: {}", e))?;

        match reply.result().map_err(|e| eyre!("Missing result: {}", e))? {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::EvalResult(eval_result) => {
                        let result_ref = eval_result
                            .result()
                            .map_err(|e| eyre!("Missing result: {}", e))?;
                        let result_struct = moor_schema::var::Var::try_from(result_ref)
                            .map_err(|e| eyre!("Failed to convert result: {}", e))?;
                        let var = var_from_flatbuffer(&result_struct)
                            .map_err(|e| eyre!("Failed to decode result: {}", e))?;
                        Ok(MoorResult::Success(var))
                    }
                    _ => Err(eyre!("Unexpected eval response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let error_info = failure.error().ok();
                let error = if let Some(err) = error_info {
                    let code = err
                        .error_code()
                        .ok()
                        .map(|c| format!("{:?}", c))
                        .unwrap_or_else(|| "Unknown".to_string());
                    let message = err.message().ok().flatten().map(|s| s.to_string());
                    // Check if there's a scheduler error with more details
                    let sched_err = err.scheduler_error().ok().flatten().and_then(|se| {
                        // Try to get the error variant name
                        se.error().ok().map(|e| format!("{:?}", e))
                    });

                    match (message, sched_err) {
                        (Some(msg), Some(sched)) => format!("{}: {} ({})", code, msg, sched),
                        (Some(msg), None) => format!("{}: {}", code, msg),
                        (None, Some(sched)) => format!("{}: {}", code, sched),
                        (None, None) => code,
                    }
                } else {
                    "unknown error".to_string()
                };
                Ok(MoorResult::Error(error))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }
}

/// Information about a verb
#[derive(Debug, Clone)]
pub struct VerbInfo {
    pub name: String,
    pub owner: String,
    pub flags: String,
    pub args: String,
}

/// Verb code
#[derive(Debug, Clone)]
pub struct VerbCode {
    pub name: String,
    pub code: Vec<String>,
}

/// Information about a property
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub name: String,
    pub owner: String,
    pub flags: String,
}

/// Information about an object
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub obj: String,
    pub name: String,
    pub flags: String,
}

/// Extract text content from a narrative Event
fn extract_narrative_text(event: &Event) -> Option<String> {
    match event {
        Event::Notify { value, .. } => {
            // Convert the Var value to a string representation
            Some(format_var_for_narrative(value))
        }
        Event::Traceback(exception) => {
            // Format exception as text
            Some(format!("** {} **", exception.error))
        }
        Event::Present(_) | Event::Unpresent(_) => {
            // These are UI-related events, not text output
            None
        }
    }
}

/// Format a Var for narrative output
fn format_var_for_narrative(var: &Var) -> String {
    use moor_var::Variant;
    match var.variant() {
        Variant::Str(s) => s.to_string(),
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => f.to_string(),
        Variant::Obj(o) => format!("{}", o),
        Variant::List(l) => {
            let items: Vec<String> = l.iter().map(|v| format_var_for_narrative(&v)).collect();
            format!("{{{}}}", items.join(", "))
        }
        Variant::Map(m) => {
            let items: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{} -> {}",
                        format_var_for_narrative(&k),
                        format_var_for_narrative(&v)
                    )
                })
                .collect();
            format!("[{}]", items.join(", "))
        }
        Variant::Err(e) => format!("{}", e),
        Variant::None => "".to_string(),
        Variant::Sym(s) => format!("'{}", s.as_string()),
        Variant::Binary(b) => format!("~<{} bytes>~", b.as_bytes().len()),
        Variant::Lambda(_) => "*lambda*".to_string(),
        Variant::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Variant::Flyweight(f) => format!("{:?}", f),
    }
}
