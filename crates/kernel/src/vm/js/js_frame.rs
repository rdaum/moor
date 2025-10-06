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

//! JavaScript frame execution using V8.
//! Implements async/await based suspend/resume for JavaScript verbs.

use moor_var::{List, Symbol, Var};

/// Information about a pending verb call from JavaScript
#[derive(Clone, Debug)]
pub struct PendingVerbCall {
    /// The object to call the verb on
    pub this: Var,
    /// The verb name to call
    pub verb_name: Symbol,
    /// Arguments to pass to the verb
    pub args: List,
    /// Result from the verb (filled in when verb completes)
    pub result: Option<Var>,
}

/// JavaScript execution frame state.
/// Stores the continuation point for async JavaScript execution.
/// NOTE: No V8 isolate stored here - isolates are acquired from thread-local pool on execution.
#[derive(Clone, Debug)]
pub struct JSFrame {
    /// The JavaScript source code for this verb
    pub(crate) source: String,

    /// Arguments passed to this JavaScript function
    pub(crate) args: Vec<Var>,

    /// Current continuation state
    pub(crate) continuation: JSContinuation,

    /// Return value for this frame (set when execution completes)
    pub(crate) return_value: Option<Var>,
}

/// Tracks where we are in JavaScript execution.
#[derive(Clone, Debug)]
pub enum JSContinuation {
    /// Initial state - need to start executing the function
    Initial,

    /// Waiting for a MOO verb call to complete
    /// Context is destroyed at this point - will be recreated on resume
    AwaitingVerbCall {
        /// Information about the verb call in progress
        call_info: PendingVerbCall,
    },

    /// Waiting for a Promise to resolve (from builtin or suspend)
    /// Context is destroyed at this point - will be recreated on resume
    /// NOTE: Not yet implemented - reserved for future functionality
    #[allow(dead_code)]
    AwaitingPromise {
        /// Name of the builtin we're waiting on (for debugging)
        waiting_on: String,
    },

    /// Completed execution
    Complete {
        /// The return value from JavaScript
        result: Var,
    },
}

impl JSFrame {
    /// Create a new JavaScript frame for executing source code
    pub fn new(source: String, args: Vec<Var>) -> Self {
        Self {
            source,
            args,
            continuation: JSContinuation::Initial,
            return_value: None,
        }
    }

    /// Set the return value for this frame
    pub fn set_return_value(&mut self, value: Var) {
        self.return_value = Some(value.clone());

        // Only mark as Complete if we're not awaiting a verb call
        // (preserve AwaitingVerbCall state for resume logic)
        match &self.continuation {
            JSContinuation::AwaitingVerbCall { .. } => {
                // Don't change continuation - execute_js_resume will handle this
            }
            _ => {
                self.continuation = JSContinuation::Complete { result: value };
            }
        }
    }
}
