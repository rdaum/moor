// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use std::fmt::Debug;
use std::time::SystemTime;

pub use crate::model::defset::{Defs, DefsIter, HasUuid, Named};
pub use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag};
pub use crate::model::objset::{ObjSet, ObjSetIter};
pub use crate::model::permissions::Perms;
pub use crate::model::propdef::{PropDef, PropDefs};
pub use crate::model::props::{PropAttr, PropAttrs, PropFlag, PropPerms};
pub use crate::model::r#match::{ArgSpec, PrepSpec, Preposition, VerbArgsSpec};
pub use crate::model::verb_info::VerbInfo;
pub use crate::model::verbdef::{VerbDef, VerbDefs};
pub use crate::model::verbs::{BinaryType, VerbAttr, VerbAttrs, VerbFlag, Vid};
pub use crate::model::world_state::{WorldState, WorldStateSource};
use crate::AsByteBuffer;

use crate::var::Objid;

mod defset;
mod r#match;
mod objects;
mod objset;
mod permissions;
mod propdef;
mod props;
mod tasks;
mod verb_info;
mod verbdef;
mod verbs;
mod world_state;

pub use tasks::{
    AbortLimitReason, CommandError, CompileError, SchedulerError, TaskId, TaskResult,
    UncaughtException, VerbProgramError,
};

pub use world_state::WorldStateError;

/// The result code from a commit/complete operation on the world's state.
#[derive(Debug, Eq, PartialEq)]
pub enum CommitResult {
    Success,       // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx
}

pub trait ValSet<V: AsByteBuffer>: FromIterator<V> {
    fn empty() -> Self;
    fn from_items(items: &[V]) -> Self;
    fn iter(&self) -> impl Iterator<Item = V>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

/// A narrative event is a record of something that happened in the world, and is what `bf_notify`
/// or similar ultimately create.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct NarrativeEvent {
    /// When the event happened, in the server's system time.
    timestamp: SystemTime,
    /// The object that authored or caused the event.
    author: Objid,
    /// The event itself.
    pub event: Event,
}

/// Types of events we can send to the session.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum Event {
    /// The typical "something happened" descriptive event.
    TextNotify(String),
    // TODO: Other Event types on Session stream
    //   other events that might happen here would be things like (local) "object moved" or "object
    //   created."
}

impl NarrativeEvent {
    #[must_use]
    pub fn notify_text(author: Objid, event: String) -> Self {
        Self {
            timestamp: SystemTime::now(),
            author,
            event: Event::TextNotify(event),
        }
    }

    #[must_use]
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
    #[must_use]
    pub fn author(&self) -> Objid {
        self.author
    }
    #[must_use]
    pub fn event(&self) -> Event {
        self.event.clone()
    }
}
