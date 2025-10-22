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
pub use host::{HostType, extract_host_type};
pub use tokens::{
    AuthToken, ClientToken, KeyError, MOOR_AUTH_TOKEN_FOOTER, MOOR_SESSION_TOKEN_FOOTER,
    load_keypair, parse_keypair,
};
pub use worker::DaemonToWorkerReply;

pub use tokens::{auth_token_from_ref, client_token_from_ref};

// Re-export extraction helpers
pub use extract::{
    extract_field, extract_field_rpc, extract_obj, extract_obj_rpc, extract_object_ref,
    extract_object_ref_rpc, extract_string, extract_string_list, extract_string_list_rpc,
    extract_string_rpc, extract_symbol, extract_symbol_list, extract_symbol_rpc, extract_uuid,
    extract_uuid_rpc, extract_var, extract_var_list, extract_var_rpc,
};

pub use errors::*;

// Re-export FlatBuffer construction helpers
pub use helpers::{
    auth_token_fb, client_token_fb, obj_fb, objectref_fb, string_list_fb, symbol_fb,
    symbol_list_fb, uuid_fb, var_fb,
};

// Re-export client message builders
pub use client_messages::{
    mk_attach_msg, mk_client_pong_msg, mk_command_msg, mk_connection_establish_msg,
    mk_delete_event_log_history_msg, mk_detach_msg, mk_dismiss_presentation_msg, mk_eval_msg,
    mk_get_event_log_pubkey_msg, mk_invoke_verb_msg, mk_list_objects_msg, mk_login_command_msg,
    mk_out_of_band_msg, mk_program_msg, mk_properties_msg, mk_request_current_presentations_msg,
    mk_request_history_msg, mk_request_sys_prop_msg, mk_requested_input_msg, mk_resolve_msg,
    mk_retrieve_msg, mk_set_client_attribute_msg, mk_set_event_log_pubkey_msg,
    mk_update_property_msg, mk_verbs_msg,
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

// Re-export reply builders
pub use reply_builders::{
    mk_client_attribute_set_reply, mk_daemon_to_host_ack, mk_disconnected_reply,
    mk_new_connection_reply, mk_presentation_dismissed_reply, mk_thanks_pong_reply,
    var_to_flatbuffer_rpc,
};

pub use enrollment::{EnrollmentRequest, EnrollmentResponse};

// Public modules - allow direct access to generated types and client args
pub mod client_args;

// Private domain-organized modules
mod client_messages;
mod enrollment;
mod errors;
mod extract;
mod helpers;
mod host;
mod host_messages;
mod reply_builders;
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
