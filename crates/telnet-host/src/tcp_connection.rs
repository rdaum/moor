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

//! Raw TCP connection implementation for non-telnet clients

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

use eyre::Context;
use eyre::bail;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use moor_common::model::ObjectRef;
use moor_common::tasks::{Event, SchedulerError, VerbProgramError};
use moor_common::util::parse_into_words;
use moor_var::{Obj, Symbol, Var, v_bool};

use crate::connection_shared::{
    LineMode, PendingTask, TASK_TIMEOUT, describe_compile_error, format_task_error,
    handle_builtin_command, output_format, process_command_line, process_requested_input_line,
};
use rpc_async_client::pubsub_client::{broadcast_recv, events_recv};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, ClientEvent, ClientToken, ClientsBroadcastEvent, ConnectType, HostType, ReplyResult,
    RpcMessageError, VerbProgramResponse,
};
use rpc_common::{DaemonToClientReply, HostClientToDaemonMessage};
use tmq::subscribe::Subscribe;
use tokio::net::TcpStream;
use tokio::select;
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Default flush command
pub(crate) const DEFAULT_FLUSH_COMMAND: &str = ".flush";

pub(crate) struct TcpConnection {
    pub(crate) peer_addr: SocketAddr,
    /// The "handler" object, who is responsible for this connection, defaults to SYSTEM_OBJECT,
    /// but custom listeners can be set up to handle connections differently.
    pub(crate) handler_object: Obj,
    /// The MOO connection object ID (negative ID for actual connection).
    pub(crate) connection_oid: Obj,
    /// The player object ID (set after authentication).
    pub(crate) player_obj: Option<Obj>,
    pub(crate) client_id: Uuid,
    /// Current PASETO token.
    pub(crate) client_token: ClientToken,
    pub(crate) write: SplitSink<Framed<TcpStream, LinesCodec>, String>,
    pub(crate) read: SplitStream<Framed<TcpStream, LinesCodec>>,
    pub(crate) kill_switch: Arc<AtomicBool>,

    pub(crate) broadcast_sub: Subscribe,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) auth_token: Option<AuthToken>,
    pub(crate) rpc_client: RpcSendClient,
    pub(crate) pending_task: Option<PendingTask>,

    /// Output prefix for command-output delimiters
    pub(crate) output_prefix: Option<String>,
    /// Output suffix for command-output delimiters
    pub(crate) output_suffix: Option<String>,
    /// Flush command for this connection
    pub(crate) flush_command: String,
    /// Connection attributes
    pub(crate) connection_attributes: HashMap<Symbol, Var>,
}

impl TcpConnection {
    pub(crate) async fn run(&mut self) -> Result<(), eyre::Error> {
        // Set basic connection attributes for raw TCP
        self.connection_attributes
            .insert(Symbol::mk("host-type"), Var::from("tcp".to_string()));
        self.connection_attributes
            .insert(Symbol::mk("supports-telnet-protocol"), v_bool(false));

        // Provoke welcome message, which is a login command with no arguments, and we
        // don't care about the reply at this point.
        self.rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::LoginCommand {
                    client_token: self.client_token.clone(),
                    handler_object: self.handler_object,
                    connect_args: vec![],
                    do_attach: false,
                },
            )
            .await
            .expect("Unable to send login request to RPC server");

        let Ok((auth_token, player, connect_type)) = self.authorization_phase().await else {
            bail!("Unable to authorize connection");
        };

        self.auth_token = Some(auth_token);

        let connect_message = match connect_type {
            ConnectType::Connected => "*** Connected ***",
            ConnectType::Reconnected => "*** Reconnected ***",
            ConnectType::Created => "*** Created ***",
        };
        self.write.send(connect_message.to_string()).await?;

        debug!(?player, client_id = ?self.client_id, "Entering command dispatch loop");

        // Send connection attributes to daemon
        for (key, value) in self.connection_attributes.clone() {
            self.update_connection_attribute(key, Some(value)).await;
        }

        if self.command_loop().await.is_err() {
            info!("Connection closed");
        };

        // Let the server know this client is gone.
        self.rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::Detach(self.client_token.clone(), true),
            )
            .await?;

        Ok(())
    }

    /// Send connection attribute updates to the daemon
    async fn update_connection_attribute(&mut self, key: Symbol, value: Option<Var>) {
        if let Some(auth_token) = &self.auth_token {
            let _ = self
                .rpc_client
                .make_client_rpc_call(
                    self.client_id,
                    HostClientToDaemonMessage::SetClientAttribute(
                        self.client_token.clone(),
                        auth_token.clone(),
                        key,
                        value,
                    ),
                )
                .await;
        }
    }

    async fn authorization_phase(&mut self) -> Result<(AuthToken, Obj, ConnectType), eyre::Error> {
        debug!(client_id = ?self.client_id, "Entering auth loop");

        loop {
            select! {
                Ok(event) = broadcast_recv(&mut self.broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = &mut self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(), self.connection_oid, HostType::TCP, self.peer_addr)).await?;
                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ClientEvent::SystemMessage(_author, msg) => {
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ClientEvent::Narrative(_author, event) => {
                            self.output(event.event()).await?;
                        }
                        ClientEvent::RequestInput(_request_id) => {
                            bail!("RequestInput before login");
                        }
                        ClientEvent::Disconnect() => {
                            self.write.close().await?;
                            bail!("Disconnect before login");
                        }
                        ClientEvent::TaskError(_ti, te) => {
                            self.handle_task_error(te).await?;
                        }
                        ClientEvent::TaskSuccess(_ti, result) => {
                            trace!(?result, "TaskSuccess")
                            // We don't need to do anything with successes.
                        }
                        ClientEvent::PlayerSwitched { new_player, new_auth_token } => {
                            info!("Switching player from {:?} to {} during authorization for client {}", self.player_obj, new_player, self.client_id);
                            self.player_obj = Some(new_player);
                            self.auth_token = Some(new_auth_token);
                            info!("Player switched successfully to {} during authorization for client {}", new_player, self.client_id);
                        }
                        ClientEvent::SetConnectionOption { connection_obj: _, option_name: _, value: _ } => {
                            // Ignore connection options before authentication
                        }
                    }
                }
                // Main input loop for raw text
                line = self.read.next() => {
                    let Some(line) = line else {
                        bail!("Connection closed before login");
                    };
                    let line = line?;
                    let words = parse_into_words(&line);
                    let response = &mut self.rpc_client.make_client_rpc_call(
                        self.client_id,
                        HostClientToDaemonMessage::LoginCommand {
                            client_token: self.client_token.clone(),
                            handler_object: self.handler_object,
                            connect_args: words,
                            do_attach: true,
                        },
                    ).await?;
                    if let ReplyResult::ClientSuccess(DaemonToClientReply::LoginResult(
                        Some((auth_token, connect_type, player)))) = response {
                        info!(?player, client_id = ?self.client_id, "Login successful");
                        self.player_obj = Some(*player);
                        return Ok((auth_token.clone(), *player, *connect_type))
                    }
                }
            }
        }
    }

    async fn command_loop(&mut self) -> Result<(), eyre::Error> {
        if self.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        let mut line_mode = LineMode::Input;
        let mut expecting_input = VecDeque::new();
        let mut program_input = Vec::new();

        loop {
            // Check for task timeout
            if let Some(pt) = &self.pending_task
                && expecting_input.is_empty()
                && pt.start_time.elapsed() > TASK_TIMEOUT
            {
                error!(
                    "Task {} stuck without response for more than {TASK_TIMEOUT:?}",
                    pt.task_id
                );
                self.pending_task = None;
            }

            select! {
                biased;
                Ok(event) = broadcast_recv(&mut self.broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(),
                                    self.handler_object, HostType::TCP, self.peer_addr)).await.expect("Unable to send pong to RPC server");

                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    match event {
                        ClientEvent::RequestInput(request_id) => {
                            expecting_input.push_back(request_id);
                        }
                        ClientEvent::SystemMessage(_author, msg) => {
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ClientEvent::Narrative(_author, event) => {
                            self.output(event.event()).await?;
                        }
                        ClientEvent::Disconnect() => {
                            self.write.close().await?;
                            bail!("Disconnect during command loop");
                        }
                        ClientEvent::TaskError(_ti, te) => {
                            let error_msg = format_task_error(te);
                            self.write.send(error_msg).await?;
                            self.send_output_suffix().await?;
                        }
                        ClientEvent::TaskSuccess(_ti, _s) => {
                            if let Some(_pending_event) = self.pending_task.take() {
                                // Task completed successfully
                            }
                            self.send_output_suffix().await?;
                        }
                        ClientEvent::PlayerSwitched { new_player, new_auth_token } => {
                            info!("Switching player from {:?} to {} for client {}", self.player_obj, new_player, self.client_id);
                            self.player_obj = Some(new_player);
                            self.auth_token = Some(new_auth_token);
                        }
                        ClientEvent::SetConnectionOption { connection_obj: _, option_name: _, value: _ } => {
                            // TCP connections don't support telnet options
                        }
                    }
                }
                // Main input loop for raw text lines
                line = self.read.next() => {
                    let Some(line) = line else {
                        info!("Connection closed");
                        break;
                    };
                    let line = line?;

                    // Handle input replies first
                    if !expecting_input.is_empty() {
                        let Some(auth_token) = &self.auth_token else {
                            bail!("Received input reply before auth token was set");
                        };
                        if let Some(pending_task) = process_requested_input_line(
                            line, &mut expecting_input, self.client_id, &self.client_token,
                            auth_token, &mut self.rpc_client).await? {
                            self.pending_task = Some(pending_task);
                        }
                        continue;
                    }

                    // Skip processing new commands if we have a pending task,
                    // but only if we're not expecting input (read() replies must go through!)
                    if self.pending_task.is_some() && expecting_input.is_empty() {
                        continue;
                    }

                    // Handle flush command first - it should be processed immediately.
                    if line.trim() == self.flush_command {
                        // We don't support flush, but just move on.
                        continue;
                    }

                    if let LineMode::SpoolingProgram(target, verb) = line_mode.clone() {
                        // If the line is "." that means we're done, and we can send the program off and switch modes back.
                        if line == "." {
                            line_mode = LineMode::Input;

                            // Clear the program input, and send it off.
                            let Some(auth_token) = self.auth_token.clone() else {
                                bail!("Received program command before auth token was set");
                            };
                            let code = std::mem::take(&mut program_input);
                            let target = ObjectRef::Match(target.clone());
                            let verb = Symbol::mk(&verb);
                            match self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::Program(self.client_token.clone(), auth_token,
                                    target, verb, code)).await? {
                                        ReplyResult::ClientSuccess(DaemonToClientReply::ProgramResponse(resp)) => match resp {
                                VerbProgramResponse::Success(o, verb) => {
                                    self.write
                                        .send(format!(
                                            "0 error(s).\nVerb {verb} programmed on object {o}"
                                        ))
                                        .await?;
                                }
                                VerbProgramResponse::Failure(VerbProgramError::CompilationError(e)) => {
                                    let desc = describe_compile_error(e);
                                    self.write.send(desc).await?;
                                }
                                VerbProgramResponse::Failure(VerbProgramError::NoVerbToProgram) => {
                                    self.write
                                        .send("That object does not have that verb.".to_string())
                                        .await?;
                                }
                                VerbProgramResponse::Failure(e) => {
                                    error!("Unhandled verb program error: {:?}", e);
                                }
                            },
                            ReplyResult::Failure(RpcMessageError::TaskError(e)) => {
                                self.handle_task_error(e).await?;
                            }
                            ReplyResult::Failure(e) => {
                                bail!("Unhandled RPC error: {:?}", e);
                            }
                            ReplyResult::ClientSuccess(s) => {
                                bail!("Unexpected RPC success: {:?}", s);
                            }
                            ReplyResult::HostSuccess(hs) => {
                                bail!("Unexpected host success: {:?}", hs);
                            }
                        }
                        } else {
                            // Otherwise, we're still spooling up the program, so just keep spooling.
                            program_input.push(line);
                        }
                        continue;
                    }

                    // Handle special built-in commands before regular command processing
                    if handle_builtin_command(&line, &mut self.output_prefix, &mut self.output_suffix,
                        self.client_id, &self.client_token, &self.auth_token, &mut self.rpc_client).await? {
                        continue;
                    }

                    if line.starts_with(".program") {
                        let words = parse_into_words(&line);
                        let usage_msg = "Usage: .program <target>:<verb>";
                        if words.len() != 2 {
                            self.write.send(usage_msg.to_string()).await?;
                            continue;
                        }
                        let verb_spec = words[1].split(':').collect::<Vec<_>>();
                        if verb_spec.len() != 2 {
                            self.write.send(usage_msg.to_string()).await?;
                            continue;
                        }
                        let target = verb_spec[0].to_string();
                        let verb = verb_spec[1].to_string();

                        // verb must be a valid identifier
                        if !verb.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            self.write
                                .send("You must specify a verb; use the format object:verb.".to_string())
                                .await?;
                            continue;
                        }

                        // target should be a valid object #number, $objref, ident, or
                        //  a string inside quotes
                        if !target.starts_with('$')
                            && !target.starts_with('#')
                            && !target.starts_with('"')
                            && !target.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            self.write
                                .send("You must specify a target; use the format object:verb.".to_string())
                                .await?;
                            continue;
                        }

                        self.write
                            .send(format!("Now programming {}. Use \".\" to end.", words[1]))
                            .await?;

                        line_mode = LineMode::SpoolingProgram(target, verb);
                        continue;
                    }

                    let Some(auth_token) = self.auth_token.clone() else {
                        bail!("Received command before auth token was set");
                    };

                    // Send output prefix before executing command
                    self.send_output_prefix().await?;

                    if let Some(pending_task) = process_command_line(line, self.client_id, &self.client_token,
                        &auth_token, self.handler_object, &mut self.rpc_client).await? {
                        self.pending_task = Some(pending_task);
                    } else {
                        // Command processed immediately, send suffix
                        self.send_output_suffix().await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn output(&mut self, event: Event) -> Result<(), eyre::Error> {
        match event {
            Event::Notify {
                value: msg,
                content_type,
                no_flush: _,
                no_newline,
            } => {
                let Ok(formatted) = output_format(&msg, content_type) else {
                    warn!("Failed to format message: {:?}", msg);
                    return Ok(());
                };

                // For raw TCP, just send the text directly
                if no_newline {
                    // Send without adding newline
                    self.write.feed(formatted).await?;
                    self.write.flush().await?;
                } else {
                    // LinesCodec will add the newline
                    self.write.send(formatted).await?;
                }
            }

            Event::Traceback(e) => {
                for frame in e.backtrace {
                    let Some(s) = frame.as_string() else {
                        continue;
                    };
                    self.write
                        .send(s.to_string())
                        .await
                        .with_context(|| "Unable to send message to client")?;
                }
            }
            _ => {
                self.write
                    .send(format!("Unsupported event for TCP: {event:?}"))
                    .await
                    .with_context(|| "Unable to send message to client")?;
            }
        }

        Ok(())
    }

    async fn handle_task_error(&mut self, task_error: SchedulerError) -> Result<(), eyre::Error> {
        let error_msg = format_task_error(task_error);
        self.write.send(error_msg).await?;
        Ok(())
    }

    /// Send output prefix if defined
    async fn send_output_prefix(&mut self) -> Result<(), eyre::Error> {
        if let Some(ref prefix) = self.output_prefix {
            self.write.send(prefix.clone()).await?;
        }
        Ok(())
    }

    /// Send output suffix if defined
    async fn send_output_suffix(&mut self) -> Result<(), eyre::Error> {
        if let Some(ref suffix) = self.output_suffix {
            self.write.send(suffix.clone()).await?;
        }
        Ok(())
    }
}
