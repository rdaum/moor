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

//! MCP Resources for browsing the mooR world
//!
//! This module provides MCP resources that allow browsing the MOO database
//! as a hierarchical structure.

use crate::mcp_types::{Resource, ResourceContents, ResourceReadResult};
use crate::moor_client::MoorClient;
use eyre::Result;
use moor_common::model::ObjectRef;
use serde_json::json;
use tracing::debug;

/// Get the list of available resources
pub fn get_resources() -> Vec<Resource> {
    vec![
        Resource {
            uri: "moo://world".to_string(),
            name: "MOO World Overview".to_string(),
            description: Some("Overview of the MOO virtual world".to_string()),
            mime_type: Some("text/plain".to_string()),
        },
        Resource {
            uri: "moo://objects".to_string(),
            name: "All Objects".to_string(),
            description: Some("List of all objects in the MOO database".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "moo://system".to_string(),
            name: "System Object".to_string(),
            description: Some("The system object (#0) and its properties".to_string()),
            mime_type: Some("text/plain".to_string()),
        },
    ]
}

/// Read a resource by URI
pub async fn read_resource(client: &mut MoorClient, uri: &str) -> Result<ResourceReadResult> {
    debug!("Reading resource: {}", uri);

    // Parse the URI
    if !uri.starts_with("moo://") {
        return Ok(ResourceReadResult {
            contents: vec![ResourceContents::text(
                uri,
                format!("Unknown URI scheme: {}", uri),
            )],
        });
    }

    let path = &uri[6..]; // Remove "moo://"
    let parts: Vec<&str> = path.split('/').collect();

    match parts.first().copied() {
        Some("world") => read_world_overview(client, uri).await,
        Some("objects") => read_objects_list(client, uri, &parts[1..]).await,
        Some("object") if parts.len() >= 2 => read_object(client, uri, parts[1], &parts[2..]).await,
        Some("system") => read_system(client, uri).await,
        Some("verb") if parts.len() >= 3 => read_verb(client, uri, parts[1], parts[2]).await,
        Some("property") if parts.len() >= 3 => {
            read_property(client, uri, parts[1], parts[2]).await
        }
        _ => Ok(ResourceReadResult {
            contents: vec![ResourceContents::text(
                uri,
                format!("Unknown resource path: {}", path),
            )],
        }),
    }
}

async fn read_world_overview(client: &mut MoorClient, uri: &str) -> Result<ResourceReadResult> {
    let mut output = String::new();
    output.push_str("=== mooR World Overview ===\n\n");

    if let Some(player) = client.player() {
        output.push_str(&format!("Logged in as: {}\n\n", player));
    } else {
        output.push_str("Not logged in\n\n");
    }

    // Try to get basic world info
    if let Ok(objects) = client.list_objects().await {
        output.push_str(&format!("Total objects in world: {}\n", objects.len()));

        // Count by rough category based on flags
        let mut rooms = 0;
        let mut players = 0;
        let mut things = 0;
        for obj in &objects {
            // This is a rough heuristic - proper categorization would need property inspection
            if obj.name.contains("room") || obj.name.contains("Room") {
                rooms += 1;
            } else if obj.flags.contains('u') {
                // User/player flag
                players += 1;
            } else {
                things += 1;
            }
        }
        output.push_str(&format!("  Estimated rooms: {}\n", rooms));
        output.push_str(&format!("  Players: {}\n", players));
        output.push_str(&format!("  Other objects: {}\n", things));
    }

    output.push_str("\nAvailable resources:\n");
    output.push_str("  moo://objects - List all objects\n");
    output.push_str("  moo://object/<id> - Object details (e.g., moo://object/0)\n");
    output.push_str("  moo://system - System object (#0) details\n");
    output.push_str("  moo://verb/<obj>/<name> - Verb source code\n");
    output.push_str("  moo://property/<obj>/<name> - Property value\n");

    Ok(ResourceReadResult {
        contents: vec![ResourceContents::text(uri, output)],
    })
}

async fn read_objects_list(
    client: &mut MoorClient,
    uri: &str,
    _parts: &[&str],
) -> Result<ResourceReadResult> {
    let objects = client.list_objects().await?;

    let objects_json: Vec<serde_json::Value> = objects
        .iter()
        .map(|obj| {
            json!({
                "id": obj.obj,
                "name": obj.name,
                "flags": obj.flags
            })
        })
        .collect();

    Ok(ResourceReadResult {
        contents: vec![ResourceContents::json(
            uri,
            serde_json::to_string_pretty(&objects_json)?,
        )],
    })
}

async fn read_object(
    client: &mut MoorClient,
    uri: &str,
    obj_str: &str,
    parts: &[&str],
) -> Result<ResourceReadResult> {
    // Parse object reference
    let obj_ref = crate::tools::parse_object_ref(obj_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", obj_str))?;

    // Handle sub-resources
    match parts.first().copied() {
        Some("verbs") => {
            let verbs = client.list_verbs(&obj_ref, false).await?;
            let mut output = String::new();
            output.push_str(&format!("Verbs on {}:\n\n", obj_str));
            for verb in &verbs {
                output.push_str(&format!("{}\n", verb.name));
            }
            Ok(ResourceReadResult {
                contents: vec![ResourceContents::text(uri, output)],
            })
        }
        Some("properties") => {
            let props = client.list_properties(&obj_ref, false).await?;
            let mut output = String::new();
            output.push_str(&format!("Properties on {}:\n\n", obj_str));
            for prop in &props {
                output.push_str(&format!("{}\n", prop.name));
            }
            Ok(ResourceReadResult {
                contents: vec![ResourceContents::text(uri, output)],
            })
        }
        _ => {
            // Return object overview
            let mut output = String::new();
            output.push_str(&format!("=== Object {} ===\n\n", obj_str));

            // Get verbs
            if let Ok(verbs) = client.list_verbs(&obj_ref, false).await {
                output.push_str(&format!("Verbs ({}):\n", verbs.len()));
                for verb in verbs.iter().take(20) {
                    output.push_str(&format!("  {}\n", verb.name));
                }
                if verbs.len() > 20 {
                    output.push_str(&format!("  ... and {} more\n", verbs.len() - 20));
                }
                output.push('\n');
            }

            // Get properties
            if let Ok(props) = client.list_properties(&obj_ref, false).await {
                output.push_str(&format!("Properties ({}):\n", props.len()));
                for prop in props.iter().take(20) {
                    output.push_str(&format!("  {}\n", prop.name));
                }
                if props.len() > 20 {
                    output.push_str(&format!("  ... and {} more\n", props.len() - 20));
                }
            }

            Ok(ResourceReadResult {
                contents: vec![ResourceContents::text(uri, output)],
            })
        }
    }
}

async fn read_system(client: &mut MoorClient, uri: &str) -> Result<ResourceReadResult> {
    let obj_ref = ObjectRef::Id(moor_var::SYSTEM_OBJECT);
    let mut output = String::new();
    output.push_str("=== System Object (#0) ===\n\n");

    // Get verbs
    if let Ok(verbs) = client.list_verbs(&obj_ref, false).await {
        output.push_str(&format!("Verbs ({}):\n", verbs.len()));
        for verb in &verbs {
            output.push_str(&format!(
                "  {} (flags: {}, args: {})\n",
                verb.name, verb.flags, verb.args
            ));
        }
        output.push('\n');
    }

    // Get properties
    if let Ok(props) = client.list_properties(&obj_ref, false).await {
        output.push_str(&format!("Properties ({}):\n", props.len()));
        for prop in &props {
            output.push_str(&format!("  {} (flags: {})\n", prop.name, prop.flags));
        }
    }

    Ok(ResourceReadResult {
        contents: vec![ResourceContents::text(uri, output)],
    })
}

async fn read_verb(
    client: &mut MoorClient,
    uri: &str,
    obj_str: &str,
    verb_name: &str,
) -> Result<ResourceReadResult> {
    let obj_ref = crate::tools::parse_object_ref(obj_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", obj_str))?;

    let verb = client.get_verb(&obj_ref, verb_name).await?;

    let mut output = String::new();
    output.push_str(&format!("// Verb: {}:{}\n\n", obj_str, verb_name));
    for line in &verb.code {
        output.push_str(line);
        output.push('\n');
    }

    Ok(ResourceReadResult {
        contents: vec![ResourceContents {
            uri: uri.to_string(),
            mime_type: Some("text/x-moo".to_string()),
            text: Some(output),
            blob: None,
        }],
    })
}

async fn read_property(
    client: &mut MoorClient,
    uri: &str,
    obj_str: &str,
    prop_name: &str,
) -> Result<ResourceReadResult> {
    let obj_ref = crate::tools::parse_object_ref(obj_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", obj_str))?;

    let value = client.get_property(&obj_ref, prop_name).await?;

    // Format the value
    let formatted = crate::tools::format_var_for_resource(&value);

    Ok(ResourceReadResult {
        contents: vec![ResourceContents::text(
            uri,
            format!("{}.{} = {}", obj_str, prop_name, formatted),
        )],
    })
}
