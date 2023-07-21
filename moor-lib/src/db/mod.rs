pub mod matching;
pub mod state;

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
