pub mod matching;
pub mod state;
pub mod inmem_db;
pub mod inmem_db_tx;

#[doc(hidden)]
pub mod mock_matching_env;
mod relations;
pub mod tx;

pub enum CommitResult {
    Success,       // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx
}

