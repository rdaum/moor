//! Shared functionality between telnet and TCP connections

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eyre::bail;
use moor_common::model::CompileError;
use moor_common::tasks::{AbortLimitReason, CommandError, SchedulerError, VerbProgramError};
use moor_common::util::parse_into_words;
use moor_var::{Obj, Symbol, Var, Variant, v_str};
use rpc_async_client::rpc_client::RpcSendClient;
use rpc_common::{AuthToken, ClientToken, ReplyResult, RpcMessageError};
use rpc_common::{DaemonToClientReply, HostClientToDaemonMessage};
use termimad::MadSkin;
use uuid::Uuid;

pub const CONTENT_TYPE_MARKDOWN: &str = "text_markdown";
pub const CONTENT_TYPE_DJOT: &str = "text_djot";

#[derive(Debug, PartialEq, Eq)]
pub struct PendingTask {
    pub task_id: usize,
    pub start_time: Instant,
}

pub const TASK_TIMEOUT: Duration = Duration::from_secs(10);

/// The input modes the session can be in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineMode {
    /// Receiving input
    Input,
    /// Spooling up .program input.
    SpoolingProgram(String, String),
}

pub fn describe_compile_error(compile_error: CompileError) -> String {
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

/// Process a requested input line
pub async fn process_requested_input_line(
    line: String,
    expecting_input: &mut VecDeque<Uuid>,
    client_id: Uuid,
    client_token: &ClientToken,
    auth_token: &AuthToken,
    rpc_client: &mut RpcSendClient,
) -> Result<Option<PendingTask>, eyre::Error> {
    let cmd = line.trim().to_string();

    let Some(input_request_id) = expecting_input.front() else {
        bail!("Attempt to send reply to input request without an input request");
    };

    match rpc_client
        .make_client_rpc_call(
            client_id,
            HostClientToDaemonMessage::RequestedInput(
                client_token.clone(),
                auth_token.clone(),
                *input_request_id,
                cmd,
            ),
        )
        .await
        .expect("Unable to send input to RPC server")
    {
        ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(_task_id)) => {
            bail!("Got TaskSubmitted when expecting input-thanks")
        }
        ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
            expecting_input.pop_front();
            Ok(None)
        }
        ReplyResult::Failure(RpcMessageError::TaskError(e)) => {
            bail!("Task error in input processing: {:?}", e);
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
}

/// Process a command line
pub async fn process_command_line(
    line: String,
    client_id: Uuid,
    client_token: &ClientToken,
    auth_token: &AuthToken,
    handler_object: Obj,
    rpc_client: &mut RpcSendClient,
) -> Result<Option<PendingTask>, eyre::Error> {
    // Out of band messages are prefixed with this string
    const OUT_OF_BAND_PREFIX: &str = "#$#";

    let result = if line.starts_with(OUT_OF_BAND_PREFIX) {
        rpc_client
            .make_client_rpc_call(
                client_id,
                HostClientToDaemonMessage::OutOfBand(
                    client_token.clone(),
                    auth_token.clone(),
                    handler_object,
                    line,
                ),
            )
            .await?
    } else {
        let line = line.trim().to_string();
        rpc_client
            .make_client_rpc_call(
                client_id,
                HostClientToDaemonMessage::Command(
                    client_token.clone(),
                    auth_token.clone(),
                    handler_object,
                    line,
                ),
            )
            .await?
    };

    match result {
        ReplyResult::ClientSuccess(DaemonToClientReply::TaskSubmitted(ti)) => {
            Ok(Some(PendingTask {
                task_id: ti,
                start_time: Instant::now(),
            }))
        }
        ReplyResult::ClientSuccess(DaemonToClientReply::InputThanks) => {
            bail!("Received input thanks unprovoked, out of order")
        }
        ReplyResult::Failure(RpcMessageError::TaskError(e)) => {
            bail!("Task error: {:?}", e);
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
}

/// Handle built-in commands like PREFIX and SUFFIX
pub async fn handle_builtin_command(
    line: &str,
    output_prefix: &mut Option<String>,
    output_suffix: &mut Option<String>,
    client_id: Uuid,
    client_token: &ClientToken,
    auth_token: &Option<AuthToken>,
    rpc_client: &mut RpcSendClient,
) -> Result<bool, eyre::Error> {
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
                *output_prefix = None;
            } else {
                // Set prefix to everything after the command
                let prefix = line[words[0].len()..].trim_start();
                *output_prefix = if prefix.is_empty() {
                    None
                } else {
                    Some(prefix.to_string())
                };
            }

            // Notify daemon of prefix change
            if let Some(auth_token) = auth_token {
                let prefix_value = output_prefix.as_ref().map(|s| v_str(s));
                let _ = rpc_client
                    .make_client_rpc_call(
                        client_id,
                        HostClientToDaemonMessage::SetClientAttribute(
                            client_token.clone(),
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
                *output_suffix = None;
            } else {
                // Set suffix to everything after the command
                let suffix = line[words[0].len()..].trim_start();
                *output_suffix = if suffix.is_empty() {
                    None
                } else {
                    Some(suffix.to_string())
                };
            }

            // Notify daemon of suffix change
            if let Some(auth_token) = auth_token {
                let suffix_value = output_suffix.as_ref().map(|s| v_str(s));
                let _ = rpc_client
                    .make_client_rpc_call(
                        client_id,
                        HostClientToDaemonMessage::SetClientAttribute(
                            client_token.clone(),
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

/// Handle task errors
pub fn format_task_error(task_error: SchedulerError) -> String {
    match task_error {
        SchedulerError::CommandExecutionError(CommandError::CouldNotParseCommand) => {
            "I couldn't understand that.".to_string()
        }
        SchedulerError::CommandExecutionError(CommandError::NoObjectMatch) => {
            "I don't see that here.".to_string()
        }
        SchedulerError::CommandExecutionError(CommandError::NoCommandMatch) => {
            "I couldn't understand that.".to_string()
        }
        SchedulerError::CommandExecutionError(CommandError::PermissionDenied) => {
            "You can't do that.".to_string()
        }
        SchedulerError::VerbProgramFailed(VerbProgramError::CompilationError(compile_error)) => {
            let ce = describe_compile_error(compile_error);
            format!("{ce}\nVerb not programmed.")
        }
        SchedulerError::VerbProgramFailed(VerbProgramError::NoVerbToProgram) => {
            "That object does not have that verb definition.".to_string()
        }
        SchedulerError::TaskAbortedLimit(AbortLimitReason::Ticks(_)) => {
            "Task ran out of ticks".to_string()
        }
        SchedulerError::TaskAbortedLimit(AbortLimitReason::Time(_)) => {
            "Task ran out of seconds".to_string()
        }
        SchedulerError::TaskAbortedError => "Task aborted".to_string(),
        SchedulerError::TaskAbortedException(e) => {
            format!("Task exception: {e}")
        }
        SchedulerError::TaskAbortedCancelled => "Task cancelled".to_string(),
        _ => format!("Unhandled task error: {task_error:?}"),
    }
}

/// Format output content for display
pub fn output_format(content: &Var, content_type: Option<Symbol>) -> Result<String, eyre::Error> {
    match content.variant() {
        Variant::Str(s) => output_str_format(s.as_str(), content_type),
        Variant::Sym(s) => output_str_format(&s.as_arc_string(), content_type),
        Variant::List(l) => {
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
        CONTENT_TYPE_DJOT => markdown_to_ansi(content), // Treat as markdown for now
        _ => content.to_string(),
    })
}

fn markdown_to_ansi(markdown: &str) -> String {
    let skin = MadSkin::default_dark();
    skin.text(markdown, None).to_string()
}
