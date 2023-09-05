use std::future::ready;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use metrics_macros::increment_counter;

use crate::server::var_as_json;
use moor_value::model::world_state::WorldStateSource;
use moor_value::var::variant::Variant;
use moor_value::SYSTEM_OBJECT;

// Properties of the form $sysprop.property.  E.g. $login.welcome_message
// TODO: support for top-level props.  e.g. $welcome_message, should we need that.
const WHITELISTED_PUBLIC_SYSTEM_PROPERTIES: [(&str, &str); 1] = [("login", "welcome_message")];

use crate::server::ws_server::{ws_connect_handler, ws_create_handler, WebSocketServer};

fn setup_metrics_recorder() -> PrometheusHandle {
    PrometheusBuilder::new().install_recorder().unwrap()
}

pub fn mk_routes(state_source: Arc<dyn WorldStateSource>, ws_server: WebSocketServer) -> Router {
    let recorder_handle = setup_metrics_recorder();

    // The router for websocket requests
    let websocket_router = Router::new()
        .route("/connect", get(ws_connect_handler))
        .route("/create", get(ws_create_handler))
        .with_state(ws_server);

    // Public exposed properties available via GET and output as JSON.
    // These have to be explicitly whitelisted. Used for this like e.g. $login.welcome_message
    let property_router = Router::new()
        .route("/:object/:property", get(prop_get_handler))
        .with_state(state_source.clone());

    Router::new()
        .nest("/ws", websocket_router)
        .nest("/properties", property_router)
        .route("/metrics", get(move || ready(recorder_handle.render())))
}

/// Resource handler to retrieve unauthenticated system property. Properties must be whitelisted.
/// This pokes straight into the database to retrieve the value, without any permissions checks.
// TODO for now the only whitelisted property is $login.welcome_message, in the future the database
//  itself will be able to define which properties are available, but this requires core support.
async fn prop_get_handler(
    State(world_state_source): State<Arc<dyn WorldStateSource>>,
    Path((object, property)): Path<(String, String)>,
) -> impl IntoResponse {
    increment_counter!("server.welcome_message_handler");

    // We only support system properties, and only whitelisted ones, for now.
    // Anything else will require auth.
    if !object.starts_with('$') {
        return (
            StatusCode::NOT_FOUND,
            Json("Property unavailable or not found"),
        )
            .into_response();
    }

    let object = object[1..].to_string();

    if !WHITELISTED_PUBLIC_SYSTEM_PROPERTIES.contains(&(object.as_str(), property.as_str())) {
        return (
            StatusCode::NOT_FOUND,
            Json("Property unavailable or not found"),
        )
            .into_response();
    }

    let Ok(world_state) = world_state_source.new_world_state().await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Unable to get property"),
        )
            .into_response();
    };

    let Ok(sysprop) = world_state
        .retrieve_property(SYSTEM_OBJECT, SYSTEM_OBJECT, object.as_str())
        .await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json("Property unavailable or not found"),
        )
            .into_response();
    };

    let Variant::Obj(sysprop) = sysprop.variant() else {
        return (
            StatusCode::NOT_FOUND,
            Json("Property unavailable or not found"),
        )
            .into_response();
    };

    let Ok(property_value) = world_state
        .retrieve_property(SYSTEM_OBJECT, *sysprop, property.as_str())
        .await
    else {
        return (
            StatusCode::NOT_FOUND,
            Json("Property unavailable or not found"),
        )
            .into_response();
    };

    (StatusCode::OK, Json(var_as_json(&property_value))).into_response()
}
