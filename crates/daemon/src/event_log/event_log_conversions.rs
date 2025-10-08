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

//! Conversions between domain types and FlatBuffer event log types

use moor_common::tasks::{Event, NarrativeEvent, Presentation};
use moor_schema::{
    convert::{
        narrative_event_to_flatbuffer_struct, obj_to_flatbuffer_struct, uuid_to_flatbuffer_struct,
    },
    event_log::LoggedNarrativeEvent,
};
use moor_var::Obj;
use std::time::SystemTime;
use uuid::Uuid;

/// Presentation action extracted from an event (for updating presentation state)
pub enum PresentationAction {
    Add(Presentation), // Full presentation to add (will be encrypted)
    Remove(String),    // presentation_id to remove
}

/// Convert from domain types to FlatBuffer LoggedNarrativeEvent
/// Pubkey is REQUIRED - all events must be encrypted
/// Returns the encrypted event and optional presentation action for state tracking
pub fn logged_narrative_event_to_flatbuffer(
    player: Obj,
    event: Box<NarrativeEvent>,
    pubkey: String,
) -> Result<(LoggedNarrativeEvent, Option<PresentationAction>), moor_var::EncodingError> {
    // Extract presentation action before encryption
    let presentation_action = match &event.event {
        Event::Present(pres) => Some(PresentationAction::Add(pres.clone())),
        Event::Unpresent(presentation_id) => {
            Some(PresentationAction::Remove(presentation_id.clone()))
        }
        _ => None,
    };

    let player_fb = obj_to_flatbuffer_struct(&player);
    let event_fb = narrative_event_to_flatbuffer_struct(&event)?;

    // Generate event ID and timestamp
    let event_id = Uuid::now_v7();
    let event_id_fb = uuid_to_flatbuffer_struct(&event_id);
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    // Serialize event to bytes
    let mut builder = ::planus::Builder::new();
    let event_bytes = builder.finish(&event_fb, None);

    // Encrypt with the provided pubkey (REQUIRED - no plaintext storage)
    let encrypted_blob = super::encryption::encrypt(event_bytes, &pubkey)
        .map_err(|e| moor_var::EncodingError::CouldNotEncode(format!("Encryption failed: {e}")))?;

    Ok((
        LoggedNarrativeEvent {
            event_id: Box::new(event_id_fb),
            timestamp,
            player: Box::new(player_fb),
            encrypted_blob,
        },
        presentation_action,
    ))
}

/// Convert from FlatBuffer PresentationRef to domain Presentation
pub fn presentation_from_flatbuffer(
    pres: &moor_schema::common::PresentationRef,
) -> Result<Presentation, String> {
    Ok(Presentation {
        id: pres.id().map_err(|e| e.to_string())?.to_string(),
        content_type: pres.content_type().map_err(|e| e.to_string())?.to_string(),
        content: pres.content().map_err(|e| e.to_string())?.to_string(),
        target: pres.target().map_err(|e| e.to_string())?.to_string(),
        attributes: pres
            .attributes()
            .map_err(|e| e.to_string())?
            .iter()
            .filter_map(|attr_result| {
                attr_result.ok().and_then(|attr| {
                    Some((attr.key().ok()?.to_string(), attr.value().ok()?.to_string()))
                })
            })
            .collect(),
    })
}
