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

//! Event and presentation conversions between moor types and FlatBuffer types
//!
//! This module handles conversion of narrative events, presentations, and related types.

use crate::{
    schema::{
        common,
        common::EventUnionRef,
        convert_common::{
            symbol_from_ref, uuid_from_ref, uuid_to_flatbuffer_struct, var_from_ref,
            var_to_flatbuffer_bytes,
        },
        convert_errors::{error_to_flatbuffer_struct, exception_from_ref},
    },
    tasks::{Event, NarrativeEvent, Presentation},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Convert from FlatBuffer NarrativeEventRef to NarrativeEvent
pub fn narrative_event_from_ref(
    event_ref: common::NarrativeEventRef<'_>,
) -> Result<NarrativeEvent, String> {
    let event_id = uuid_from_ref(event_ref.event_id().map_err(|_| "Missing event_id")?)?;
    let timestamp_nanos = event_ref.timestamp().map_err(|_| "Missing timestamp")?;
    let timestamp = UNIX_EPOCH + Duration::from_nanos(timestamp_nanos);
    let author = var_from_ref(event_ref.author().map_err(|_| "Missing author")?)?;
    let event = event_from_ref(event_ref.event().map_err(|_| "Missing event")?)?;

    Ok(NarrativeEvent {
        event_id,
        timestamp,
        author,
        event,
    })
}

/// Convert from FlatBuffer EventRef to Event
pub fn event_from_ref(event_ref: common::EventRef<'_>) -> Result<Event, String> {
    match event_ref
        .event()
        .map_err(|_| "Failed to read Event union")?
    {
        EventUnionRef::NotifyEvent(notify) => {
            let value = var_from_ref(notify.value().map_err(|_| "Missing value")?)?;
            let content_type = notify
                .content_type()
                .ok()
                .flatten()
                .and_then(|ct| symbol_from_ref(ct).ok());
            let no_flush = notify.no_flush().map_err(|_| "Missing no_flush")?;
            let no_newline = notify.no_newline().map_err(|_| "Missing no_newline")?;
            Ok(Event::Notify {
                value,
                content_type,
                no_flush,
                no_newline,
            })
        }
        EventUnionRef::PresentEvent(present) => {
            let presentation_ref = present.presentation().map_err(|_| "Missing presentation")?;
            let presentation = presentation_from_ref(presentation_ref)?;
            Ok(Event::Present(presentation))
        }
        EventUnionRef::UnpresentEvent(unpresent) => {
            let presentation_id = unpresent
                .presentation_id()
                .map_err(|_| "Missing presentation_id")?
                .to_string();
            Ok(Event::Unpresent(presentation_id))
        }
        EventUnionRef::TracebackEvent(traceback) => {
            let exception_ref = traceback.exception().map_err(|_| "Missing exception")?;
            let exception = exception_from_ref(exception_ref)?;
            Ok(Event::Traceback(exception))
        }
    }
}

/// Convert from FlatBuffer PresentationRef to Presentation
pub fn presentation_from_ref(
    pres_ref: common::PresentationRef<'_>,
) -> Result<Presentation, String> {
    let id = pres_ref.id().map_err(|_| "Missing id")?.to_string();
    let content_type = pres_ref
        .content_type()
        .map_err(|_| "Missing content_type")?
        .to_string();
    let content = pres_ref
        .content()
        .map_err(|_| "Missing content")?
        .to_string();
    let target = pres_ref.target().map_err(|_| "Missing target")?.to_string();

    let attrs_vec = pres_ref.attributes().map_err(|_| "Missing attributes")?;
    let mut attributes = Vec::new();
    for attr in attrs_vec.iter() {
        let attr = attr.map_err(|_| "Failed to read attribute")?;
        let key = attr.key().map_err(|_| "Missing attribute key")?.to_string();
        let value = attr
            .value()
            .map_err(|_| "Missing attribute value")?
            .to_string();
        attributes.push((key, value));
    }

    Ok(Presentation {
        id,
        content_type,
        content,
        target,
        attributes,
    })
}

/// Convert Presentation to FlatBuffer struct
pub fn presentation_to_flatbuffer_struct(
    presentation: &Presentation,
) -> Result<common::Presentation, moor_var::EncodingError> {
    let attributes = presentation
        .attributes
        .iter()
        .map(|(k, v)| common::PresentationAttribute {
            key: k.clone(),
            value: v.clone(),
        })
        .collect();

    Ok(common::Presentation {
        id: presentation.id.clone(),
        content_type: presentation.content_type.clone(),
        content: presentation.content.clone(),
        target: presentation.target.clone(),
        attributes,
    })
}

/// Convert Event to FlatBuffer struct
pub fn event_to_flatbuffer_struct(event: &Event) -> Result<common::Event, moor_var::EncodingError> {
    let event_union = match event {
        Event::Notify {
            value,
            content_type,
            no_flush,
            no_newline,
        } => {
            let value_bytes = var_to_flatbuffer_bytes(value)?;
            common::EventUnion::NotifyEvent(Box::new(common::NotifyEvent {
                value: Box::new(common::VarBytes { data: value_bytes }),
                content_type: content_type.as_ref().map(|s| {
                    Box::new(common::Symbol {
                        value: s.as_string(),
                    })
                }),
                no_flush: *no_flush,
                no_newline: *no_newline,
            }))
        }
        Event::Present(presentation) => {
            let fb_presentation = presentation_to_flatbuffer_struct(presentation)?;
            common::EventUnion::PresentEvent(Box::new(common::PresentEvent {
                presentation: Box::new(fb_presentation),
            }))
        }
        Event::Unpresent(id) => {
            common::EventUnion::UnpresentEvent(Box::new(common::UnpresentEvent {
                presentation_id: id.clone(),
            }))
        }
        Event::Traceback(exception) => {
            let error_bytes = error_to_flatbuffer_struct(&exception.error).map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!("Failed to encode error: {e}"))
            })?;
            let stack_bytes: Result<Vec<_>, _> = exception
                .stack
                .iter()
                .map(|v| var_to_flatbuffer_bytes(v).map(|data| common::VarBytes { data }))
                .collect();
            let backtrace_bytes: Result<Vec<_>, _> = exception
                .backtrace
                .iter()
                .map(|v| var_to_flatbuffer_bytes(v).map(|data| common::VarBytes { data }))
                .collect();

            common::EventUnion::TracebackEvent(Box::new(common::TracebackEvent {
                exception: Box::new(common::Exception {
                    error: Box::new(error_bytes),
                    stack: stack_bytes?,
                    backtrace: backtrace_bytes?,
                }),
            }))
        }
    };

    Ok(common::Event { event: event_union })
}

/// Convert NarrativeEvent to FlatBuffer struct
pub fn narrative_event_to_flatbuffer_struct(
    narrative_event: &NarrativeEvent,
) -> Result<common::NarrativeEvent, moor_var::EncodingError> {
    let author_bytes = var_to_flatbuffer_bytes(&narrative_event.author)?;
    let event_fb = event_to_flatbuffer_struct(&narrative_event.event)?;

    let timestamp_nanos = narrative_event
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    Ok(common::NarrativeEvent {
        event_id: Box::new(uuid_to_flatbuffer_struct(&narrative_event.event_id)),
        timestamp: timestamp_nanos,
        author: Box::new(common::VarBytes { data: author_bytes }),
        event: Box::new(event_fb),
    })
}
