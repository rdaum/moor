//! Thread-local transaction context for eliminating WorldState parameter threading.
//! Provides RAII-based transaction management with automatic cleanup.

use std::cell::RefCell;

use moor_common::model::{CommitResult, WorldState, WorldStateError};

thread_local! {
    static CURRENT_TRANSACTION: RefCell<Option<Box<dyn WorldState>>> = RefCell::new(None);
}

/// RAII guard that ensures transaction cleanup on drop.
/// Transaction must be explicitly committed or rolled back before drop.
pub struct TransactionGuard(());

impl TransactionGuard {
    /// Start a new transaction on the current thread.
    /// Panics if a transaction is already active.
    pub fn new(tx: Box<dyn WorldState>) -> Self {
        CURRENT_TRANSACTION.with(|t| {
            let mut current = t.borrow_mut();
            assert!(
                current.is_none(),
                "Transaction already active on this thread"
            );
            *current = Some(tx);
        });
        TransactionGuard(())
    }
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        // Emergency cleanup - rollback any remaining transaction
        CURRENT_TRANSACTION.with(|t| {
            if let Some(tx) = t.borrow_mut().take() {
                tracing::warn!(
                    "Transaction dropped without explicit commit/rollback, rolling back"
                );
                let _ = tx.rollback(); // Best effort cleanup
            }
        });
    }
}

/// Execute a closure with access to the current transaction.
/// Panics if no transaction is active.
pub fn with_current_transaction<R>(f: impl FnOnce(&dyn WorldState) -> R) -> R {
    CURRENT_TRANSACTION.with(|t| {
        let tx_ref = t.borrow();
        let tx = tx_ref
            .as_ref()
            .expect("No active transaction on this thread");
        f(tx.as_ref())
    })
}

/// Execute a closure with mutable access to the current transaction.
/// Panics if no transaction is active.
pub fn with_current_transaction_mut<R>(f: impl FnOnce(&mut dyn WorldState) -> R) -> R {
    CURRENT_TRANSACTION.with(|t| {
        let mut tx_ref = t.borrow_mut();
        let tx = tx_ref
            .as_mut()
            .expect("No active transaction on this thread");
        f(tx.as_mut())
    })
}

/// Commit the current thread's active transaction.
/// Panics if no transaction is active.
pub fn commit_current_transaction() -> Result<CommitResult, WorldStateError> {
    CURRENT_TRANSACTION.with(|t| {
        let tx = t
            .borrow_mut()
            .take()
            .expect("No active transaction to commit");
        tx.commit()
    })
}

/// Rollback the current thread's active transaction.
/// Panics if no transaction is active.
pub fn rollback_current_transaction() -> Result<(), WorldStateError> {
    CURRENT_TRANSACTION.with(|t| {
        let tx = t
            .borrow_mut()
            .take()
            .expect("No active transaction to rollback");
        tx.rollback()
    })
}

/// Check if there's an active transaction on the current thread.
pub fn has_active_transaction() -> bool {
    CURRENT_TRANSACTION.with(|t| t.borrow().is_some())
}

/// Extract the current transaction from thread-local storage.
/// This is a transitional helper for compatibility with existing parameter-passing code.
/// Panics if no transaction is active.
pub fn extract_current_transaction() -> Box<dyn WorldState> {
    CURRENT_TRANSACTION.with(|t| {
        t.borrow_mut()
            .take()
            .expect("No active transaction to extract")
    })
}

/// Replace the current transaction in thread-local storage.
/// This is a transitional helper for compatibility with existing parameter-passing code.
/// Panics if a transaction is already active.
pub fn replace_current_transaction(tx: Box<dyn WorldState>) {
    CURRENT_TRANSACTION.with(|t| {
        let mut current = t.borrow_mut();
        assert!(
            current.is_none(),
            "Transaction already active when trying to replace"
        );
        *current = Some(tx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // For now, we just test the basic guard functionality without a full WorldState mock
    // since implementing the full WorldState trait would be quite large

    #[test]
    fn test_no_transaction_initially() {
        assert!(!has_active_transaction());
    }

    #[test]
    #[should_panic(expected = "No active transaction")]
    fn test_panic_on_no_transaction() {
        with_current_transaction(|_| ());
    }

    #[test]
    #[should_panic(expected = "No active transaction to commit")]
    fn test_panic_on_commit_no_transaction() {
        commit_current_transaction().unwrap();
    }

    #[test]
    #[should_panic(expected = "No active transaction to rollback")]
    fn test_panic_on_rollback_no_transaction() {
        rollback_current_transaction().unwrap();
    }
}
