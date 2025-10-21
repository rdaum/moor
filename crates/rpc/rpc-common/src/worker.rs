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

use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DaemonToWorkerReply {
    Ack,
    Rejected(String),
    /// Let the worker know that it is attached to the daemon.
    Attached(Uuid),
    AuthFailed(String),
    InvalidPayload(String),
    UnknownRequest(Uuid),
    NotRegistered(Uuid),
}
