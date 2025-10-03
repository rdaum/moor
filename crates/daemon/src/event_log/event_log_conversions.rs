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

use moor_common::tasks::{NarrativeEvent, Presentation};
use moor_schema::{
    convert::{narrative_event_to_flatbuffer_struct, obj_to_flatbuffer_struct},
    event_log::LoggedNarrativeEvent,
};
use moor_var::Obj;

/// Convert from domain types to FlatBuffer LoggedNarrativeEvent
pub fn logged_narrative_event_to_flatbuffer(
    player: Obj,
    event: Box<NarrativeEvent>,
) -> Result<LoggedNarrativeEvent, moor_var::EncodingError> {
    let player_fb = obj_to_flatbuffer_struct(&player);
    let event_fb = narrative_event_to_flatbuffer_struct(&event)?;

    Ok(LoggedNarrativeEvent {
        player: Box::new(player_fb),
        event: Box::new(event_fb),
    })
}

/// Convert from FlatBuffer Presentation to domain Presentation
pub fn presentation_from_flatbuffer(
    pres: &moor_schema::common::Presentation,
) -> Result<Presentation, String> {
    Ok(Presentation {
        id: pres.id.to_string(),
        content_type: pres.content_type.to_string(),
        content: pres.content.to_string(),
        target: pres.target.to_string(),
        attributes: pres
            .attributes
            .iter()
            .map(|attr| (attr.key.to_string(), attr.value.to_string()))
            .collect(),
    })
}
