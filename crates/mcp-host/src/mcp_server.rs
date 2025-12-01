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

//! MCP Server implementation
//!
//! This module implements the Model Context Protocol server that communicates
//! over stdio using JSON-RPC 2.0.

use crate::mcp_types::*;
use crate::moor_client::MoorClient;
use crate::{prompts, resources, tools};
use eyre::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// MCP protocol version we support
const PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP Server state
pub struct McpServer {
    client: MoorClient,
    initialized: bool,
    /// Credentials to use after connection is established
    pending_credentials: Option<(String, String)>,
}

impl McpServer {
    /// Create a new MCP server with a mooR client
    pub fn new(client: MoorClient) -> Self {
        Self {
            client,
            initialized: false,
            pending_credentials: None,
        }
    }

    /// Set credentials to use after connection is established
    pub fn set_credentials(&mut self, username: String, password: String) {
        self.pending_credentials = Some((username, password));
    }

    /// Run the MCP server over stdio
    pub async fn run_stdio(&mut self) -> Result<()> {
        info!("Starting MCP server on stdio");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                info!("EOF on stdin, shutting down");
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            // Parse the JSON-RPC message
            let response = match serde_json::from_str::<Value>(line) {
                Ok(msg) => self.handle_message(msg).await,
                Err(e) => {
                    error!("Failed to parse JSON: {}", e);
                    Some(JsonRpcResponse::error(
                        RequestId::Number(0),
                        JsonRpcError::parse_error(e.to_string()),
                    ))
                }
            };

            // Send response if we have one (notifications don't get responses)
            if let Some(resp) = response {
                let response_json = serde_json::to_string(&resp)?;
                debug!("Sending: {}", response_json);
                stdout.write_all(response_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    /// Handle an incoming JSON-RPC message
    async fn handle_message(&mut self, msg: Value) -> Option<JsonRpcResponse> {
        // Check if this is a notification (no id) or request (has id)
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str());
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        let method = match method {
            Some(m) => m,
            None => {
                return id.map(|id| {
                    JsonRpcResponse::error(
                        parse_request_id(&id),
                        JsonRpcError::invalid_request("Missing method"),
                    )
                });
            }
        };

        debug!("Handling method: {}", method);

        // Handle the method
        let result = match method {
            // Lifecycle methods
            "initialize" => self.handle_initialize(&params).await,
            "initialized" => {
                self.initialized = true;
                info!("Client initialized");
                return None; // Notification, no response
            }
            "shutdown" => {
                info!("Shutdown requested");
                Ok(json!({}))
            }

            // Tool methods
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tools_call(&params).await,

            // Resource methods
            "resources/list" => self.handle_resources_list().await,
            "resources/read" => self.handle_resources_read(&params).await,

            // Prompt methods
            "prompts/list" => self.handle_prompts_list().await,
            "prompts/get" => self.handle_prompts_get(&params).await,

            // Ping
            "ping" => Ok(json!({})),

            // Unknown method
            _ => {
                warn!("Unknown method: {}", method);
                Err(JsonRpcError::method_not_found(method))
            }
        };

        // Convert result to response
        let request_id = id
            .map(|id| parse_request_id(&id))
            .unwrap_or(RequestId::Number(0));

        Some(match result {
            Ok(value) => JsonRpcResponse::success(request_id, value),
            Err(error) => JsonRpcResponse::error(request_id, error),
        })
    }

    /// Handle initialize request
    async fn handle_initialize(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let _init_params: InitializeParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        info!("Initializing MCP server");

        // Try to connect to mooR daemon
        if let Err(e) = self.client.connect().await {
            warn!(
                "Failed to connect to mooR daemon: {} - continuing without connection",
                e
            );
        } else {
            // Connection succeeded - try to login with pending credentials
            if let Some((username, password)) = self.pending_credentials.take() {
                info!("Authenticating as {}...", username);
                if let Err(e) = self.client.login(&username, &password).await {
                    error!("Failed to authenticate: {}", e);
                } else {
                    info!("Successfully authenticated as {}", username);
                }
            }
        }

        let result = InitializeResult {
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: false,
                }),
                resources: Some(ResourcesCapability {
                    subscribe: false,
                    list_changed: false,
                }),
                prompts: Some(PromptsCapability {
                    list_changed: false,
                }),
            },
            server_info: ServerInfo {
                name: "moor-mcp-host".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        let result = ToolsListResult {
            tools: tools::get_tools(),
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle tools/call request
    async fn handle_tools_call(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let call_params: ToolCallParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        // Check if we need authentication for this tool
        if !self.client.is_authenticated() && needs_auth(&call_params.name) {
            return Err(JsonRpcError::internal_error(
                "Not authenticated. Use moo_login tool first or configure credentials.",
            ));
        }

        let result =
            tools::execute_tool(&mut self.client, &call_params.name, &call_params.arguments)
                .await
                .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle resources/list request
    async fn handle_resources_list(&self) -> Result<Value, JsonRpcError> {
        let result = ResourcesListResult {
            resources: resources::get_resources(),
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle resources/read request
    async fn handle_resources_read(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let read_params: ResourceReadParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        if !self.client.is_authenticated() {
            return Err(JsonRpcError::internal_error(
                "Not authenticated. Configure credentials to browse resources.",
            ));
        }

        let result = resources::read_resource(&mut self.client, &read_params.uri)
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle prompts/list request
    async fn handle_prompts_list(&self) -> Result<Value, JsonRpcError> {
        let result = PromptsListResponse {
            prompts: prompts::get_prompts(),
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle prompts/get request
    async fn handle_prompts_get(&self, params: &Value) -> Result<Value, JsonRpcError> {
        let get_params: PromptGetParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        let result = prompts::get_prompt(&get_params.name)
            .ok_or_else(|| JsonRpcError::invalid_params(format!("Unknown prompt: {}", get_params.name)))?;

        Ok(serde_json::to_value(result).unwrap())
    }

    /// Login to the mooR daemon
    #[allow(dead_code)]
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        self.client.login(username, password).await
    }
}

/// Check if a tool requires authentication
fn needs_auth(tool_name: &str) -> bool {
    // These tools work without authentication (sort of)
    !matches!(tool_name, "moo_resolve")
}

/// Parse a request ID from JSON value
fn parse_request_id(value: &Value) -> RequestId {
    match value {
        Value::String(s) => RequestId::String(s.clone()),
        Value::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0)),
        _ => RequestId::Number(0),
    }
}
