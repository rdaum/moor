// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
use crate::moor_client::MoorClient;
use crate::tools::dynamic::{
    DynamicResource, DynamicTool, execute_dynamic_tool, fetch_dynamic_resources,
    fetch_dynamic_tools,
};
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
    /// Dynamically-defined tools fetched from the MOO world
    dynamic_tools: Vec<DynamicTool>,
    /// Whether dynamic tools have been fetched
    dynamic_tools_loaded: bool,
    /// Dynamically-defined resources fetched from the MOO world
    dynamic_resources: Vec<DynamicResource>,
    /// Whether dynamic resources have been fetched
    dynamic_resources_loaded: bool,
}

impl McpServer {
    /// Create a new MCP server with a connection manager
    pub fn new(connections: ConnectionManager) -> Self {
        Self {
            connections,
            initialized: false,
            shutdown_requested: false,
            dynamic_tools: Vec::new(),
            dynamic_tools_loaded: false,
            dynamic_resources: Vec::new(),
            dynamic_resources_loaded: false,
        }
    }

    /// Refresh dynamic tools from the MOO world
    ///
    /// Calls #0:external_agent_tools() and updates the stored tool list.
    async fn refresh_dynamic_tools(&mut self) -> Result<usize, String> {
        // Use programmer connection for fetching tools
        let client = self
            .connections
            .programmer()
            .await
            .map_err(|e| e.to_string())?;

        match fetch_dynamic_tools(client).await {
            Ok(tools) => {
                let count = tools.len();
                self.dynamic_tools = tools;
                self.dynamic_tools_loaded = true;
                info!("Loaded {} dynamic tools from MOO world", count);
                Ok(count)
            }
            Err(e) => {
                warn!("Failed to fetch dynamic tools: {}", e);
                self.dynamic_tools_loaded = true; // Mark as loaded even on error
                Err(e.to_string())
            }
        }
    }

    /// Refresh dynamic resources from the MOO world
    ///
    /// Calls #0:external_agent_resources() and updates the stored resource list.
    async fn refresh_dynamic_resources(&mut self) -> Result<usize, String> {
        let client = self
            .connections
            .programmer()
            .await
            .map_err(|e| e.to_string())?;

        match fetch_dynamic_resources(client).await {
            Ok(resources) => {
                let count = resources.len();
                self.dynamic_resources = resources;
                self.dynamic_resources_loaded = true;
                info!("Loaded {} dynamic resources from MOO world", count);
                Ok(count)
            }
            Err(e) => {
                warn!("Failed to fetch dynamic resources: {}", e);
                self.dynamic_resources_loaded = true;
                Err(e.to_string())
            }
        }
    }

    /// Find a dynamic resource by URI
    fn find_dynamic_resource(&self, uri: &str) -> Option<&DynamicResource> {
        self.dynamic_resources.iter().find(|r| r.uri == uri)
    }

    /// Find a dynamic tool by name
    fn find_dynamic_tool(&self, name: &str) -> Option<&DynamicTool> {
        self.dynamic_tools.iter().find(|t| t.name == name)
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
    async fn handle_tools_list(&mut self) -> Result<Value, JsonRpcError> {
        // Fetch dynamic tools on first access if not already loaded
        if !self.dynamic_tools_loaded {
            let _ = self.refresh_dynamic_tools().await;
        }

        // Merge static tools with dynamic tools
        let mut all_tools = tools::get_tools();

        // Add dynamic tools
        for dynamic_tool in &self.dynamic_tools {
            all_tools.push(dynamic_tool.to_mcp_tool());
        }

        // Add the refresh_dynamic_tools meta-tool
        all_tools.push(Tool {
            name: "moo_refresh_dynamic_tools".to_string(),
            description: "Refresh the list of dynamic tools from the MOO world. \
                Call this after tools have been added or modified in #0:external_agent_tools()."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        });

        let result = ToolsListResult { tools: all_tools };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle tools/call request
    ///
    /// Attempts to execute the tool, with automatic reconnection on connection failures.
    async fn handle_tools_call(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let call_params: ToolCallParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        // Handle meta-tools that need special handling
        if call_params.name == "moo_refresh_dynamic_tools" {
            return self.handle_refresh_dynamic_tools().await;
        }
        if call_params.name == "moo_reconnect" {
            return self.handle_reconnect().await;
        }

        // Determine if wizard privileges are needed
        let dynamic_tool = self.find_dynamic_tool(&call_params.name).cloned();
        let is_wizard_only =
            dynamic_tool.is_none() && tools::WIZARD_ONLY_TOOLS.contains(&call_params.name.as_str());
        let wizard = is_wizard_only
            || call_params
                .arguments
                .get("wizard")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        if wizard && !is_wizard_only {
            warn!(
                "Tool {} called with wizard privileges - use with caution!",
                call_params.name
            );
        }

        // Execute the tool with automatic reconnection on connection failures
        self.execute_tool_with_reconnect(&call_params, dynamic_tool.as_ref(), wizard)
            .await
    }

    /// Handle the refresh_dynamic_tools meta-tool
    async fn handle_refresh_dynamic_tools(&mut self) -> Result<Value, JsonRpcError> {
        let result = match self.refresh_dynamic_tools().await {
            Ok(count) => ToolCallResult::text(format!(
                "Refreshed dynamic tools. Loaded {} tools from MOO world.",
                count
            )),
            Err(e) => ToolCallResult::error(format!("Failed to refresh dynamic tools: {}", e)),
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle the reconnect meta-tool
    ///
    /// Reconnects all established connections (both programmer and wizard).
    async fn handle_reconnect(&mut self) -> Result<Value, JsonRpcError> {
        info!("Manual reconnect requested for all connections");
        let result = match self.connections.reconnect_all().await {
            Ok(msg) => ToolCallResult::text(msg),
            Err(e) => ToolCallResult::error(format!("Reconnect failed: {}", e)),
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Execute a tool call, with automatic reconnection on connection failures
    async fn execute_tool_with_reconnect(
        &mut self,
        call_params: &ToolCallParams,
        dynamic_tool: Option<&DynamicTool>,
        wizard: bool,
    ) -> Result<Value, JsonRpcError> {
        let client = self
            .connections
            .get(wizard)
            .await
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        let result = Self::execute_tool_dispatch(client, call_params, dynamic_tool).await;

        match result {
            Ok(result) => Ok(serde_json::to_value(result).unwrap()),
            Err(e) => {
                let error_str = e.to_string();
                if !Self::is_connection_error(&error_str) {
                    return Err(JsonRpcError::internal_error(error_str));
                }

                warn!(
                    "Tool call failed with connection error, attempting reconnect: {}",
                    error_str
                );

                self.connections
                    .reconnect(wizard)
                    .await
                    .map_err(|reconnect_err| {
                        error!("Reconnection failed: {}", reconnect_err);
                        JsonRpcError::internal_error(format!(
                            "Connection lost and reconnection failed: {}. Original error: {}",
                            reconnect_err, error_str
                        ))
                    })?;

                info!("Reconnected successfully, retrying tool call");

                let client = self
                    .connections
                    .get(wizard)
                    .await
                    .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

                let retry_result = Self::execute_tool_dispatch(client, call_params, dynamic_tool)
                    .await
                    .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

                Ok(serde_json::to_value(retry_result).unwrap())
            }
        }
    }

    /// Dispatch to the appropriate tool executor (dynamic or static)
    async fn execute_tool_dispatch(
        client: &mut MoorClient,
        call_params: &ToolCallParams,
        dynamic_tool: Option<&DynamicTool>,
    ) -> Result<ToolCallResult> {
        if let Some(tool) = dynamic_tool {
            execute_dynamic_tool(client, tool, &call_params.arguments).await
        } else {
            tools::execute_tool(client, &call_params.name, &call_params.arguments).await
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
    async fn handle_resources_list(&mut self) -> Result<Value, JsonRpcError> {
        // Fetch dynamic resources on first access if not already loaded
        if !self.dynamic_resources_loaded {
            let _ = self.refresh_dynamic_resources().await;
        }

        // Merge static resources with dynamic resources
        let mut all_resources = resources::get_resources();

        // Add dynamic resources
        for dynamic_resource in &self.dynamic_resources {
            all_resources.push(Resource {
                uri: dynamic_resource.uri.clone(),
                name: dynamic_resource.name.clone(),
                description: Some(dynamic_resource.description.clone()),
                mime_type: Some(dynamic_resource.mime_type.clone()),
            });
        }

        let result = ResourcesListResult {
            resources: all_resources,
        };
        Ok(serde_json::to_value(result).unwrap())
    }

    /// Handle resources/read request
    async fn handle_resources_read(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let read_params: ResourceReadParams = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

        // Check if this is a dynamic resource first
        if let Some(dynamic_resource) = self.find_dynamic_resource(&read_params.uri).cloned() {
            let result = ResourceReadResult {
                contents: vec![ResourceContents {
                    uri: dynamic_resource.uri,
                    mime_type: Some(dynamic_resource.mime_type),
                    text: Some(dynamic_resource.content),
                    blob: None,
                }],
            };
            return Ok(serde_json::to_value(result).unwrap());
        }

        // Fall back to static resource handling
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
