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

//! Emergency administration tool for mooR databases.
//! Provides direct database access when normal logins are unavailable.

use clap::Parser;
use clap_derive::Parser as DeriveParser;
use eyre::{Report, bail, eyre};
use fs2::FileExt;
use moor_common::model::ObjectKind;
use moor_common::{
    build,
    model::{Named, ObjFlag, ValSet},
    tasks::{
        ConnectionDetails, Event, NarrativeEvent, NoopSystemControl, SchedulerError, Session,
        SessionError, SessionFactory,
    },
};
use moor_compiler::to_literal;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_var::{Obj, SYSTEM_OBJECT, Sequence, Symbol, Var};
use rustyline::ExternalPrinter;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::time::Duration;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use termimad::{MadSkin, crossterm::style::Color};
use tracing::{error, info, warn};
use tracing_subscriber::fmt::MakeWriter;
use uuid::Uuid;

/// Writer that outputs through rustyline's ExternalPrinter
struct ExternalPrinterWriter {
    printer: Arc<Mutex<dyn ExternalPrinter + Send>>,
}

impl Write for ExternalPrinterWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let Ok(s) = std::str::from_utf8(buf) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid UTF-8",
            ));
        };

        let mut printer = self.printer.lock().unwrap();
        printer
            .print(s.to_string())
            .map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// MakeWriter implementation that creates ExternalPrinterWriter instances
struct ExternalPrinterMakeWriter {
    printer: Arc<Mutex<dyn ExternalPrinter + Send>>,
}

impl ExternalPrinterMakeWriter {
    fn new(printer: impl ExternalPrinter + Send + 'static) -> Self {
        Self {
            printer: Arc::new(Mutex::new(printer)),
        }
    }
}

impl<'a> MakeWriter<'a> for ExternalPrinterMakeWriter {
    type Writer = ExternalPrinterWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ExternalPrinterWriter {
            printer: self.printer.clone(),
        }
    }
}

#[derive(DeriveParser, Debug)]
#[command(name = "moor-emh")]
#[command(
    about = "Emergency Medical Hologram for mooR databases",
    long_about = None
)]
struct Args {
    #[arg(
        value_name = "data-dir",
        help = "Directory containing the database files",
        default_value = "./moor-data"
    )]
    data_dir: PathBuf,

    #[command(flatten)]
    db_args: DatabaseArgs,

    #[arg(
        long,
        value_name = "wizard",
        help = "Object ID to use as wizard for commands (defaults to first valid wizard)"
    )]
    wizard: Option<i32>,

    #[arg(long, help = "Enable debug logging")]
    debug: bool,
}

#[derive(DeriveParser, Debug)]
struct DatabaseArgs {
    #[arg(
        long,
        value_name = "db",
        help = "Main database filename (relative to data-dir if not absolute)",
        default_value = "world.db"
    )]
    db: PathBuf,
}

impl Args {
    fn resolved_db_path(&self) -> PathBuf {
        if self.db_args.db.is_absolute() {
            self.db_args.db.clone()
        } else {
            self.data_dir.join(&self.db_args.db)
        }
    }
}

/// Acquire an exclusive lock on the data directory
fn acquire_data_directory_lock(data_dir: &PathBuf) -> Result<File, Report> {
    std::fs::create_dir_all(data_dir)?;

    let lock_file_path = data_dir.join(".moor-emh.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_file_path)?;

    let Err(e) = lock_file.try_lock_exclusive() else {
        info!("Acquired exclusive lock on data directory: {:?}", data_dir);
        return Ok(lock_file);
    };

    error!(
        "Failed to acquire lock on data directory {:?}. Another moor process may be running.",
        data_dir
    );
    bail!("Directory lock acquisition failed: {}", e);
}

/// Find a wizard object to use for admin commands
fn find_wizard(database: &dyn Database, requested_wizard: Option<i32>) -> Result<Obj, Report> {
    let tx = database.new_world_state()?;

    if let Some(wiz_id) = requested_wizard {
        let wizard = Obj::mk_id(wiz_id);
        if tx.valid(&wizard)? && tx.flags_of(&wizard)?.contains(ObjFlag::Wizard) {
            return Ok(wizard);
        }
        warn!("Requested wizard #{} is not valid or not a wizard", wiz_id);
    }

    // Find first wizard by scanning all objects
    let all_objects = tx.all_objects()?;
    info!("Scanning {} objects for wizard", all_objects.len());

    for obj in all_objects.iter() {
        if tx.flags_of(&obj)?.contains(ObjFlag::Wizard) {
            info!("Using wizard object: {}", obj.to_literal());
            return Ok(obj);
        }
    }

    bail!("No wizard objects found in database");
}

/// A session that outputs to the console (stdout).
/// Buffers narrative events and prints them on commit.
struct ConsoleSession {
    buffer: Mutex<Vec<NarrativeEvent>>,
}

impl ConsoleSession {
    fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
        }
    }
}

impl Session for ConsoleSession {
    fn commit(&self) -> Result<(), SessionError> {
        let events = {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.drain(..).collect::<Vec<_>>()
        };

        for event in events {
            match &event.event {
                Event::Notify {
                    value, no_newline, ..
                } => {
                    let output = to_literal(value);
                    if *no_newline {
                        print!("{output}");
                    } else {
                        println!("{output}");
                    }
                }
                Event::Traceback(exception) => {
                    eprintln!("** Error: {} **", exception.error);
                    for line in &exception.stack {
                        eprintln!("  {}", to_literal(line));
                    }
                }
                Event::Present(_) | Event::Unpresent(_) => {
                    // Ignore presentation events in console mode
                }
            }
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), SessionError> {
        self.buffer.lock().unwrap().clear();
        Ok(())
    }

    fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(ConsoleSession::new()))
    }

    fn request_input(
        &self,
        player: Obj,
        _input_request_id: Uuid,
        _metadata: Option<Vec<(Symbol, Var)>>,
    ) -> Result<(), SessionError> {
        panic!("ConsoleSession::request_input called for player {player}")
    }

    fn send_event(&self, _player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        self.buffer.lock().unwrap().push(*event);
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, msg: &str) -> Result<(), SessionError> {
        println!("** {msg} **");
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        if let Some(msg) = msg {
            println!("** Server shutting down: {msg} **");
        } else {
            println!("** Server shutting down **");
        }
        Ok(())
    }

    fn connection_name(&self, player: Obj) -> Result<String, SessionError> {
        Ok(format!("console:{player}"))
    }

    fn disconnect(&self, _player: Obj) -> Result<(), SessionError> {
        Ok(())
    }

    fn connected_players(&self) -> Result<Vec<Obj>, SessionError> {
        Ok(vec![])
    }

    fn connected_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn idle_seconds(&self, _player: Obj) -> Result<f64, SessionError> {
        Ok(0.0)
    }

    fn connections(&self, _player: Option<Obj>) -> Result<Vec<Obj>, SessionError> {
        Err(SessionError::NoConnectionForPlayer(SYSTEM_OBJECT))
    }

    fn connection_details(
        &self,
        _player: Option<Obj>,
    ) -> Result<Vec<ConnectionDetails>, SessionError> {
        Err(SessionError::NoConnectionForPlayer(SYSTEM_OBJECT))
    }

    fn connection_attributes(&self, _obj: Obj) -> Result<Var, SessionError> {
        use moor_var::v_list;
        Ok(v_list(&[]))
    }

    fn set_connection_attribute(
        &self,
        _connection_obj: Obj,
        _key: Symbol,
        _value: Var,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

/// A minimal session factory for admin mode that creates ConsoleSession instances
struct AdminSessionFactory;

impl SessionFactory for AdminSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(ConsoleSession::new()))
    }
}

/// Helper for rustyline with tab completion
struct MooAdminHelper {
    scheduler_client: SchedulerClient,
    wizard: Obj,
}

impl MooAdminHelper {
    fn new(scheduler_client: SchedulerClient, wizard: Obj) -> Self {
        Self {
            scheduler_client,
            wizard,
        }
    }

    /// Get all valid object IDs from scheduler
    fn get_all_objects(&self) -> Vec<Obj> {
        self.scheduler_client
            .request_all_objects(self.wizard)
            .unwrap_or_default()
    }

    /// Get properties for a given object
    fn get_properties(&self, obj: &Obj) -> Vec<String> {
        use moor_common::model::ObjectRef;

        let obj_ref = ObjectRef::Id(*obj);
        let Ok(props) =
            self.scheduler_client
                .request_properties(&self.wizard, &self.wizard, &obj_ref, false)
        else {
            return vec![];
        };

        props
            .iter()
            .map(|(pd, _)| pd.name().as_string().to_string())
            .collect()
    }

    /// Get verbs for a given object - returns the first name of each verb
    fn get_verbs(&self, obj: &Obj) -> Vec<String> {
        use moor_common::model::ObjectRef;

        let obj_ref = ObjectRef::Id(*obj);
        let Ok(verbs) =
            self.scheduler_client
                .request_verbs(&self.wizard, &self.wizard, &obj_ref, false)
        else {
            return vec![];
        };

        verbs
            .iter()
            .filter_map(|vd| vd.names().first().map(|s| s.as_string().to_string()))
            .collect()
    }

    /// Complete filenames in the current directory
    fn complete_filename(
        &self,
        partial: &str,
        start_pos: usize,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        use std::fs;

        // Determine the directory and filename prefix
        let (dir_path, file_prefix) = if let Some(last_slash) = partial.rfind('/') {
            let dir = &partial[..=last_slash];
            let prefix = &partial[last_slash + 1..];
            (dir, prefix)
        } else {
            ("./", partial)
        };

        // Read directory entries
        let Ok(entries) = fs::read_dir(dir_path) else {
            return Ok((start_pos, vec![]));
        };

        let mut matches = Vec::new();
        for entry in entries.flatten() {
            let Ok(file_name) = entry.file_name().into_string() else {
                continue;
            };

            if file_name.starts_with(file_prefix) {
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let replacement = if dir_path == "./" {
                    file_name.clone()
                } else {
                    format!("{dir_path}{file_name}")
                };

                let display = if is_dir {
                    format!("{file_name}/")
                } else {
                    file_name
                };

                matches.push(Pair {
                    display,
                    replacement: if is_dir {
                        format!("{replacement}/")
                    } else {
                        replacement
                    },
                });
            }
        }

        matches.sort_by(|a, b| a.display.cmp(&b.display));
        Ok((start_pos, matches))
    }
}

impl Completer for MooAdminHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_before_cursor = &line[..pos];

        // Command completion at start of line
        if !line_before_cursor.contains(' ') {
            let commands = [
                "help", "?", "quit", "exit", "get", "set", "props", "verbs", "list", "prog",
                "dump", "load", "reload", "su",
            ];
            let matches: Vec<Pair> = commands
                .iter()
                .filter(|cmd| cmd.starts_with(line_before_cursor))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            return Ok((0, matches));
        }

        // Flag completion for dump/load/reload commands
        if line_before_cursor.starts_with("dump ")
            || line_before_cursor.starts_with("load ")
            || line_before_cursor.starts_with("reload ")
        {
            // Check if we're in a flag context
            if let Some(flag_start) = line_before_cursor.rfind("--") {
                let after_dashes = &line_before_cursor[flag_start + 2..];

                // Check if there's a space after the flag (completing the value)
                if let Some(space_pos) = after_dashes.find(' ') {
                    let flag_name = &after_dashes[..space_pos];
                    let value_start = flag_start + 2 + space_pos + 1;
                    let partial_value = &line_before_cursor[value_start..];

                    // Complete flag values
                    if flag_name == "file" || flag_name == "constants" {
                        // Filename completion for both --file and --constants
                        return self.complete_filename(partial_value, value_start);
                    } else if flag_name == "conflict-mode" {
                        let modes = ["clobber", "skip", "detect"];
                        let matches: Vec<Pair> = modes
                            .iter()
                            .filter(|mode| mode.starts_with(partial_value))
                            .map(|mode| Pair {
                                display: mode.to_string(),
                                replacement: mode.to_string(),
                            })
                            .collect();
                        return Ok((value_start, matches));
                    } else if flag_name == "as" {
                        // Complete object kinds or object IDs
                        if partial_value.starts_with('#') {
                            // Object ID completion
                            let objects = self.get_all_objects();
                            let matches: Vec<Pair> = objects
                                .iter()
                                .map(|obj| obj.to_literal())
                                .filter(|lit| lit.starts_with(partial_value))
                                .take(50)
                                .map(|lit| Pair {
                                    display: lit.clone(),
                                    replacement: lit.clone(),
                                })
                                .collect();
                            return Ok((value_start, matches));
                        } else {
                            // Complete descriptive words
                            let kinds = ["new", "anonymous", "anon", "uuid"];
                            let matches: Vec<Pair> = kinds
                                .iter()
                                .filter(|kind| kind.starts_with(partial_value))
                                .map(|kind| Pair {
                                    display: kind.to_string(),
                                    replacement: kind.to_string(),
                                })
                                .collect();
                            return Ok((value_start, matches));
                        }
                    }
                } else if !after_dashes.contains('=') {
                    // Completing the flag name itself
                    let is_dump = line_before_cursor.starts_with("dump ");
                    let is_reload = line_before_cursor.starts_with("reload ");
                    let is_load = line_before_cursor.starts_with("load ");

                    let flags = if is_dump {
                        vec!["--file"]
                    } else if is_reload {
                        vec!["--file", "--constants"]
                    } else if is_load {
                        vec![
                            "--file",
                            "--constants",
                            "--dry-run",
                            "--conflict-mode",
                            "--as",
                            "--return-conflicts",
                        ]
                    } else {
                        vec![]
                    };

                    let matches: Vec<Pair> = flags
                        .iter()
                        .filter(|flag| flag.starts_with(&format!("--{after_dashes}")))
                        .map(|flag| Pair {
                            display: flag.to_string(),
                            replacement: format!("{flag} "),
                        })
                        .collect();
                    return Ok((flag_start, matches));
                }
            }
        }

        // Verb completion for list/prog commands (check this BEFORE property completion)
        let is_verb_command =
            line_before_cursor.starts_with("list ") || line_before_cursor.starts_with("prog ");
        if is_verb_command && line_before_cursor.contains(':') {
            let Some(colon_pos) = line_before_cursor.rfind(':') else {
                return Ok((pos, vec![]));
            };

            let before_colon = &line_before_cursor[..colon_pos];
            let after_colon = &line_before_cursor[colon_pos + 1..];

            let Some(obj_str_start) = before_colon.rfind(|c: char| c.is_whitespace()) else {
                return Ok((pos, vec![]));
            };

            let obj_str = before_colon[obj_str_start..].trim();

            let Ok(obj) = parse_objref_with_scheduler(
                obj_str,
                Some(&self.scheduler_client),
                Some(&self.wizard),
            ) else {
                return Ok((pos, vec![]));
            };

            let verbs = self.get_verbs(&obj);
            let matches: Vec<Pair> = verbs
                .iter()
                .filter(|verb| verb.starts_with(after_colon))
                .map(|verb| Pair {
                    display: verb.clone(),
                    replacement: verb.clone(),
                })
                .collect();

            return Ok((colon_pos + 1, matches));
        }

        // Property completion for get/set commands
        let is_prop_command =
            line_before_cursor.starts_with("get ") || line_before_cursor.starts_with("set ");
        if is_prop_command && line_before_cursor.contains('.') {
            let Some(dot_pos) = line_before_cursor.rfind('.') else {
                return Ok((pos, vec![]));
            };

            let before_dot = &line_before_cursor[..dot_pos];
            let after_dot = &line_before_cursor[dot_pos + 1..];

            let Some(obj_str_start) = before_dot.rfind(|c: char| c.is_whitespace()) else {
                return Ok((pos, vec![]));
            };

            let obj_str = before_dot[obj_str_start..].trim();
            let Ok(obj) = parse_objref_with_scheduler(
                obj_str,
                Some(&self.scheduler_client),
                Some(&self.wizard),
            ) else {
                return Ok((pos, vec![]));
            };

            let properties = self.get_properties(&obj);
            let matches: Vec<Pair> = properties
                .iter()
                .filter(|prop| prop.starts_with(after_dot))
                .map(|prop| Pair {
                    display: prop.clone(),
                    replacement: prop.clone(),
                })
                .collect();

            return Ok((dot_pos + 1, matches));
        }

        // Object ID completion for commands that need them
        let is_obj_command = line_before_cursor.starts_with("get ")
            || line_before_cursor.starts_with("set ")
            || line_before_cursor.starts_with("props ")
            || line_before_cursor.starts_with("verbs ")
            || line_before_cursor.starts_with("list ")
            || line_before_cursor.starts_with("prog ")
            || line_before_cursor.starts_with("dump ")
            || line_before_cursor.starts_with("reload ")
            || line_before_cursor.starts_with("su ");

        if is_obj_command {
            let after_command = line_before_cursor
                .split_whitespace()
                .skip(1)
                .collect::<String>();

            // If we're typing an object ID
            if after_command.starts_with('#') {
                let objects = self.get_all_objects();
                let matches: Vec<Pair> = objects
                    .iter()
                    .map(|obj| obj.to_literal())
                    .filter(|lit| lit.starts_with(&after_command))
                    .take(50) // Limit to 50 matches
                    .map(|lit| Pair {
                        display: lit.clone(),
                        replacement: lit.clone(),
                    })
                    .collect();

                let start_pos = line_before_cursor.rfind('#').unwrap_or(pos);
                return Ok((start_pos, matches));
            }
        }

        Ok((pos, vec![]))
    }
}

impl Hinter for MooAdminHelper {
    type Hint = String;
}

impl Highlighter for MooAdminHelper {}

impl Validator for MooAdminHelper {}

impl Helper for MooAdminHelper {}

/// Parsed command-line flags for dump/load commands
#[derive(Debug, Default)]
struct ParsedFlags {
    /// Positional arguments (e.g., object references)
    positional: Vec<String>,
    /// Flag values keyed by flag name (without the --)
    flags: std::collections::HashMap<String, Option<String>>,
}

impl ParsedFlags {
    /// Get a boolean flag value (true if present, false if absent)
    fn get_bool(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    /// Get a string flag value
    fn get_string(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|v| v.as_deref())
    }

    /// Get the first positional argument
    fn first_positional(&self) -> Option<&str> {
        self.positional.first().map(|s| s.as_str())
    }
}

/// Parse command arguments into flags and positional args
/// Supports: --flag, --flag value, --flag=value
fn parse_flags(args: &str) -> ParsedFlags {
    let mut result = ParsedFlags::default();
    let mut tokens: Vec<String> = Vec::new();

    // Simple tokenization respecting quotes
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for ch in args.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => escape_next = true,
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    // Parse tokens into flags and positional args
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];

        if let Some(flag_part) = token.strip_prefix("--") {
            // Check for --flag=value syntax
            if let Some(eq_pos) = flag_part.find('=') {
                let flag_name = flag_part[..eq_pos].to_string();
                let flag_value = flag_part[eq_pos + 1..].to_string();
                result.flags.insert(flag_name, Some(flag_value));
                i += 1;
            } else {
                // Check if next token is a value or another flag
                let flag_name = flag_part.to_string();
                if i + 1 < tokens.len() && !tokens[i + 1].starts_with("--") {
                    result.flags.insert(flag_name, Some(tokens[i + 1].clone()));
                    i += 2;
                } else {
                    // Boolean flag
                    result.flags.insert(flag_name, None);
                    i += 1;
                }
            }
        } else {
            // Positional argument
            result.positional.push(token.clone());
            i += 1;
        }
    }

    result
}

fn print_help() {
    let skin = create_skin();
    let markdown = r#"# Emergency Medical Hologram - Available Procedures

## Database Operations

|Command|Description|
|---|---|
|`;EXPR`|Evaluate MOO expression, print result|
|`;;CODE`|Execute MOO code block, print result|
|`get #OBJ.PROP`|Read property value|
|`set #OBJ.PROP VALUE`|Write property value|
|`props #OBJ`|List all properties on object|
|`verbs #OBJ`|List all verbs on object|
|`prog #OBJ:VERB`|Program a verb (multi-line)|
|`list #OBJ:VERB`|Show verb code|
|`dump #OBJ [--file PATH]`|Dump object to file or console|
|`load [--file PATH] [options]`|Load object from file or console|
|`reload [#OBJ] [--file PATH]`|Replace object contents from file or console|
|`su #OBJ`|Switch to different player object|
|`help, ?`|Show this help|
|`quit, exit`|Save and exit|

## Dump & Load

**Dump command:**
- `dump #OBJ` - Display object definition on console
- `dump #OBJ --file filename.moo` - Save object definition to file

**Load command:**
- `load` - Paste object definition, then type `.` to finish
- `load --file filename.moo` - Load object definition from file

**Load options:**
- `--file PATH` - Load from file instead of stdin
- `--constants PATH` - MOO file with constant definitions
- `--dry-run` - Validate without making changes
- `--conflict-mode MODE` - How to handle conflicts: clobber, skip, or detect
- `--as SPEC` - Where to load: `new`, `anonymous` (or `anon`), `uuid`, or `#OBJ`
- `--return-conflicts` - Return detailed conflict information

**Load examples:**
- `load --file obj.moo --dry-run` - Validate without loading
- `load --file obj.moo --as #123` - Load into specific object
- `load --file obj.moo --as new` - Create new numbered object
- `load --file obj.moo --as anonymous` - Create anonymous object
- `load --file obj.moo --conflict-mode skip` - Skip conflicting properties
- `load --file obj.moo --constants defs.moo` - Load with constants file

**Reload command:**
- `reload [#OBJ]` - Paste object definition, then type `.` to finish
- `reload [#OBJ] --file filename.moo` - Replace object with definition from file
- `reload [#OBJ] --constants defs.moo` - Use constants file for compilation

**Reload examples:**
- `reload --file obj.moo` - Reload object (uses objid from file)
- `reload #123 --file obj.moo` - Force reload into #123
- `reload $player --file player.moo` - Replace player object from file
- `reload --file obj.moo --constants defs.moo` - Reload with constants

"#;
    println!("{}", skin.term_text(markdown));
}

/// Create a MadSkin for terminal output
fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.set_headers_fg(Color::Yellow);
    skin.bold.set_fg(Color::Cyan);
    skin.italic.set_fg(Color::Green);
    skin
}

/// Format a SchedulerError for user-friendly display
fn format_scheduler_error(err: &SchedulerError) -> String {
    match err {
        SchedulerError::TaskAbortedException(exception) => {
            let mut msg = format!("MOO Error: {}\n", exception.error);
            if !exception.backtrace.is_empty() {
                msg.push_str("Traceback:\n");
                for line in &exception.backtrace {
                    if let Some(s) = line.as_string() {
                        msg.push_str(&format!("  {s}\n"));
                    }
                }
            }
            msg
        }
        _ => format!("{err}"),
    }
}

/// Parse an object reference with optional scheduler client for $property resolution
/// Supports "#123" format and "$player" format (which looks up #0.player)
fn parse_objref_with_scheduler(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<Obj, Report> {
    let s = s.trim();

    if let Some(prop_name) = s.strip_prefix('$') {
        // $property reference - need scheduler client to resolve
        let Some(client) = scheduler_client else {
            bail!("Cannot resolve $property references without scheduler client");
        };
        let Some(wiz) = wizard else {
            bail!("Cannot resolve $property references without wizard");
        };

        if prop_name.is_empty() {
            bail!("Invalid $property reference: missing property name");
        }

        let system_obj = Obj::mk_id(0);
        let prop_symbol = Symbol::mk(prop_name);

        let value = client
            .request_system_property(
                wiz,
                &moor_common::model::ObjectRef::Id(system_obj),
                prop_symbol,
            )
            .map_err(|e| eyre!("Failed to retrieve property ${}: {:?}", prop_name, e))?;

        let Some(obj) = value.as_object() else {
            bail!(
                "Property ${} is not an object reference (value: {})",
                prop_name,
                to_literal(&value)
            );
        };

        return Ok(obj);
    }

    if !s.starts_with('#') {
        bail!("Object reference must start with '#' or '$'");
    }
    let num_str = &s[1..];
    let num: i32 = num_str
        .parse()
        .map_err(|_| eyre!("Invalid object number: {}", num_str))?;
    Ok(Obj::mk_id(num))
}

/// Parse "#OBJ.PROP" or "$obj.PROP" into (object, property_name)
fn parse_propref(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    if parts.len() != 2 {
        bail!("Property reference must be in format #OBJ.PROP or $obj.PROP");
    }
    let obj = parse_objref_with_scheduler(parts[0], scheduler_client, wizard)?;
    let prop = Symbol::mk(parts[1]);
    Ok((obj, prop))
}

/// Parse "#OBJ:VERB" or "$obj:VERB" into (object, verb_name)
fn parse_verbref(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        bail!("Verb reference must be in format #OBJ:VERB or $obj:VERB");
    }
    let obj = parse_objref_with_scheduler(parts[0], scheduler_client, wizard)?;
    let verb = Symbol::mk(parts[1]);
    Ok((obj, verb))
}

fn eval_expression(
    scheduler_client: &SchedulerClient,
    session_factory: &Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    wizard: &Obj,
    expr: &str,
) -> Result<(), Report> {
    let session = session_factory
        .clone()
        .mk_background_session(wizard)
        .map_err(|e| eyre!("Failed to create session: {:?}", e))?;

    let code = format!("return {expr};");

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, code, session, features)
        .map_err(|e| eyre!("Failed to submit eval task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out after 30 seconds"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    match result {
        Ok(TaskNotification::Result(value)) => {
            let skin = create_skin();
            let output = to_literal(&value);
            let markdown = format!("**=>** `{output}`");
            println!("{}", skin.term_text(&markdown));
        }
        Ok(TaskNotification::Suspended) => {
            bail!("Received unexpected suspension notification while waiting for eval result");
        }
        Err(e) => {
            error!("Task failed: {:?}", e);
            bail!("Execution failed");
        }
    }
    Ok(())
}

/// Get a property value directly from the database
fn cmd_get(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    let (obj, prop) = parse_propref(args, Some(scheduler_client), Some(wizard))?;

    info!(
        "Getting property {} on {}",
        prop.as_string(),
        obj.to_literal()
    );

    let eval_code = format!("return {};", args.trim());
    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, eval_code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    let Ok(TaskNotification::Result(value)) = result else {
        if let Err(e) = result {
            eprintln!("{}", format_scheduler_error(&e));
            bail!("Failed to get property");
        }
        bail!("Unexpected result");
    };

    let skin = create_skin();
    let markdown = format!(
        "**{}**.`{}` = `{}`",
        obj.to_literal(),
        prop.as_string(),
        to_literal(&value)
    );
    println!("{}", skin.term_text(&markdown));
    Ok(())
}

/// Set a property value
fn cmd_set(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    // Try to parse with '=' first, fall back to space-separated
    let (propref, value_expr) = if args.contains('=') {
        let parts: Vec<&str> = args.splitn(2, '=').collect();
        if parts.len() != 2 {
            bail!("Usage: set #OBJ.PROP VALUE or set #OBJ.PROP = VALUE");
        }
        (parts[0].trim(), parts[1].trim())
    } else {
        // Parse as space-separated: set #OBJ.PROP VALUE
        let parts: Vec<&str> = args.splitn(2, |c: char| c.is_whitespace()).collect();
        if parts.len() != 2 {
            bail!("Usage: set #OBJ.PROP VALUE or set #OBJ.PROP = VALUE");
        }
        (parts[0].trim(), parts[1].trim())
    };

    let (obj, prop) = parse_propref(propref, Some(scheduler_client), Some(wizard))?;

    info!(
        "Setting property {} on {} to {}",
        prop.as_string(),
        obj.to_literal(),
        value_expr
    );

    let code = format!(
        "{}.{} = {};",
        obj.to_literal(),
        prop.as_string(),
        value_expr
    );

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out"))?;

    let Err(e) = result else {
        let skin = create_skin();
        let markdown = format!(
            "**✓** Property `{}`.`{}` set successfully",
            obj.to_literal(),
            prop.as_string()
        );
        println!("{}", skin.term_text(&markdown));
        return Ok(());
    };

    eprintln!("{}", format_scheduler_error(&e));
    bail!("Failed to set property");
}

/// List all properties on an object
fn cmd_props(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Listing properties on {}", obj.to_literal());

    use moor_common::model::ObjectRef;
    let obj_ref = ObjectRef::Id(obj);
    let props = scheduler_client
        .request_properties(wizard, wizard, &obj_ref, false)
        .map_err(|e| eyre!("Failed to get properties: {:?}", e))?;

    let skin = create_skin();

    // Build markdown table
    let mut markdown = format!("# Properties on {}\n\n", obj.to_literal());
    markdown.push_str("|Property|\n");
    markdown.push_str("|---|\n");

    for (prop, _) in props.iter() {
        markdown.push_str(&format!("|{}|\n", prop.name().as_string()));
    }

    markdown.push_str(&format!("\n*{} properties*\n", props.len()));

    println!("{}", skin.term_text(&markdown));
    Ok(())
}

/// List all verbs on an object
fn cmd_verbs(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Listing verbs on {}", obj.to_literal());

    use moor_common::model::ObjectRef;
    let obj_ref = ObjectRef::Id(obj);
    let verbs = scheduler_client
        .request_verbs(wizard, wizard, &obj_ref, false)
        .map_err(|e| eyre!("Failed to get verbs: {:?}", e))?;

    let skin = create_skin();

    // Build markdown table
    let mut markdown = format!("# Verbs on {}\n\n", obj.to_literal());
    markdown.push_str("|Verb|\n");
    markdown.push_str("|---|\n");

    for verb in verbs.iter() {
        // Get the first name of the verb
        if let Some(name) = verb.names().first() {
            markdown.push_str(&format!("|**{}**|\n", name.as_string()));
        }
    }

    markdown.push_str(&format!("\n*{} verbs*\n", verbs.len()));

    println!("{}", skin.term_text(&markdown));
    Ok(())
}

/// Program a verb with multi-line input
fn cmd_prog(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    let (obj, verb) = parse_verbref(args, Some(scheduler_client), Some(wizard))?;
    info!(
        "Programming verb {} on {}",
        verb.as_string(),
        obj.to_literal()
    );

    let skin = create_skin();
    let intro = format!(
        "**Programming** `{}:{}`\n\nEnter code (type `.` on a line by itself to finish):",
        obj.to_literal(),
        verb.as_string()
    );
    println!("{}", skin.term_text(&intro));

    // Collect lines until we see a line with just "."
    let mut code_lines = Vec::new();
    loop {
        let line_result = rl.readline(">> ");
        match line_result {
            Ok(line) => {
                if line.trim() == "." {
                    break;
                }
                code_lines.push(line);
            }
            Err(ReadlineError::Interrupted) => {
                println!("Cancelled.");
                return Ok(());
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                bail!("Error reading input: {}", e);
            }
        }
    }

    if code_lines.is_empty() {
        println!("No code entered, verb unchanged.");
        return Ok(());
    }

    // Build the set_verb_code call - it expects a list of strings
    // Escape quotes and backslashes in each line
    let escaped_lines: Vec<String> = code_lines
        .iter()
        .map(|line| {
            let escaped = line.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        })
        .collect();

    let code_list = format!("{{{}}}", escaped_lines.join(", "));
    let moo_code = format!(
        "set_verb_code({}, \"{}\", {});",
        obj.to_literal(),
        verb.as_string(),
        code_list
    );

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, moo_code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    let Err(e) = result else {
        let markdown = format!(
            "**✓** Verb `{}:{}` programmed successfully ({} lines)",
            obj.to_literal(),
            verb.as_string(),
            code_lines.len()
        );
        println!("{}", skin.term_text(&markdown));
        return Ok(());
    };

    eprintln!("{}", format_scheduler_error(&e));
    bail!("Failed to program verb");
}

/// Switch to a different player object
fn cmd_su(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<Obj, Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Attempting to switch to object {}", obj.to_literal());

    // Check if object is valid and has User flag by evaluating MOO code
    let check_code = format!("return {{valid({0}), is_player({0})}};", obj.to_literal());

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, check_code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    let Ok(TaskNotification::Result(value)) = result else {
        if let Err(e) = result {
            eprintln!("{}", format_scheduler_error(&e));
            bail!("Failed to check object");
        }
        bail!("Unexpected result");
    };

    // Parse the result list {valid, is_player}
    let Some(list) = value.as_list() else {
        bail!("Unexpected result format");
    };

    if list.len() != 2 {
        bail!("Unexpected result format");
    }

    let Some(valid) = list[0].as_integer() else {
        bail!("Unexpected result format");
    };

    let Some(is_player) = list[1].as_integer() else {
        bail!("Unexpected result format");
    };

    if valid == 0 {
        bail!("Object {} does not exist", obj.to_literal());
    }

    if is_player == 0 {
        bail!(
            "Object {} is not a player object (missing User flag)",
            obj.to_literal()
        );
    }

    let skin = create_skin();
    let markdown = format!("**✓** Switched to player `{}`", obj.to_literal());
    println!("{}", skin.term_text(&markdown));
    Ok(obj)
}

/// List a verb's code
fn cmd_list(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    let (obj, verb) = parse_verbref(args, Some(scheduler_client), Some(wizard))?;
    info!("Listing verb {} on {}", verb.as_string(), obj.to_literal());

    let code = format!(
        "return verb_code({}, \"{}\");",
        obj.to_literal(),
        verb.as_string()
    );

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    let Ok(TaskNotification::Result(value)) = result else {
        if let Err(e) = result {
            eprintln!("{}", format_scheduler_error(&e));
            bail!("Failed to list verb");
        }
        bail!("Unexpected result");
    };

    let Some(lines) = value.as_list() else {
        bail!("verb_code did not return a list");
    };

    let skin = create_skin();

    // Build markdown with code block
    let mut markdown = format!("# Verb: {}:{}\n\n", obj.to_literal(), verb.as_string());
    markdown.push_str("```moo\n");

    for line in lines.iter() {
        let Some(s) = line.as_string() else {
            continue;
        };
        markdown.push_str(s);
        markdown.push('\n');
    }

    markdown.push_str("```\n");
    markdown.push_str(&format!("\n*{} lines*\n", lines.len()));

    println!("{}", skin.term_text(&markdown));
    Ok(())
}

/// Dump an object definition
fn cmd_dump(scheduler_client: &SchedulerClient, wizard: &Obj, args: &str) -> Result<(), Report> {
    // Parse args with flag parser
    let parsed = parse_flags(args);

    // First positional arg must be the object
    let Some(obj_str) = parsed.first_positional() else {
        bail!("Usage: dump #OBJ [--file FILENAME]");
    };

    let obj = parse_objref_with_scheduler(obj_str, Some(scheduler_client), Some(wizard))?;
    let filename = parsed.get_string("file").map(PathBuf::from);

    info!("Dumping object {}", obj.to_literal());

    let code = format!("return dump_object({});", obj.to_literal());

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, code, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let result = loop {
        let (_task_id, result) = handle
            .receiver()
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| eyre!("Task timed out"))?;
        match result {
            Ok(TaskNotification::Suspended) => continue,
            other => break other,
        }
    };

    let Ok(TaskNotification::Result(value)) = result else {
        if let Err(e) = result {
            eprintln!("{}", format_scheduler_error(&e));
            bail!("Failed to dump object");
        }
        bail!("Unexpected result");
    };

    let Some(lines) = value.as_list() else {
        bail!("dump_object did not return a list");
    };

    let skin = create_skin();

    // If filename provided, write to file; otherwise print to console
    if let Some(path) = filename {
        let mut file =
            File::create(&path).map_err(|e| eyre!("Failed to create file {:?}: {}", path, e))?;

        for line in lines.iter() {
            let Some(s) = line.as_string() else {
                continue;
            };
            writeln!(file, "{s}")
                .map_err(|e| eyre!("Failed to write to file {:?}: {}", path, e))?;
        }

        let markdown = format!(
            "**✓** Object `{}` dumped to `{}`\n\n*{} lines written*",
            obj.to_literal(),
            path.display(),
            lines.len()
        );
        println!("{}", skin.term_text(&markdown));
    } else {
        let markdown = format!("# Object Definition: {}\n\n", obj.to_literal());
        println!("{}", skin.term_text(&markdown));

        // Print the definition lines
        for line in lines.iter() {
            let Some(s) = line.as_string() else {
                continue;
            };
            println!("{s}");
        }

        let summary = format!("\n*{} lines dumped*", lines.len());
        println!("{}", skin.term_text(&summary));
    }

    Ok(())
}

/// Load an object definition
fn cmd_load(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    use moor_objdef::{ConflictMode, ObjDefLoaderOptions};

    let skin = create_skin();

    // Parse args with flag parser
    let parsed = parse_flags(args);

    let filename = parsed.get_string("file").map(PathBuf::from);

    let definition_lines = if let Some(path) = &filename {
        // Read from file
        let content = std::fs::read_to_string(path)
            .map_err(|e| eyre!("Failed to read file {:?}: {}", path, e))?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        info!("Loaded {} lines from {:?}", lines.len(), path);
        lines
    } else {
        // Read from stdin
        let intro = "**Loading object definition**\n\nPaste object definition (type `.` on a line by itself to finish):";
        println!("{}", skin.term_text(intro));

        let mut lines = Vec::new();
        loop {
            let line_result = rl.readline(">> ");
            match line_result {
                Ok(line) => {
                    if line.trim() == "." {
                        break;
                    }
                    lines.push(line);
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Cancelled.");
                    return Ok(());
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    bail!("Error reading input: {}", e);
                }
            }
        }
        lines
    };

    if definition_lines.is_empty() {
        println!("No definition entered, nothing loaded.");
        return Ok(());
    }

    let object_definition = definition_lines.join("\n");

    // Parse options from flags
    let mut dry_run = parsed.get_bool("dry-run");
    let return_conflicts = parsed.get_bool("return-conflicts");

    let conflict_mode = if let Some(mode) = parsed.get_string("conflict-mode") {
        match mode {
            "clobber" => ConflictMode::Clobber,
            "skip" => ConflictMode::Skip,
            "detect" => {
                // "detect" mode is dry_run + return_conflicts
                dry_run = true;
                ConflictMode::Skip
            }
            _ => bail!("Invalid conflict-mode: must be clobber, skip, or detect"),
        }
    } else {
        ConflictMode::Clobber
    };

    // Parse object specification: "new", "anonymous"/"anon", "uuid", #N=specific ID, omitted=use objdef's ID
    let object_kind = if let Some(spec_str) = parsed.get_string("as") {
        if spec_str.starts_with('#') {
            // Object ID like #123
            let obj = parse_objref_with_scheduler(spec_str, Some(scheduler_client), Some(wizard))?;
            Some(ObjectKind::Objid(obj))
        } else {
            // Named specification
            match spec_str {
                "new" => Some(ObjectKind::NextObjid),
                "anonymous" | "anon" => Some(ObjectKind::Anonymous),
                "uuid" => Some(ObjectKind::UuObjId),
                _ => bail!(
                    "Invalid --as value: {spec_str}. Must be 'new', 'anonymous', 'uuid', or #ID"
                ),
            }
        }
    } else {
        None
    };

    // Read constants file if provided
    let constants = if let Some(constants_path) = parsed.get_string("constants") {
        let constants_path = PathBuf::from(constants_path);
        let content = std::fs::read_to_string(&constants_path)
            .map_err(|e| eyre!("Failed to read constants file {:?}: {}", constants_path, e))?;
        info!("Loaded constants from {:?}", constants_path);
        Some(moor_objdef::Constants::FileContent(content))
    } else {
        None
    };

    // No explicit permission check needed - load_object will check permissions internally
    let loader_options = ObjDefLoaderOptions {
        dry_run,
        conflict_mode,
        object_kind,
        constants,
        validate_parent_changes: true, // Individual load_object command should validate
        ..Default::default()
    };

    // Load through the scheduler client so it uses the scheduler's database
    let result = scheduler_client
        .load_object(object_definition, loader_options, return_conflicts)
        .map_err(|e| eyre!("Failed to load object: {}", e))?;

    // Display results
    if return_conflicts {
        let markdown = if result.commit {
            format!(
                "**✓** Load completed successfully\n\n**Loaded objects:** {}\n**Conflicts:** {}\n\n*{} lines processed*",
                result.loaded_objects.len(),
                result.conflicts.len(),
                definition_lines.len()
            )
        } else {
            format!(
                "**⚠** Load would have conflicts (dry-run or detect mode)\n\n**Would load:** {}\n**Conflicts:** {}\n\n*{} lines processed*",
                result.loaded_objects.len(),
                result.conflicts.len(),
                definition_lines.len()
            )
        };
        println!("{}", skin.term_text(&markdown));
    } else if result.loaded_objects.is_empty() {
        bail!("No objects were loaded");
    } else {
        let obj = result.loaded_objects[0];
        let markdown = format!(
            "**✓** Object `{}` loaded successfully\n\n*{} lines processed*",
            obj.to_literal(),
            definition_lines.len()
        );
        println!("{}", skin.term_text(&markdown));
    }

    Ok(())
}

/// Reload an object definition, completely replacing its contents
fn cmd_reload(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    let skin = create_skin();

    // Parse args with flag parser
    let parsed = parse_flags(args);

    // First positional arg is optional target object
    let target_obj = if let Some(obj_str) = parsed.first_positional() {
        Some(parse_objref_with_scheduler(
            obj_str,
            Some(scheduler_client),
            Some(wizard),
        )?)
    } else {
        None
    };

    let filename = parsed.get_string("file").map(PathBuf::from);

    let definition_lines = if let Some(path) = &filename {
        // Read from file
        let content = std::fs::read_to_string(path)
            .map_err(|e| eyre!("Failed to read file {:?}: {}", path, e))?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        info!("Loaded {} lines from {:?}", lines.len(), path);
        lines
    } else {
        // Read from stdin
        let intro = if let Some(obj) = target_obj {
            format!(
                "**Reloading object `{}`**\n\nPaste object definition (type `.` on a line by itself to finish):",
                obj.to_literal()
            )
        } else {
            "**Reloading object**\n\nPaste object definition (type `.` on a line by itself to finish):".to_string()
        };
        println!("{}", skin.term_text(&intro));

        let mut lines = Vec::new();
        loop {
            let line_result = rl.readline(">> ");
            match line_result {
                Ok(line) => {
                    if line.trim() == "." {
                        break;
                    }
                    lines.push(line);
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Cancelled.");
                    return Ok(());
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    bail!("Error reading input: {}", e);
                }
            }
        }
        lines
    };

    if definition_lines.is_empty() {
        println!("No definition entered, nothing reloaded.");
        return Ok(());
    }

    let object_definition = definition_lines.join("\n");

    // Read constants file if provided
    let constants = if let Some(constants_path) = parsed.get_string("constants") {
        let constants_path = PathBuf::from(constants_path);
        let content = std::fs::read_to_string(&constants_path)
            .map_err(|e| eyre!("Failed to read constants file {:?}: {}", constants_path, e))?;
        info!("Loaded constants from {:?}", constants_path);
        Some(moor_objdef::Constants::FileContent(content))
    } else {
        None
    };

    // Reload through the scheduler client
    let result = scheduler_client
        .reload_object(object_definition, constants, target_obj)
        .map_err(|e| eyre!("Failed to reload object: {}", e))?;

    // Display results
    if result.loaded_objects.is_empty() {
        bail!("No objects were reloaded");
    }

    let obj = result.loaded_objects[0];
    let markdown = format!(
        "**✓** Object `{}` reloaded successfully\n\n*{} lines processed*",
        obj.to_literal(),
        definition_lines.len()
    );
    println!("{}", skin.term_text(&markdown));

    Ok(())
}

fn repl(
    scheduler_client: SchedulerClient,
    session_factory: Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    wizard: Obj,
    mut rl: Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    let mut current_wizard = wizard;
    let skin = create_skin();
    let intro = format!(
        r#"
# Emergency Medical Hologram - Database Administration Subroutine

**Please state the nature of the database emergency.**

*Running as wizard: `{}`*

Type `help` for available commands or `quit` to deactivate.
"#,
        current_wizard.to_literal()
    );

    println!("{}", skin.term_text(&intro));

    loop {
        let prompt = format!("({}): ", current_wizard.to_literal());
        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                // Handle commands
                if line == "quit" || line == "exit" {
                    let skin = create_skin();
                    let goodbye = "**Emergency Medical Hologram deactivated.** Database will be saved on shutdown.";
                    println!("{}", skin.term_text(goodbye));
                    break;
                } else if line == "help" || line == "?" {
                    print_help();
                } else if line.starts_with(';') {
                    // Eval expression or execute code block
                    if let Some(code) = line.strip_prefix(";;") {
                        // ;; executes code as-is without wrapping in return
                        let session_result = session_factory
                            .clone()
                            .mk_background_session(&current_wizard);

                        let Ok(session) =
                            session_result.map_err(|e| eyre!("Failed to create session: {:?}", e))
                        else {
                            continue;
                        };

                        let Ok(handle) = scheduler_client.submit_eval_task(
                            &current_wizard,
                            &current_wizard,
                            code.trim().to_string(),
                            session,
                            features.clone(),
                        ) else {
                            continue;
                        };

                        let result = loop {
                            match handle.receiver().recv_timeout(Duration::from_secs(30)) {
                                Ok((_task_id, Ok(TaskNotification::Suspended))) => continue,
                                other => break other,
                            }
                        };

                        match result {
                            Ok((_task_id, Ok(TaskNotification::Result(value)))) => {
                                let skin = create_skin();
                                let output = to_literal(&value);
                                let markdown = format!("**=>** `{output}`");
                                println!("{}", skin.term_text(&markdown));
                            }
                            Ok((_task_id, Err(e))) => {
                                error!("Execution failed: {:?}", e);
                            }
                            Err(_) => {
                                error!("Task timed out after 30 seconds");
                            }
                            _ => {
                                warn!("Task was replaced by scheduler");
                            }
                        }
                    } else if let Some(expr) = line.strip_prefix(';') {
                        // ; wraps expression in return
                        if let Err(e) = eval_expression(
                            &scheduler_client,
                            &session_factory,
                            features.clone(),
                            &current_wizard,
                            expr.trim(),
                        ) {
                            error!("Eval failed: {}", e);
                        }
                    }
                } else if line.starts_with("get ") {
                    let args = line.strip_prefix("get ").unwrap().trim();
                    if let Err(e) = cmd_get(&scheduler_client, &current_wizard, args) {
                        error!("Get failed: {}", e);
                    }
                } else if line.starts_with("set ") {
                    let args = line.strip_prefix("set ").unwrap().trim();
                    if let Err(e) = cmd_set(&scheduler_client, &current_wizard, args) {
                        error!("Set failed: {}", e);
                    }
                } else if line.starts_with("props ") {
                    let args = line.strip_prefix("props ").unwrap().trim();
                    if let Err(e) = cmd_props(&scheduler_client, &current_wizard, args) {
                        error!("Props failed: {}", e);
                    }
                } else if line.starts_with("verbs ") {
                    let args = line.strip_prefix("verbs ").unwrap().trim();
                    if let Err(e) = cmd_verbs(&scheduler_client, &current_wizard, args) {
                        error!("Verbs failed: {}", e);
                    }
                } else if line.starts_with("prog ") {
                    let args = line.strip_prefix("prog ").unwrap().trim();
                    if let Err(e) = cmd_prog(&scheduler_client, &current_wizard, args, &mut rl) {
                        error!("Prog failed: {}", e);
                    }
                } else if line.starts_with("list ") {
                    let args = line.strip_prefix("list ").unwrap().trim();
                    if let Err(e) = cmd_list(&scheduler_client, &current_wizard, args) {
                        error!("List failed: {}", e);
                    }
                } else if line.starts_with("dump ") {
                    let args = line.strip_prefix("dump ").unwrap().trim();
                    if let Err(e) = cmd_dump(&scheduler_client, &current_wizard, args) {
                        error!("Dump failed: {}", e);
                    }
                } else if line.starts_with("load") {
                    let args = line.strip_prefix("load").unwrap_or("").trim();
                    if let Err(e) = cmd_load(&scheduler_client, &current_wizard, args, &mut rl) {
                        error!("Load failed: {}", e);
                    }
                } else if line.starts_with("reload ") {
                    let args = line.strip_prefix("reload ").unwrap().trim();
                    if let Err(e) = cmd_reload(&scheduler_client, &current_wizard, args, &mut rl) {
                        error!("Reload failed: {}", e);
                    }
                } else if line.starts_with("su ") {
                    let args = line.strip_prefix("su ").unwrap().trim();
                    match cmd_su(&scheduler_client, &current_wizard, args) {
                        Ok(new_wizard) => {
                            current_wizard = new_wizard;
                        }
                        Err(e) => {
                            error!("Su failed: {}", e);
                        }
                    }
                } else {
                    error!("Unknown command. Type 'help' for available commands.");
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C - use 'quit' to exit");
            }
            Err(ReadlineError::Eof) => {
                println!("EOF - exiting");
                break;
            }
            Err(err) => {
                error!("Error reading line: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let args = Args::parse();
    let version = semver::Version::parse(build::PKG_VERSION)
        .map_err(|e| eyre!("Invalid moor version '{}': {}", build::PKG_VERSION, e))?;

    eprintln!("moor-emh {version} starting");

    // We'll create the editor after we have the database
    let rl_config = rustyline::Config::builder().auto_add_history(true).build();

    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    // Build filter with gdt_cpus suppression
    let filter = if let Ok(env_filter) = EnvFilter::try_from_default_env() {
        // User has set RUST_LOG, respect it but still suppress gdt_cpus
        env_filter.add_directive("gdt_cpus=off".parse().unwrap())
    } else {
        // No RUST_LOG set, build filter from scratch with gdt_cpus suppressed
        let level = if args.debug { "debug" } else { "info" };
        EnvFilter::new(format!("{level},gdt_cpus=off"))
    };

    // Create a temporary editor just for the ExternalPrinter
    let mut temp_rl = Editor::<(), rustyline::history::DefaultHistory>::new()?;

    // Try to create ExternalPrinter for proper log handling
    if let Ok(printer) = temp_rl.create_external_printer() {
        let make_writer = ExternalPrinterMakeWriter::new(printer);
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(make_writer))
            .init();
    } else {
        // Fall back to stderr if ExternalPrinter fails (e.g., not a TTY)
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stderr))
            .init();
    }

    // Acquire lock
    let _lock = acquire_data_directory_lock(&args.data_dir)?;

    // Open database
    let resolved_db_path = args.resolved_db_path();
    info!("Opening database at {:?}", resolved_db_path);
    let (database, _freshly_made) = TxDB::open(Some(&resolved_db_path), DatabaseConfig::default());
    let database = Box::new(database);

    // Find wizard before handing database to scheduler
    let wizard = find_wizard(database.as_ref(), args.wizard)?;

    // Create scheduler with the database
    let tasks_db = Box::new(NoopTasksDb {});
    let features = Arc::new(FeaturesConfig::default());
    let config = Config {
        features: features.clone(),
        ..Default::default()
    };

    let scheduler = Scheduler::new(
        version,
        database,
        tasks_db,
        Arc::new(config),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );

    let scheduler_client = scheduler
        .client()
        .map_err(|e| eyre!("Failed to get scheduler client: {}", e))?;

    // Now create the real editor with completion helper (needs scheduler_client)
    let helper = MooAdminHelper::new(scheduler_client.clone(), wizard);
    let mut rl = Editor::with_config(rl_config)?;
    rl.set_helper(Some(helper));

    let session_factory = Arc::new(AdminSessionFactory);

    // Start scheduler thread
    let scheduler_session_factory = session_factory.clone();
    let scheduler_thread = std::thread::Builder::new()
        .name("moor-emh-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_session_factory))
        .map_err(|e| eyre!("Failed to spawn scheduler thread: {}", e))?;

    // Sleep a little to let the scheduler finish its start-up jobs.
    std::thread::sleep(Duration::from_secs(1));

    // Run REPL
    let repl_result = repl(
        scheduler_client.clone(),
        session_factory,
        features,
        wizard,
        rl,
    );

    // Shutdown
    info!("Shutting down...");
    if let Err(e) = scheduler_client.submit_shutdown("Admin mode exiting") {
        error!("Failed to send shutdown signal to scheduler: {}", e);
    }

    if let Err(e) = scheduler_thread.join() {
        error!("Scheduler thread panicked: {:?}", e);
    }

    repl_result
}
