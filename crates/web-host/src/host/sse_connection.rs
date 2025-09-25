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

//! Server-Sent Events connection handler for real-time narrative event streaming.
//! Uses EventLog as single source of truth with ZMQ for real-time notifications.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::{Stream, StreamExt};
use moor_common::tasks::Event;
use moor_var::v_obj;
use rpc_async_client::pubsub_client::events_recv;
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, ClientEvent, ClientToken, ConnectType, DaemonToClientReply, HistoryRecall,
    HostClientToDaemonMessage, ReplyResult,
};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use tmq::subscribe::Subscribe;
use tracing::{error, warn};
use uuid::Uuid;

use crate::host::{WebHost, var_as_json};

/// SSE connection handler for streaming narrative events
pub struct SseConnection {
    pub(crate) client_id: Uuid,
    pub(crate) client_token: ClientToken,
    pub(crate) auth_token: AuthToken,
    pub(crate) rpc_client: RpcSendClient,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) last_event_id: Option<Uuid>,
}

impl SseConnection {
    /// Create event stream from historical events and live ZMQ subscription
    pub async fn create_event_stream(
        mut self,
    ) -> impl Stream<Item = Result<SseEvent, Infallible>> + Send {
        async_stream::stream! {
            if let Some(since_id) = self.last_event_id {
                match self.get_historical_events(since_id).await {
                    Ok(events) => {
                        for event in events {
                            if let Ok(sse_event) = Self::narrative_event_to_sse(&event, true) {
                                yield Ok(sse_event);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to retrieve historical events: {}", e);
                        let error_event = SseEvent::default()
                            .event("error")
                            .data("Failed to retrieve historical events");
                        yield Ok(error_event);
                        return;
                    }
                }
            }

            loop {
                match events_recv(self.client_id, &mut self.narrative_sub).await {
                    Ok(event) => {
                        if let Some(sse_event) = Self::handle_client_event(event) {
                            yield Ok(sse_event);
                        }
                    }
                    Err(e) => {
                        error!("ZMQ event receive error: {}", e);
                        let error_event = SseEvent::default()
                            .event("error")
                            .data("Connection error occurred");
                        yield Ok(error_event);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }

    async fn get_historical_events(
        &mut self,
        since_id: Uuid,
    ) -> Result<Vec<ClientEvent>, eyre::Error> {
        let history_recall = HistoryRecall::SinceEvent(since_id, None);

        match self
            .rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::RequestHistory(
                    self.client_token.clone(),
                    self.auth_token.clone(),
                    history_recall,
                ),
            )
            .await?
        {
            ReplyResult::ClientSuccess(DaemonToClientReply::HistoryResponse(history)) => {
                let mut events = Vec::new();
                for historical_event in history.events {
                    let client_event =
                        ClientEvent::Narrative(historical_event.player, historical_event.event);
                    events.push(client_event);
                }
                Ok(events)
            }
            ReplyResult::Failure(e) => Err(eyre::eyre!("History request failed: {:?}", e)),
            other => Err(eyre::eyre!("Unexpected response: {:?}", other)),
        }
    }

    fn handle_client_event(event: ClientEvent) -> Option<SseEvent> {
        match event {
            ClientEvent::Narrative(_author, narrative_event) => {
                match Self::narrative_event_to_sse(
                    &ClientEvent::Narrative(_author, narrative_event),
                    false,
                ) {
                    Ok(sse_event) => Some(sse_event),
                    Err(e) => {
                        warn!("Failed to convert narrative event to SSE: {}", e);
                        None
                    }
                }
            }
            ClientEvent::SystemMessage(author, msg) => {
                let event_data = json!({
                    "author": var_as_json(&v_obj(author)),
                    "system_message": msg,
                    "server_time": SystemTime::now(),
                    "is_historical": false
                });

                Some(
                    SseEvent::default()
                        .event("system_message")
                        .data(event_data.to_string()),
                )
            }
            ClientEvent::Disconnect() => Some(
                SseEvent::default()
                    .event("disconnect")
                    .data("Connection terminated"),
            ),
            _ => None,
        }
    }

    fn narrative_event_to_sse(
        event: &ClientEvent,
        is_historical: bool,
    ) -> Result<SseEvent, eyre::Error> {
        if let ClientEvent::Narrative(_author, narrative_event) = event {
            let event_id = narrative_event.event_id().to_string();
            let msg = narrative_event.event();

            let event_data = match msg {
                Event::Notify {
                    value: msg,
                    content_type,
                    no_flush: _, // Not used in web context
                    no_newline,
                } => {
                    let normalized_content_type =
                        content_type
                            .as_ref()
                            .map(|ct| match ct.as_string().as_str() {
                                "text_djot" => "text/djot".to_string(),
                                "text_html" => "text/html".to_string(),
                                "text_plain" => "text/plain".to_string(),
                                _ => ct.as_string(),
                            });

                    json!({
                        "event_id": event_id,
                        "author": var_as_json(narrative_event.author()),
                        "message": var_as_json(&msg),
                        "content_type": normalized_content_type,
                        "no_newline": if no_newline { Some(true) } else { None },
                        "server_time": narrative_event.timestamp(),
                        "is_historical": is_historical
                    })
                }
                Event::Traceback(exception) => {
                    let mut traceback = vec![];
                    for frame in &exception.backtrace {
                        if let Some(s) = frame.as_string() {
                            traceback.push(s.to_string());
                        }
                    }

                    json!({
                        "event_id": event_id,
                        "author": var_as_json(narrative_event.author()),
                        "traceback": {
                            "error": format!("{}", exception),
                            "traceback": traceback
                        },
                        "server_time": narrative_event.timestamp(),
                        "is_historical": is_historical
                    })
                }
                Event::Present(presentation) => {
                    json!({
                        "event_id": event_id,
                        "author": var_as_json(narrative_event.author()),
                        "present": presentation,
                        "server_time": narrative_event.timestamp(),
                        "is_historical": is_historical
                    })
                }
                Event::Unpresent(id) => {
                    json!({
                        "event_id": event_id,
                        "author": var_as_json(narrative_event.author()),
                        "unpresent": id,
                        "server_time": narrative_event.timestamp(),
                        "is_historical": is_historical
                    })
                }
            };

            Ok(SseEvent::default()
                .id(event_id)
                .event("narrative")
                .data(event_data.to_string()))
        } else {
            Err(eyre::eyre!("Expected narrative event"))
        }
    }
}

use axum::extract::Query;
use std::collections::HashMap;

pub async fn sse_events_handler(
    State(host): State<WebHost>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let last_event_id = headers.get("Last-Event-ID").and_then(|header_value| {
        let id_str = header_value.to_str().ok()?;
        match Uuid::parse_str(id_str) {
            Ok(uuid) => Some(uuid),
            Err(e) => {
                warn!("Invalid Last-Event-ID format '{}': {}", id_str, e);
                None
            }
        }
    });

    let auth_token = match params.get("token") {
        Some(token) => AuthToken(token.clone()),
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let (player, client_id, client_token, mut rpc_client) = match host
        .attach_authenticated(auth_token.clone(), Some(ConnectType::Connected), addr)
        .await
    {
        Ok(connection_details) => connection_details,
        Err(e) => {
            error!("SSE authentication failed: {}", e);
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    if let Err(e) = rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::ConnectionEstablish {
                peer_addr: addr.to_string(),
                local_port: 8080,
                remote_port: addr.port(),
                acceptable_content_types: Some(vec![
                    moor_var::Symbol::mk("text/plain"),
                    moor_var::Symbol::mk("text/html"),
                    moor_var::Symbol::mk("text/djot"),
                ]),
                connection_attributes: None,
            },
        )
        .await
    {
        error!("Failed to establish SSE connection: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    {
        let mut connections = host.client_connections.write().await;
        connections.insert(
            auth_token.clone(),
            crate::host::web_host::ClientConnection {
                client_id,
                client_token: client_token.clone(),
                player,
            },
        );
    }

    let narrative_sub = match tmq::subscribe(&host.zmq_context)
        .connect(host.pubsub_addr.as_str())
        .and_then(|sub| sub.subscribe(&client_id.as_bytes()[..]))
    {
        Ok(sub) => sub,
        Err(e) => {
            error!("Failed to subscribe to narrative events: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let sse_connection = SseConnection {
        client_id,
        client_token,
        auth_token,
        rpc_client,
        narrative_sub,
        last_event_id,
    };

    let stream = sse_connection.create_event_stream().await;
    let stream_with_messages = async_stream::stream! {
        yield Ok(SseEvent::default()
            .event("connected")
            .data("Connection established"));

        let connect_message = "*** Connected ***";
        let event_data = json!({
            "author": var_as_json(&v_obj(player)),
            "system_message": connect_message,
            "server_time": SystemTime::now(),
            "is_historical": false
        });
        yield Ok(SseEvent::default()
            .event("system_message")
            .data(event_data.to_string()));

        tokio::pin!(stream);
        while let Some(item) = stream.next().await {
            yield item;
        }
    };

    Sse::new(stream_with_messages)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text("keep-alive"),
        )
        .into_response()
}
