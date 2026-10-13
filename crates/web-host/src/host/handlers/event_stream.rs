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

//! SSE payload delivery from daemon-owned queues with browser acknowledgements.

use crate::host::{
    WebHost,
    actions::action_request,
    auth::{extract_auth_token_header, extract_client_credentials},
    web_host::WsHostError,
};
use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response, Sse, sse::Event},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::stream;
use moor_runtime_api::{
    AuthToken, ClientToken, RpcError, RpcMessageError,
    api::{
        ClientEventSubscription, ClientReply, ClientRequest, EventStreamOperation,
        EventStreamRequest, EventStreamState, InvocationMode, RuntimeClient,
    },
};
use moor_schema::{convert::var_from_flatbuffer_ref, rpc as moor_rpc, var::VarRef};
use moor_var::v_str;
use planus::ReadAsRoot;
use serde::Deserialize;
use serde_json::json;
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};
use uuid::Uuid;

fn rpc_status(error: RpcError) -> StatusCode {
    match error {
        RpcError::Daemon(RpcMessageError::NoConnection | RpcMessageError::EventStreamExpired) => {
            StatusCode::GONE
        }
        RpcError::Daemon(RpcMessageError::PermissionDenied) | RpcError::AuthenticationError(_) => {
            StatusCode::UNAUTHORIZED
        }
        RpcError::Daemon(RpcMessageError::InvalidRequest(_)) => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// A connection credential is portable between web-host instances sharing a daemon.
pub struct StreamClient {
    client_id: Uuid,
    client_token: ClientToken,
    rpc: Arc<dyn RuntimeClient>,
}
impl FromRequestParts<WebHost> for StreamClient {
    type Rejection = StatusCode;
    async fn from_request_parts(
        parts: &mut Parts,
        host: &WebHost,
    ) -> Result<Self, Self::Rejection> {
        let (client_id, client_token) =
            extract_client_credentials(&parts.headers).ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(Self {
            client_id,
            client_token,
            rpc: host.create_rpc_client(),
        })
    }
}
impl StreamClient {
    async fn request(&self, request: EventStreamRequest) -> Result<EventStreamState, StatusCode> {
        let reply = self
            .rpc
            .client_call(
                self.client_id,
                ClientRequest::EventStream {
                    client_token: self.client_token.clone(),
                    request,
                },
            )
            .await
            .map_err(rpc_status)?;
        let ClientReply::EventStream(state) = reply else {
            return Err(StatusCode::BAD_GATEWAY);
        };
        Ok(state)
    }
}

#[derive(Deserialize)]
pub struct AttachStream {
    #[serde(default)]
    initial: bool,
    #[serde(default)]
    create: bool,
}

/// Attach once; all subsequent delivery requests use the returned connection and stream IDs.
pub async fn attach_stream_handler(
    State(host): State<WebHost>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(query): Json<AttachStream>,
) -> Response {
    let auth_token = match extract_auth_token_header(&headers) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let hint = extract_client_credentials(&headers);
    let had_hint = hint.is_some();
    let mut attached = None;
    if !query.initial
        && let Some((id, token)) = hint
    {
        match host
            .reattach_authenticated(auth_token.clone(), id, token, addr)
            .await
        {
            Ok(details) => attached = Some(details),
            Err(WsHostError::StaleConnection | WsHostError::AuthenticationFailed) => {}
            Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
        }
    }
    let details = match attached {
        Some(details) => details,
        None => {
            let connect_type = if !query.initial && had_hint {
                moor_rpc::ConnectType::Reconnected
            } else if query.create {
                moor_rpc::ConnectType::Created
            } else {
                moor_rpc::ConnectType::Connected
            };
            match host
                .attach_authenticated(auth_token, Some(connect_type), addr)
                .await
            {
                Ok(details) => details,
                Err(WsHostError::AuthenticationFailed) => {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
            }
        }
    };
    let (_, client_id, client_token, rpc, _) = details;
    let client = StreamClient {
        client_id,
        client_token,
        rpc,
    };
    let state = match client
        .request(EventStreamRequest {
            operation: EventStreamOperation::Open,
            stream_id: None,
            sequence: 0,
            limit: 0,
        })
        .await
    {
        Ok(state) => state,
        Err(status) => return status.into_response(),
    };
    // Decimal strings preserve all 64 bits in JavaScript.
    let mut response = Json(json!({
        "client_id": client.client_id, "client_token": client.client_token.0,
        "stream_id": state.stream_id, "acknowledged_sequence": state.acknowledged_sequence.to_string(),
        "available_after": state.available_after.to_string(), "latest_sequence": state.latest_sequence.to_string(),
    })).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}

#[derive(Deserialize)]
pub struct StreamQuery {
    stream_id: Uuid,
    #[serde(default)]
    after: u64,
}

#[derive(Deserialize)]
pub struct AcknowledgeEvents {
    stream_id: Uuid,
    sequence: String,
}

pub async fn acknowledge_events_handler(
    client: StreamClient,
    Json(ack): Json<AcknowledgeEvents>,
) -> Response {
    let sequence = match ack.sequence.parse::<u64>() {
        Ok(sequence) => sequence,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match client
        .request(EventStreamRequest {
            operation: EventStreamOperation::Acknowledge,
            stream_id: Some(ack.stream_id),
            sequence,
            limit: 0,
        })
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(status) => status.into_response(),
    }
}

/// Holds at most one bounded RPC batch while its HTTP response is being consumed.
struct DeliveryStream {
    client: StreamClient,
    stream_id: Uuid,
    subscription: Box<dyn ClientEventSubscription>,
    tick: tokio::time::Interval,
    pending: std::vec::IntoIter<Arc<[u8]>>,
    cursor: u64,
    latest_sequence: u64,
    closed: bool,
}

impl DeliveryStream {
    fn failure(&mut self, status: StatusCode) -> Event {
        self.closed = true;
        if status == StatusCode::GONE {
            return Event::default().event("reset").data("delivery_expired");
        }
        Event::default().event("retry").data("delivery_unavailable")
    }

    async fn next_event(&mut self) -> Event {
        loop {
            if let Some(bytes) = self.pending.next() {
                let sequence = moor_rpc::ClientEventRef::read_as_root(&bytes)
                    .ok()
                    .and_then(|event| event.sequence().ok());
                let Some(sequence) = sequence.filter(|s| self.cursor.checked_add(1) == Some(*s))
                else {
                    return self.failure(StatusCode::BAD_GATEWAY);
                };
                self.cursor = sequence;
                return Event::default()
                    .event("delivery")
                    .id(format!("{}:{sequence}", self.stream_id))
                    .data(STANDARD.encode(&bytes));
            }
            if self.cursor >= self.latest_sequence {
                // Notifications only wake delivery; authenticated reads supply the payloads.
                // The immediate first tick closes the subscribe race. Later ticks recover loss.
                let notification = tokio::select! {
                    event = self.subscription.recv_client_event() => event.map(|event| Some(event.sequence)),
                    _ = self.tick.tick() => Ok(None),
                };
                match notification {
                    Ok(Some(sequence)) if sequence <= self.cursor => continue,
                    Ok(_) | Err(RpcError::Recoverable(_)) => {}
                    Err(_) => return self.failure(StatusCode::BAD_GATEWAY),
                }
            }
            let batch = self
                .client
                .request(EventStreamRequest {
                    operation: EventStreamOperation::Read,
                    stream_id: Some(self.stream_id),
                    sequence: self.cursor,
                    limit: 128,
                })
                .await;
            let batch = match batch {
                Ok(batch) if batch.available_after <= batch.acknowledged_sequence => batch,
                Ok(_) => return self.failure(StatusCode::GONE),
                Err(status) => return self.failure(status),
            };
            self.latest_sequence = batch.latest_sequence;
            self.pending = batch.payloads.into_iter();
            if self.pending.len() == 0 {
                if self.cursor < self.latest_sequence {
                    return self.failure(StatusCode::BAD_GATEWAY);
                }
                // Heartbeats do not advance the browser's processed cursor or renew liveness.
                return Event::default()
                    .event("heartbeat")
                    .data(self.cursor.to_string());
            }
        }
    }
}

/// Stream base64 FlatBuffers after the browser's processed cursor, without acknowledging them.
pub async fn event_stream_handler(
    State(host): State<WebHost>,
    client: StreamClient,
    Query(query): Query<StreamQuery>,
) -> Response {
    let initial = match client
        .request(EventStreamRequest {
            operation: EventStreamOperation::Read,
            stream_id: Some(query.stream_id),
            sequence: query.after,
            limit: 128,
        })
        .await
    {
        Ok(state) => state,
        Err(status) => return status.into_response(),
    };
    if initial.available_after > initial.acknowledged_sequence {
        return StatusCode::GONE.into_response();
    }
    // Authenticate before allocating a subscription. No recovering-host ACKs are allowed here.
    let subscription = match host
        .host_services
        .client_event_notifications(client.client_id)
    {
        Ok(subscription) => subscription,
        Err(error) => return rpc_status(error).into_response(),
    };
    let state = DeliveryStream {
        client,
        stream_id: query.stream_id,
        subscription,
        tick: tokio::time::interval(Duration::from_secs(5)),
        pending: initial.payloads.into_iter(),
        cursor: query.after,
        latest_sequence: initial.latest_sequence,
        closed: false,
    };
    let events = stream::unfold(state, |mut state| async move {
        if state.closed {
            return None;
        }
        let event = state.next_event().await;
        Some((Ok::<_, Infallible>(event), state))
    });
    let mut response = Sse::new(events).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-accel-buffering", "no".parse().unwrap());
    response
}

/// Connected command execution retains the connection identity across HTTP requests.
pub async fn session_command_handler(
    State(host): State<WebHost>,
    client: StreamClient,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth_token = match extract_auth_token_header(&headers) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let mode = InvocationMode::Connected {
        client_token: client.client_token.clone(),
    };
    let request = if headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            h.split(';')
                .next()
                .is_some_and(|t| t.trim() == "application/json")
        }) {
        match action_request(
            &body,
            host.handler_object,
            auth_token,
            client.client_token.clone(),
        ) {
            Ok(request) => request,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        let command = match std::str::from_utf8(&body) {
            Ok(command) => command.trim().to_owned(),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        ClientRequest::Command {
            auth_token,
            handler_object: host.handler_object,
            command,
            mode,
        }
    };
    match client.rpc.client_call(client.client_id, request).await {
        Ok(ClientReply::TaskSubmitted { .. }) => StatusCode::ACCEPTED.into_response(),
        Ok(_) => StatusCode::BAD_GATEWAY.into_response(),
        Err(error) => rpc_status(error).into_response(),
    }
}

pub async fn session_input_handler(
    client: StreamClient,
    Path(request_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth_token: AuthToken = match extract_auth_token_header(&headers) {
        Ok(token) => token,
        Err(status) => return status.into_response(),
    };
    let input = if headers
        .get("content-type")
        .is_some_and(|value| value == "application/x-flatbuffers")
    {
        match VarRef::read_as_root(&body)
            .ok()
            .and_then(|value| var_from_flatbuffer_ref(value).ok())
        {
            Some(input) => input,
            None => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        match std::str::from_utf8(&body) {
            Ok(input) => v_str(input),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    };
    match client
        .rpc
        .client_call(
            client.client_id,
            ClientRequest::RequestedInput {
                client_token: client.client_token,
                auth_token,
                request_id,
                input,
            },
        )
        .await
    {
        Ok(ClientReply::InputThanks) => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => StatusCode::BAD_GATEWAY.into_response(),
        Err(error) => rpc_status(error).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::StreamExt;
    use moor_runtime_api::api::{
        ClientSubscriptions, HostEventSubscription, HostReply, HostRequest, HostServices,
    };
    use std::sync::Mutex;

    struct Runtime {
        id: Uuid,
        requests: Mutex<Vec<EventStreamRequest>>,
        invocations: Mutex<Vec<ClientRequest>>,
        events: Mutex<Vec<Arc<[u8]>>>,
        denied: bool,
        subscription_closed: bool,
    }
    #[async_trait]
    impl RuntimeClient for Runtime {
        async fn client_call(
            &self,
            _: Uuid,
            request: ClientRequest,
        ) -> Result<ClientReply, RpcError> {
            if matches!(
                &request,
                ClientRequest::Command { .. } | ClientRequest::InvokeVerb { .. }
            ) {
                self.invocations.lock().unwrap().push(request);
                return if self.denied {
                    Err(RpcError::Daemon(RpcMessageError::PermissionDenied))
                } else {
                    Ok(ClientReply::TaskSubmitted { task_id: 1 })
                };
            }
            let ClientRequest::EventStream { request, .. } = request else {
                panic!("unexpected request")
            };
            self.requests.lock().unwrap().push(request.clone());
            if self.denied {
                return Err(RpcError::Daemon(RpcMessageError::PermissionDenied));
            }
            let events = self.events.lock().unwrap();
            Ok(ClientReply::EventStream(EventStreamState {
                stream_id: self.id,
                acknowledged_sequence: 0,
                available_after: 0,
                latest_sequence: events.len() as u64,
                payloads: if request.operation == EventStreamOperation::Read {
                    events
                        .iter()
                        .skip(request.sequence as usize)
                        .take(2)
                        .cloned()
                        .collect()
                } else {
                    vec![]
                },
            }))
        }
        async fn host_call(&self, _: Uuid, _: HostRequest) -> Result<HostReply, RpcError> {
            panic!("unexpected host request")
        }
    }
    struct Notifications(bool);
    #[async_trait]
    impl ClientEventSubscription for Notifications {
        async fn recv_client_event(
            &mut self,
        ) -> Result<moor_runtime_api::api::ClientEventMessage, RpcError> {
            if self.0 {
                return Err(RpcError::Daemon(RpcMessageError::NoConnection));
            }
            std::future::pending().await
        }
    }
    struct Services(Arc<Runtime>);
    impl HostServices for Services {
        fn runtime_client(&self) -> Arc<dyn RuntimeClient> {
            self.0.clone()
        }
        fn client_subscriptions(
            &self,
            _: Uuid,
            _: ClientToken,
        ) -> Result<ClientSubscriptions, RpcError> {
            panic!("SSE must not use host-acknowledged subscriptions")
        }
        fn client_event_notifications(
            &self,
            _: Uuid,
        ) -> Result<Box<dyn ClientEventSubscription>, RpcError> {
            assert!(!self.0.denied, "unauthenticated subscription allocation");
            Ok(Box::new(Notifications(self.0.subscription_closed)))
        }
        fn host_events(&self) -> Result<Box<dyn HostEventSubscription>, RpcError> {
            panic!("unexpected subscription")
        }
    }
    fn host(runtime: Arc<Runtime>) -> WebHost {
        WebHost::new(
            moor_var::Obj::mk_id(0),
            8081,
            Uuid::new_v4(),
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
            Arc::new(Services(runtime)),
            Arc::new(vec![]),
            Arc::new(crate::host::WebRtcConfig::default()),
        )
    }
    fn client(runtime: Arc<Runtime>) -> StreamClient {
        StreamClient {
            client_id: Uuid::new_v4(),
            client_token: ClientToken("token".into()),
            rpc: runtime,
        }
    }
    fn query(id: Uuid) -> Query<StreamQuery> {
        Query(StreamQuery {
            stream_id: id,
            after: 0,
        })
    }

    fn runtime() -> Arc<Runtime> {
        Arc::new(Runtime {
            id: Uuid::new_v4(),
            requests: Mutex::new(vec![]),
            invocations: Mutex::new(vec![]),
            events: Mutex::new(vec![]),
            denied: false,
            subscription_closed: false,
        })
    }

    fn payload(sequence: u64) -> Arc<[u8]> {
        moor_runtime_api::api_codec::encode_client_event_bytes(
            &moor_runtime_api::api::ClientEventMessage {
                sequence,
                event: moor_runtime_api::api::ClientEvent::SystemMessage {
                    player: moor_var::Obj::mk_id(1),
                    message: "private output".into(),
                },
            },
        )
        .unwrap()
        .into()
    }

    #[tokio::test]
    async fn session_commands_keep_action_context_and_plain_text_distinct() {
        let runtime = runtime();
        let mut headers = HeaderMap::new();
        headers.insert("x-moor-auth-token", "actor".parse().unwrap());
        headers.insert(
            "content-type",
            "application/json; charset=utf-8".parse().unwrap(),
        );
        let body = br#"{"kind":"command","command":"go east","exit":{"passage":"captured"}}"#;
        let response = session_command_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            headers.clone(),
            Bytes::from_static(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        headers.insert("content-type", "text/plain".parse().unwrap());
        let response = session_command_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            headers,
            Bytes::from_static(b" go west "),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let invocations = runtime.invocations.lock().unwrap();
        assert_eq!(invocations.len(), 2);
        let ClientRequest::InvokeVerb {
            verb,
            args,
            mode: InvocationMode::Connected { client_token },
            ..
        } = &invocations[0]
        else {
            panic!("action must invoke the connected handler")
        };
        assert_eq!(*verb, moor_var::Symbol::mk("do_action"));
        assert_eq!(
            args[0],
            moor_var::json::json_to_var(&serde_json::from_slice(body).unwrap(), false)
        );
        assert_eq!(client_token, &ClientToken("token".into()));
        assert!(
            matches!(&invocations[1], ClientRequest::Command { command, .. } if command == "go west")
        );
    }

    #[tokio::test]
    async fn malformed_actions_do_not_fall_back_to_commands() {
        let runtime = runtime();
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        let response = session_command_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            headers.clone(),
            Bytes::from_static(b"{}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        headers.insert("x-moor-auth-token", "actor".parse().unwrap());
        for body in [b"go east".as_slice(), b"null", b"[]", b"{"] {
            let response = session_command_handler(
                State(host(runtime.clone())),
                client(runtime.clone()),
                headers.clone(),
                Bytes::copy_from_slice(body),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        assert!(runtime.invocations.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn streams_exact_payloads_without_ack_or_read_ahead() {
        let runtime = runtime();
        *runtime.events.lock().unwrap() = (1..=3).map(payload).collect();
        let response = event_stream_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            query(runtime.id),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-accel-buffering"], "no");
        assert_eq!(response.headers()["cache-control"], "no-store");
        let mut body = response.into_body().into_data_stream();
        for sequence in 1..=3 {
            let frame = body.next().await.unwrap().unwrap();
            let text = std::str::from_utf8(&frame).unwrap();
            assert!(text.contains("event: delivery"));
            assert!(text.contains(&format!("id: {}:{sequence}", runtime.id)));
            let data = text.lines().find_map(|l| l.strip_prefix("data: ")).unwrap();
            assert_eq!(
                STANDARD.decode(data).unwrap().as_slice(),
                payload(sequence).as_ref()
            );
            // The mock returns two payloads per batch. Consumption drives the next read.
            assert_eq!(
                runtime.requests.lock().unwrap().len(),
                if sequence < 3 { 1 } else { 2 }
            );
        }
        assert!(
            runtime
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| r.operation == EventStreamOperation::Read)
        );
        drop(body);
    }

    #[tokio::test]
    async fn reconnect_replays_from_processed_cursor_without_acknowledging_it() {
        let runtime = runtime();
        *runtime.events.lock().unwrap() = (1..=3).map(payload).collect();
        let mut resume = query(runtime.id);
        resume.0.after = 1;
        let response = event_stream_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            resume,
        )
        .await;
        let mut body = response.into_body().into_data_stream();
        let frame = body.next().await.unwrap().unwrap();
        let text = std::str::from_utf8(&frame).unwrap();
        assert!(text.contains(&format!("id: {}:2", runtime.id)));
        let requests = runtime.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, EventStreamOperation::Read);
        assert_eq!(requests[0].sequence, 1);
    }

    #[tokio::test]
    async fn periodic_read_recovers_events_without_notifications() {
        let runtime = runtime();
        let response = event_stream_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            query(runtime.id),
        )
        .await;
        // Arrival between initial read and subscription wake-up must not wait for another event.
        runtime.events.lock().unwrap().push(payload(1));
        let mut body = response.into_body().into_data_stream();
        let frame = tokio::time::timeout(Duration::from_secs(1), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            std::str::from_utf8(&frame)
                .unwrap()
                .contains("event: delivery")
        );
        assert_eq!(runtime.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn invalid_credentials_cannot_allocate_a_subscription() {
        let mut runtime = runtime();
        Arc::get_mut(&mut runtime).unwrap().denied = true;
        let response = event_stream_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            query(runtime.id),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn explicit_post_forwards_the_ack() {
        let runtime = runtime();
        assert_eq!(
            acknowledge_events_handler(
                client(runtime.clone()),
                Json(AcknowledgeEvents {
                    stream_id: runtime.id,
                    sequence: "9".into(),
                }),
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        let requests = runtime.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, EventStreamOperation::Acknowledge);
        assert_eq!(requests[0].sequence, 9);
    }

    #[tokio::test]
    async fn failed_subscription_closes_instead_of_polling_the_daemon() {
        let mut runtime = runtime();
        Arc::get_mut(&mut runtime).unwrap().subscription_closed = true;
        let response = event_stream_handler(
            State(host(runtime.clone())),
            client(runtime.clone()),
            query(runtime.id),
        )
        .await;
        let bytes = tokio::time::timeout(
            Duration::from_secs(1),
            axum::body::to_bytes(response.into_body(), 4096),
        )
        .await
        .expect("failed subscription must close the response")
        .unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("event: retry"));
        assert!(runtime.requests.lock().unwrap().len() <= 2);
    }
}
