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

use eyre::Context;
use eyre::bail;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use moor_common::model::{CompileError, ObjectRef};
use moor_common::tasks::{AbortLimitReason, CommandError, Event, SchedulerError, VerbProgramError};
use moor_common::util::parse_into_words;
use moor_var::{Obj, Symbol, Var, Variant, v_bool, v_str};
use nectar::{
    TelnetCodec, constants, event::TelnetEvent, option::TelnetOption,
    subnegotiation::SubnegotiationType,
};
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
    /// The MOO connection object ID / player object ID.
    pub(crate) connection_oid: Obj,
    pub(crate) client_id: Uuid,
    /// Current PASETO token.
    pub(crate) client_token: ClientToken,
    pub(crate) write: SplitSink<Framed<TcpStream, TelnetCodec>, TelnetEvent>,
    pub(crate) read: SplitStream<Framed<TcpStream, TelnetCodec>>,
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

const TASK_TIMEOUT: Duration = Duration::from_secs(10);

impl TelnetConnection {
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

    /// Handle unknown telnet subnegotiations (primarily GMCP)
    async fn handle_unknown_subnegotiation(&mut self, option: &TelnetOption, data: &[u8]) {
        // Check if this is GMCP
        let is_gmcp = match option {
            TelnetOption::Unknown(opt_num) => *opt_num == constants::GMCP,
            _ => format!("{option:?}").contains("GMCP"),
        };

        if is_gmcp {
            self.handle_gmcp_message(data).await;
        } else {
            debug!(
                "Unhandled subnegotiation: option={:?}, data={:?}",
                option, data
            );
        }
    }

    /// Parse and handle GMCP messages
    async fn handle_gmcp_message(&mut self, data: &[u8]) {
        let Ok(gmcp_msg) = String::from_utf8(data.to_vec()) else {
            debug!("Failed to parse GMCP data as UTF-8");
            return;
        };

        debug!("GMCP: {}", gmcp_msg);

        // Parse GMCP message format: "Package.Message JSON"
        let Some(space_pos) = gmcp_msg.find(' ') else {
            debug!("Invalid GMCP format: no space separator");
            return;
        };

        let package_msg = &gmcp_msg[..space_pos];
        let json_data = &gmcp_msg[space_pos + 1..];

        match package_msg {
            "Core.Hello" => {
                let Ok(client_info) = self.parse_gmcp_core_hello(json_data) else {
                    debug!("Failed to parse Core.Hello JSON");
                    return;
                };

                self.connection_attributes
                    .insert(Symbol::mk("gmcp-client"), client_info.clone());
                self.update_connection_attribute(Symbol::mk("gmcp-client"), Some(client_info))
                    .await;
            }
            _ => {
                debug!("Unhandled GMCP package: {}", package_msg);
            }
        }
    }

    /// Parse GMCP Core.Hello JSON into MOO-friendly format
    fn parse_gmcp_core_hello(&self, json_data: &str) -> Result<Var, serde_json::Error> {
        use moor_var::v_list;

        let parsed: serde_json::Value = serde_json::from_str(json_data)?;

        let Some(obj) = parsed.as_object() else {
            return Ok(v_list(&[])); // Empty list for non-objects
        };

        // Convert JSON to MOO list format: [["key", value], ...]
        let pairs: Vec<Var> = obj
            .iter()
            .map(|(key, value)| {
                let moo_key = Var::from(key.clone());
                let moo_value = match value {
                    serde_json::Value::String(s) => Var::from(s.clone()),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Var::from(i)
                        } else {
                            // MOO doesn't have native float support, convert to string
                            Var::from(n.to_string())
                        }
                    }
                    serde_json::Value::Bool(b) => v_bool(*b),
                    _ => Var::from(value.to_string()),
                };

                v_list(&[moo_key, moo_value])
            })
            .collect();

        Ok(v_list(&pairs))
    }

    /// Handle telnet negotiation events and update connection attributes
    async fn handle_telnet_negotiation(&mut self, event: &TelnetEvent) {
        use nectar::event::TelnetEvent;
        match event {
            TelnetEvent::Subnegotiate(subneg_type) => {
                match subneg_type {
                    // NAWS window size
                    SubnegotiationType::WindowSize(width, height) => {
                        let width_var = Var::from(*width as i64);
                        let height_var = Var::from(*height as i64);

                        self.connection_attributes
                            .insert(Symbol::mk("terminal-width"), width_var.clone());
                        self.connection_attributes
                            .insert(Symbol::mk("terminal-height"), height_var.clone());

                        // Send updates to daemon
                        self.update_connection_attribute(
                            Symbol::mk("terminal-width"),
                            Some(width_var),
                        )
                        .await;
                        self.update_connection_attribute(
                            Symbol::mk("terminal-height"),
                            Some(height_var),
                        )
                        .await;

                        debug!("NAWS: terminal size {}x{}", width, height);
                    }
                    // Handle unknown subnegotiations that might be terminal type
                    SubnegotiationType::Unknown(option, data) => {
                        match option {
                            // Terminal Type (RFC 1091) - option 24
                            TelnetOption::Unknown(24) if !data.is_empty() && data[0] == 0 => {
                                // IS (0) subcommand - client is telling us their terminal type
                                if let Ok(terminal_type) = String::from_utf8(data[1..].to_vec()) {
                                    let term_var = Var::from(terminal_type.clone());
                                    self.connection_attributes
                                        .insert(Symbol::mk("terminal-type"), term_var.clone());

                                    // Send update to daemon
                                    self.update_connection_attribute(
                                        Symbol::mk("terminal-type"),
                                        Some(term_var),
                                    )
                                    .await;

                                    debug!("Terminal type: {}", terminal_type);
                                }
                            }
                            _ => {
                                self.handle_unknown_subnegotiation(option, data).await;
                            }
                        }
                    }
                    // Charset negotiation
                    SubnegotiationType::CharsetAccepted(charset) => {
                        debug!("Client sent CharsetAccepted: {:?}", charset);
                        if let Ok(charset_str) = String::from_utf8(charset.to_vec()) {
                            let charset_var = Var::from(charset_str.clone());
                            self.connection_attributes
                                .insert(Symbol::mk("charset"), charset_var.clone());
                            self.update_connection_attribute(
                                Symbol::mk("charset"),
                                Some(charset_var),
                            )
                            .await;

                            debug!("Client accepted charset: {}", charset_str);
                        }
                    }
                    SubnegotiationType::CharsetRejected => {
                        debug!("Client sent CharsetRejected");
                        self.connection_attributes
                            .insert(Symbol::mk("charset-rejected"), v_bool(true));
                        debug!("Client rejected charset negotiation");
                    }

                    // Environment variables
                    SubnegotiationType::Environment(env_op) => {
                        debug!("Environment operation: {:?}", env_op);
                        // Note: Could extract specific environment variables here if needed
                        let env_active = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("environ-active"), env_active.clone());
                        self.update_connection_attribute(
                            Symbol::mk("environ-active"),
                            Some(env_active),
                        )
                        .await;
                    }

                    // Linemode configuration
                    SubnegotiationType::LineMode(linemode_opt) => {
                        debug!("Linemode option: {:?}", linemode_opt);
                        let linemode_active = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("linemode-active"), linemode_active.clone());
                        self.update_connection_attribute(
                            Symbol::mk("linemode-active"),
                            Some(linemode_active),
                        )
                        .await;
                    }

                    _ => {
                        debug!("Unhandled subnegotiation type: {:?}", subneg_type);
                        // Log any charset-related subnegotiation we might be missing
                        if format!("{subneg_type:?}")
                            .to_lowercase()
                            .contains("charset")
                        {
                            debug!("Missed charset subnegotiation: {:?}", subneg_type);
                        }
                    }
                }
            }
            TelnetEvent::Will(option) => {
                match option {
                    TelnetOption::NAWS => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-naws"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-naws"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client will send NAWS");
                    }
                    TelnetOption::Unknown(24) => {
                        // Terminal Type
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-terminal-type"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-terminal-type"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client will send terminal type");
                    }
                    TelnetOption::Environ => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-environ"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-environ"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports environment variables");
                    }
                    TelnetOption::LineMode => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-linemode"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-linemode"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports linemode");
                    }
                    TelnetOption::Charset => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-charset"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-charset"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports charset negotiation");

                        // Now request UTF-8 charset
                        debug!("About to send UTF-8 charset request");
                        if let Err(e) = self.request_utf8_charset().await {
                            debug!("Failed to request UTF-8 charset: {}", e);
                        } else {
                            debug!("UTF-8 charset request sent successfully");
                        }
                    }
                    TelnetOption::GMCP => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-gmcp"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-gmcp"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports GMCP - attribute set");
                    }
                    TelnetOption::MCCP2 => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-mccp2"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-mccp2"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports MCCP2 compression");
                    }
                    TelnetOption::MSSP => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-mssp"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-mssp"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports MSP (sound)");
                    }
                    TelnetOption::MSP => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-msp"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-msp"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports MSP (sound)");
                    }
                    TelnetOption::MXP => {
                        let supports_var = v_bool(true);
                        self.connection_attributes
                            .insert(Symbol::mk("supports-mxp"), supports_var.clone());
                        self.update_connection_attribute(
                            Symbol::mk("supports-mxp"),
                            Some(supports_var),
                        )
                        .await;
                        debug!("Client supports MXP (markup)");
                    }
                    _ => debug!("Client will: {:?}", option),
                }
            }
            TelnetEvent::Wont(option) => {
                match option {
                    TelnetOption::NAWS => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-naws"));
                        self.update_connection_attribute(Symbol::mk("supports-naws"), None)
                            .await;
                        debug!("Client won't send NAWS");
                    }
                    TelnetOption::Unknown(24) => {
                        // Terminal Type
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-terminal-type"));
                        self.update_connection_attribute(
                            Symbol::mk("supports-terminal-type"),
                            None,
                        )
                        .await;
                        debug!("Client won't send terminal type");
                    }
                    TelnetOption::Environ => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-environ"));
                        self.update_connection_attribute(Symbol::mk("supports-environ"), None)
                            .await;
                        debug!("Client won't send environment variables");
                    }
                    TelnetOption::LineMode => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-linemode"));
                        self.update_connection_attribute(Symbol::mk("supports-linemode"), None)
                            .await;
                        debug!("Client won't support linemode");
                    }
                    TelnetOption::Charset => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-charset"));
                        self.update_connection_attribute(Symbol::mk("supports-charset"), None)
                            .await;
                        debug!("Client won't support charset negotiation");
                    }
                    TelnetOption::GMCP => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-gmcp"));
                        self.update_connection_attribute(Symbol::mk("supports-gmcp"), None)
                            .await;
                        debug!("Client won't support GMCP");
                    }
                    TelnetOption::MCCP2 => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-mccp2"));
                        self.update_connection_attribute(Symbol::mk("supports-mccp2"), None)
                            .await;
                        debug!("Client won't support MCCP2 compression");
                    }
                    TelnetOption::MSP => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-msp"));
                        self.update_connection_attribute(Symbol::mk("supports-msp"), None)
                            .await;
                        debug!("Client won't support MSP (sound)");
                    }
                    TelnetOption::MXP => {
                        self.connection_attributes
                            .remove(&Symbol::mk("supports-mxp"));
                        self.update_connection_attribute(Symbol::mk("supports-mxp"), None)
                            .await;
                        debug!("Client won't support MXP (markup)");
                    }
                    _ => debug!("Client won't: {:?}", option),
                }
            }
            _ => {
                // Handle other negotiation events if needed
            }
        }
    }

    /// Send telnet negotiation to request client capabilities
    async fn negotiate_telnet_capabilities(&mut self) -> Result<(), eyre::Error> {
        // Only request basic, widely-supported options that won't break simple telnet clients

        // Request NAWS (window size) - well supported
        let naws_request = TelnetEvent::Do(TelnetOption::NAWS);
        self.write.send(naws_request).await?;

        // Request terminal type - well supported
        let term_type_request = TelnetEvent::Do(TelnetOption::Unknown(24)); // Terminal Type
        self.write.send(term_type_request).await?;

        // Test GMCP - modern MUD clients should handle this gracefully
        let gmcp_request = TelnetEvent::Do(TelnetOption::GMCP);
        self.write.send(gmcp_request).await?;

        // Request charset negotiation - useful for internationalization
        let charset_request = TelnetEvent::Do(TelnetOption::Charset);
        self.write.send(charset_request).await?;

        Ok(())
    }

    /// Request UTF-8 charset from client after they indicate charset support
    async fn request_utf8_charset(&mut self) -> Result<(), eyre::Error> {
        use nectar::{event::TelnetEvent, subnegotiation::SubnegotiationType};

        // According to RFC 2066, charset request format is:
        // IAC SB CHARSET REQUEST "[sep]charset[sep]charset..." IAC SE
        // We'll request UTF-8 with space separator
        let utf8_request = b"REQUEST UTF-8".to_vec();
        let charset_request = TelnetEvent::Subnegotiate(SubnegotiationType::CharsetRequest(vec![
            utf8_request.into(),
        ]));

        self.write.send(charset_request).await?;
        debug!("Requested UTF-8 charset from client");

        Ok(())
    }

    pub(crate) async fn run(&mut self) -> Result<(), eyre::Error> {
        // Set basic connection attributes
        self.connection_attributes
            .insert(Symbol::mk("host-type"), Var::from("telnet".to_string()));
        self.connection_attributes
            .insert(Symbol::mk("supports-telnet-protocol"), v_bool(true));

        // Send telnet capability negotiation first
        if let Err(e) = self.negotiate_telnet_capabilities().await {
            warn!("Failed to negotiate telnet capabilities: {}", e);
        }

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
        self.write
            .send(TelnetEvent::Message(connect_message.to_string()))
            .await?;

        debug!(?player, client_id = ?self.client_id, "Entering command dispatch loop");

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
                value: msg,
                content_type,
                no_flush,
                no_newline,
            } => {
                let Ok(formatted) = output_format(&msg, content_type) else {
                    warn!("Failed to format message: {:?}", msg);
                    return Ok(());
                };

                // TODO: Handle no_flush - for now telnet doesn't have buffer control
                //   but we could potentially batch messages or use different send methods
                let _ = no_flush; // Acknowledge parameter for now

                let telnet_event = if no_newline {
                    TelnetEvent::RawMessage(formatted)
                } else {
                    TelnetEvent::Message(formatted)
                };

                self.write
                    .send(telnet_event)
                    .await
                    .with_context(|| "Unable to send message to client")?;
            }

            Event::Traceback(e) => {
                for frame in e.backtrace {
                    let Some(s) = frame.as_string() else {
                        continue;
                    };
                    self.write
                        .send(TelnetEvent::Message(s.to_string()))
                        .await
                        .with_context(|| "Unable to send message to client")?;
                }
            }
            _ => {
                self.write
                    .send(TelnetEvent::Message(format!(
                        "Unsupported event for telnet: {event:?}"
                    )))
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
                                HostClientToDaemonMessage::ClientPong(self.client_token.clone(), SystemTime::now(), self.connection_oid, HostType::TCP, self.peer_addr)).await?;
                        }
                    }
                }
                Ok(event) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    trace!(?event, "narrative_event");
                    match event {
                        ClientEvent::SystemMessage(_author, msg) => {
                            self.write.send(TelnetEvent::Message(msg)).await.with_context(|| "Unable to send message to client")?;
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
                            info!("Switching player from {} to {} during authorization for client {}", self.connection_oid, new_player, self.client_id);
                            self.connection_oid = new_player;
                            self.auth_token = Some(new_auth_token);
                            info!("Player switched successfully to {} during authorization for client {}", new_player, self.client_id);
                        }
                    }
                }
                // Auto loop
                event = self.read.next() => {
                    let Some(event) = event else {
                        bail!("Connection closed before login");
                    };
                    let event = event.unwrap();
                    let line = match event {
                        TelnetEvent::Message(text) => text,
                        ref negotiation_event => {
                            self.handle_telnet_negotiation(negotiation_event).await;
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
                        self.connection_oid = *player;
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
                    if let Some(input_request) = self.handle_narrative_event(event).await? {
                        expecting_input.push_back(input_request);
                    }
                }
                event = self.read.next() => {
                    let Some(event) = event else {
                        info!("Connection closed");
                        break;
                    };
                    let event = event?;

                    match event {
                        TelnetEvent::Message(text) => {
                            // Handle input replies first
                            if !expecting_input.is_empty() {
                                self.process_requested_input_line(text, &mut expecting_input).await?;
                                continue;
                            }

                            // Skip processing new commands if we have a pending task,
                            // but only if we're not expecting input (read() replies must go through!)
                            if self.pending_task.is_some() && expecting_input.is_empty() {
                                continue;
                            }

                            let line = text;

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
                                                    .send(TelnetEvent::Message(format!(
                                                        "0 error(s).\nVerb {verb} programmed on object {o}"
                                                    )))
                                                    .await?;
                                            }
                                            VerbProgramResponse::Failure(VerbProgramError::CompilationError(e)) => {
                                                let desc = describe_compile_error(e);
                                                self.write.send(TelnetEvent::Message(desc)).await?;
                                            }
                                            VerbProgramResponse::Failure(VerbProgramError::NoVerbToProgram) => {
                                                self.write
                                                    .send(TelnetEvent::Message("That object does not have that verb.".to_string()))
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
                            if self.handle_builtin_command(&line).await? {
                                continue;
                            }

                            if line.starts_with(".program") {
                                let words = parse_into_words(&line);
                                let usage_msg = "Usage: .program <target>:<verb>";
                                if words.len() != 2 {
                                    self.write.send(TelnetEvent::Message(usage_msg.to_string())).await?;
                                    continue;
                                }
                                let verb_spec = words[1].split(':').collect::<Vec<_>>();
                                if verb_spec.len() != 2 {
                                    self.write.send(TelnetEvent::Message(usage_msg.to_string())).await?;
                                    continue;
                                }
                                let target = verb_spec[0].to_string();
                                let verb = verb_spec[1].to_string();

                                // verb must be a valid identifier
                                if !verb.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                    self.write
                                        .send(TelnetEvent::Message("You must specify a verb; use the format object:verb.".to_string()))
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
                                        .send(TelnetEvent::Message("You must specify a target; use the format object:verb.".to_string()))
                                        .await?;
                                    continue;
                                }

                                self.write
                                    .send(TelnetEvent::Message(format!("Now programming {}. Use \".\" to end.", words[1])))
                                    .await?;

                                line_mode = LineMode::SpoolingProgram(target, verb);
                                continue;
                            }

                            self.process_command_line(line).await?;
                        }
                        ref negotiation_event => {
                            self.handle_telnet_negotiation(negotiation_event).await;
                        }
                    }
                }
            }
        }
        Ok(())
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
                self.write
                    .send(TelnetEvent::Message(msg))
                    .await
                    .expect("Unable to send message to client");
            }
            ClientEvent::Narrative(_author, event) => {
                let msg = event.event();
                match &msg {
                    Event::Notify {
                        value: msg,
                        content_type,
                        no_flush,
                        no_newline,
                    } => {
                        let output_str = output_format(msg, *content_type)?;

                        // TODO: Handle no_flush - acknowledge for now
                        let _ = no_flush;

                        let telnet_event = if *no_newline {
                            TelnetEvent::RawMessage(output_str)
                        } else {
                            TelnetEvent::Message(output_str)
                        };

                        self.write
                            .send(telnet_event)
                            .await
                            .expect("Unable to send message to client");
                    }
                    Event::Traceback(exception) => {
                        for frame in &exception.backtrace {
                            let Some(s) = frame.as_string() else {
                                continue;
                            };
                            self.write
                                .send(TelnetEvent::Message(s.to_string()))
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
                return Ok(Some(request_id));
            }
            ClientEvent::Disconnect() => {
                self.pending_task = None;
                self.write
                    .send(TelnetEvent::Message("** Disconnected **".to_string()))
                    .await
                    .expect("Unable to send disconnect message to client");
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
                    self.connection_oid, new_player, self.client_id
                );
                self.connection_oid = new_player;
                self.auth_token = Some(new_auth_token);
                info!(
                    "Player switched successfully to {} for client {}",
                    new_player, self.client_id
                );
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

        let result = if line.starts_with(OUT_OF_BAND_PREFIX) {
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
        line: String,
        expecting_input: &mut VecDeque<Uuid>,
    ) -> Result<(), eyre::Error> {
        let cmd = line.trim().to_string();

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
                    cmd,
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
                self.write
                    .send(TelnetEvent::Message(
                        "I couldn't understand that.".to_string(),
                    ))
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoObjectMatch) => {
                self.write
                    .send(TelnetEvent::Message("I don't see that here.".to_string()))
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::NoCommandMatch) => {
                self.write
                    .send(TelnetEvent::Message(
                        "I couldn't understand that.".to_string(),
                    ))
                    .await?;
            }
            SchedulerError::CommandExecutionError(CommandError::PermissionDenied) => {
                self.write
                    .send(TelnetEvent::Message("You can't do that.".to_string()))
                    .await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(
                compile_error,
            )) => {
                let ce = describe_compile_error(compile_error);
                self.write.send(TelnetEvent::Message(ce)).await?;
                self.write
                    .send(TelnetEvent::Message("Verb not programmed.".to_string()))
                    .await?;
            }
            SchedulerError::VerbProgramFailed(VerbProgramError::NoVerbToProgram) => {
                self.write
                    .send(TelnetEvent::Message(
                        "That object does not have that verb definition.".to_string(),
                    ))
                    .await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Ticks(_)) => {
                self.write
                    .send(TelnetEvent::Message("Task ran out of ticks".to_string()))
                    .await?;
            }
            SchedulerError::TaskAbortedLimit(AbortLimitReason::Time(_)) => {
                self.write
                    .send(TelnetEvent::Message("Task ran out of seconds".to_string()))
                    .await?;
            }
            SchedulerError::TaskAbortedError => {
                self.write
                    .send(TelnetEvent::Message("Task aborted".to_string()))
                    .await?;
            }
            SchedulerError::TaskAbortedException(e) => {
                // This should not really be happening here... but?
                self.write
                    .send(TelnetEvent::Message(format!("Task exception: {e}")))
                    .await?;
            }
            SchedulerError::TaskAbortedCancelled => {
                self.write
                    .send(TelnetEvent::Message("Task cancelled".to_string()))
                    .await?;
            }
            _ => {
                warn!(?task_error, "Unhandled unexpected task error");
            }
        }
        Ok(())
    }

    /// Send output prefix if defined
    async fn send_output_prefix(&mut self) -> Result<(), eyre::Error> {
        if let Some(ref prefix) = self.output_prefix {
            self.write
                .send(TelnetEvent::Message(prefix.clone()))
                .await?;
        }
        Ok(())
    }

    /// Send output suffix if defined  
    async fn send_output_suffix(&mut self) -> Result<(), eyre::Error> {
        if let Some(ref suffix) = self.output_suffix {
            self.write
                .send(TelnetEvent::Message(suffix.clone()))
                .await?;
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
