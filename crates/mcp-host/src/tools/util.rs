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

//! Utility tools: notify, reconnect

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use serde_json::{Value, json};
use tracing::info;

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_reconnect() -> Tool {
    Tool {
        name: "moo_reconnect".to_string(),
        description: "Reconnect to the mooR daemon. Use this if the connection has been lost \
            (e.g., after the daemon was restarted). This clears stale connection state, \
            re-establishes the connection, and re-authenticates using stored credentials."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn tool_moo_notify() -> Tool {
    Tool {
        name: "moo_notify".to_string(),
        description: "Send a notification message to a connected player. The message appears in \
            their client output. Useful for testing or sending messages programmatically."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "player": {
                    "type": "string",
                    "description": "Player object to notify (e.g., '#2', '$wizard')"
                },
                "message": {
                    "type": "string",
                    "description": "Message text to send"
                },
                "no_flush": {
                    "type": "boolean",
                    "description": "If true, don't flush output buffer immediately",
                    "default": false
                },
                "no_newline": {
                    "type": "boolean",
                    "description": "If true, don't append a newline to the message",
                    "default": false
                }
            },
            "required": ["player", "message"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_reconnect(
    client: &mut MoorClient,
    _args: &Value,
) -> Result<ToolCallResult> {
    info!("Manual reconnect requested");

    match client.reconnect().await {
        Ok(()) => {
            let status = if client.is_authenticated() {
                format!(
                    "Reconnected and authenticated as {:?}",
                    client.player().map(|p| p.to_string()).unwrap_or_default()
                )
            } else {
                "Reconnected (not authenticated - no stored credentials)".to_string()
            };
            Ok(ToolCallResult::text(status))
        }
        Err(e) => Ok(ToolCallResult::error(format!("Reconnect failed: {}", e))),
    }
}

pub async fn execute_moo_notify(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let player_str = args
        .get("player")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'player' parameter"))?;

    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'message' parameter"))?;

    let no_flush = args
        .get("no_flush")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let no_newline = args
        .get("no_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Escape the message for MOO string
    let escaped_message = message.replace('\\', "\\\\").replace('"', "\\\"");

    // Build notify() call with optional flags
    let expr = if no_flush || no_newline {
        format!(
            "notify({}, \"{}\", {}, {});",
            player_str,
            escaped_message,
            if no_flush { "1" } else { "0" },
            if no_newline { "1" } else { "0" }
        )
    } else {
        format!("notify({}, \"{}\");", player_str, escaped_message)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Sent notification to {}: {}",
            player_str, message
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}
