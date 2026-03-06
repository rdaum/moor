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

            // Parse and handle the JSON-RPC message
            let response = match serde_json::from_str::<Value>(line) {
                Ok(msg) => self.handle_envelope(msg).await,
                Err(e) => {
                    error!("Failed to parse JSON: {}", e);
                    let response =
                        JsonRpcResponse::error_without_id(JsonRpcError::parse_error(e.to_string()));
                    Some(serde_json::to_value(response).unwrap())
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

    /// Handle either a JSON-RPC request/notification object or batch envelope.
    async fn handle_envelope(&mut self, msg: Value) -> Option<Value> {
        match msg {
            Value::Object(_) => self
                .handle_message(msg)
                .await
                .map(|response| serde_json::to_value(response).unwrap()),
            Value::Array(batch) => self.handle_batch(batch).await,
            _ => {
                let response = JsonRpcResponse::error_without_id(JsonRpcError::invalid_request(
                    "JSON-RPC message must be an object or a batch array",
                ));
                Some(serde_json::to_value(response).unwrap())
            }
        }
    }

    /// Handle a JSON-RPC batch request.
    async fn handle_batch(&mut self, batch: Vec<Value>) -> Option<Value> {
        if batch.is_empty() {
            let response =
                JsonRpcResponse::error_without_id(JsonRpcError::invalid_request("Empty batch"));
            return Some(serde_json::to_value(response).unwrap());
        }

        let mut responses = Vec::new();
        for item in batch {
            if !item.is_object() {
                let response = JsonRpcResponse::error_without_id(JsonRpcError::invalid_request(
                    "Batch element must be a JSON object",
                ));
                responses.push(serde_json::to_value(response).unwrap());
                continue;
            }

            if let Some(response) = self.handle_message(item).await {
                responses.push(serde_json::to_value(response).unwrap());
            }
        }

        if responses.is_empty() {
            None
        } else {
            Some(Value::Array(responses))
        }
    }

    /// Handle an incoming JSON-RPC message
    async fn handle_message(&mut self, msg: Value) -> Option<JsonRpcResponse> {
        if !msg.is_object() {
            return Some(JsonRpcResponse::error_without_id(
                JsonRpcError::invalid_request("Message must be a JSON object"),
            ));
        }

        let request_id = match msg.get("id") {
            Some(id_value) => match parse_request_id(id_value) {
                Some(id) => Some(id),
                None => {
                    return Some(JsonRpcResponse::error_without_id(
                        JsonRpcError::invalid_request("id must be a string or integer"),
                    ));
                }
            },
            None => None,
        };

        let jsonrpc = msg.get("jsonrpc").and_then(|v| v.as_str());
        if jsonrpc != Some("2.0") {
            return Some(match request_id {
                Some(id) => JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_request("jsonrpc must be \"2.0\""),
                ),
                None => JsonRpcResponse::error_without_id(JsonRpcError::invalid_request(
                    "jsonrpc must be \"2.0\"",
                )),
            });
        }
        let method = msg.get("method").and_then(|m| m.as_str());
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        let method = match method {
            Some(m) => m,
            None => {
                return Some(match request_id {
                    Some(id) => {
                        JsonRpcResponse::error(id, JsonRpcError::invalid_request("Missing method"))
                    }
                    None => JsonRpcResponse::error_without_id(JsonRpcError::invalid_request(
                        "Missing method",
                    )),
                });
            }
        };

        debug!("Handling method: {}", method);

        // Handle notifications (messages without an id), which must not produce responses.
        if request_id.is_none() {
            match method {
                "initialized" | "notifications/initialized" => {
                    self.initialized = true;
                    info!("Client initialized");
                    return None;
                }
                "notifications/cancelled" => {
                    debug!("Request cancelled by client");
                    return None;
                }
                _ if method.starts_with("notifications/") => {
                    debug!("Ignoring unknown notification: {}", method);
                    return None;
                }
                _ => {}
            }
        }

        // Notification methods are invalid if sent as requests (with an id).
        if let Some(id) = request_id.clone()
            && (method == "initialized" || method.starts_with("notifications/"))
        {
            return Some(JsonRpcResponse::error(
                id,
                JsonRpcError::invalid_request(format!(
                    "Method '{}' is a notification and must not include id",
                    method
                )),
            ));
        }

        // Handle requests (have an id, expect a response)
        let result = match method {
            "initialize" => self.handle_initialize(&params).await,
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

        // Only produce a response for requests (messages with an id).
        // Per JSON-RPC 2.0, notifications (no id) must not receive responses.
        let request_id = match request_id {
            Some(id) => id,
            None => {
                if let Err(e) = &result {
                    warn!(
                        "Dropping error for notification '{}': {}",
                        method, e.message
                    );
                }
                return None;
            }
        };

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
fn parse_request_id(value: &Value) -> Option<RequestId> {
    match value {
        Value::String(s) => Some(RequestId::String(s.clone())),
        Value::Number(n) => n.as_i64().map(RequestId::Number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ConnectionConfig, ConnectionManager};
    use crate::moor_client::MoorClientConfig;
    use serde_json::json;

    fn test_server() -> McpServer {
        let config = ConnectionConfig {
            client_config: MoorClientConfig {
                rpc_address: "tcp://127.0.0.1:1".to_string(),
                events_address: "tcp://127.0.0.1:1".to_string(),
                curve_keys: None,
            },
            programmer_credentials: None,
            wizard_credentials: None,
        };
        McpServer::new(ConnectionManager::new(config))
    }

    #[tokio::test]
    async fn initialized_notification_without_id_gets_no_response() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }))
            .await;

        assert!(response.is_none());
        assert!(server.initialized);
    }

    #[tokio::test]
    async fn initialized_with_id_returns_error_response_instead_of_being_dropped() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "notifications/initialized"
            }))
            .await
            .expect("id-bearing message should receive a response");

        assert_eq!(response.id, Some(RequestId::Number(7)));
        let error = response
            .error
            .expect("unknown id-bearing method should return an error");
        assert_eq!(error.code, -32600);
        assert!(!server.initialized);
    }

    #[tokio::test]
    async fn unknown_notification_without_id_is_ignored() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "method": "notifications/something-custom"
            }))
            .await;

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn unknown_notification_with_id_returns_error_response() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": "abc",
                "method": "notifications/something-custom"
            }))
            .await
            .expect("id-bearing message should receive a response");

        assert_eq!(response.id, Some(RequestId::String("abc".to_string())));
        let error = response
            .error
            .expect("id-bearing notification method should return an error");
        assert_eq!(error.code, -32600);
    }

    #[tokio::test]
    async fn missing_method_without_id_returns_invalid_request_without_response_id() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0"
            }))
            .await
            .expect("invalid request should produce an error response");

        assert_eq!(response.id, None);
        let error = response.error.expect("invalid request should return error");
        assert_eq!(error.code, -32600);
    }

    #[tokio::test]
    async fn invalid_id_type_returns_invalid_request_without_response_id() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": {"bad": true},
                "method": "ping"
            }))
            .await
            .expect("invalid id should produce an error response");

        assert_eq!(response.id, None);
        let error = response.error.expect("invalid request should return error");
        assert_eq!(error.code, -32600);
    }

    #[tokio::test]
    async fn wrong_jsonrpc_version_with_valid_id_preserves_response_id() {
        let mut server = test_server();
        let response = server
            .handle_message(json!({
                "jsonrpc": "1.0",
                "id": 42,
                "method": "ping"
            }))
            .await
            .expect("invalid request should produce an error response");

        assert_eq!(response.id, Some(RequestId::Number(42)));
        let error = response.error.expect("invalid request should return error");
        assert_eq!(error.code, -32600);
    }

    #[tokio::test]
    async fn empty_batch_returns_invalid_request() {
        let mut server = test_server();
        let response = server
            .handle_batch(Vec::new())
            .await
            .expect("empty batch should return an error response");

        let response = response
            .as_object()
            .expect("empty batch response should be a single error object");
        let error = response
            .get("error")
            .and_then(|e| e.as_object())
            .expect("error object should be present");
        assert_eq!(error.get("code").and_then(|v| v.as_i64()), Some(-32600));
    }

    #[tokio::test]
    async fn batch_with_only_notifications_returns_no_response() {
        let mut server = test_server();
        let response = server
            .handle_batch(vec![json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })])
            .await;

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn batch_mixes_responses_and_notifications() {
        let mut server = test_server();
        let response = server
            .handle_batch(vec![
                json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }),
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "ping"
                }),
            ])
            .await
            .expect("batch with request should return responses");

        let responses = response
            .as_array()
            .expect("batch response should be an array");
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].get("id").and_then(|v| v.as_i64()), Some(1));
        assert!(responses[0].get("result").is_some());
    }
}
