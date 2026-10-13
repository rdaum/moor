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

//! WebSocket frame classification for the client session event loop.

use axum::extract::ws::{CloseFrame, Message};
use futures_util::{Stream, StreamExt};
use std::time::Duration;
use tracing::{debug, error, trace};

use super::{PendingTask, webrtc};
use crate::host::actions::is_action_frame;

const TASK_TIMEOUT: Duration = Duration::from_secs(10);
const HEARTBEAT_RESPONSE: u8 = 0x01;

pub enum ReadEvent {
    Command(Message),
    InputReply(Message),
    ConnectionClose {
        close_code: Option<u16>,
        is_logout: bool,
    },
    PendingEvent,
    Ping(Vec<u8>),
    HeartbeatResponse,
    WebRtcSignaling(Vec<u8>),
}

pub async fn read_websocket_event(
    ws_receiver: &mut (impl Stream<Item = Result<Message, axum::Error>> + Unpin),
    expecting_input: bool,
    pending_task: &mut Option<PendingTask>,
) -> ReadEvent {
    if let Some(pt) = pending_task
        && !expecting_input
        && pt.start_time.elapsed() > TASK_TIMEOUT
    {
        error!(
            "Task {} stuck without response for more than {TASK_TIMEOUT:?}",
            pt.task_id
        );
        *pending_task = None;
    } else if pending_task.is_some() && !expecting_input {
        return ReadEvent::PendingEvent;
    }

    loop {
        let Some(Ok(msg)) = ws_receiver.next().await else {
            return ReadEvent::ConnectionClose {
                close_code: None,
                is_logout: false,
            };
        };

        match msg {
            Message::Binary(ref data) if data.len() == 1 && data[0] == 0x00 => {
                trace!("Received keepalive from client");
                continue;
            }
            Message::Binary(ref data) if data.len() == 1 && data[0] == HEARTBEAT_RESPONSE => {
                trace!("Received heartbeat response from client");
                return ReadEvent::HeartbeatResponse;
            }
            Message::Binary(ref data)
                if !data.is_empty() && data[0] == webrtc::SIGNALING_PREFIX =>
            {
                return ReadEvent::WebRtcSignaling(data.to_vec());
            }
            Message::Binary(ref data) if is_action_frame(data) => {
                // An action is never a reply to an outstanding read().
                return ReadEvent::Command(msg);
            }
            Message::Text(_) | Message::Binary(_) if expecting_input => {
                return ReadEvent::InputReply(msg);
            }
            Message::Text(_) | Message::Binary(_) => {
                return ReadEvent::Command(msg);
            }
            Message::Ping(payload) => {
                trace!("Received ping from client");
                return ReadEvent::Ping(payload.to_vec());
            }
            Message::Pong(_) => {
                trace!("Received pong from client");
                continue;
            }
            Message::Close(close_frame) => return read_close_event(close_frame),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    #[tokio::test]
    async fn structured_actions_are_not_prompt_replies() {
        let action = Message::Binary(b"\x04{\"command\":\"go east\"}".to_vec().into());
        let reply = Message::Text("the prompt answer".into());
        let mut messages = stream::iter(vec![Ok(action), Ok(reply)]);
        let mut pending = None;
        assert!(matches!(
            read_websocket_event(&mut messages, true, &mut pending).await,
            ReadEvent::Command(Message::Binary(_))
        ));
        assert!(
            matches!(read_websocket_event(&mut messages, true, &mut pending).await, ReadEvent::InputReply(Message::Text(text)) if text == "the prompt answer")
        );
    }

    #[tokio::test]
    async fn ordinary_text_still_uses_the_command_path() {
        let mut messages = stream::iter(vec![Ok(Message::Text("go east".into()))]);
        assert!(
            matches!(read_websocket_event(&mut messages, false, &mut None).await, ReadEvent::Command(Message::Text(text)) if text == "go east")
        );
    }
}

fn read_close_event(close_frame: Option<CloseFrame>) -> ReadEvent {
    let close_code = close_frame.as_ref().map(|f| f.code);
    let close_reason = close_frame.as_ref().map(|f| f.reason.to_string());

    if let Some(frame) = &close_frame {
        debug!(
            "WebSocket close frame received: code={}, reason={:?}",
            frame.code, frame.reason
        );
    }

    let is_logout = close_reason.as_deref() == Some("LOGOUT");
    if is_logout {
        debug!("Detected explicit logout from close reason");
    }

    ReadEvent::ConnectionClose {
        close_code,
        is_logout,
    }
}
