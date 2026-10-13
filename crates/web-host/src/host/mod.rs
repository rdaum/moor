// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Web host state, routing, authentication, session, and HTTP handler modules.

mod actions;
mod auth;
mod handlers;
pub(crate) mod negotiate;
mod session;
pub mod web_host;

pub(crate) use session::webrtc;
pub use session::webrtc::WebRtcConfig;

pub use auth::{
    OAuth2Config, OAuth2Manager, OAuth2State, PendingOAuth2Store, oauth2_account_choice_handler,
    oauth2_app_account_choice_handler, oauth2_app_exchange_handler, oauth2_app_start_handler,
    oauth2_authorize_handler, oauth2_callback_handler, oauth2_config_handler,
    oauth2_exchange_handler,
};
pub use auth::{connect_auth_handler, create_auth_handler, logout_handler, validate_auth_handler};
pub use handlers::{
    acknowledge_events_handler, attach_stream_handler, batch_handler, command_handler,
    delete_history_handler, dismiss_presentation_handler, event_stream_handler, get_pubkey_handler,
    history_handler, invoke_verb_handler, list_objects_handler, presentations_handler,
    properties_handler, property_retrieval_handler, query_objects_handler, session_command_handler,
    session_input_handler, set_pubkey_handler, update_property_handler, verb_program_handler,
    verb_retrieval_handler, verbs_handler, web_hook_handler,
};
pub use web_host::{
    WebHost, eval_handler, features_handler, health_handler, invoke_welcome_message_handler,
    openapi_handler, resolve_objref_handler, system_property_handler, version_handler,
    ws_connect_attach_handler, ws_create_attach_handler,
};

pub(crate) use negotiate::flatbuffer_response;
