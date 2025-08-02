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

use crate::model::{ObjSet, PrepSpec, WorldStateError};
use bincode::{Decode, Encode};
use moor_var::{Obj, Symbol, Var};

pub mod command_parse;
pub mod complex_match;
pub mod complex_object_matcher;
pub mod match_env;
#[doc(hidden)]
pub mod mock_matching_env;
mod prepositions;
pub mod ws_match_env;

pub use command_parse::DefaultParseCommand;
pub use complex_match::{ComplexMatchResult, complex_match_strings, complex_match_objects_keys, parse_ordinal, parse_input_token};
pub use complex_object_matcher::ComplexObjectNameMatcher;
pub use match_env::DefaultObjectNameMatcher;
pub use ws_match_env::WsMatchEnv;

pub use prepositions::{Preposition, find_preposition};

/// Output from command matching, which is then used to match against the verb present in the
/// environment.
#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub struct ParsedCommand {
    pub verb: Symbol,
    pub argstr: String,
    pub args: Vec<Var>,

    pub dobjstr: Option<String>,
    pub dobj: Option<Obj>,

    pub prepstr: Option<String>,
    pub prep: PrepSpec,

    pub iobjstr: Option<String>,
    pub iobj: Option<Obj>,
}

/// The command parser interface. This is used to parse a command string into a ParsedCommand, or
/// return an error if the command is invalid.
pub trait CommandParser<M: ObjectNameMatcher> {
    fn parse_command(&self, command: &str, env: &M) -> Result<ParsedCommand, ParseCommandError>;
}

/// This is the interface that the matching code needs to be able to call into the world state.
/// Separated out so can be more easily mocked.
pub trait MatchEnvironment {
    // Test whether a given object is valid in this environment.
    fn obj_valid(&self, oid: &Obj) -> Result<bool, WorldStateError>;

    // Return all match names & aliases for an object.
    fn get_names(&self, oid: &Obj) -> Result<Vec<String>, WorldStateError>;

    // Returns location, contents, and player, all the things we'd search for matches on.
    fn get_surroundings(&self, player: &Obj) -> Result<ObjSet, WorldStateError>;

    // Return the location of a given object.
    fn location_of(&self, player: &Obj) -> Result<Obj, WorldStateError>;
}

#[derive(thiserror::Error, Debug, Clone, Decode, Encode)]
pub enum ParseCommandError {
    #[error("Empty command")]
    EmptyCommand,
    #[error("Unimplemented built-in command")]
    UnimplementedBuiltInCommand,
    #[error("Error occurred during object matching")]
    ErrorDuringMatch(WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
}

/// Trait for matching names in the environment. This is used by the command parser to find
/// objects in the world state that match the entities given in the command.
pub trait ObjectNameMatcher {
    fn match_object(&self, name: &str) -> Result<Option<Obj>, WorldStateError>;
}
