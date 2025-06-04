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

use moor_var::Symbol;
use thiserror::Error;

/// Error type for the Datalog system
#[derive(Error, Debug, Clone, PartialEq)]
pub enum DatalogError {
    /// Program contains cycles through negation that make it unstratifiable
    #[error("Program is not stratifiable: cycle detected involving predicates {predicates:?}")]
    Unstratifiable { predicates: Vec<Symbol> },

    /// Predicate arity mismatch between facts or between rule head and body
    #[error("Arity mismatch for predicate '{predicate}': expected {expected}, got {actual}")]
    ArityMismatch {
        predicate: Symbol,
        expected: usize,
        actual: usize,
    },

    /// Internal index inconsistency detected
    #[error("Index inconsistency detected for predicate '{predicate}': {details}")]
    IndexInconsistency { predicate: Symbol, details: String },

    /// Unexpected internal error that should not occur in normal operation
    #[error("Internal error: {details}")]
    Internal { details: String },
}

impl DatalogError {
    /// Create a stratification error from a list of predicates involved in the cycle
    pub fn unstratifiable(predicates: Vec<Symbol>) -> Self {
        Self::Unstratifiable { predicates }
    }

    /// Create an arity mismatch error
    pub fn arity_mismatch(predicate: Symbol, expected: usize, actual: usize) -> Self {
        Self::ArityMismatch {
            predicate,
            expected,
            actual,
        }
    }

    /// Create an internal error
    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal {
            details: details.into(),
        }
    }

    /// Create an index inconsistency error
    pub fn index_inconsistency(predicate: Symbol, details: impl Into<String>) -> Self {
        Self::IndexInconsistency {
            predicate,
            details: details.into(),
        }
    }

    /// Check if this error is recoverable (i.e., the system can continue operation)
    pub fn is_recoverable(&self) -> bool {
        match self {
            // These errors indicate fundamental problems that make continued operation unsafe
            Self::IndexInconsistency { .. } | Self::Internal { .. } => false,
            // Validation errors are recoverable - just need better input
            Self::Unstratifiable { .. } | Self::ArityMismatch { .. } => true,
        }
    }

    /// Check if this error indicates a programming bug vs user error
    pub fn is_programming_error(&self) -> bool {
        matches!(
            self,
            Self::IndexInconsistency { .. } | Self::Internal { .. }
        )
    }
}

/// Result type for Datalog operations
pub type DatalogResult<T> = Result<T, DatalogError>;
