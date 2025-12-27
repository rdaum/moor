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

//! mooR RPC client for LSP server
//!
//! Provides connection to the mooR daemon for server-side operations like
//! verb compilation, property resolution, and sysprop lookups.

use eyre::{Result, eyre};
use moor_schema::convert::var_from_flatbuffer_ref;
use moor_schema::rpc as moor_rpc;
use moor_var::{Obj, SYSTEM_OBJECT, Var};
use rpc_async_client::rpc_client::{CurveKeys, RpcClient};
use rpc_common::{AuthToken, ClientToken, read_reply_result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};
use uuid::Uuid;

/// Timeout for RPC operations
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// Configuration for connecting to the mooR daemon
#[derive(Debug, Clone)]
pub struct MoorClientConfig {
    pub rpc_address: String,
    #[allow(dead_code)]
    pub events_address: String,
    pub curve_keys: Option<(String, String, String)>,
}

/// mooR client for LSP
///
/// Simplified client focused on LSP needs: eval, verb retrieval, sysprop resolution.
pub struct MoorLspClient {
    rpc_client: RpcClient,
    config: MoorClientConfig,
    client_id: Uuid,
    client_token: Option<ClientToken>,
    auth_token: Option<AuthToken>,
    player: Option<Obj>,
    stored_credentials: Option<(String, String)>,
}

impl MoorLspClient {
    /// Create a new client (not yet connected)
    pub fn new(config: MoorClientConfig) -> Result<Self> {
        let zmq_context = tmq::Context::new();
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
            Arc::new(zmq_context),
            config.rpc_address.clone(),
            curve_keys,
        );

        Ok(Self {
            rpc_client,
            config,
            client_id,
            client_token: None,
            auth_token: None,
            player: None,
            stored_credentials: None,
        })
    }

    /// Make an RPC call with timeout
    async fn rpc_call(
        &self,
        msg: moor_rpc::HostClientToDaemonMessage,
        operation: &str,
    ) -> Result<Vec<u8>> {
        match timeout(
            RPC_TIMEOUT,
            self.rpc_client.make_client_rpc_call(self.client_id, msg),
        )
        .await
        {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(rpc_error)) => {
                let error_msg = format!("{} failed: {:?}", operation, rpc_error);
                error!("{}", error_msg);
                Err(eyre!(error_msg))
            }
            Err(_elapsed) => {
                let error_msg = format!(
                    "{} timed out after {:?}. The mooR daemon at {} may be down.",
                    operation, RPC_TIMEOUT, self.config.rpc_address
                );
                error!("{}", error_msg);
                Err(eyre!(error_msg))
            }
        }
    }

    /// Establish a connection to the mooR daemon
    pub async fn connect(&mut self) -> Result<()> {
        info!(
            "LSP: Connecting to mooR daemon at {}...",
            self.config.rpc_address
        );

        let content_types = vec![moor_rpc::Symbol {
            value: "text_plain".to_string(),
        }];

        let establish_msg = rpc_common::mk_connection_establish_msg(
            "moor-lsp".to_string(),
            0,
            0,
            Some(content_types),
            Some(vec![]),
        );

        let reply_bytes = self.rpc_call(establish_msg, "Connection").await?;

        let reply =
            read_reply_result(&reply_bytes).map_err(|e| eyre!("Failed to parse reply: {}", e))?;

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
                        info!("LSP: Connection established");
                        Ok(())
                    }
                    other => Err(eyre!("Unexpected response: {:?}", other)),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let msg = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                Err(eyre!("Connection failed: {}", msg))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Authenticate as a player
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        self.stored_credentials = Some((username.to_string(), password.to_string()));

        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?;

        let login_msg = rpc_common::mk_login_command_msg(
            client_token,
            &SYSTEM_OBJECT,
            vec![
                "connect".to_string(),
                username.to_string(),
                password.to_string(),
            ],
            true,
            None,
            None,
        );

        let reply_bytes = self
            .rpc_call(login_msg, &format!("Login as '{}'", username))
            .await?;

        let reply =
            read_reply_result(&reply_bytes).map_err(|e| eyre!("Failed to parse reply: {}", e))?;

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
                        if !login_result.success().unwrap_or(false) {
                            return Err(eyre!("Login failed"));
                        }
                        if let Ok(Some(auth_token_ref)) = login_result.auth_token() {
                            self.auth_token = Some(AuthToken(
                                auth_token_ref
                                    .token()
                                    .map_err(|e| eyre!("Missing auth token: {}", e))?
                                    .to_string(),
                            ));
                        }
                        if let Ok(Some(player_ref)) = login_result.player() {
                            self.player = Some(
                                moor_schema::convert::obj_from_ref(player_ref)
                                    .map_err(|e| eyre!("Failed to decode player: {}", e))?,
                            );
                        }
                        info!("LSP: Authenticated as {} ({:?})", username, self.player);
                        Ok(())
                    }
                    _ => Err(eyre!("Unexpected login response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let msg = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                Err(eyre!("Login failed: {}", msg))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Evaluate MOO code on the server
    #[allow(dead_code)]
    pub async fn eval(&self, code: &str) -> Result<Var> {
        let client_token = self
            .client_token
            .as_ref()
            .ok_or_else(|| eyre!("Not connected"))?;
        let auth_token = self
            .auth_token
            .as_ref()
            .ok_or_else(|| eyre!("Not authenticated"))?;

        let eval_msg = rpc_common::mk_eval_msg(client_token, auth_token, code.to_string());

        let reply_bytes = self.rpc_call(eval_msg, "Eval").await?;

        let reply =
            read_reply_result(&reply_bytes).map_err(|e| eyre!("Failed to parse reply: {}", e))?;

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
                        var_from_flatbuffer_ref(result_ref)
                            .map_err(|e| eyre!("Failed to decode result: {}", e))
                    }
                    _ => Err(eyre!("Unexpected eval response")),
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let msg = failure
                    .error()
                    .ok()
                    .and_then(|e| e.message().ok().flatten())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                Err(eyre!("Eval failed: {}", msg))
            }
            _ => Err(eyre!("Unexpected response type")),
        }
    }

    /// Check if authenticated
    #[allow(dead_code)]
    pub fn is_authenticated(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Get the authenticated player
    #[allow(dead_code)]
    pub fn player(&self) -> Option<&Obj> {
        self.player.as_ref()
    }

    /// Check connection health
    #[allow(dead_code)]
    pub async fn check_connection(&self) -> Result<()> {
        if self.client_token.is_none() {
            return Err(eyre!("Not connected"));
        }
        if self.auth_token.is_none() {
            return Err(eyre!("Not authenticated"));
        }

        // Simple eval to verify connection
        self.eval("return 1;").await?;
        Ok(())
    }

    /// Reconnect with stored credentials
    #[allow(dead_code)]
    pub async fn reconnect(&mut self) -> Result<()> {
        info!("LSP: Reconnecting...");
        self.rpc_client.clear_pool().await;
        self.client_token = None;
        self.auth_token = None;
        self.player = None;

        self.connect().await?;

        if let Some((username, password)) = self.stored_credentials.clone() {
            self.login(&username, &password).await?;
        }

        Ok(())
    }
}
