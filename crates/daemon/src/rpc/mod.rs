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

//! RPC server module for client and host connections
//!
//! Despite the name "RPC", this module actually handles:
//! - Client connections and session management
//! - Host connections and listener registration  
//! - Narrative event publishing (pub/sub)
//! - Task completion delivery
//! - System messages and input requests

#[cfg(test)]
pub mod hosts;
#[cfg(not(test))]
mod hosts;
#[cfg(test)]
pub mod message_handler;
#[cfg(not(test))]
mod message_handler;
mod message_handler_auth;
mod message_handler_history;
mod message_handler_tasks;
mod server;
mod session;
pub mod transport;

pub use message_handler::MessageHandler;
pub use server::RpcServer;
pub use session::SessionActions;
pub use transport::Transport;
