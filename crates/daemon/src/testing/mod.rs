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

//! Testing utilities for the daemon

pub mod mock_event_log;
pub mod mock_transport;

#[cfg(test)]
mod rpc_integration_test;

#[cfg(test)]
mod scheduler_integration_test;

pub use crate::event_log::EventLogOps;
pub use mock_event_log::MockEventLog;
pub use mock_transport::MockTransport;
