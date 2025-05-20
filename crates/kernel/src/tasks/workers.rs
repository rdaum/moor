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

use moor_common::tasks::WorkerError;
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

#[derive(Debug)]
pub enum WorkerRequest {
    /// A request to a worker of X type, with an optional response channel.
    /// We will pick a worker of the given type to send the request to.
    Request {
        request_id: Uuid,
        request_type: Symbol,
        perms: Obj,
        request: Vec<Var>,
        timeout: Option<std::time::Duration>,
    },
}

#[derive(Debug)]
pub enum WorkerResponse {
    Error {
        request_id: Uuid,
        error: WorkerError,
    },
    Response {
        request_id: Uuid,
        response: Vec<Var>,
    },
}
