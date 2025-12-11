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

use crate::connection::ConnectionManager;
use crate::mcp_types::*;
use crate::{prompts, resources, tools};
use eyre::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// MCP protocol version we support
const PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP Server state
pub struct McpServer {
    connections: ConnectionManager,
    initialized: bool,
    shutdown_requested: bool,
}

impl McpServer {
    /// Create a new MCP server with a connection manager
    pub fn new(connections: ConnectionManager) -> Self {
        Self {
            connections,
            initialized: false,
            shutdown_requested: false,
        }
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

            // Exit after responding to shutdown request
            if self.shutdown_requested {
                info!("Shutdown complete");
                break;
            }
        }

        // Gracefully disconnect from daemon
        self.connections.disconnect_all().await;

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
                self.shutdown_requested = true;
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

        // Log configured connections (connections are established lazily on first use)
        if self.connections.has_programmer_credentials() {
            info!("Programmer connection configured (will connect on first use)");
        }
        if self.connections.has_wizard_credentials() {
            info!("Wizard connection configured (will connect on first use)");
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
    ///
    /// Attempts to execute the tool, with automatic reconnection on connection failures.
    async fn handle_tools_call(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let call_params: ToolCallParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        // Check if this tool requires wizard privileges
        let is_wizard_only = tools::WIZARD_ONLY_TOOLS.contains(&call_params.name.as_str());

        // Extract wizard flag from arguments (defaults to false, unless wizard-only)
        let wizard = if is_wizard_only {
            true
        } else {
            call_params
                .arguments
                .get("wizard")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };

        if wizard {
            if is_wizard_only {
                debug!("Tool {} requires wizard privileges", call_params.name);
            } else {
                warn!(
                    "Tool {} called with wizard privileges - use with caution!",
                    call_params.name
                );
            }
        }

        // Get the appropriate client (lazy connection)
        let client = self
            .connections
            .get(wizard)
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        // Try to execute the tool
        let result = tools::execute_tool(client, &call_params.name, &call_params.arguments).await;

        match result {
            Ok(result) => Ok(serde_json::to_value(result).unwrap()),
            Err(e) => {
                let error_str = e.to_string();

                // Check if this looks like a connection error
                if Self::is_connection_error(&error_str) {
                    warn!(
                        "Tool call failed with connection error, attempting reconnect: {}",
                        error_str
                    );

                    // Attempt to reconnect with backoff
                    match self.connections.reconnect(wizard).await {
                        Ok(()) => {
                            info!("Reconnected successfully, retrying tool call");

                            // Get client again after reconnect
                            let client = self
                                .connections
                                .get(wizard)
                                .await
                                .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

                            // Retry the tool call
                            let retry_result = tools::execute_tool(
                                client,
                                &call_params.name,
                                &call_params.arguments,
                            )
                            .await
                            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

                            Ok(serde_json::to_value(retry_result).unwrap())
                        }
                        Err(reconnect_err) => {
                            error!("Reconnection failed: {}", reconnect_err);
                            Err(JsonRpcError::internal_error(format!(
                                "Connection lost and reconnection failed: {}. Original error: {}",
                                reconnect_err, error_str
                            )))
                        }
                    }
                } else {
                    Err(JsonRpcError::internal_error(e.to_string()))
                }
            }
        }
    }

    /// Check if an error message indicates a connection problem
    fn is_connection_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        error_lower.contains("could not send")
            || error_lower.contains("could not receive")
            || error_lower.contains("connection")
            || error_lower.contains("timeout")
            || error_lower.contains("not connected")
            || error_lower.contains("host unreachable")
            || error_lower.contains("network")
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

        // Resources always use programmer connection (read-only browsing)
        let client = self
            .connections
            .programmer()
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let result = resources::read_resource(client, &read_params.uri)
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

        let result = prompts::get_prompt(&get_params.name).ok_or_else(|| {
            JsonRpcError::invalid_params(format!("Unknown prompt: {}", get_params.name))
        })?;

        Ok(serde_json::to_value(result).unwrap())
    }
}

/// Parse a request ID from JSON value
fn parse_request_id(value: &Value) -> RequestId {
    match value {
        Value::String(s) => RequestId::String(s.clone()),
        Value::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0)),
        _ => RequestId::Number(0),
    }
}
