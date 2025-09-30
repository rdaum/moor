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

//! RPC common types and message builders for the moor distributed system
//!
//! This crate provides shared types, message builders, and conversion functions
//! for RPC communication between moor components (hosts, clients, workers, daemon).

use moor_common::tasks::SchedulerError;
use thiserror::Error;

// Re-export domain types
pub use host::HostType;
pub use tokens::{
    AuthToken, ClientToken, HostToken, KeyError, MOOR_AUTH_TOKEN_FOOTER, MOOR_HOST_TOKEN_FOOTER,
    MOOR_SESSION_TOKEN_FOOTER, MOOR_WORKER_TOKEN_FOOTER, WorkerToken, load_keypair,
    make_host_token, parse_keypair,
};
pub use worker::DaemonToWorkerReply;

// Re-export generic type conversions
pub use convert::{
    obj_from_flatbuffer_struct, obj_from_ref, obj_to_flatbuffer_struct, objectref_from_ref,
    objectref_to_flatbuffer_struct, symbol_from_flatbuffer_struct, symbol_from_ref,
    symbol_to_flatbuffer_struct, uuid_from_ref, uuid_to_flatbuffer_struct,
    var_from_flatbuffer_bytes, var_from_ref, var_to_flatbuffer_bytes,
};

pub use tokens::{auth_token_from_ref, client_token_from_ref};

// Re-export extraction helpers
pub use extract::{
    extract_field, extract_obj, extract_object_ref, extract_string, extract_string_list,
    extract_symbol, extract_symbol_list, extract_uuid, extract_var, extract_var_list,
};

// Re-export FlatBuffer construction helpers
pub use helpers::{
    auth_token_fb, client_token_fb, mk_worker_token, obj_fb, objectref_fb, string_list_fb,
    symbol_fb, symbol_list_fb, uuid_fb, var_fb,
};

// Re-export client message builders
pub use client_messages::{
    mk_attach_msg, mk_client_pong_msg, mk_command_msg, mk_connection_establish_msg, mk_detach_msg,
    mk_dismiss_presentation_msg, mk_eval_msg, mk_invoke_verb_msg, mk_login_command_msg,
    mk_out_of_band_msg, mk_program_msg, mk_properties_msg, mk_request_current_presentations_msg,
    mk_request_history_msg, mk_request_sys_prop_msg, mk_requested_input_msg, mk_resolve_msg,
    mk_retrieve_msg, mk_set_client_attribute_msg, mk_verbs_msg,
};
pub use errors::{
    command_error_to_flatbuffer_struct, compilation_error_from_ref,
    compilation_error_to_flatbuffer_struct, error_from_flatbuffer_struct,
    error_to_flatbuffer_struct, scheduler_error_from_ref, scheduler_error_to_flatbuffer_struct,
    verb_program_error_to_flatbuffer_struct, worker_error_from_flatbuffer_struct,
    worker_error_to_flatbuffer_struct, world_state_error_to_flatbuffer_struct,
};
pub use events::{
    event_from_ref, event_to_flatbuffer_struct, narrative_event_from_ref,
    narrative_event_to_flatbuffer_struct, presentation_from_ref, presentation_to_flatbuffer_struct,
};
pub use host_messages::{
    mk_detach_host_msg, mk_host_pong_msg, mk_register_host_msg, mk_request_performance_counters_msg,
};

// Re-export worker message builders
pub use worker_messages::{
    mk_attach_worker_msg, mk_detach_worker_msg, mk_ping_workers_msg, mk_request_error_msg,
    mk_request_result_msg, mk_worker_ack_reply, mk_worker_attached_reply,
    mk_worker_auth_failed_reply, mk_worker_invalid_payload_reply, mk_worker_not_registered_reply,
    mk_worker_pong_msg, mk_worker_rejected_reply, mk_worker_request_msg,
    mk_worker_unknown_request_reply,
};

// Public modules - allow direct access to generated types and client args
pub mod client_args;

// Private domain-organized modules
mod client_messages;
mod convert;
mod errors;
mod events;
mod extract;
mod helpers;
mod host;
mod host_messages;
mod tokens;
mod worker;
mod worker_messages;

/// A ZMQ topic for broadcasting to all clients of all hosts.
pub const CLIENT_BROADCAST_TOPIC: &[u8; 9] = b"broadcast";

/// A ZMQ topic for broadcasting to just the hosts.
pub const HOST_BROADCAST_TOPIC: &[u8; 5] = b"hosts";

/// A ZMQ topic for broadcasting to just the workers.
pub const WORKER_BROADCAST_TOPIC: &[u8; 7] = b"workers";

/// Errors at the RPC transport / encoding layer.
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("could not initiate session: {0}")]
    CouldNotInitiateSession(String),
    #[error("could not authenticate: {0}")]
    AuthenticationError(String),
    #[error("could not send RPC request: {0}")]
    CouldNotSend(String),
    #[error("could not receive RPC response: {0}")]
    CouldNotReceive(String),
    #[error("could not decode RPC response: {0}")]
    CouldNotDecode(String),
    #[error("unexpected reply: {0}")]
    UnexpectedReply(String),
}

/// Errors at the message passing level.
/// Note: This is an internal Rust error type, converted to FlatBuffer format for serialization.
#[derive(Debug, PartialEq, Error, Clone)]
pub enum RpcMessageError {
    #[error("Already connected")]
    AlreadyConnected,
    #[error("Invalid request")]
    InvalidRequest(String),
    #[error("No connection for client")]
    NoConnection,
    #[error("Could not retrieve system property")]
    ErrorCouldNotRetrieveSysProp(String),
    #[error("Could not login: {0}")]
    LoginTaskFailed(String),
    #[error("Could not create session")]
    CreateSessionFailed,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Error scheduling task")]
    TaskError(SchedulerError),
    #[error("Error retreiving entity: {0}")]
    EntityRetrievalError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}
