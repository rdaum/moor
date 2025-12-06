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

use crate::convert::var_from_flatbuffer_ref;
use crate::{
    StrErr, common,
    common::EventUnionRef,
    convert_common::{symbol_from_ref, uuid_from_ref, uuid_to_flatbuffer_struct},
    convert_errors::{error_to_flatbuffer_struct, exception_from_ref},
    convert_var::var_to_flatbuffer,
    fb_read,
};
use moor_common::tasks::{Event, NarrativeEvent, Presentation};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Convert from FlatBuffer NarrativeEventRef to NarrativeEvent
pub fn narrative_event_from_ref(
    event_ref: common::NarrativeEventRef<'_>,
) -> Result<NarrativeEvent, String> {
    let event_id = uuid_from_ref(fb_read!(event_ref, event_id))?;
    let timestamp_nanos = fb_read!(event_ref, timestamp);
    let timestamp = UNIX_EPOCH + Duration::from_nanos(timestamp_nanos);
    let author_ref = fb_read!(event_ref, author);
    let author = var_from_flatbuffer_ref(author_ref).str_err()?;
    let event = event_from_ref(fb_read!(event_ref, event))?;

    Ok(NarrativeEvent {
        event_id,
        timestamp,
        author,
        event,
    })
}

/// Convert from FlatBuffer EventRef to Event
pub fn event_from_ref(event_ref: common::EventRef<'_>) -> Result<Event, String> {
    match fb_read!(event_ref, event) {
        EventUnionRef::NotifyEvent(notify) => {
            let value_ref = fb_read!(notify, value);
            let value = var_from_flatbuffer_ref(value_ref).str_err()?;
            let content_type = notify
                .content_type()
                .ok()
                .flatten()
                .and_then(|ct| symbol_from_ref(ct).ok());
            let no_flush = fb_read!(notify, no_flush);
            let no_newline = fb_read!(notify, no_newline);

            let metadata = match notify.metadata().ok().flatten() {
                Some(metadata_vec) => {
                    let mut metadata_result = Vec::new();
                    for metadata_ref in metadata_vec.iter() {
                        let metadata_item = metadata_ref
                            .map_err(|e| format!("Failed to read metadata item: {e}"))?;
                        let key = symbol_from_ref(fb_read!(metadata_item, key))?;
                        let value_ref = fb_read!(metadata_item, value);
                        let value = var_from_flatbuffer_ref(value_ref).str_err()?;
                        metadata_result.push((key, value));
                    }
                    Some(metadata_result)
                }
                None => None,
            };

            Ok(Event::Notify {
                value,
                content_type,
                no_flush,
                no_newline,
                metadata,
            })
        }
        EventUnionRef::PresentEvent(present) => {
            let presentation = presentation_from_ref(fb_read!(present, presentation))?;
            Ok(Event::Present(presentation))
        }
        EventUnionRef::UnpresentEvent(unpresent) => {
            let presentation_id = fb_read!(unpresent, presentation_id).to_string();
            Ok(Event::Unpresent(presentation_id))
        }
        EventUnionRef::TracebackEvent(traceback) => {
            let exception = exception_from_ref(fb_read!(traceback, exception))?;
            Ok(Event::Traceback(exception))
        }
    }
}

/// Convert from FlatBuffer PresentationRef to Presentation
pub fn presentation_from_ref(
    pres_ref: common::PresentationRef<'_>,
) -> Result<Presentation, String> {
    let id = fb_read!(pres_ref, id).to_string();
    let content_type = fb_read!(pres_ref, content_type).to_string();
    let content = fb_read!(pres_ref, content).to_string();
    let target = fb_read!(pres_ref, target).to_string();

    let attrs_vec = fb_read!(pres_ref, attributes);
    let mut attributes = Vec::new();
    for attr in attrs_vec.iter() {
        let attr = attr.map_err(|e| format!("Failed to read attribute: {e}"))?;
        let key = fb_read!(attr, key).to_string();
        let value = fb_read!(attr, value).to_string();
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
            metadata,
        } => {
            let value_fb = var_to_flatbuffer(value).map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!("Failed to encode value: {e}"))
            })?;

            let metadata_fb = match metadata {
                Some(metadata_vec) => {
                    let metadata_result: Result<Vec<_>, _> = metadata_vec
                        .iter()
                        .map(|(key, val)| {
                            let value_fb = var_to_flatbuffer(val).map_err(|e| {
                                moor_var::EncodingError::CouldNotEncode(format!(
                                    "Failed to encode metadata value: {e}"
                                ))
                            })?;
                            Ok(common::EventMetadata {
                                key: Box::new(common::Symbol {
                                    value: key.as_string(),
                                }),
                                value: Box::new(value_fb),
                            })
                        })
                        .collect();
                    Some(metadata_result?)
                }
                None => None,
            };

            common::EventUnion::NotifyEvent(Box::new(common::NotifyEvent {
                value: Box::new(value_fb),
                content_type: content_type.as_ref().map(|s| {
                    Box::new(common::Symbol {
                        value: s.as_string(),
                    })
                }),
                no_flush: *no_flush,
                no_newline: *no_newline,
                metadata: metadata_fb,
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
            let error_fb = error_to_flatbuffer_struct(&exception.error).map_err(|e| {
                moor_var::EncodingError::CouldNotEncode(format!("Failed to encode error: {e}"))
            })?;
            let stack_fb: Result<Vec<_>, _> = exception
                .stack
                .iter()
                .map(|v| {
                    var_to_flatbuffer(v).map_err(|e| {
                        moor_var::EncodingError::CouldNotEncode(format!(
                            "Failed to encode stack item: {e}"
                        ))
                    })
                })
                .collect();
            let backtrace_fb: Result<Vec<_>, _> = exception
                .backtrace
                .iter()
                .map(|v| {
                    var_to_flatbuffer(v).map_err(|e| {
                        moor_var::EncodingError::CouldNotEncode(format!(
                            "Failed to encode backtrace item: {e}"
                        ))
                    })
                })
                .collect();

            common::EventUnion::TracebackEvent(Box::new(common::TracebackEvent {
                exception: Box::new(common::Exception {
                    error: Box::new(error_fb),
                    stack: stack_fb?,
                    backtrace: backtrace_fb?,
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
    let author_fb = var_to_flatbuffer(&narrative_event.author).map_err(|e| {
        moor_var::EncodingError::CouldNotEncode(format!("Failed to encode author: {e}"))
    })?;
    let event_fb = event_to_flatbuffer_struct(&narrative_event.event)?;

    let timestamp_nanos = narrative_event
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    Ok(common::NarrativeEvent {
        event_id: Box::new(uuid_to_flatbuffer_struct(&narrative_event.event_id)),
        timestamp: timestamp_nanos,
        author: Box::new(author_fb),
        event: Box::new(event_fb),
    })
}
