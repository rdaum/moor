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

use crate::{Symbol, Var};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// A narrative event is a record of something that happened in the world, and is what `bf_notify`
/// or similar ultimately create.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct NarrativeEvent {
    /// When the event happened, in the server's system time.
    pub timestamp: SystemTime,
    /// The object that authored or caused the event.
    pub author: Var,
    /// The event itself.
    pub event: Event,
}

/// Types of events we can send to the session.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum Event {
    /// The typical "something happened" descriptive event.
    /// Value & Content-Type
    Notify(Var, Option<Symbol>),
    /// A "presentation" event, which is a recommendation to the client to present something to the
    /// user in a particular way.
    Present(Presentation),
    /// A "unpresent" event, which is a recommendation to the client to remove a presentation (identified with a string)
    /// from the user interface.
    Unpresent(String),
    // TODO: Other Event types on Session stream
    //   other events that might happen here would be things like (local) "object moved" or "object
    //   created."
}

/// A recommended "presentation" to the client. E.g. a pop-up, a panel, widget, etc. Not necessarily
/// "momentary" event in the narrative like a "notify" event, but something that should be placed
/// in the user interface in a client-interpreted fashion.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Presentation {
    /// A unique identifier for this presentation. If a new presentation is sent with the same id,
    /// the client should replace the existing presentation with the new one.
    pub id: String,
    /// The content-type of the presentation, e.g. text/html, text/plain, text/djot, etc.
    pub content_type: String,
    /// The actual content. String for now. We might want to support binary content in the future.
    pub content: String,
    /// A client-interpretable identifier for "where" this should be presented. E.g. a window or
    /// geometry identifier. ("right", "bottom", "popup", etc.)
    pub target: String,
    /// A bag of attributes that the client can use to interpret the presentation. E.g. "title",
    /// "width", "height", etc.
    pub attributes: Vec<(String, String)>,
}

impl NarrativeEvent {
    #[must_use]
    pub fn notify(author: Var, value: Var, content_type: Option<Symbol>) -> Self {
        Self {
            timestamp: SystemTime::now(),
            author,
            event: Event::Notify(value, content_type),
        }
    }

    #[must_use]
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
    #[must_use]
    pub fn author(&self) -> &Var {
        &self.author
    }
    #[must_use]
    pub fn event(&self) -> Event {
        self.event.clone()
    }
}
