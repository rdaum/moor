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

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime};

use crate::connection_codec::{ConnectionCodec, ConnectionFrame, ConnectionItem};
use eyre::Context;
use eyre::bail;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use moor_common::model::{CompileError, ObjectRef};
use moor_common::tasks::{AbortLimitReason, CommandError, Event, SchedulerError, VerbProgramError};
use moor_common::util::parse_into_words;
use moor_var::{Obj, Symbol, Var, Variant, v_str};
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
use tokio_util::codec::Framed;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Out of band messages are prefixed with this string, e.g. for MCP clients.
const OUT_OF_BAND_PREFIX: &str = "#$#";

/// Default flush command
pub(crate) const DEFAULT_FLUSH_COMMAND: &str = ".flush";

// TODO: switch to djot
const CONTENT_TYPE_MARKDOWN: &str = "text_markdown";
const CONTENT_TYPE_DJOT: &str = "text_djot";

pub(crate) struct TelnetConnection {
    pub(crate) peer_addr: SocketAddr,
    /// The "handler" object, who is responsible for this connection, defaults to SYSTEM_OBJECT,
    /// but custom listeners can be set up to handle connections differently.
    pub(crate) handler_object: Obj,
    /// The MOO connection connection ID
    pub(crate) connection_object: Obj,
    /// The player we're authenticated to, if any.
    pub(crate) player_object: Option<Obj>,
    pub(crate) client_id: Uuid,
    /// Current PASETO token.
    pub(crate) client_token: ClientToken,
    pub(crate) write: SplitSink<Framed<TcpStream, ConnectionCodec>, ConnectionFrame>,
    pub(crate) read: SplitStream<Framed<TcpStream, ConnectionCodec>>,
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
    /// Connection attributes (terminal size, type, etc.)
    pub(crate) connection_attributes: HashMap<Symbol, Var>,

    /// Connection option states
    pub(crate) is_binary_mode: bool,
    /// When Some, input is held in the buffer for read() calls; when None, input is processed as commands
    pub(crate) hold_input: Option<Vec<String>>,
    pub(crate) disable_oob: bool,
}

/// The input modes the telnet session can be in.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LineMode {
    /// Receiving input
    Input,
    /// Spooling up .program input.
    SpoolingProgram(String, String),
}

fn describe_compile_error(compile_error: CompileError) -> String {
    match compile_error {
        CompileError::StringLexError(_, le) => {
            format!("String format error: {le}")
        }
        CompileError::ParseError {
            error_position,
            context: _,
            end_line_col,
            message,
        } => {
            let mut err = format!(
                "Parse error at line {} column {}: {}",
                error_position.line_col.0, error_position.line_col.1, message
            );
            if let Some(end_line_col) = end_line_col {
                err.push_str(&format!(
                    " (to line {} column {})",
                    end_line_col.0, end_line_col.1
                ));
            }
            err.push_str(format!(": {message}").as_str());
            err
        }
        CompileError::UnknownBuiltinFunction(_, bf) => {
            format!("Unknown builtin function: {bf}")
        }
        CompileError::UnknownLoopLabel(_, ll) => {
            format!("Unknown break/loop label: {ll}")
        }
        CompileError::DuplicateVariable(_, dv) => {
            format!("Duplicate variable: {dv}")
        }
        CompileError::AssignToConst(_, ac) => {
            format!("Assignment to constant: {ac}")
        }
        CompileError::DisabledFeature(_, df) => {
            format!("Disabled feature: {df}")
        }
        CompileError::BadSlotName(_, bs) => {
            format!("Bad slot name in flyweight: {bs}")
        }
        CompileError::InvalidAssignemnt(_) => "Invalid l-value for assignment".to_string(),
        CompileError::UnknownTypeConstant(_, t) => {
            format!("Unknown type constant: {t}")
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PendingTask {
    task_id: usize,
    start_time: Instant,
}

pub enum ReadEvent {
    Command(String),
    InputReply(Var),
    ConnectionClose,
    PendingEvent,
}

const TASK_TIMEOUT: Duration = Duration::from_secs(10);

impl TelnetConnection {
    /// Send a line with automatic newline appending (like LambdaMOO's network_send_line)
    pub async fn send_line(&mut self, line: &str) -> Result<(), eyre::Error> {
        self.write
            .send(ConnectionFrame::Line(line.to_string()))
            .await
            .with_context(|| "Unable to send line to client")
    }

    /// Send raw text without newline (for no_newline attribute)
    pub async fn send_raw_text(&mut self, text: &str) -> Result<(), eyre::Error> {
        self.write
            .send(ConnectionFrame::RawText(text.to_string()))
            .await
            .with_context(|| "Unable to send raw text to client")
    }

    /// Send raw bytes without modification (like LambdaMOO's network_send_bytes)
    pub async fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), eyre::Error> {
        self.write
            .send(ConnectionFrame::Bytes(bytes::Bytes::copy_from_slice(bytes)))
            .await
            .with_context(|| "Unable to send bytes to client")
    }

    /// Explicitly flush the output (like LambdaMOO's flush control)
    pub async fn flush(&mut self) -> Result<(), eyre::Error> {
        self.write
            .send(ConnectionFrame::Flush)
            .await
            .with_context(|| "Unable to flush output to client")
    }

    /// Handle connection option changes
    async fn handle_connection_option(
        &mut self,
        option_name: Symbol,
        value: Option<Var>,
    ) -> Result<(), eyre::Error> {
        let option_str = option_name.as_arc_string();

        match option_str.as_str() {
            "binary" => {
                let binary_mode = value.as_ref().map(|v| v.is_true()).unwrap_or(false);

                debug!("Setting binary mode to {}", binary_mode);
                self.is_binary_mode = binary_mode;

                // Switch the codec mode by sending a SetMode frame
                use crate::connection_codec::ConnectionMode;
                let new_mode = if binary_mode {
                    ConnectionMode::Binary
                } else {
                    ConnectionMode::Text
                };

                self.write
                    .send(ConnectionFrame::SetMode(new_mode))
                    .await
                    .with_context(|| "Unable to set codec mode")?;
            }
            "hold-input" => {
                let hold = value.as_ref().map(|v| v.is_true()).unwrap_or(false);

                debug!("Setting hold-input to {}", hold);
                self.hold_input = if hold { Some(Vec::new()) } else { None };
            }
            "disable-oob" => {
                let disable = value.as_ref().map(|v| v.is_true()).unwrap_or(false);

                debug!("Setting disable-oob to {}", disable);
                self.disable_oob = disable;
            }
            "client-echo" => {
                let echo_on = value.as_ref().map(|v| v.is_true()).unwrap_or(true);

                debug!("Setting client-echo to {}", echo_on);
                self.send_telnet_echo_command(echo_on).await?;
            }
            "flush-command" => {
                let flush_cmd = value
                    .as_ref()
                    .and_then(|v| v.as_string())
                    .unwrap_or(DEFAULT_FLUSH_COMMAND);

                debug!("Setting flush-command to '{}'", flush_cmd);
                self.flush_command = flush_cmd.to_string();
            }
            _ => {
                warn!("Unsupported connection option: {}", option_str);
            }
        }

        Ok(())
    }

    /// Send telnet WILL/WONT ECHO command
    async fn send_telnet_echo_command(&mut self, echo_on: bool) -> Result<(), eyre::Error> {
        // These values taken from RFC 854 and RFC 857
        const TN_IAC: u8 = 255; // Interpret As Command
        const TN_WILL: u8 = 251;
        const TN_WONT: u8 = 252;
        const TN_ECHO: u8 = 1;

        let telnet_cmd = if echo_on {
            [TN_IAC, TN_WONT, TN_ECHO] // Client should echo
        } else {
            [TN_IAC, TN_WILL, TN_ECHO] // Server will echo (client should not)
        };

        self.send_bytes(&telnet_cmd).await
    }

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
    pub(crate) async fn run(&mut self) -> Result<(), eyre::Error> {
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
        self.send_line(connect_message).await?;
        self.flush().await?;

        debug!(?player, client_id = ?self.client_id, connection_obj = ?self.connection_object, "Entering command dispatch loop");

        // Now that we're authenticated, send all current connection attributes to daemon
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

    async fn output(&mut self, event: Event) -> Result<(), eyre::Error> {
        match event {
            Event::Notify {
                value,
                content_type,
                no_newline,
                ..
            } => {
                if let Variant::Binary(b) = value.variant() {
                    self.send_bytes(b.as_bytes()).await?;
                    return Ok(());
                }
                let Ok(formatted) = output_format(&value, content_type) else {
                    warn!("Failed to format message: {:?}", value);
                    return Ok(());
                };
                if no_newline {
                    self.send_raw_text(&formatted)
                        .await
                        .with_context(|| "Unable to send raw text to client")?;
                } else {
                    self.send_line(&formatted)
                        .await
                        .with_context(|| "Unable to send message to client")?;
                }
            }

            Event::Traceback(e) => {
                for frame in e.backtrace {
                    let Some(s) = frame.as_string() else {
                        continue;
                    };
                    self.send_line(s)
                        .await
                        .with_context(|| "Unable to send message to client")?;
                }
            }
            _ => {
                self.send_line(&format!("Unsupported event for telnet: {event:?}"))
                    .await
                    .with_context(|| "Unable to send message to client")?;
            }
        }

        Ok(())
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
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(), self.connection_object, HostType::TCP, self.peer_addr)).await?;
                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ClientEvent::SystemMessage(_author, msg) => {
                            self.send_line(&msg).await.with_context(|| "Unable to send message to client")?;
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
                            info!("Switching player from {:?} to {} during authorization for client {}", self.player_object, new_player, self.client_id);
                            self.player_object = Some(new_player);
                            self.auth_token = Some(new_auth_token);
                            info!("Player switched successfully to {} during authorization for client {}", new_player, self.client_id);
                        }
                        ClientEvent::SetConnectionOption { connection_obj, option_name, value } => {
                            debug!("Received SetConnectionOption: connection_obj={}, option_name={}, value={:?}, our_connection={}",
                                   connection_obj, option_name, value, self.connection_object);
                            // Only handle if this event is for our connection
                            if connection_obj == self.connection_object {
                                self.handle_connection_option(option_name, Some(value)).await?;
                            } else {
                                debug!("Ignoring SetConnectionOption for different connection");
                            }
                        }
                    }
                }
                // Auto loop
                item = self.read.next() => {
                    let Some(item) = item else {
                        bail!("Connection closed before login");
                    };
                    let item = item.unwrap();
                    let line = match item {
                        ConnectionItem::Line(line) => line,
                        ConnectionItem::Bytes(_) => {
                            // Binary data during login is not expected
                            continue;
                        }
                    };
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
                        self.player_object = Some(*player);
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
            // We should not send the next line until we've received a narrative event for the
            // previous.
            let input_future = async {
                if let Some(pt) = &self.pending_task
                    && expecting_input.is_empty()
                    && pt.start_time.elapsed() > TASK_TIMEOUT
                {
                    error!(
                        "Task {} stuck without response for more than {TASK_TIMEOUT:?}",
                        pt.task_id
                    );
                    self.pending_task = None;
                } else if self.pending_task.is_some() && expecting_input.is_empty() {
                    return ReadEvent::PendingEvent;
                }

                let Some(Ok(item)) = self.read.next().await else {
                    return ReadEvent::ConnectionClose;
                };

                match item {
                    ConnectionItem::Line(line) => {
                        if !expecting_input.is_empty() {
                            ReadEvent::InputReply(v_str(&line))
                        } else {
                            ReadEvent::Command(line)
                        }
                    }
                    ConnectionItem::Bytes(bytes) => {
                        if !self.is_binary_mode {
                            // Binary data in text mode is not expected
                            return ReadEvent::PendingEvent;
                        }

                        if !expecting_input.is_empty() {
                            // Convert binary data to Var::Binary for input reply
                            ReadEvent::InputReply(Var::mk_binary(bytes.to_vec()))
                        } else {
                            // Binary data as unprompted command not yet supported
                            return ReadEvent::PendingEvent;
                        }
                    }
                }
            };

            select! {
                line = input_future => {
                    match line {
                        ReadEvent::Command(line) => {
                            if let Some(ref mut buffer) = self.hold_input {
                                // When hold-input is active, store input in buffer for read() calls
                                debug!("Holding input due to hold-input option: {}", line);
                                buffer.push(line);
                            } else {
                                line_mode = self.handle_command(&mut program_input, line_mode, line).await.expect("Unable to process command");
                            }
                        }
                        ReadEvent::InputReply(input_data) =>{
                            self.process_requested_input_line(input_data, &mut expecting_input).await.expect("Unable to process input reply");
                        }
                        ReadEvent::ConnectionClose => {
                            info!("Connection closed");
                            return Ok(());
                        }
                        ReadEvent::PendingEvent => {
                            continue
                        }
                    }
                }
                Ok(event) = broadcast_recv(&mut self.broadcast_sub) => {
                    match event {
                        ClientsBroadcastEvent::PingPong(_server_time) => {
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id,
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(),
                                    self.handler_object, HostType::WebSocket, self.peer_addr)).await.expect("Unable to send pong to RPC server");

                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    if let Some(input_request) = self.handle_narrative_event(event).await? {
                        expecting_input.push_back(input_request);
                    }
                }
            }
        }
    }

    async fn handle_command(
        &mut self,
        program_input: &mut Vec<String>,
        line_mode: LineMode,
        line: String,
    ) -> Result<LineMode, eyre::Error> {
        // Handle flush command first - it should be processed immediately.
        if line.trim() == self.flush_command {
            self.flush().await?;
            return Ok(line_mode);
        }

        if let LineMode::SpoolingProgram(target, verb) = &line_mode {
            // If the line is "." that means we're done, and we can send the program off and switch modes back.
            if line == "." {
                // Clear the program input, and send it off.
                let Some(auth_token) = self.auth_token.clone() else {
                    bail!("Received program command before auth token was set");
                };
                let code = std::mem::take(program_input);
                let target = ObjectRef::Match(target.clone());
                let verb = Symbol::mk(verb);
                match self
                    .rpc_client
                    .make_client_rpc_call(
                        self.client_id,
                        HostClientToDaemonMessage::Program(
                            self.client_token.clone(),
                            auth_token,
                            target,
                            verb,
                            code,
                        ),
                    )
                    .await?
                {
                    ReplyResult::ClientSuccess(DaemonToClientReply::ProgramResponse(resp)) => {
                        match resp {
                            VerbProgramResponse::Success(o, verb) => {
                                self.send_line(&format!(
                                    "0 error(s).\nVerb {verb} programmed on object {o}"
                                ))
                                .await?;
                            }
                            VerbProgramResponse::Failure(VerbProgramError::CompilationError(e)) => {
                                let desc = describe_compile_error(e);
                                self.send_line(&desc).await?;
                            }
                            VerbProgramResponse::Failure(VerbProgramError::NoVerbToProgram) => {
                                self.send_line("That object does not have that verb.")
                                    .await?;
                            }
                            VerbProgramResponse::Failure(e) => {
                                error!("Unhandled verb program error: {:?}", e);
                            }
                        }
                    }
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
                return Ok(LineMode::Input);
            } else {
                // Otherwise, we're still spooling up the program, so just keep spooling.
                program_input.push(line);
            }
            return Ok(line_mode);
        }

        // Handle special built-in commands before regular command processing
        if self.handle_builtin_command(&line).await? {
            return Ok(line_mode);
        }

        if line.starts_with(".program") {
            let words = parse_into_words(&line);
            let usage_msg = "Usage: .program <target>:<verb>";
            if words.len() != 2 {
                self.send_line(usage_msg).await?;
                return Ok(line_mode);
            }
            let verb_spec = words[1].split(':').collect::<Vec<_>>();
            if verb_spec.len() != 2 {
                self.send_line(usage_msg).await?;
                return Ok(line_mode);
            }
            let target = verb_spec[0].to_string();
            let verb = verb_spec[1].to_string();

            // verb must be a valid identifier
            if !verb.chars().all(|c| c.is_alphanumeric() || c == '_') {
                self.send_line("You must specify a verb; use the format object:verb.")
                    .await?;
                return Ok(line_mode);
            }

            // target should be a valid object #number, $objref, ident, or
            //  a string inside quotes
            if !target.starts_with('$')
                && !target.starts_with('#')
                && !target.starts_with('"')
                && !target.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                self.send_line("You must specify a target; use the format object:verb.")
                    .await?;
                return Ok(line_mode);
            }

            self.send_line(&format!("Now programming {}. Use \".\" to end.", words[1]))
                .await?;
            self.flush().await?;

            return Ok(LineMode::SpoolingProgram(target, verb));
        }

        self.process_command_line(line)
            .await
            .expect("Unable to process command line");
        Ok(line_mode)
    }

    /// Handle built-in commands that are processed by the telnet host itself.
    /// Returns true if the command was handled, false if it should be passed through to normal processing.
    async fn handle_builtin_command(&mut self, line: &str) -> Result<bool, eyre::Error> {
        let words = parse_into_words(line);
        if words.is_empty() {
            return Ok(false);
        }

        let command = words[0].to_uppercase();
        match command.as_str() {
            "PREFIX" | "OUTPUTPREFIX" => {
                // Set output prefix
                if words.len() == 1 {
                    // Clear prefix
                    self.output_prefix = None;
                } else {
                    // Set prefix to everything after the command
                    let prefix = line[words[0].len()..].trim_start();
                    self.output_prefix = if prefix.is_empty() {
                        None
                    } else {
                        Some(prefix.to_string())
                    };
                }

                // Notify daemon of prefix change
                if let Some(auth_token) = &self.auth_token {
                    let prefix_value = self.output_prefix.as_ref().map(|s| v_str(s));
                    let _ = self
                        .rpc_client
                        .make_client_rpc_call(
                            self.client_id,
                            HostClientToDaemonMessage::SetClientAttribute(
                                self.client_token.clone(),
                                auth_token.clone(),
                                Symbol::mk("line-output-prefix"),
                                prefix_value,
                            ),
                        )
                        .await;
                }
                Ok(true)
            }
            "SUFFIX" | "OUTPUTSUFFIX" => {
                // Set output suffix
                if words.len() == 1 {
                    // Clear suffix
                    self.output_suffix = None;
                } else {
                    // Set suffix to everything after the command
                    let suffix = line[words[0].len()..].trim_start();
                    self.output_suffix = if suffix.is_empty() {
                        None
                    } else {
                        Some(suffix.to_string())
                    };
                }

                // Notify daemon of suffix change
                if let Some(auth_token) = &self.auth_token {
                    let suffix_value = self.output_suffix.as_ref().map(|s| v_str(s));
                    let _ = self
                        .rpc_client
                        .make_client_rpc_call(
                            self.client_id,
                            HostClientToDaemonMessage::SetClientAttribute(
                                self.client_token.clone(),
                                auth_token.clone(),
                                Symbol::mk("line-output-suffix"),
                                suffix_value,
                            ),
                        )
                        .await;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn handle_narrative_event(
        &mut self,
        event: ClientEvent,
    ) -> Result<Option<Uuid>, eyre::Error> {
        trace!(?event, "narrative_event");
        match event {
            ClientEvent::SystemMessage(_, msg) => {
                self.send_line(&msg)
                    .await
                    .expect("Unable to send message to client");
            }
            ClientEvent::Narrative(_author, event) => {
                let msg = event.event();
                match &msg {
                    Event::Notify {
                        value: msg,
                        content_type,
                        ..
                    } => {
                        let output_str = output_format(msg, *content_type)?;
                        self.send_line(&output_str_format(&output_str, *content_type)?)
                            .await
                            .expect("Unable to send message to client");
                    }
                    Event::Traceback(exception) => {
                        for frame in &exception.backtrace {
                            let Some(s) = frame.as_string() else {
                                continue;
                            };
                            self.send_line(s)
                                .await
                                .with_context(|| "Unable to send message to client")?;
                        }
                    }
                    _ => {
                        // We don't handle these events in the telnet client.
                        warn!("Unhandled event in telnet client: {:?}", msg);
                    }
                }
            }
            ClientEvent::RequestInput(request_id) => {
                // If hold_input is active and has buffered input, return it immediately
                if let Some(ref mut buffer) = self.hold_input {
                    if let Some(input_line) = buffer.drain(..1).next() {
                        debug!("Returning held input for read() call: {}", input_line);

                        // Send the buffered input as an input reply
                        let Some(auth_token) = self.auth_token.clone() else {
                            bail!("Received input request before auth token was set");
                        };

                        self.rpc_client
                            .make_client_rpc_call(
                                self.client_id,
                                HostClientToDaemonMessage::RequestedInput(
                                    self.client_token.clone(),
                                    auth_token,
                                    request_id,
                                    v_str(&input_line),
                                ),
                            )
                            .await?;

                        return Ok(None);
                    }
                }

                // No buffered input available, wait for user input
                return Ok(Some(request_id));
            }
            ClientEvent::Disconnect() => {
                self.pending_task = None;
                self.send_line("** Disconnected **")
                    .await
                    .expect("Unable to send disconnect message to client");
                self.flush()
                    .await
                    .expect("Unable to flush disconnect message");
                self.write
                    .close()
                    .await
                    .expect("Unable to close connection");
            }
            ClientEvent::TaskError(ti, te) => {
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                self.handle_task_error(te)
                    .await
                    .expect("Unable to handle task error");
                // Send suffix after task error
                self.send_output_suffix()
                    .await
                    .expect("Unable to send output suffix");
            }
            ClientEvent::TaskSuccess(ti, _s) => {
                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                // Send suffix after task success
                self.send_output_suffix()
                    .await
                    .expect("Unable to send output suffix");
            }
            ClientEvent::PlayerSwitched {
                new_player,
                new_auth_token,
            } => {
                info!(
                    "Switching player from {} to {} for client {}",
                    self.connection_object, new_player, self.client_id
                );
                self.connection_object = new_player;
                self.auth_token = Some(new_auth_token);
                info!(
                    "Player switched successfully to {} for client {}",
                    new_player, self.client_id
                );
            }
            ClientEvent::SetConnectionOption {
                connection_obj,
                option_name,
                value,
            } => {
                debug!(
                    "Received SetConnectionOption: connection_obj={}, option_name={}, value={:?}, our_connection={}",
                    connection_obj, option_name, value, self.connection_object
                );
                // Only handle if this event is for our connection
                if connection_obj == self.connection_object {
                    self.handle_connection_option(option_name, Some(value))
                        .await?;
                } else {
                    debug!("Ignoring SetConnectionOption for different connection");
                }
            }
        }

        Ok(None)
    }

    async fn process_command_line(&mut self, line: String) -> Result<(), eyre::Error> {
        let Some(auth_token) = self.auth_token.clone() else {
            bail!("Received command before auth token was set");
        };

        // Send output prefix before executing command
        self.send_output_prefix().await?;

        let result = if line.starts_with(OUT_OF_BAND_PREFIX) && !self.disable_oob {
            self.rpc_client
                .make_client_rpc_call(
                    self.client_id,
                    HostClientToDaemonMessage::OutOfBand(
                        self.client_token.clone(),
                        auth_token.clone(),
                        self.handler_object,
                        line,
                    ),
                )
                .await?
        } else {
            let line = line.trim().to_string();
            self.rpc_client
                .make_client_rpc_call(
                    self.client_id,
                    HostClientToDaemonMessage::Command(
                        self.client_token.clone(),
                        auth_token.clone(),
                        self.handler_object,
                        line,
                    ),
                )
                .await?
        };
        match result {
            ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(ti)) => {
                self.pending_task = Some(PendingTask {
                    task_id: ti,
                    start_time: Instant::now(),
                });
            }
            ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                bail!("Received input thanks unprovoked, out of order")
            }
            ReplyResult::Failure(RpcMessageError::TaskError(e)) => {
                self.handle_task_error(e)
                    .await
                    .with_context(|| "Unable to handle task error")?;
                // Send suffix after task error
                self.send_output_suffix().await?;
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
        Ok(())
    }

    async fn process_requested_input_line(
        &mut self,
        input_data: Var,
        expecting_input: &mut VecDeque<Uuid>,
    ) -> Result<(), eyre::Error> {
        let Some(input_request_id) = expecting_input.front() else {
            bail!("Attempt to send reply to input request without an input request");
        };

        let Some(auth_token) = self.auth_token.clone() else {
            bail!("Received input reply before auth token was set");
        };

        match self
            .rpc_client
            .make_client_rpc_call(
                self.client_id,
                HostClientToDaemonMessage::RequestedInput(
                    self.client_token.clone(),
                    auth_token,
                    *input_request_id,
                    input_data,
                ),
            )
            .await
            .expect("Unable to send input to RPC server")
        {
            ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(task_id)) => {
                self.pending_task = Some(PendingTask {
                    task_id,
                    start_time: Instant::now(),
                });
                bail!("Got TaskSubmitted when expecting input-thanks")
            }
            ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
                expecting_input.pop_front();
            }
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
        Ok(())
    }

    async fn handle_task_error(&mut self, task_error: SchedulerError) -> Result<(), eyre::Error> {
        match task_error {
            SchedulerError::CommandExecutionError(CommandError::CouldNotParseCommand) => {
                self.send_line("I couldn't understand that.").await?;
                self.flush().await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoObjectMatch) => {
                self.send_line("I don't see that here.").await?;
                self.flush().await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoCommandMatch) => {
                self.send_line("I couldn't understand that.").await?;
                self.flush().await?;
            }
            SchedulerError::CommandExecutionError(CommandError::PermissionDenied) => {
                self.send_line("You can't do that.").await?;
                self.flush().await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(
                compile_error,
            )) => {
                let ce = describe_compile_error(compile_error);
                self.send_line(&ce).await?;
                self.send_line("Verb not programmed.").await?;
                self.flush().await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::NoVerbToProgram) => {
                self.send_line("That object does not have that verb definition.")
                    .await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Ticks(_)) => {
                self.send_line("Task ran out of ticks").await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Time(_)) => {
                self.send_line("Task ran out of seconds").await?;
            }
            SchedulerError::TaskAbortedError => {
                self.send_line("Task aborted").await?;
            }
            SchedulerError::TaskAbortedException(e) => {
                // This should not really be happening here... but?
                self.send_line(&format!("Task exception: {e}")).await?;
            }
            SchedulerError::TaskAbortedCancelled => {
                self.send_line("Task cancelled").await?;
            }
            _ => {
                warn!(?task_error, "Unhandled unexpected task error");
            }
        }
        Ok(())
    }

    /// Send output prefix if defined
    async fn send_output_prefix(&mut self) -> Result<(), eyre::Error> {
        if let Some(prefix) = self.output_prefix.clone() {
            self.send_line(&prefix).await?;
        }
        Ok(())
    }

    /// Send output suffix if defined
    async fn send_output_suffix(&mut self) -> Result<(), eyre::Error> {
        if let Some(suffix) = self.output_suffix.clone() {
            self.send_line(&suffix).await?;
        }
        Ok(())
    }
}

fn markdown_to_ansi(markdown: &str) -> String {
    let skin = MadSkin::default_dark();
    // TODO: permit different text stylings here. e.g. user themes for colours, styling, etc.
    //   will require custom host-side commands to set these.
    skin.text(markdown, None).to_string()
}

/// Produce the right kind of "telnet" compatible output for the given content.
fn output_format(content: &Var, content_type: Option<Symbol>) -> Result<String, eyre::Error> {
    match content.variant() {
        Variant::Str(s) => output_str_format(s.as_str(), content_type),
        Variant::Sym(s) => output_str_format(&s.as_arc_string(), content_type),
        Variant::List(l) => {
            // If the content is a list, it must be a list of strings.
            let mut output = String::new();
            for item in l.iter() {
                let Some(item_str) = item.as_string() else {
                    bail!("Expected list item to be a string, got: {:?}", item);
                };
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(item_str);
            }
            output_str_format(&output, content_type)
        }
        _ => bail!("Unsupported content type: {:?}", content.variant()),
    }
}

fn output_str_format(content: &str, content_type: Option<Symbol>) -> Result<String, eyre::Error> {
    let Some(content_type) = content_type else {
        return Ok(content.to_string());
    };
    let content_type = content_type.as_arc_string();
    Ok(match content_type.as_str() {
        CONTENT_TYPE_MARKDOWN => markdown_to_ansi(content),
        CONTENT_TYPE_DJOT => {
            // For now, we treat Djot as markdown.
            // In the future, we might want to support Djot specifically, but in realiy, djot
            // is mainly a (safer) subset of markdown.
            markdown_to_ansi(content)
        }
        // text/plain, None, or unknown
        _ => content.to_string(),
    })
}
