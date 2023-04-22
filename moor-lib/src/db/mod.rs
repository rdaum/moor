pub mod moor_db;
pub mod moor_db_worldstate;
pub mod matching;
pub mod state;

#[doc(hidden)]
pub mod mock_matching_env;
pub mod match_env;

pub enum CommitResult {
    Success,       // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx
}
