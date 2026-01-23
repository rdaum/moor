// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Fatal error handling utilities for unrecoverable errors.
//!
//! This module provides a mechanism to handle fatal errors (like database I/O failures)
//! that require application shutdown. It ensures that:
//! - The error is logged only once (no log flooding)
//! - An emergency checkpoint is attempted before shutdown (via SIGUSR1)
//! - Subsequent errors are silently suppressed

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to track if we've already reported a fatal database error.
/// Once set, subsequent fatal errors are silently suppressed to prevent log flooding.
static FATAL_DB_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

/// Signal that a fatal database I/O error has occurred.
///
/// This function:
/// 1. Logs an error message (only on first call)
/// 2. Sends SIGUSR1 to trigger emergency checkpoint then graceful shutdown
/// 3. Returns `true` if this was the first call (error was newly reported)
///
/// Use this when encountering unrecoverable database errors like fjall's "Poisoned"
/// error, which indicates an fsync failure and data integrity concerns.
///
/// # Arguments
/// * `operation` - Description of the operation that failed (e.g., "insert", "delete")
/// * `error_details` - The error message/details to include in the log
///
/// # Returns
/// * `true` if this was the first fatal error reported (message was logged, signal sent)
/// * `false` if a fatal error was already reported (this call was suppressed)
pub fn signal_fatal_db_error(operation: &str, error_details: &str) -> bool {
    // Use swap to atomically check and set - returns previous value
    if FATAL_DB_ERROR_REPORTED.swap(true, Ordering::SeqCst) {
        // Already reported - suppress this error
        return false;
    }

    tracing::error!(
        "FATAL: Database I/O failure during {operation}: {error_details}. \
        This typically indicates disk full, filesystem error, or hardware failure. \
        The database is now in an unrecoverable state. \
        Attempting emergency checkpoint before shutdown. \
        Check disk space and filesystem health before restarting."
    );

    // Send SIGUSR1 to trigger checkpoint-then-shutdown
    // SAFETY: kill() with our own PID is safe
    unsafe {
        libc::kill(libc::getpid(), libc::SIGUSR1);
    }

    true
}

/// Check if a fatal database error has already been reported.
///
/// Useful for callers that want to skip expensive operations if shutdown is imminent.
pub fn is_fatal_db_error_reported() -> bool {
    FATAL_DB_ERROR_REPORTED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We can't easily test the SIGTERM behavior in unit tests,
    // but we can test the atomic flag behavior
    #[test]
    fn test_fatal_error_reported_flag() {
        // Reset for test (this is a bit hacky but ok for tests)
        FATAL_DB_ERROR_REPORTED.store(false, Ordering::SeqCst);

        assert!(!is_fatal_db_error_reported());

        // Note: We don't call signal_fatal_db_error in tests because it sends SIGUSR1
    }
}
