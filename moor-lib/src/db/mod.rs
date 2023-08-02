pub mod matching;

pub mod match_env;
#[doc(hidden)]
pub mod mock_matching_env;
#[doc(hidden)]
pub mod mock_world_state;
pub mod rocksdb;

pub enum CommitResult {
    Success, // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx
             // TODO: timeout/task-too-long
}

// TODO: not sure this is the most appropriate place; used to be in tasks/command_parse.rs, but
// is needed elsewhere (by verb_args, etc)
// Putting here in DB because it's kinda version/DB specific, but not sure it's the best place.
pub const PREP_LIST: [&str; 15] = [
    "with/using",
    "at/to",
    "in front of",
    "in/inside/into",
    "on top of/on/onto/upon",
    "out of/from inside/from",
    "over",
    "through",
    "under/underneath/beneath",
    "behind",
    "beside",
    "for/about",
    "is",
    "as",
    "off/off of",
];
