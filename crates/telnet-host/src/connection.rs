// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;

use eyre::bail;
use eyre::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use futures_util::StreamExt;
use moor_compiler::to_literal;
use moor_values::model::ObjectRef;
use moor_values::tasks::{AbortLimitReason, CommandError, Event, SchedulerError, VerbProgramError};
use moor_values::util::parse_into_words;
use moor_values::{Objid, Symbol, Variant};
use rpc_async_client::pubsub_client::{broadcast_recv, events_recv};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{
    AuthToken, ClientEvent, ClientToken, ClientsBroadcastEvent, ConnectType, HostType, ReplyResult,
    RpcMessageError, VerbProgramResponse,
};
use rpc_common::{DaemonToClientReply, HostClientToDaemonMessage};
use termimad::MadSkin;
use tmq::subscribe::Subscribe;
use tokio::net::TcpStream;
use tokio::select;
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Out of band messages are prefixed with this string, e.g. for MCP clients.
const OUT_OF_BAND_PREFIX: &str = "#$#";

// TODO: switch to djot
const CONTENT_TYPE_MARKDOWN: &str = "text/markdown";

pub(crate) struct TelnetConnection {
    pub(crate) peer_addr: SocketAddr,
    /// The "handler" object, who is responsible for this connection, defaults to SYSTEM_OBJECT,
    /// but custom listeners can be set up to handle connections differently.
    pub(crate) handler_object: Objid,
    /// The MOO connection object ID.
    pub(crate) connection_oid: Objid,
    pub(crate) client_id: Uuid,
    /// Current PASETO token.
    pub(crate) client_token: ClientToken,
    pub(crate) write: SplitSink<Framed<TcpStream, LinesCodec>, String>,
    pub(crate) read: SplitStream<Framed<TcpStream, LinesCodec>>,
    pub(crate) kill_switch: Arc<AtomicBool>,
}

/// The input modes the telnet session can be in.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LineMode {
    /// Typical command input mode.
    Input,
    /// Waiting for a reply to a prompt.
    WaitingReply(u128),
    /// Spooling up .program input.
    SpoolingProgram(String, String),
}

impl TelnetConnection {
    pub(crate) async fn run(
        &mut self,
        events_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(), eyre::Error> {
        // Provoke welcome message, which is a login command with no arguments, and we
        // don't care about the reply at this point.
        rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::LoginCommand(
                    self.client_token.clone(),
                    self.handler_object,
                    vec![],
                    false,
                ),
            )
            .await
            .expect("Unable to send login request to RPC server");

        let Ok((auth_token, player, connect_type)) = self
            .authorization_phase(events_sub, broadcast_sub, rpc_client)
            .await
        else {
            bail!("Unable to authorize connection");
        };

        let connect_message = match connect_type {
            ConnectType::Connected => "*** Connected ***",
            ConnectType::Reconnected => "*** Reconnected ***",
            ConnectType::Created => "*** Created ***",
        };
        self.write.send(connect_message.to_string()).await?;

        debug!(?player, client_id = ?self.client_id, "Entering command dispatch loop");
        if self
            .command_loop(auth_token.clone(), events_sub, broadcast_sub, rpc_client)
            .await
            .is_err()
        {
            info!("Connection closed");
        };

        // Let the server know this client is gone.
        rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::Detach(self.client_token.clone()),
            )
            .await?;

        Ok(())
    }

    async fn output(&mut self, Event::Notify(msg, content_type): Event) -> Result<(), eyre::Error> {
        // Strings output as text lines to the client, otherwise send the
        // literal form (for e.g. lists, objrefs, etc)
        match msg.variant() {
            Variant::Str(msg_text) => {
                let formatted = output_format(msg_text.as_string(), content_type);
                self.write
                    .send(formatted)
                    .await
                    .with_context(|| "Unable to send message to client")?;
            }
            Variant::List(lines) => {
                for line in lines.iter() {
                    let Variant::Str(line) = line.variant() else {
                        trace!("Non-string in list output");
                        continue;
                    };
                    let formatted = output_format(line.as_string(), content_type);
                    self.write
                        .send(formatted)
                        .await
                        .with_context(|| "Unable to send message to client")?;
                }
            }
            _ => {
                self.write
                    .send(to_literal(&msg))
                    .await
                    .with_context(|| "Unable to send message to client")?;
            }
        }
        Ok(())
    }

    async fn authorization_phase(
        &mut self,
        narrative_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(AuthToken, Objid, ConnectType), eyre::Error> {
        debug!(client_id = ?self.client_id, "Entering auth loop");
        loop {
            select! {
                Ok(event) = broadcast_recv(broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(), self.connection_oid, HostType::TCP, self.peer_addr)).await?;
                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, narrative_sub) => {
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
                        ClientEvent::TaskError(te) => {
                            self.handle_task_error(te).await?;
                        }
                        ClientEvent::TaskSuccess(result) => {
                            trace!(?result, "TaskSuccess")
                            // We don't need to do anything with successes.
                        }
                    }
                }
                // Auto loop
                line = self.read.next() => {
                    let Some(line) = line else {
                        bail!("Connection closed before login");
                    };
                    let line = line.unwrap();
                    let words = parse_into_words(&line);
                    let response = rpc_client.make_client_rpc_call(self.client_id,
                        HostClientToDaemonMessage::LoginCommand(self.client_token.clone(), self.handler_object, words, true)).await.expect("Unable to send login request to RPC server");
                    if let ReplyResult::ClientSuccess(DaemonToClientReply::LoginResult(Some((auth_token, connect_type, player)))) = response {
                        info!(?player, client_id = ?self.client_id, "Login successful");
                        self.connection_oid = player;
                        return Ok((auth_token, player, connect_type))
                    }
                }
            }
        }
    }

    async fn command_loop(
        &mut self,
        auth_token: AuthToken,
        events_sub: &mut Subscribe,
        broadcast_sub: &mut Subscribe,
        rpc_client: &mut RpcSendClient,
    ) -> Result<(), eyre::Error> {
        let mut line_mode = LineMode::Input;
        let mut program_input = vec![];
        loop {
            if self.kill_switch.load(std::sync::atomic::Ordering::Relaxed) {
                return Ok(());
            }
            select! {
                line = self.read.next() => {
                    let Some(line) = line else {
                        info!("Connection closed");
                        return Ok(());
                    };
                    let line = line.unwrap();

                    let response = match line_mode.clone() {
                        LineMode::Input => {
                            // If the line is .program <verb> ... then we need to start spooling up a program.
                            // But we do need to do some very basic parsing to get the target and verb and reject complete nonsense.
                            // Note that LambdaMOO is more fussy and the server validates the object and verb etc. before accepting the program.
                            if line.starts_with(".program") {
                                let words = parse_into_words(&line);
                                let usage_msg = "Usage: .program <target>:<verb>";
                                if words.len() != 2 {
                                    self.write.send(usage_msg.to_string()).await?;
                                    continue
                                }
                                let verb_spec = words[1].split(':').collect::<Vec<_>>();
                                if verb_spec.len() != 2 {
                                    self.write.send(usage_msg.to_string()).await?;
                                    continue
                                }
                                let target = verb_spec[0].to_string();
                                let verb = verb_spec[1].to_string();

                                // verb must be a valid identifier
                                if !verb.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                    self.write.send("You must specify a verb; use the format object:verb.".to_string()).await?;
                                    continue
                                }

                                // target should be a valid object #number, $objref, ident, or
                                //  a string inside quotes
                                if !target.starts_with('$') && !target.starts_with('#') && !target.starts_with('"') && !target.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                    self.write.send("You must specify a target; use the format object:verb.".to_string()).await?;
                                    continue
                                }

                                self.write.send(format!("Now programming {}. Use \".\" to end.", words[1])).await?;

                                line_mode = LineMode::SpoolingProgram(target, verb);
                                continue
                            }

                            // If the line begins with the out of band prefix, then send it that way,
                            // instead. And really just fire and forget.
                            if line.starts_with(OUT_OF_BAND_PREFIX) {
                                rpc_client.make_client_rpc_call(self.client_id, HostClientToDaemonMessage::OutOfBand(self.client_token.clone(), auth_token.clone(), self.handler_object, line)).await?
                            } else {
                                rpc_client.make_client_rpc_call(self.client_id, HostClientToDaemonMessage::Command(self.client_token.clone(), auth_token.clone(), self.handler_object, line)).await?
                            }
                        },
                        // Are we expecting to respond to prompt input? If so, send this through to that, and switch the mode back to input
                        LineMode::WaitingReply(ref input_reply_id) => {
                            line_mode = LineMode::Input;
                            rpc_client.make_client_rpc_call(self.client_id, HostClientToDaemonMessage::RequestedInput(self.client_token.clone(), auth_token.clone(), *input_reply_id, line)).await?

                        }
                        LineMode::SpoolingProgram(target, verb) => {
                            // If the line is "." that means we're done, and we can send the program off and switch modes back.
                            if line == "." {
                                line_mode = LineMode::Input;

                                // Clear the program input, and send it off.
                                let code = std::mem::take(&mut program_input);
                                let target = ObjectRef::Match(target);
                                let verb = Symbol::mk(&verb);
                                rpc_client.make_client_rpc_call(self.client_id, HostClientToDaemonMessage::Program(self.client_token.clone(), auth_token.clone(), target, verb, code)).await?
                            } else {
                                // Otherwise, we're still spooling up the program, so just keep spooling.
                                program_input.push(line);
                                continue
                            }
                        }
                    };

                    match response {
                        ReplyResult::ClientSuccess(DaemonToClientReply::CommandSubmitted(_)) |
                        ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                            // Nothing to do
                        }
                        ReplyResult::HostSuccess(_) => {
                            error!("Unexpected host response to client message!");
                        }
                        ReplyResult::Failure(RpcMessageError::TaskError(te)) => {
                            self.handle_task_error(te).await?;
                        }
                        ReplyResult::Failure(e) => {
                            error!("Unhandled RPC error: {:?}", e);
                            continue;
                        }
                        ReplyResult::ClientSuccess(DaemonToClientReply::ProgramResponse(resp)) => {
                            match resp {
                                VerbProgramResponse::Success(o,verb) => {
                                    self.write.send(format!("0 error(s).\nVerb {} programmed on object {}", verb, o)).await?;
                                }
                                VerbProgramResponse::Failure(VerbProgramError::CompilationError(e)) => {
                                    self.write.send(format!("{} error(s).\n{}", e.len(), e.join("\n"))).await?;
                                }
                                VerbProgramResponse::Failure(VerbProgramError::NoVerbToProgram) => {
                                    self.write.send("That object does not have that verb.".to_string()).await?;
                                }
                                VerbProgramResponse::Failure(e) => {
                                    error!("Unhandled verb program error: {:?}", e);
                                }
                            }
                            continue;
                        }
                        ReplyResult::ClientSuccess(s) => {
                            error!("Unexpected RPC success: {:?}", s);
                            continue;
                        }
                    }
                }
                Ok(event) = broadcast_recv(broadcast_sub) => {
                    trace!(?event, "broadcast_event");
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(), self.connection_oid, HostType::TCP, self.peer_addr)).await?;
                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, events_sub) => {
                    match event {
                        ClientEvent::SystemMessage(_author, msg) => {
                            self.write.send(msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        ClientEvent::Narrative(_author, event) => {
                            self.output(event.event()).await?;
                        }
                        ClientEvent::RequestInput(request_id) => {
                            // Server is requesting that the next line of input get sent through as a response to this request.
                            line_mode = LineMode::WaitingReply(request_id);
                        }
                        ClientEvent::Disconnect() => {
                            self.write.send("** Disconnected **".to_string()).await.expect("Unable to send disconnect message to client");
                            self.write.close().await.expect("Unable to close connection");
                            return Ok(())
                        }
                        ClientEvent::TaskError(te) => {
                            self.handle_task_error(te).await?;
                        }
                        ClientEvent::TaskSuccess(result) => {
                            trace!(?result, "TaskSuccess")
                            // We don't need to do anything with successes.

                        }
                    }
                }
            }
        }
    }

    async fn handle_task_error(&mut self, task_error: SchedulerError) -> Result<(), eyre::Error> {
        match task_error {
            SchedulerError::CommandExecutionError(CommandError::CouldNotParseCommand) => {
                self.write
                    .send("I couldn't understand that.".to_string())
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoObjectMatch) => {
                self.write
                    .send("I don't see that here.".to_string())
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoCommandMatch) => {
                self.write
                    .send("I couldn't understand that.".to_string())
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::PermissionDenied) => {
                self.write.send("You can't do that.".to_string()).await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(lines)) => {
                for line in lines {
                    self.write.send(line).await?;
                }
                self.write.send("Verb not programmed.".to_string()).await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::NoVerbToProgram) => {
                self.write
                    .send("That object does not have that verb definition.".to_string())
                    .await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Ticks(_)) => {
                self.write.send("Task ran out of ticks".to_string()).await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Time(_)) => {
                self.write
                    .send("Task ran out of seconds".to_string())
                    .await?;
            }
            SchedulerError::TaskAbortedError => {
                self.write.send("Task aborted".to_string()).await?;
            }
            SchedulerError::TaskAbortedException(e) => {
                // This should not really be happening here... but?
                self.write.send(format!("Task exception: {}", e)).await?;
            }
            SchedulerError::TaskAbortedCancelled => {
                self.write.send("Task cancelled".to_string()).await?;
            }
            _ => {
                warn!(?task_error, "Unhandled unexpected task error");
            }
        }
        Ok(())
    }
}

fn markdown_to_ansi(markdown: &str) -> String {
    let skin = MadSkin::default_dark();
    // TODO: permit different text stylings here. e.g. user themes for colours, styling, etc.
    //   will require custom host-side commands to set these.
    skin.inline(markdown).to_string()
}

/// Produce the right kind of "telnet" compatible output for the given content.
fn output_format(content: &str, content_type: Option<Symbol>) -> String {
    let Some(content_type) = content_type else {
        return content.to_string();
    };
    let content_type = content_type.as_str();
    match content_type {
        CONTENT_TYPE_MARKDOWN => markdown_to_ansi(content),
        // text/plain, None, or unknown
        _ => content.to_string(),
    }
}
