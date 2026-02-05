// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

mod auth;
mod event_log;
pub(crate) mod negotiate;
mod oauth2;
mod oauth2_handlers;
mod objects;
mod props;
mod verbs;
pub mod web_host;
mod webhooks;
mod ws_connection;

pub use auth::{connect_auth_handler, create_auth_handler, logout_handler, validate_auth_handler};
pub use event_log::{
    delete_history_handler, dismiss_presentation_handler, get_pubkey_handler, history_handler,
    presentations_handler, set_pubkey_handler,
};
pub use oauth2::{OAuth2Config, OAuth2Manager, PendingOAuth2Store};
pub use oauth2_handlers::{
    OAuth2State, oauth2_account_choice_handler, oauth2_authorize_handler, oauth2_callback_handler,
    oauth2_config_handler, oauth2_exchange_handler,
};
pub use objects::{list_objects_handler, update_property_handler};
pub use props::{properties_handler, property_retrieval_handler};
pub use verbs::{invoke_verb_handler, verb_program_handler, verb_retrieval_handler, verbs_handler};
pub use web_host::{
    WebHost, eval_handler, features_handler, health_handler, invoke_welcome_message_handler,
    openapi_handler, resolve_objref_handler, system_property_handler, version_handler,
    ws_connect_attach_handler, ws_create_attach_handler,
};

pub use webhooks::web_hook_handler;

pub(crate) use negotiate::flatbuffer_response;
