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

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    os::fd::RawFd,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant, SystemTime},
};

use crate::connection_codec::{ConnectionCodec, ConnectionFrame, ConnectionItem};
use eyre::{Context, bail};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use moor_common::{
    model::{CompileError, ObjectRef},
    tasks::{AbortLimitReason, CommandError, Event, SchedulerError, VerbProgramError},
    util::parse_into_words,
};
use moor_schema::{
    convert::{compilation_error_from_ref, narrative_event_from_ref, obj_from_ref},
    rpc as moor_rpc,
};
use moor_var::{Obj, Symbol, Var, Variant, v_str};
use rpc_async_client::{
    pubsub_client::{broadcast_recv, events_recv},
    rpc_client::RpcClient,
};
use rpc_common::{
    AuthToken, ClientToken, extract_obj, extract_symbol, extract_var, mk_client_pong_msg,
    mk_command_msg, mk_detach_msg, mk_login_command_msg, mk_out_of_band_msg, mk_program_msg,
    mk_requested_input_msg, mk_set_client_attribute_msg, read_reply_result,
    scheduler_error_from_rpc_error,
};
use socket2::{SockRef, TcpKeepalive};
use std::pin::Pin;
use tmq::subscribe::Subscribe;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    select,
};
use tokio_util::codec::Framed;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Combined trait for async read/write streams (needed for trait objects).
pub(crate) trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

/// Type alias for a boxed async stream that can be either TcpStream or TlsStream.
pub(crate) type BoxedAsyncIo = Pin<Box<dyn AsyncStream>>;

use crate::djot_formatter::djot_to_ansi;

/// Out of band messages are prefixed with this string, e.g. for MCP clients.
const OUT_OF_BAND_PREFIX: &str = "#$#";

/// Default flush command
pub(crate) const DEFAULT_FLUSH_COMMAND: &str = ".flush";

const CONTENT_TYPE_MARKDOWN: &str = "text_markdown";
const CONTENT_TYPE_DJOT: &str = "text_djot";
const CONTENT_TYPE_DJOT_SLASH: &str = "text/djot";
const CONTENT_TYPE_MARKDOWN_SLASH: &str = "text/markdown";

/// Output formatting result - indicates how the content should be sent
enum FormattedOutput {
    /// Plain text - needs newline added via send_line
    Plain(String),
    /// Rich formatted (markdown/djot) - djot formatter handles line endings
    Rich(String),
}

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
    pub(crate) write: SplitSink<Framed<BoxedAsyncIo, ConnectionCodec>, ConnectionFrame>,
    pub(crate) read: SplitStream<Framed<BoxedAsyncIo, ConnectionCodec>>,
    pub(crate) kill_switch: Arc<AtomicBool>,

    pub(crate) broadcast_sub: Subscribe,
    pub(crate) narrative_sub: Subscribe,
    pub(crate) auth_token: Option<AuthToken>,
    pub(crate) rpc_client: RpcClient,
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
    /// Pending line mode to switch to (for text_area input)
    pub(crate) pending_line_mode: Option<LineMode>,
    /// Currently collecting input (allows input even when pending_task is set)
    pub(crate) collecting_input: bool,
    /// Raw file descriptor for the socket (used for setting socket options like keep-alive)
    pub(crate) socket_fd: RawFd,
}

/// The input modes the telnet session can be in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LineMode {
    /// Receiving input
    Input,
    /// Spooling up .program input.
    SpoolingProgram(String, String),
    /// Collecting multiline text_area input
    CollectingTextArea(Uuid),
}

/// Metadata for input requests, matching the web client's InputMetadata
#[derive(Debug, Clone)]
struct InputMetadata {
    input_type: Option<String>,
    prompt: Option<String>,
    choices: Option<Vec<String>>,
    min: Option<i64>,
    max: Option<i64>,
    default: Option<Var>,
    placeholder: Option<String>,
    rows: Option<i64>,
    alternative_label: Option<String>,
    alternative_placeholder: Option<String>,
}

impl InputMetadata {
    /// Parse metadata from RequestInputEvent
    fn from_metadata_pairs(
        metadata: Option<planus::Vector<'_, planus::Result<moor_rpc::MetadataPairRef<'_>>>>,
    ) -> Self {
        let mut result = Self {
            input_type: None,
            prompt: None,
            choices: None,
            min: None,
            max: None,
            default: None,
            placeholder: None,
            rows: None,
            alternative_label: None,
            alternative_placeholder: None,
        };

        let Some(metadata) = metadata else {
            return result;
        };

        for pair_result in metadata {
            let Ok(pair) = pair_result else {
                continue;
            };

            let Ok(key_ref) = pair.key() else {
                continue;
            };
            let Ok(key_str) = key_ref.value() else {
                continue;
            };

            let Ok(_value_ref) = pair.value() else {
                continue;
            };
            let Ok(value) = extract_var(&pair, "value", |p| p.value()) else {
                continue;
            };

            match key_str {
                "input_type" => {
                    result.input_type = value.as_string().map(|s| s.to_string());
                }
                "prompt" => {
                    result.prompt = value.as_string().map(|s| s.to_string());
                }
                "choices" => {
                    if let Variant::List(list) = value.variant() {
                        let choices: Vec<String> = list
                            .iter()
                            .filter_map(|v| v.as_string().map(|s| s.to_string()))
                            .collect();
                        if !choices.is_empty() {
                            result.choices = Some(choices);
                        }
                    }
                }
                "min" => {
                    result.min = value.as_integer();
                }
                "max" => {
                    result.max = value.as_integer();
                }
                "default" => {
                    result.default = Some(value.clone());
                }
                "placeholder" => {
                    result.placeholder = value.as_string().map(|s| s.to_string());
                }
                "rows" => {
                    result.rows = value.as_integer();
                }
                "alternative_label" => {
                    result.alternative_label = value.as_string().map(|s| s.to_string());
                }
                "alternative_placeholder" => {
                    result.alternative_placeholder = value.as_string().map(|s| s.to_string());
                }
                _ => {}
            }
        }

        result
    }
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
            details: _,
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
        CompileError::InvalidAssignmentTarget(_) => "Invalid l-value for assignment".to_string(),
        CompileError::UnknownTypeConstant(_, t) => {
            format!("Unknown type constant: {t}")
        }
        CompileError::InvalidTypeLiteralAssignment(t, _) => {
            format!("Illegal type literal `{t}` as assignment target")
        }
        CompileError::AssignmentToCapturedVariable(_, var) => {
            format!("Cannot assign to captured variable `{var}`; lambdas capture by value")
        }
    }
}

fn failure_error_context<'a>(
    failure: moor_rpc::FailureRef<'a>,
) -> Result<
    (
        moor_rpc::RpcMessageErrorRef<'a>,
        moor_rpc::RpcMessageErrorCode,
    ),
    eyre::Error,
> {
    let error_ref = failure
        .error()
        .map_err(|e| eyre::eyre!("Missing error: {e}"))?;
    let error_code = error_ref
        .error_code()
        .map_err(|e| eyre::eyre!("Missing error_code: {e}"))?;
    Ok((error_ref, error_code))
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

impl InputMetadata {
    /// Display the input prompt to the user based on the input type
    async fn display_prompt(&self, conn: &mut TelnetConnection) -> Result<(), eyre::Error> {
        // Render the prompt using markdown if present
        if let Some(prompt) = &self.prompt {
            // Try to get terminal width from connection attributes
            let _width = conn
                .connection_attributes
                .get(&Symbol::mk("columns"))
                .and_then(|v| v.as_integer())
                .and_then(|w| if w > 0 { Some(w as usize) } else { None });

            let formatted = djot_to_ansi(prompt);
            conn.send_line(&formatted).await?;
        }

        let input_type = self.input_type.as_deref().unwrap_or("text");

        match input_type {
            "yes_no" => {
                conn.send_line("Enter 'yes' or 'no'").await?;
            }
            "yes_no_alternative" => {
                conn.send_line("Enter 'yes', 'no', or describe an alternative")
                    .await?;
            }
            "choice" => {
                if let Some(choices) = &self.choices {
                    conn.send_line("Choose one of:").await?;
                    for (i, choice) in choices.iter().enumerate() {
                        conn.send_line(&format!("  {}. {}", i + 1, choice)).await?;
                    }
                    conn.send_line("Enter the number or text of your choice")
                        .await?;
                }
            }
            "number" => {
                let mut msg = "Enter a number".to_string();
                if let Some(min) = self.min {
                    if let Some(max) = self.max {
                        msg.push_str(&format!(" (between {} and {})", min, max));
                    } else {
                        msg.push_str(&format!(" (minimum {})", min));
                    }
                } else if let Some(max) = self.max {
                    msg.push_str(&format!(" (maximum {})", max));
                }
                conn.send_line(&msg).await?;
            }
            "text_area" => {
                conn.send_line("Enter your text. Use '.' on a line by itself to finish")
                    .await?;
            }
            "confirmation" => {
                conn.send_line("Press Enter to continue").await?;
            }
            "text" => {
                if let Some(placeholder) = &self.placeholder {
                    conn.send_line(&format!("({})", placeholder)).await?;
                }
            }
            _ => {
                // Unknown input type, treat as text
                if let Some(placeholder) = &self.placeholder {
                    conn.send_line(&format!("({})", placeholder)).await?;
                }
            }
        }

        conn.flush().await?;
        Ok(())
    }

    /// Validate and convert user input based on the input type
    fn validate_input(&self, input: &str) -> Result<Var, String> {
        let input_type = self.input_type.as_deref().unwrap_or("text");

        match input_type {
            "yes_no" => {
                let normalized = input.trim().to_lowercase();
                match normalized.as_str() {
                    "yes" | "y" => Ok(v_str("yes")),
                    "no" | "n" => Ok(v_str("no")),
                    _ => Err("Please enter 'yes' or 'no'".to_string()),
                }
            }
            "yes_no_alternative" => {
                let normalized = input.trim().to_lowercase();
                match normalized.as_str() {
                    "yes" | "y" => Ok(v_str("yes")),
                    "no" | "n" => Ok(v_str("no")),
                    _ => {
                        // Treat anything else as alternative text
                        if !normalized.is_empty() {
                            Ok(v_str(&format!("alternative: {}", input.trim())))
                        } else {
                            Err("Please enter 'yes', 'no', or describe an alternative".to_string())
                        }
                    }
                }
            }
            "choice" => {
                if let Some(choices) = &self.choices {
                    // Try to parse as a number first
                    if let Ok(num) = input.trim().parse::<usize>()
                        && num > 0
                        && num <= choices.len()
                    {
                        return Ok(v_str(&choices[num - 1]));
                    }
                    // Try to match the text
                    let normalized = input.trim().to_lowercase();
                    for choice in choices {
                        if choice.to_lowercase() == normalized {
                            return Ok(v_str(choice));
                        }
                    }
                    Err(format!(
                        "Please enter a number 1-{} or one of the listed choices",
                        choices.len()
                    ))
                } else {
                    Ok(v_str(input))
                }
            }
            "number" => {
                match input.trim().parse::<i64>() {
                    Ok(num) => {
                        // Validate min/max
                        if let Some(min) = self.min
                            && num < min
                        {
                            return Err(format!("Number must be at least {}", min));
                        }
                        if let Some(max) = self.max
                            && num > max
                        {
                            return Err(format!("Number must be at most {}", max));
                        }
                        Ok(Var::mk_integer(num))
                    }
                    Err(_) => Err("Please enter a valid number".to_string()),
                }
            }
            "confirmation" => Ok(v_str("ok")),
            "text" | "text_area" => Ok(v_str(input)),
            _ => Ok(v_str(input)), // Unknown input type, treat as text
        }
    }
}

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
        let option_str = option_name.as_arc_str();

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
            "keep-alive" => {
                self.set_tcp_keepalive(value)?;
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

    /// Set TCP keepalive options on the socket.
    /// Value can be:
    /// - An integer (1 to enable with defaults, 0 to disable)
    /// - A map with keys: "idle", "interval", "count"
    fn set_tcp_keepalive(&self, value: Option<Var>) -> Result<(), eyre::Error> {
        use std::os::fd::BorrowedFd;

        // Default values matching ToastStunt
        const DEFAULT_IDLE: u64 = 300; // 5 minutes
        const DEFAULT_INTERVAL: u64 = 120; // 2 minutes
        const DEFAULT_COUNT: u32 = 5;

        // SAFETY: We know the fd is valid because the connection is still active
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(self.socket_fd) };
        let sock_ref = SockRef::from(&borrowed_fd);

        let Some(value) = value else {
            // No value means disable
            if let Err(e) = sock_ref.set_tcp_keepalive(&TcpKeepalive::new()) {
                warn!("Failed to disable TCP keepalive: {}", e);
            } else {
                debug!("TCP keepalive disabled");
            }
            return Ok(());
        };

        // Check if it's a simple integer (0 = disable, non-zero = enable with defaults)
        if let Some(int_val) = value.as_integer() {
            if int_val == 0 {
                if let Err(e) = sock_ref.set_tcp_keepalive(&TcpKeepalive::new()) {
                    warn!("Failed to disable TCP keepalive: {}", e);
                } else {
                    debug!("TCP keepalive disabled");
                }
            } else {
                let keepalive = TcpKeepalive::new()
                    .with_time(Duration::from_secs(DEFAULT_IDLE))
                    .with_interval(Duration::from_secs(DEFAULT_INTERVAL))
                    .with_retries(DEFAULT_COUNT);
                if let Err(e) = sock_ref.set_tcp_keepalive(&keepalive) {
                    warn!("Failed to set TCP keepalive: {}", e);
                } else {
                    debug!(
                        "TCP keepalive enabled: idle={}s, interval={}s, count={}",
                        DEFAULT_IDLE, DEFAULT_INTERVAL, DEFAULT_COUNT
                    );
                }
            }
            return Ok(());
        }

        // Check if it's a map with specific values
        if let Some(m) = value.as_map() {
            let idle = m
                .iter()
                .find(|(k, _)| {
                    k.as_symbol()
                        .map(|s| s.as_string() == "idle")
                        .unwrap_or(false)
                })
                .and_then(|(_, v)| v.as_integer())
                .map(|v| v as u64)
                .unwrap_or(DEFAULT_IDLE);

            let interval = m
                .iter()
                .find(|(k, _)| {
                    k.as_symbol()
                        .map(|s| s.as_string() == "interval")
                        .unwrap_or(false)
                })
                .and_then(|(_, v)| v.as_integer())
                .map(|v| v as u64)
                .unwrap_or(DEFAULT_INTERVAL);

            let count = m
                .iter()
                .find(|(k, _)| {
                    k.as_symbol()
                        .map(|s| s.as_string() == "count")
                        .unwrap_or(false)
                })
                .and_then(|(_, v)| v.as_integer())
                .map(|v| v as u32)
                .unwrap_or(DEFAULT_COUNT);

            let keepalive = TcpKeepalive::new()
                .with_time(Duration::from_secs(idle))
                .with_interval(Duration::from_secs(interval))
                .with_retries(count);

            if let Err(e) = sock_ref.set_tcp_keepalive(&keepalive) {
                warn!("Failed to set TCP keepalive: {}", e);
            } else {
                debug!(
                    "TCP keepalive enabled: idle={}s, interval={}s, count={}",
                    idle, interval, count
                );
            }
            return Ok(());
        }

        // Boolean true enables with defaults
        if value.is_true() {
            let keepalive = TcpKeepalive::new()
                .with_time(Duration::from_secs(DEFAULT_IDLE))
                .with_interval(Duration::from_secs(DEFAULT_INTERVAL))
                .with_retries(DEFAULT_COUNT);
            if let Err(e) = sock_ref.set_tcp_keepalive(&keepalive) {
                warn!("Failed to set TCP keepalive: {}", e);
            } else {
                debug!(
                    "TCP keepalive enabled: idle={}s, interval={}s, count={}",
                    DEFAULT_IDLE, DEFAULT_INTERVAL, DEFAULT_COUNT
                );
            }
        } else if let Err(e) = sock_ref.set_tcp_keepalive(&TcpKeepalive::new()) {
            warn!("Failed to disable TCP keepalive: {}", e);
        } else {
            debug!("TCP keepalive disabled");
        }

        Ok(())
    }

    async fn update_connection_attribute(&mut self, key: Symbol, value: Option<Var>) {
        if let Some(auth_token) = &self.auth_token
            && let Some(set_attr_msg) =
                mk_set_client_attribute_msg(&self.client_token, auth_token, &key, value.as_ref())
        {
            let _ = self
                .rpc_client
                .make_client_rpc_call(self.client_id, set_attr_msg)
                .await;
        }
    }
    pub(crate) async fn run(&mut self) -> Result<(), eyre::Error> {
        // Provoke welcome message, which is a login command with no arguments, and we
        // don't care about the reply at this point.
        let login_msg = mk_login_command_msg(
            &self.client_token,
            &self.handler_object,
            vec![],
            false,
            None,
            None,
        );
        self.rpc_client
            .make_client_rpc_call(self.client_id, login_msg)
            .await
            .expect("Unable to send login request to RPC server");

        let (auth_token, player, connect_type) = match self.authorization_phase().await {
            Ok(result) => result,
            Err(e) => bail!("Unable to authorize connection: {}", e),
        };
        debug!("Authorized player: {:?}", player);

        self.auth_token = Some(auth_token);

        let connect_message = match connect_type {
            moor_rpc::ConnectType::Connected => "*** Connected ***",
            moor_rpc::ConnectType::Reconnected => "*** Reconnected ***",
            moor_rpc::ConnectType::Created => "*** Created ***",
            moor_rpc::ConnectType::NoConnect => {
                unreachable!("NoConnect should not reach telnet connection handler")
            }
        };
        self.send_line(connect_message).await?;
        self.flush().await?;

        // Now that we're authenticated, send all current connection attributes to daemon
        for (key, value) in self.connection_attributes.clone() {
            self.update_connection_attribute(key, Some(value)).await;
        }

        if self.command_loop().await.is_err() {
            info!("Connection closed");
        };

        // Let the server know this client is gone.
        let detach_msg = mk_detach_msg(&self.client_token, true);
        self.rpc_client
            .make_client_rpc_call(self.client_id, detach_msg)
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
                // Get terminal width from connection attributes if available
                let width = self
                    .connection_attributes
                    .get(&Symbol::mk("columns"))
                    .and_then(|v| v.as_integer())
                    .and_then(|w| if w > 0 { Some(w as usize) } else { None });

                let Ok(formatted) = output_format(&value, content_type, width) else {
                    warn!("Failed to format message: {:?}", value);
                    return Ok(());
                };
                match formatted {
                    FormattedOutput::Plain(text) => {
                        if no_newline || self.is_binary_mode {
                            self.send_raw_text(&text).await
                        } else {
                            self.send_line(&text).await
                        }
                    }
                    FormattedOutput::Rich(text) => {
                        // Rich content (markdown/djot) already has proper line endings
                        self.send_raw_text(&text).await
                    }
                }
                .with_context(|| "Unable to send message to client")?;
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

    async fn authorization_phase(
        &mut self,
    ) -> Result<(AuthToken, Obj, moor_rpc::ConnectType), eyre::Error> {
        loop {
            select! {
                Ok(event_msg) = broadcast_recv(&mut self.broadcast_sub) => {
                    let event = event_msg.event()?;
                    trace!("broadcast_event");

                    match event.event().map_err(|e| eyre::eyre!("Missing event: {}", e))? {
                        moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_server_time) => {
                            let timestamp = SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos() as u64;
                            let pong_msg = mk_client_pong_msg(
                                &self.client_token,
                                timestamp,
                                &self.connection_object,
                                moor_rpc::HostType::Tcp,
                                self.peer_addr.to_string(),
                            );
                            let _ = &mut self.rpc_client.make_client_rpc_call(self.client_id, pong_msg).await?;
                        }
                    }
                }
                Ok(event_msg) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    let event = event_msg.event()?;
                    match event.event().map_err(|e| eyre::eyre!("Missing event: {}", e))? {
                        moor_rpc::ClientEventUnionRef::SystemMessageEvent(sys_msg) => {
                            let msg = sys_msg.message().map_err(|e| eyre::eyre!("Missing message: {}", e))?.to_string();
                            self.send_line(&msg).await.with_context(|| "Unable to send message to client")?;
                        }
                        moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative) => {
                            let event_ref = narrative.event().map_err(|e| eyre::eyre!("Missing event: {}", e))?;
                            let narrative_event = narrative_event_from_ref(event_ref)
                                .map_err(|e| eyre::eyre!("Failed to convert narrative event: {}", e))?;
                            self.output(narrative_event.event()).await?;
                        }
                        moor_rpc::ClientEventUnionRef::RequestInputEvent(_request_id) => {
                            bail!("RequestInput before login");
                        }
                        moor_rpc::ClientEventUnionRef::DisconnectEvent(_) => {
                            self.write.close().await?;
                            bail!("Disconnect before login");
                        }
                        moor_rpc::ClientEventUnionRef::TaskErrorEvent(task_err) => {
                            let err_ref = task_err.error().map_err(|e| eyre::eyre!("Missing error: {}", e))?;
                            let scheduler_error = rpc_common::scheduler_error_from_ref(err_ref)
                                .map_err(|e| eyre::eyre!("Failed to convert scheduler error: {}", e))?;
                            self.handle_task_error(scheduler_error).await?;
                        }
                        moor_rpc::ClientEventUnionRef::TaskSuccessEvent(_) |
                        moor_rpc::ClientEventUnionRef::TaskSuspendedEvent(_) => {
                            trace!("TaskSuccess")
                            // We don't need to do anything with successes.
                        }
                        moor_rpc::ClientEventUnionRef::PlayerSwitchedEvent(switch) => {
                            let new_player = extract_obj(&switch, "new_player", |s| s.new_player())
                                .map_err(|e| eyre::eyre!("{}", e))?;

                            let new_auth_token_ref = switch.new_auth_token().map_err(|e| eyre::eyre!("Missing new_auth_token: {}", e))?;
                            let new_auth_token = AuthToken(new_auth_token_ref.token().map_err(|e| eyre::eyre!("Missing token: {}", e))?.to_string());

                            info!("Switching player from {:?} to {} during authorization for client {}", self.player_object, new_player, self.client_id);
                            self.player_object = Some(new_player);
                            self.auth_token = Some(new_auth_token);
                            info!("Player switched successfully to {} during authorization for client {}", new_player, self.client_id);
                        }
                        moor_rpc::ClientEventUnionRef::SetConnectionOptionEvent(set_opt) => {
                            let connection_obj = extract_obj(&set_opt, "connection_obj", |s| s.connection_obj())
                                .map_err(|e| eyre::eyre!("{}", e))?;

                            let option_name = extract_symbol(&set_opt, "option_name", |s| s.option_name())
                                .map_err(|e| eyre::eyre!("{}", e))?;

                            let value = extract_var(&set_opt, "value", |s| s.value())
                                .map_err(|e| eyre::eyre!("{}", e))?;

                            // Only handle if this event is for our connection
                            if connection_obj == self.connection_object {
                                self.handle_connection_option(option_name, Some(value)).await?;
                            }
                        }
                    }
                }
                // Auto loop
                item = self.read.next() => {
                    let Some(item) = item else {
                        bail!("Connection closed before login");
                    };
                    let item = match item {
                        Ok(i) => i,
                        Err(e) => {
                            warn!("Failure to decode: {:?}", e);
                            continue;
                        }
                    };
                    let line = match item {
                        ConnectionItem::Line(line) => line,
                        ConnectionItem::Bytes(_) => continue,
                    };
                    let words = parse_into_words(&line);
                    let login_msg = mk_login_command_msg(&self.client_token, &self.handler_object, words, true, None, None);
                    let response_bytes = self.rpc_client.make_client_rpc_call(
                        self.client_id,
                        login_msg,
                    ).await?;

                    let reply_ref = read_reply_result(&response_bytes)
                        .map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

                    if let moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) = reply_ref.result().map_err(|e| eyre::eyre!("Missing result: {}", e))? {
                        let daemon_reply = client_success.reply().map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                        if let moor_rpc::DaemonToClientReplyUnionRef::LoginResult(login_result) = daemon_reply.reply().map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                            && login_result.success().map_err(|e| eyre::eyre!("Missing success: {}", e))? {
                                let auth_token_ref = login_result.auth_token().map_err(|e| eyre::eyre!("Missing auth_token: {}", e))?
                                    .ok_or_else(|| eyre::eyre!("Auth token is None"))?;
                                let auth_token = AuthToken(auth_token_ref.token().map_err(|e| eyre::eyre!("Missing token: {}", e))?.to_string());

                                let connect_type = login_result.connect_type().map_err(|e| eyre::eyre!("Missing connect_type: {}", e))?;

                                let player_opt = login_result.player().map_err(|e| eyre::eyre!("Missing player: {}", e))?
                                    .ok_or_else(|| eyre::eyre!("Player is None"))?;
                                let player = obj_from_ref(player_opt)
                                    .map_err(|e| eyre::eyre!("{}", e))?;

                                info!(?player, client_id = ?self.client_id, "Login successful");
                                self.player_object = Some(player);
                                return Ok((auth_token, player, connect_type))
                        }
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
        let mut expecting_input: VecDeque<(Uuid, InputMetadata)> = VecDeque::new();
        let mut program_input = Vec::new();
        let mut textarea_input = Vec::new();
        loop {
            // We should not send the next line until we've received a narrative event for the
            // previous.
            let input_future = async {
                if let Some(pt) = &self.pending_task
                    && expecting_input.is_empty()
                    && !self.collecting_input
                    && pt.start_time.elapsed() > TASK_TIMEOUT
                {
                    error!(
                        "Task {} stuck without response for more than {TASK_TIMEOUT:?}",
                        pt.task_id
                    );
                    self.pending_task = None;
                } else if self.pending_task.is_some()
                    && expecting_input.is_empty()
                    && !self.collecting_input
                {
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
                            ReadEvent::PendingEvent
                        } else if !expecting_input.is_empty() {
                            // Convert binary data to Var::Binary for input reply
                            ReadEvent::InputReply(Var::mk_binary(bytes.to_vec()))
                        } else {
                            // Binary data as unprompted command not yet supported
                            ReadEvent::PendingEvent
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
                                line_mode = self.handle_command(&mut program_input, &mut textarea_input, &mut expecting_input, line_mode, line).await.expect("Unable to process command");
                                // Update collecting_input flag after command processing
                                self.collecting_input = !expecting_input.is_empty() || matches!(line_mode, LineMode::CollectingTextArea(_));
                            }
                        }
                        ReadEvent::InputReply(input_data) =>{
                            self.process_requested_input_line(input_data, &mut expecting_input).await.expect("Unable to process input reply");
                            // Update collecting_input flag after processing input
                            self.collecting_input = !expecting_input.is_empty() || matches!(line_mode, LineMode::CollectingTextArea(_));
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
                Ok(event_msg) = broadcast_recv(&mut self.broadcast_sub) => {
                    let event = event_msg.event()?;
                    match event.event().map_err(|e| eyre::eyre!("Missing event: {}", e))? {
                        moor_rpc::ClientsBroadcastEventUnionRef::ClientsBroadcastPingPong(_server_time) => {
                            let timestamp = SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos() as u64;
                            let pong_msg = mk_client_pong_msg(
                                &self.client_token,
                                timestamp,
                                &self.handler_object,
                                moor_rpc::HostType::WebSocket,
                                self.peer_addr.to_string(),
                            );
                            let _ = self.rpc_client.make_client_rpc_call(self.client_id, pong_msg).await.expect("Unable to send pong to RPC server");

                        }
                    }
                }
                Ok(event_msg) = events_recv(self.client_id, &mut self.narrative_sub) => {
                    let event = event_msg.event()?;
                    let event_union = event.event().map_err(|e| eyre::eyre!("Missing event: {}", e))?;
                    if let Some(input_request) = self.handle_narrative_event(event_union).await? {
                        expecting_input.push_back(input_request);
                    }
                    // Check if we need to switch line mode (for text_area)
                    if let Some(new_mode) = self.pending_line_mode.take() {
                        line_mode = new_mode;
                    }
                    // Update collecting_input flag based on state
                    self.collecting_input = !expecting_input.is_empty() || matches!(line_mode, LineMode::CollectingTextArea(_));
                }
            }
        }
    }

    async fn handle_command(
        &mut self,
        program_input: &mut Vec<String>,
        textarea_input: &mut Vec<String>,
        expecting_input: &mut VecDeque<(Uuid, InputMetadata)>,
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
                let program_msg =
                    mk_program_msg(&self.client_token, &auth_token, &target, &verb, code);
                let response_bytes = self
                    .rpc_client
                    .make_client_rpc_call(self.client_id, program_msg)
                    .await?;

                let reply_ref = read_reply_result(&response_bytes)
                    .map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

                match reply_ref
                    .result()
                    .map_err(|e| eyre::eyre!("Missing result: {}", e))?
                {
                    moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                        let daemon_reply = client_success
                            .reply()
                            .map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                        match daemon_reply
                            .reply()
                            .map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                        {
                            moor_rpc::DaemonToClientReplyUnionRef::VerbProgramResponseReply(
                                prog_resp_reply,
                            ) => {
                                let prog_resp = prog_resp_reply
                                    .response()
                                    .map_err(|e| eyre::eyre!("Missing response: {}", e))?;
                                match prog_resp
                                    .response()
                                    .map_err(|e| eyre::eyre!("Missing response union: {}", e))?
                                {
                                    moor_rpc::VerbProgramResponseUnionRef::VerbProgramSuccess(
                                        success,
                                    ) => {
                                        let o = extract_obj(&success, "obj", |s| s.obj())
                                            .map_err(|e| eyre::eyre!("{}", e))?;

                                        let verb = success
                                            .verb_name()
                                            .map_err(|e| eyre::eyre!("Missing verb_name: {}", e))?
                                            .to_string();

                                        self.send_line(&format!(
                                            "0 error(s).\nVerb {verb} programmed on object {o}"
                                        ))
                                        .await?;
                                    }
                                    moor_rpc::VerbProgramResponseUnionRef::VerbProgramFailure(
                                        failure,
                                    ) => {
                                        let error_ref = failure
                                            .error()
                                            .map_err(|e| eyre::eyre!("Missing error: {}", e))?;
                                        match error_ref.error().map_err(|e| eyre::eyre!("Missing error union: {}", e))? {
                                            moor_rpc::VerbProgramErrorUnionRef::VerbCompilationError(comp_err) => {
                                                let compile_error = compilation_error_from_ref(
                                                    comp_err.error().map_err(|e| eyre::eyre!("Missing error: {}", e))?
                                                ).map_err(|e| eyre::eyre!("Failed to convert compilation error: {}", e))?;
                                                let error_str = describe_compile_error(compile_error);
                                                self.send_line(&format!("Compilation error: {error_str}")).await?;
                                            }
                                            moor_rpc::VerbProgramErrorUnionRef::NoVerbToProgram(_) => {
                                                self.send_line("That object does not have that verb.")
                                                    .await?;
                                            }
                                            _ => {
                                                error!("Unhandled verb program error");
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                bail!("Unexpected RPC success");
                            }
                        }
                    }
                    moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                        let (error_ref, error_code) = failure_error_context(failure)?;
                        match error_code {
                            moor_rpc::RpcMessageErrorCode::TaskError => {
                                let scheduler_error = scheduler_error_from_rpc_error(error_ref)
                                    .map_err(|e| eyre::eyre!("{e}"))?;
                                self.handle_task_error(scheduler_error).await?;
                            }
                            _ => {
                                error!("Unhandled RPC error code in .program: {:?}", error_code);
                                self.send_line("An error occurred.").await?;
                            }
                        }
                    }
                    _ => {
                        bail!("Unexpected response type");
                    }
                }
                return Ok(LineMode::Input);
            } else {
                // Otherwise, we're still spooling up the program, so just keep spooling.
                program_input.push(line);
            }
            return Ok(line_mode);
        }

        // Handle text_area collection mode
        if let LineMode::CollectingTextArea(request_id) = &line_mode {
            if line == "." || line.trim() == "@abort" {
                // Done collecting, send the input
                let Some(auth_token) = self.auth_token.clone() else {
                    bail!("Received input before auth token was set");
                };

                // If @abort, send just "@abort", otherwise join collected lines
                let input_var = if line.trim() == "@abort" {
                    v_str("@abort")
                } else {
                    let text = std::mem::take(textarea_input).join("\n");
                    v_str(&text)
                };

                if let Some(input_msg) =
                    mk_requested_input_msg(&self.client_token, &auth_token, *request_id, &input_var)
                {
                    let result = self
                        .rpc_client
                        .make_client_rpc_call(self.client_id, input_msg)
                        .await?;

                    // Process the response
                    let reply_ref = read_reply_result(&result)
                        .map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

                    match reply_ref
                        .result()
                        .map_err(|e| eyre::eyre!("Missing result: {}", e))?
                    {
                        moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                            let daemon_reply = client_success
                                .reply()
                                .map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                            match daemon_reply
                                .reply()
                                .map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                            {
                                moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(
                                    task_submitted,
                                ) => {
                                    let task_id = task_submitted
                                        .task_id()
                                        .map_err(|e| eyre::eyre!("Missing task_id: {}", e))?
                                        as usize;

                                    self.pending_task = Some(PendingTask {
                                        task_id,
                                        start_time: Instant::now(),
                                    });
                                }
                                moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_) => {
                                    // Input was accepted
                                }
                                _ => {
                                    bail!("Unexpected RPC success for text_area input");
                                }
                            }
                        }
                        moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                            let (error_ref, error_code) = failure_error_context(failure)?;
                            match error_code {
                                moor_rpc::RpcMessageErrorCode::TaskError => {
                                    let e = scheduler_error_from_rpc_error(error_ref)
                                        .map_err(|e| eyre::eyre!("{e}"))?;
                                    self.handle_task_error(e).await?;
                                }
                                _ => {
                                    error!(
                                        "Unhandled RPC error code for text_area input: {:?}",
                                        error_code
                                    );
                                    self.send_line("An error occurred processing your input.")
                                        .await?;
                                }
                            }
                        }
                        _ => {
                            bail!("Unexpected response type for text_area input");
                        }
                    }
                }

                // Remove from expecting_input queue
                expecting_input.retain(|(id, _)| id != request_id);

                return Ok(LineMode::Input);
            } else {
                // Keep collecting lines
                textarea_input.push(line);
                return Ok(line_mode);
            }
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
            if !verb
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
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
                    let key = Symbol::mk("line-output-prefix");
                    if let Some(set_attr_msg) = mk_set_client_attribute_msg(
                        &self.client_token,
                        auth_token,
                        &key,
                        prefix_value.as_ref(),
                    ) {
                        let _ = self
                            .rpc_client
                            .make_client_rpc_call(self.client_id, set_attr_msg)
                            .await;
                    }
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
                    let key = Symbol::mk("line-output-suffix");
                    if let Some(set_attr_msg) = mk_set_client_attribute_msg(
                        &self.client_token,
                        auth_token,
                        &key,
                        suffix_value.as_ref(),
                    ) {
                        let _ = self
                            .rpc_client
                            .make_client_rpc_call(self.client_id, set_attr_msg)
                            .await;
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn handle_narrative_event(
        &mut self,
        event_ref: moor_rpc::ClientEventUnionRef<'_>,
    ) -> Result<Option<(Uuid, InputMetadata)>, eyre::Error> {
        match event_ref {
            moor_rpc::ClientEventUnionRef::SystemMessageEvent(sys_msg) => {
                let msg = sys_msg
                    .message()
                    .map_err(|e| {
                        error!("Failed to get message from SystemMessageEvent: {}", e);
                        eyre::eyre!("Missing message: {}", e)
                    })?
                    .to_string();
                self.send_line(&msg)
                    .await
                    .expect("Unable to send message to client");
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::NarrativeEventMessage(narrative) => {
                let event_ref = narrative.event().map_err(|e| {
                    error!("Failed to get event from NarrativeEventMessage: {}", e);
                    eyre::eyre!("Missing event: {}", e)
                })?;
                let narrative_event = narrative_event_from_ref(event_ref).map_err(|e| {
                    error!("Failed to convert narrative event: {}", e);
                    eyre::eyre!("Failed to convert narrative event: {}", e)
                })?;
                let msg = narrative_event.event();
                match &msg {
                    Event::Notify {
                        value: msg,
                        content_type,
                        ..
                    } => {
                        // Get terminal width from connection attributes if available
                        let width = self
                            .connection_attributes
                            .get(&Symbol::mk("columns"))
                            .and_then(|v| v.as_integer())
                            .and_then(|w| if w > 0 { Some(w as usize) } else { None });

                        let formatted = output_format(msg, *content_type, width)?;
                        match formatted {
                            FormattedOutput::Plain(text) => self.send_line(&text).await,
                            FormattedOutput::Rich(text) => {
                                // Rich content (markdown/djot) already has proper line endings
                                self.send_raw_text(&text).await
                            }
                        }
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
                    Event::Present(_) => {
                        // Present events are for web UI elements (editors, etc.)
                        // Telnet clients don't support these, so just ignore
                        trace!("Ignoring Present event in telnet client");
                    }
                    _ => {
                        // We don't handle these events in the telnet client.
                        warn!("Unhandled event in telnet client: {:?}", msg);
                    }
                }
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::RequestInputEvent(request_input) => {
                let request_id_ref = request_input
                    .request_id()
                    .map_err(|e| eyre::eyre!("Missing request_id: {}", e))?;
                let request_id_data = request_id_ref
                    .data()
                    .map_err(|e| eyre::eyre!("Missing request_id data: {}", e))?;
                let request_id = Uuid::from_slice(request_id_data)
                    .map_err(|e| eyre::eyre!("Invalid request UUID: {}", e))?;

                // Parse the input metadata
                let metadata_result = request_input.metadata().ok().and_then(|opt| opt);
                let metadata = InputMetadata::from_metadata_pairs(metadata_result);

                // If hold_input is active and has buffered input, return it immediately
                if let Some(ref mut buffer) = self.hold_input
                    && let Some(input_line) = buffer.drain(..1).next()
                {
                    // Send the buffered input as an input reply
                    let Some(auth_token) = self.auth_token.clone() else {
                        bail!("Received input request before auth token was set");
                    };

                    let input_var = v_str(&input_line);
                    if let Some(input_msg) = mk_requested_input_msg(
                        &self.client_token,
                        &auth_token,
                        request_id,
                        &input_var,
                    ) {
                        self.rpc_client
                            .make_client_rpc_call(self.client_id, input_msg)
                            .await?;
                    }

                    return Ok(None);
                }

                // Display the prompt based on input type
                metadata.display_prompt(self).await?;

                // For text_area, switch to collection mode instead of adding to expecting_input
                if metadata.input_type.as_deref() == Some("text_area") {
                    self.pending_line_mode = Some(LineMode::CollectingTextArea(request_id));
                    Ok(None)
                } else {
                    // Store the request with metadata for later processing
                    Ok(Some((request_id, metadata)))
                }
            }
            moor_rpc::ClientEventUnionRef::DisconnectEvent(_) => {
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
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::TaskErrorEvent(task_err) => {
                let ti = task_err.task_id().map_err(|e| {
                    error!("Failed to get task_id from TaskErrorEvent: {}", e);
                    eyre::eyre!("Missing task_id: {}", e)
                })? as usize;

                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }

                let err_ref = task_err.error().map_err(|e| {
                    error!("Failed to get error from TaskErrorEvent: {}", e);
                    eyre::eyre!("Missing error: {}", e)
                })?;
                let te = rpc_common::scheduler_error_from_ref(err_ref).map_err(|e| {
                    error!("Failed to convert scheduler error: {}", e);
                    eyre::eyre!("Failed to convert scheduler error: {}", e)
                })?;

                self.handle_task_error(te)
                    .await
                    .expect("Unable to handle task error");
                // Send suffix after task error
                self.send_output_suffix()
                    .await
                    .expect("Unable to send output suffix");
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::TaskSuccessEvent(task_success) => {
                let ti = task_success
                    .task_id()
                    .map_err(|e| eyre::eyre!("Missing task_id: {}", e))?
                    as usize;

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
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::TaskSuspendedEvent(task_suspended) => {
                let ti = task_suspended
                    .task_id()
                    .map_err(|e| eyre::eyre!("Missing task_id: {}", e))?
                    as usize;

                if let Some(pending_event) = self.pending_task.take()
                    && pending_event.task_id != ti
                {
                    error!(
                        "Inbound task response {ti} does not belong to the event we submitted and are expecting {pending_event:?}"
                    );
                }
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::PlayerSwitchedEvent(switch) => {
                let new_player = extract_obj(&switch, "new_player", |s| s.new_player())
                    .map_err(|e| eyre::eyre!("{}", e))?;

                let new_auth_token_ref = switch
                    .new_auth_token()
                    .map_err(|e| eyre::eyre!("Missing new_auth_token: {}", e))?;
                let new_auth_token = AuthToken(
                    new_auth_token_ref
                        .token()
                        .map_err(|e| eyre::eyre!("Missing token: {}", e))?
                        .to_string(),
                );

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
                Ok(None)
            }
            moor_rpc::ClientEventUnionRef::SetConnectionOptionEvent(set_opt) => {
                let connection_obj =
                    extract_obj(&set_opt, "connection_obj", |s| s.connection_obj())
                        .map_err(|e| eyre::eyre!("{}", e))?;

                let option_name = extract_symbol(&set_opt, "option_name", |s| s.option_name())
                    .map_err(|e| eyre::eyre!("{}", e))?;

                let value = extract_var(&set_opt, "value", |s| s.value())
                    .map_err(|e| eyre::eyre!("{}", e))?;
                // Only handle if this event is for our connection
                if connection_obj == self.connection_object {
                    self.handle_connection_option(option_name, Some(value))
                        .await?;
                }
                Ok(None)
            }
        }
    }

    async fn process_command_line(&mut self, line: String) -> Result<(), eyre::Error> {
        let Some(auth_token) = self.auth_token.clone() else {
            bail!("Received command before auth token was set");
        };

        // Send output prefix before executing command
        self.send_output_prefix().await?;

        let result = if line.starts_with(OUT_OF_BAND_PREFIX) && !self.disable_oob {
            let oob_msg = mk_out_of_band_msg(
                &self.client_token,
                &auth_token,
                &self.handler_object,
                line.clone(),
            );
            self.rpc_client
                .make_client_rpc_call(self.client_id, oob_msg)
                .await?
        } else {
            let line = line.trim().to_string();

            // Silently ignore empty commands, like LambdaMOO does at parse_command level
            if line.is_empty() {
                return Ok(());
            }

            let command_msg = mk_command_msg(
                &self.client_token,
                &auth_token,
                &self.handler_object,
                line.clone(),
            );
            self.rpc_client
                .make_client_rpc_call(self.client_id, command_msg)
                .await?
        };

        let reply_ref =
            read_reply_result(&result).map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

        match reply_ref
            .result()
            .map_err(|e| eyre::eyre!("Missing result: {}", e))?
        {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        let ti = task_submitted
                            .task_id()
                            .map_err(|e| eyre::eyre!("Missing task_id: {}", e))?
                            as usize;

                        self.pending_task = Some(PendingTask {
                            task_id: ti,
                            start_time: Instant::now(),
                        });
                    }
                    moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_) => {
                        bail!("Received input thanks unprovoked, out of order")
                    }
                    _ => {
                        bail!("Unexpected RPC success");
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let (error_ref, error_code) = failure_error_context(failure)?;
                match error_code {
                    moor_rpc::RpcMessageErrorCode::TaskError => {
                        let e = scheduler_error_from_rpc_error(error_ref)
                            .map_err(|e| eyre::eyre!("{e}"))?;

                        self.handle_task_error(e)
                            .await
                            .with_context(|| "Unable to handle task error")?;
                        // Send suffix after task error
                        self.send_output_suffix().await?;
                    }
                    moor_rpc::RpcMessageErrorCode::PermissionDenied => {
                        self.send_line("Permission denied.").await?;
                        self.send_output_suffix().await?;
                    }
                    moor_rpc::RpcMessageErrorCode::InvalidRequest => {
                        self.send_line("Invalid request.").await?;
                        self.send_output_suffix().await?;
                    }
                    moor_rpc::RpcMessageErrorCode::InternalError => {
                        self.send_line("Internal server error.").await?;
                        self.send_output_suffix().await?;
                    }
                    _ => {
                        error!("Unhandled RPC error code: {:?}", error_code);
                        self.send_line("An error occurred processing your request.")
                            .await?;
                        self.send_output_suffix().await?;
                    }
                }
            }
            _ => {
                bail!("Unexpected response type");
            }
        }
        Ok(())
    }

    async fn process_requested_input_line(
        &mut self,
        input_data: Var,
        expecting_input: &mut VecDeque<(Uuid, InputMetadata)>,
    ) -> Result<(), eyre::Error> {
        let Some((input_request_id, metadata)) = expecting_input.front() else {
            bail!("Attempt to send reply to input request without an input request");
        };

        // Validate the input based on metadata
        let Some(input_str) = input_data.as_string() else {
            // Binary input, pass through as-is
            return self
                .send_validated_input(*input_request_id, input_data, expecting_input)
                .await;
        };

        // Special case: @abort always passes through without validation
        if input_str.trim() == "@abort" {
            return self
                .send_validated_input(*input_request_id, v_str("@abort"), expecting_input)
                .await;
        }

        match metadata.validate_input(input_str) {
            Ok(validated_input) => {
                self.send_validated_input(*input_request_id, validated_input, expecting_input)
                    .await
            }
            Err(err_msg) => {
                // Validation failed, show error and keep the request in queue
                self.send_line(&err_msg).await?;
                self.send_line("Please try again:").await?;
                self.flush().await?;
                Ok(())
            }
        }
    }

    async fn send_validated_input(
        &mut self,
        input_request_id: Uuid,
        input_data: Var,
        expecting_input: &mut VecDeque<(Uuid, InputMetadata)>,
    ) -> Result<(), eyre::Error> {
        let Some(auth_token) = self.auth_token.clone() else {
            bail!("Received input reply before auth token was set");
        };

        let Some(input_msg) = mk_requested_input_msg(
            &self.client_token,
            &auth_token,
            input_request_id,
            &input_data,
        ) else {
            bail!("Failed to serialize input var");
        };
        let result = self
            .rpc_client
            .make_client_rpc_call(self.client_id, input_msg)
            .await
            .expect("Unable to send input to RPC server");

        let reply_ref =
            read_reply_result(&result).map_err(|e| eyre::eyre!("Invalid flatbuffer: {}", e))?;

        match reply_ref
            .result()
            .map_err(|e| eyre::eyre!("Missing result: {}", e))?
        {
            moor_rpc::ReplyResultUnionRef::ClientSuccess(client_success) => {
                let daemon_reply = client_success
                    .reply()
                    .map_err(|e| eyre::eyre!("Missing reply: {}", e))?;
                match daemon_reply
                    .reply()
                    .map_err(|e| eyre::eyre!("Missing reply union: {}", e))?
                {
                    moor_rpc::DaemonToClientReplyUnionRef::TaskSubmitted(task_submitted) => {
                        let task_id = task_submitted
                            .task_id()
                            .map_err(|e| eyre::eyre!("Missing task_id: {}", e))?
                            as usize;

                        self.pending_task = Some(PendingTask {
                            task_id,
                            start_time: Instant::now(),
                        });
                        bail!("Got TaskSubmitted when expecting input-thanks")
                    }
                    moor_rpc::DaemonToClientReplyUnionRef::InputThanks(_) => {
                        expecting_input.pop_front();
                    }
                    _ => {
                        bail!("Unexpected RPC success");
                    }
                }
            }
            moor_rpc::ReplyResultUnionRef::Failure(failure) => {
                let (error_ref, error_code) = failure_error_context(failure)?;
                match error_code {
                    moor_rpc::RpcMessageErrorCode::TaskError => {
                        let e = scheduler_error_from_rpc_error(error_ref)
                            .map_err(|e| eyre::eyre!("{e}"))?;
                        self.handle_task_error(e).await?;
                    }
                    _ => {
                        error!(
                            "Unhandled RPC error code in input processing: {:?}",
                            error_code
                        );
                        self.send_line("An error occurred processing your input.")
                            .await?;
                    }
                }
            }
            _ => {
                bail!("Unexpected response type");
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

/// Produce the right kind of "telnet" compatible output for the given content.
fn output_format(
    content: &Var,
    content_type: Option<Symbol>,
    width: Option<usize>,
) -> Result<FormattedOutput, eyre::Error> {
    match content.variant() {
        Variant::Str(s) => output_str_format(s.as_str(), content_type, width),
        Variant::Sym(s) => output_str_format(&s.as_arc_str(), content_type, width),
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
            output_str_format(&output, content_type, width)
        }
        _ => bail!("Unsupported content type: {:?}", content.variant()),
    }
}

fn output_str_format(
    content: &str,
    content_type: Option<Symbol>,
    _width: Option<usize>,
) -> Result<FormattedOutput, eyre::Error> {
    let Some(content_type) = content_type else {
        debug!("output_str_format: no content_type, using plain");
        return Ok(FormattedOutput::Plain(content.to_string()));
    };
    let content_type_str = content_type.as_arc_str();
    debug!("output_str_format: content_type={}", content_type_str);
    Ok(match content_type_str.as_str() {
        CONTENT_TYPE_MARKDOWN | CONTENT_TYPE_MARKDOWN_SLASH => {
            // Use djot formatter for markdown too - djot handles most markdown syntax
            FormattedOutput::Rich(djot_to_ansi(content))
        }
        CONTENT_TYPE_DJOT | CONTENT_TYPE_DJOT_SLASH => FormattedOutput::Rich(djot_to_ansi(content)),
        // text/plain, None, or unknown
        _ => FormattedOutput::Plain(content.to_string()),
    })
}
