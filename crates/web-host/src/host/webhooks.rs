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

use crate::host::WebHost;
use axum::{
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use moor_common::tasks::SchedulerError;
use moor_schema::{convert::var_from_flatbuffer_ref, rpc as moor_rpc};
use moor_var::{List, SYSTEM_OBJECT, Var, Variant};
use rpc_common::{
    mk_detach_msg, mk_invoke_system_handler_msg, read_reply_result, scheduler_error_from_ref,
};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tracing::{debug, error};

/// Helper function to create internal server error responses with logging
fn internal_server_error(msg: &str) -> Response {
    error!(msg);
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

/// Helper function to handle FlatBuffer conversion errors
fn handle_conversion_error<T>(
    result: Result<T, impl std::fmt::Display>,
    context: &str,
) -> Result<T, Box<Response>> {
    result.map_err(|e| {
        error!("{}: {}", context, e);
        Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response())
    })
}

#[derive(Debug)]
pub struct WebHookRequest {
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Bytes>,
    pub client_ip: String,
}

impl WebHookRequest {
    pub fn from_http(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: Option<Bytes>,
        client_ip: SocketAddr,
    ) -> Self {
        let query_params = uri
            .query()
            .map(|q| {
                url::form_urlencoded::parse(q.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_default();

        let headers_vec = headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or("").to_string(),
                )
            })
            .collect();

        Self {
            method: method.as_str().to_string(),
            path: uri.path().to_string(),
            query_params,
            headers: headers_vec,
            body,
            client_ip: client_ip.to_string(),
        }
    }

    /// Helper function to convert a HashMap to a MOO alist of [key, value] pairs
    fn hashmap_to_alist(map: &HashMap<String, String>) -> Var {
        let pairs: Vec<Var> = map
            .iter()
            .map(|(k, v)| {
                let pair = List::mk_list(&[Var::from(k.clone()), Var::from(v.clone())]);
                Var::from(pair)
            })
            .collect();
        Var::from(List::mk_list(&pairs))
    }

    /// Helper function to convert a vector of (key, value) pairs to a MOO alist of [key, value] pairs
    fn vec_to_alist(pairs: &[(String, String)]) -> Var {
        let moo_pairs: Vec<Var> = pairs
            .iter()
            .map(|(k, v)| {
                let pair = List::mk_list(&[Var::from(k.clone()), Var::from(v.clone())]);
                Var::from(pair)
            })
            .collect();
        Var::from(List::mk_list(&moo_pairs))
    }

    pub fn to_moo_args(&self) -> Vec<Var> {
        let method_var = Var::from(self.method.clone());
        let path_var = Var::from(self.path.clone());

        // Convert query params to MOO alist of [key, value] pairs
        let query_var = Self::hashmap_to_alist(&self.query_params);

        // Convert headers to MOO alist of [key, value] pairs
        let headers_var = Self::vec_to_alist(&self.headers);

        // Convert body to MOO string or binary
        let body_var = match &self.body {
            Some(bytes) => {
                // Try to decode as UTF-8 string, fall back to binary
                match std::str::from_utf8(bytes) {
                    Ok(s) => Var::from(s.to_string()),
                    Err(_) => Var::from(bytes.to_vec()),
                }
            }
            None => Var::from(""),
        };

        let client_ip_var = Var::from(self.client_ip.clone());

        vec![
            method_var,
            path_var,
            query_var,
            headers_var,
            body_var,
            client_ip_var,
        ]
    }
}

// Main web hook handler - catch-all for all web hook paths
pub async fn web_hook_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    debug!("Webhook request received: {} {}", method, uri);

    let handler_object = SYSTEM_OBJECT;
    debug!(
        "Webhook handler: object={}, will call invoke_http_handler",
        handler_object
    );

    // Create web hook request data
    let webhook_request = WebHookRequest::from_http(method, uri, headers, Some(body), addr);
    let args = webhook_request.to_moo_args();

    // Establish temporary connection for web hook
    let (client_id, mut rpc_client, client_token) =
        match host.establish_client_connection(addr).await {
            Ok(connection) => connection,
            Err(e) => {
                return internal_server_error(&format!(
                    "Failed to establish connection for web hook: {e}"
                ));
            }
        };

    // Prepare system handler invocation
    let args_refs: Vec<&Var> = args.iter().collect();
    debug!(
        "Preparing system handler invocation for webhook with {} args",
        args_refs.len()
    );
    let invoke_msg = match mk_invoke_system_handler_msg(
        &host.host_id,
        "http",
        args_refs,
        None, // No auth token - will run as system user
    ) {
        Some(msg) => msg,
        None => {
            return internal_server_error(
                "Failed to create invoke_system_handler message for web hook",
            );
        }
    };
    debug!("System handler message created successfully");

    // Execute with timeout - daemon will now wait for task completion
    let timeout_duration = Duration::from_secs(30); // Longer timeout for task completion
    debug!(
        "Making RPC call with timeout: {}ms",
        timeout_duration.as_millis()
    );
    let invoke_result = tokio::time::timeout(
        timeout_duration,
        crate::host::web_host::rpc_call(client_id, &mut rpc_client, invoke_msg),
    )
    .await;

    let reply_bytes = match invoke_result {
        Ok(Ok(bytes)) => {
            debug!("RPC call succeeded, response size: {} bytes", bytes.len());
            bytes
        }
        Ok(Err(status)) => {
            error!("RPC call failed for web hook: {:?}", status);
            return status.into_response();
        }
        Err(_) => {
            error!("Web hook execution timed out");
            return StatusCode::REQUEST_TIMEOUT.into_response();
        }
    };

    let reply = match read_reply_result(&reply_bytes) {
        Ok(reply) => reply,
        Err(e) => {
            return internal_server_error(&format!("Failed to parse web hook reply: {e}"));
        }
    };

    // Handle the response
    let result = match reply.result() {
        Ok(result) => result,
        Err(e) => {
            return internal_server_error(&format!("Failed to get result from webhook reply: {e}"));
        }
    };

    let response = match result {
        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
            handle_client_success_response(client_success)
        }
        moor_rpc::ReplyResultUnionRef::Failure(failure) => handle_failure_response(failure),
        moor_rpc::ReplyResultUnionRef::HostSuccess(_) => {
            error!("Unexpected host success for web hook");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

    debug!("Response: {response:?}");

    // Hard detach for ephemeral HTTP connections - immediate cleanup
    let detach_msg = mk_detach_msg(&client_token, true);
    let _ = rpc_client.make_client_rpc_call(client_id, detach_msg).await;

    response
}

/// Handle client success response from daemon
fn handle_client_success_response(client_success: moor_rpc::ClientSuccessRef<'_>) -> Response {
    let daemon_reply = match handle_conversion_error(
        client_success.reply(),
        "Failed to get reply from client success",
    ) {
        Ok(reply) => reply,
        Err(response) => return *response,
    };

    let reply_union = match handle_conversion_error(
        daemon_reply.reply(),
        "Failed to get reply union from daemon reply",
    ) {
        Ok(union) => union,
        Err(response) => return *response,
    };

    match reply_union {
        moor_rpc::DaemonToClientReplyUnionRef::SystemHandlerResponseReply(reply) => {
            handle_system_handler_response(reply)
        }
        _ => internal_server_error("Unexpected daemon to client reply for web hook"),
    }
}

/// Handle system handler response
fn handle_system_handler_response(reply: moor_rpc::SystemHandlerResponseReplyRef<'_>) -> Response {
    let response_union = match handle_conversion_error(
        reply.response(),
        "Failed to get response union from system handler",
    ) {
        Ok(union) => union,
        Err(response) => return *response,
    };

    match response_union {
        moor_rpc::SystemHandlerResponseUnionRef::SystemHandlerSuccess(success) => {
            // Convert FlatBuffer Var to moor_var::Var
            let result_ref = match handle_conversion_error(
                success.result(),
                "Failed to get result from system handler success",
            ) {
                Ok(result) => result,
                Err(response) => return *response,
            };

            let result_var = match var_from_flatbuffer_ref(result_ref) {
                Ok(var) => var,
                Err(e) => {
                    return internal_server_error(&format!(
                        "Failed to convert FlatBuffer Var: {e}"
                    ));
                }
            };

            // Handle the result based on its variant
            match handle_webhook_result(&result_var) {
                Ok(response) => {
                    debug!("Webhook task completed successfully");
                    response
                }
                Err(e) => {
                    error!("Failed to handle webhook result: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        moor_rpc::SystemHandlerResponseUnionRef::SystemHandlerError(error) => {
            handle_system_handler_error(error)
        }
    }
}

/// Handle system handler error
fn handle_system_handler_error(error: moor_rpc::SystemHandlerErrorRef<'_>) -> Response {
    let scheduler_error_ref = match handle_conversion_error(
        error.error(),
        "Failed to get scheduler error from system handler error",
    ) {
        Ok(error_ref) => error_ref,
        Err(response) => return *response,
    };

    match scheduler_error_from_ref(scheduler_error_ref) {
        Ok(scheduler_error) => {
            // Map scheduler errors to appropriate HTTP status codes
            let status_code = match &scheduler_error {
                SchedulerError::TaskAbortedVerbNotFound(_, _)
                | SchedulerError::ObjectResolutionFailed(_) => {
                    debug!("Resource not found - returning 404: {:?}", scheduler_error);
                    StatusCode::NOT_FOUND
                }
                SchedulerError::TaskAbortedLimit(_)
                | SchedulerError::TaskAbortedCancelled
                | SchedulerError::SchedulerNotResponding
                | SchedulerError::CouldNotStartTask
                | SchedulerError::GarbageCollectionFailed(_) => {
                    debug!("Service unavailable error: {:?}", scheduler_error);
                    StatusCode::SERVICE_UNAVAILABLE
                }
                SchedulerError::CompilationError(_) | SchedulerError::CommandExecutionError(_) => {
                    debug!("Bad request error: {:?}", scheduler_error);
                    StatusCode::BAD_REQUEST
                }
                _ => {
                    error!("Internal server error for web hook: {:?}", scheduler_error);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            status_code.into_response()
        }
        Err(e) => internal_server_error(&format!(
            "Failed to parse scheduler error for web hook: {e}"
        )),
    }
}

/// Handle failure response from daemon
fn handle_failure_response(failure: moor_rpc::FailureRef<'_>) -> Response {
    let error_ref =
        match handle_conversion_error(failure.error(), "Failed to get error from failure response")
        {
            Ok(error_ref) => error_ref,
            Err(response) => return *response,
        };

    let error_code = match handle_conversion_error(
        error_ref.error_code(),
        "Failed to get error code from failure",
    ) {
        Ok(error_code) => error_code,
        Err(response) => return *response,
    };

    let error_msg = match error_ref.message() {
        Ok(Some(error_msg)) => error_msg,
        Ok(None) => {
            return internal_server_error("Missing error message in failure");
        }
        Err(e) => {
            return internal_server_error(&format!(
                "Failed to get error message from failure: {e}"
            ));
        }
    };

    error!(?error_code, ?error_msg, "RPC error for web hook");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

/// Handle webhook result and convert it to appropriate HTTP response
fn handle_webhook_result(result: &Var) -> Result<Response, Box<dyn std::error::Error>> {
    match result.variant() {
        Variant::Str(s) => {
            // String response - return as text/plain
            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain; charset=utf-8")
                .body(axum::body::Body::from(s.as_str().to_string()))
                .map_err(|e| format!("Failed to build response: {}", e))?;
            Ok(response)
        }
        Variant::Binary(b) => {
            // Binary response - return as application/octet-stream
            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/octet-stream")
                .body(axum::body::Body::from(b.as_bytes().to_vec()))
                .map_err(|e| format!("Failed to build response: {e}"))?;
            Ok(response)
        }
        Variant::List(l) => {
            // List response - expected format: [response_code, body, content_type, headers]
            let elements: Vec<Var> = l.iter().collect();

            if elements.len() < 2 {
                return Err(
                    "List response must have at least 2 elements: [response_code, body]".into(),
                );
            }

            // Extract response code (first element)
            let Variant::Int(response_code) = elements[0].variant() else {
                return Err("First element must be an integer response code".into());
            };

            // Extract body (second element)
            let body_bytes = match elements[1].variant() {
                Variant::Str(s) => s.as_str().as_bytes().to_vec(),
                Variant::Binary(b) => b.as_bytes().to_vec(),
                _ => return Err("Second element must be a string or binary body".into()),
            };

            // Build response builder
            let mut response_builder = axum::response::Response::builder().status(
                StatusCode::from_u16(response_code as u16)
                    .map_err(|e| format!("Invalid status code {response_code}: {e}"))?,
            );

            // Handle content type (third element, optional)
            if elements.len() > 2 {
                let Variant::Str(content_type_str) = elements[2].variant() else {
                    return Err("Third element must be a string content type".into());
                };
                response_builder =
                    response_builder.header("content-type", content_type_str.as_str());
            }

            // Handle headers (fourth element, optional list of [key, value] pairs)
            if elements.len() > 3 {
                let Variant::List(headers_list) = elements[3].variant() else {
                    return Err("Fourth element must be a list of headers".into());
                };

                for header_pair in headers_list.iter() {
                    let Variant::List(header_list) = header_pair.variant() else {
                        return Err("Header must be a list [key, value]".into());
                    };

                    let header_elements: Vec<Var> = header_list.iter().collect();
                    if header_elements.len() != 2 {
                        return Err("Header pair must have exactly 2 elements: [key, value]".into());
                    }

                    // Extract key (must be a Symbol)
                    let Ok(key) = header_elements[0].as_symbol() else {
                        return Err("Header key must be a symbol".into());
                    };

                    // Extract value (must be a String)
                    let Variant::Str(value_str) = header_elements[1].variant() else {
                        return Err("Header value must be a string".into());
                    };

                    response_builder =
                        response_builder.header(key.as_arc_str().as_str(), value_str.as_str());
                }
            }

            let response = response_builder
                .body(axum::body::Body::from(body_bytes))
                .map_err(|e| format!("Failed to build response: {e}"))?;
            Ok(response)
        }
        _ => Err(format!("Unsupported result variant: {:?}", result.variant()).into()),
    }
}
