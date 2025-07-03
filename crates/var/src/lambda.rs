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

use crate::Var;
use crate::program::opcode::ScatterArgs;
use crate::program::program::Program;
use bincode::{Decode, Encode};

/// Lambda function value containing parameter specification, compiled body, and captured environment
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct Lambda {
    /// Parameter specification (reuses scatter assignment structure)
    pub params: ScatterArgs,
    /// The lambda body as standalone executable program
    /// Compiled at compile-time into a complete, self-contained Program
    pub body: Program,
    /// Captured variable environment from lambda creation site
    pub captured_env: Vec<Vec<Var>>,
}

impl Lambda {
    /// Create a new lambda value
    pub fn new(params: ScatterArgs, body: Program, captured_env: Vec<Vec<Var>>) -> Self {
        Self {
            params,
            body,
            captured_env,
        }
    }
}
