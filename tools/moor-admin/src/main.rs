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
    tasks::{NoopTasksDb, TaskResult, scheduler::Scheduler},
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
#[command(name = "moor-admin")]
#[command(about = "Emergency admin tool for mooR databases", long_about = None)]
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

    let lock_file_path = data_dir.join(".moor-admin.lock");
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
                        print!("{}", output);
                    } else {
                        println!("{}", output);
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

    fn request_input(&self, player: Obj, _input_request_id: Uuid) -> Result<(), SessionError> {
        panic!("ConsoleSession::request_input called for player {player}")
    }

    fn send_event(&self, _player: Obj, event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        self.buffer.lock().unwrap().push(*event);
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, msg: &str) -> Result<(), SessionError> {
        println!("** {} **", msg);
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        if let Some(msg) = msg {
            println!("** Server shutting down: {} **", msg);
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
    database: Arc<Box<dyn Database>>,
}

impl MooAdminHelper {
    fn new(database: Arc<Box<dyn Database>>) -> Self {
        Self { database }
    }

    /// Get all valid object IDs from database
    fn get_all_objects(&self) -> Vec<Obj> {
        if let Ok(tx) = self.database.new_world_state() {
            tx.all_objects().unwrap_or_default().iter().collect()
        } else {
            vec![]
        }
    }

    /// Get properties for a given object
    fn get_properties(&self, obj: &Obj) -> Vec<String> {
        let Ok(tx) = self.database.new_world_state() else {
            return vec![];
        };

        // Use SYSTEM_OBJECT as perms for admin access
        let Ok(props) = tx.properties(&SYSTEM_OBJECT, obj) else {
            return vec![];
        };

        props
            .iter()
            .map(|pd| pd.name().as_string().to_string())
            .collect()
    }

    /// Get verbs for a given object - returns the first name of each verb
    fn get_verbs(&self, obj: &Obj) -> Vec<String> {
        let Ok(tx) = self.database.new_world_state() else {
            return vec![];
        };

        // Use SYSTEM_OBJECT as perms for admin access
        let Ok(verbs) = tx.verbs(&SYSTEM_OBJECT, obj) else {
            return vec![];
        };

        verbs
            .iter()
            .filter_map(|vd| vd.names().first().map(|s| s.as_string().to_string()))
            .collect()
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
                "help", "?", "quit", "exit", "get", "set", "props", "verbs", "list", "prog", "su",
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

            let Ok(obj) = parse_objref_with_db(obj_str, Some(self.database.as_ref().as_ref()))
            else {
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
            let Ok(obj) = parse_objref_with_db(obj_str, Some(self.database.as_ref().as_ref()))
            else {
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
|`su #OBJ`|Switch to different player object|
|`help, ?`|Show this help|
|`quit, exit`|Save and exit|

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
                        msg.push_str(&format!("  {}\n", s));
                    }
                }
            }
            msg
        }
        _ => format!("{}", err),
    }
}

/// Parse an object reference with optional database for $property resolution
/// Supports "#123" format and "$player" format (which looks up #0.player)
fn parse_objref_with_db(s: &str, database: Option<&dyn Database>) -> Result<Obj, Report> {
    let s = s.trim();

    if let Some(prop_name) = s.strip_prefix('$') {
        // $property reference - need database to resolve
        let Some(db) = database else {
            bail!("Cannot resolve $property references without database access");
        };

        if prop_name.is_empty() {
            bail!("Invalid $property reference: missing property name");
        }

        let tx = db
            .new_world_state()
            .map_err(|e| eyre!("Failed to access database: {}", e))?;

        let system_obj = Obj::mk_id(0);
        let prop_symbol = Symbol::mk(prop_name);

        let value = tx
            .retrieve_property(&SYSTEM_OBJECT, &system_obj, prop_symbol)
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
fn parse_propref(s: &str, database: Option<&dyn Database>) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    if parts.len() != 2 {
        bail!("Property reference must be in format #OBJ.PROP or $obj.PROP");
    }
    let obj = parse_objref_with_db(parts[0], database)?;
    let prop = Symbol::mk(parts[1]);
    Ok((obj, prop))
}

/// Parse "#OBJ:VERB" or "$obj:VERB" into (object, verb_name)
fn parse_verbref(s: &str, database: Option<&dyn Database>) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        bail!("Verb reference must be in format #OBJ:VERB or $obj:VERB");
    }
    let obj = parse_objref_with_db(parts[0], database)?;
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

    let code = format!("return {};", expr);

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, code, session, features)
        .map_err(|e| eyre!("Failed to submit eval task: {:?}", e))?;

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out after 30 seconds"))?;

    match result {
        Ok(TaskResult::Result(value)) => {
            let skin = create_skin();
            let output = to_literal(&value);
            let markdown = format!("**=>** `{}`", output);
            println!("{}", skin.term_text(&markdown));
        }
        Ok(TaskResult::Replaced(_)) => {
            warn!("Task was replaced by scheduler");
        }
        Err(e) => {
            error!("Task failed: {:?}", e);
            bail!("Execution failed");
        }
    }
    Ok(())
}

/// Get a property value directly from the database
fn cmd_get(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    database: &dyn Database,
    args: &str,
) -> Result<(), Report> {
    let (obj, prop) = parse_propref(args, Some(database))?;

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

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out"))?;

    let Ok(TaskResult::Result(value)) = result else {
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
fn cmd_set(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    database: &dyn Database,
    args: &str,
) -> Result<(), Report> {
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

    let (obj, prop) = parse_propref(propref, Some(database))?;

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
fn cmd_props(database: &dyn Database, wizard: &Obj, args: &str) -> Result<(), Report> {
    let obj = parse_objref_with_db(args.trim(), Some(database))?;
    info!("Listing properties on {}", obj.to_literal());

    let tx = database.new_world_state()?;

    let props = tx
        .properties(wizard, &obj)
        .map_err(|e| eyre!("Failed to get properties: {:?}", e))?;

    let skin = create_skin();

    // Build markdown table
    let mut markdown = format!("# Properties on {}\n\n", obj.to_literal());
    markdown.push_str("|Property|\n");
    markdown.push_str("|---|\n");

    for prop in props.iter() {
        markdown.push_str(&format!("|{}|\n", prop.name().as_string()));
    }

    markdown.push_str(&format!("\n*{} properties*\n", props.len()));

    println!("{}", skin.term_text(&markdown));
    Ok(())
}

/// List all verbs on an object
fn cmd_verbs(database: &dyn Database, wizard: &Obj, args: &str) -> Result<(), Report> {
    let obj = parse_objref_with_db(args.trim(), Some(database))?;
    info!("Listing verbs on {}", obj.to_literal());

    let tx = database.new_world_state()?;

    let verbs = tx
        .verbs(wizard, &obj)
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
    database: &dyn Database,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    let (obj, verb) = parse_verbref(args, Some(database))?;
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
            format!("\"{}\"", escaped)
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

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out"))?;

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
fn cmd_su(database: &dyn Database, args: &str) -> Result<Obj, Report> {
    let obj = parse_objref_with_db(args.trim(), Some(database))?;
    info!("Attempting to switch to object {}", obj.to_literal());

    let tx = database.new_world_state()?;

    // Check if object is valid
    if !tx.valid(&obj)? {
        bail!("Object {} does not exist", obj.to_literal());
    }

    // Check if object has the User flag (which marks player objects)
    let flags = tx.flags_of(&obj)?;
    if !flags.contains(ObjFlag::User) {
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
fn cmd_list(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    database: &dyn Database,
    args: &str,
) -> Result<(), Report> {
    let (obj, verb) = parse_verbref(args, Some(database))?;
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

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out"))?;

    let Ok(TaskResult::Result(value)) = result else {
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

fn repl(
    scheduler_client: SchedulerClient,
    session_factory: Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    database: Arc<Box<dyn Database>>,
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

                        match handle.receiver().recv_timeout(Duration::from_secs(30)) {
                            Ok((_task_id, Ok(TaskResult::Result(value)))) => {
                                let skin = create_skin();
                                let output = to_literal(&value);
                                let markdown = format!("**=>** `{}`", output);
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
                    if let Err(e) = cmd_get(
                        &scheduler_client,
                        &current_wizard,
                        database.as_ref().as_ref(),
                        args,
                    ) {
                        error!("Get failed: {}", e);
                    }
                } else if line.starts_with("set ") {
                    let args = line.strip_prefix("set ").unwrap().trim();
                    if let Err(e) = cmd_set(
                        &scheduler_client,
                        &current_wizard,
                        database.as_ref().as_ref(),
                        args,
                    ) {
                        error!("Set failed: {}", e);
                    }
                } else if line.starts_with("props ") {
                    let args = line.strip_prefix("props ").unwrap().trim();
                    if let Err(e) = cmd_props(database.as_ref().as_ref(), &current_wizard, args) {
                        error!("Props failed: {}", e);
                    }
                } else if line.starts_with("verbs ") {
                    let args = line.strip_prefix("verbs ").unwrap().trim();
                    if let Err(e) = cmd_verbs(database.as_ref().as_ref(), &current_wizard, args) {
                        error!("Verbs failed: {}", e);
                    }
                } else if line.starts_with("prog ") {
                    let args = line.strip_prefix("prog ").unwrap().trim();
                    if let Err(e) = cmd_prog(
                        &scheduler_client,
                        &current_wizard,
                        database.as_ref().as_ref(),
                        args,
                        &mut rl,
                    ) {
                        error!("Prog failed: {}", e);
                    }
                } else if line.starts_with("list ") {
                    let args = line.strip_prefix("list ").unwrap().trim();
                    if let Err(e) = cmd_list(
                        &scheduler_client,
                        &current_wizard,
                        database.as_ref().as_ref(),
                        args,
                    ) {
                        error!("List failed: {}", e);
                    }
                } else if line.starts_with("su ") {
                    let args = line.strip_prefix("su ").unwrap().trim();
                    match cmd_su(database.as_ref().as_ref(), args) {
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

    eprintln!("moor-admin {} starting", version);

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
        EnvFilter::new(format!("{},gdt_cpus=off", level))
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

    // Find wizard
    let wizard = find_wizard(database.as_ref(), args.wizard)?;

    // Now create the real editor with completion helper
    let database_arc: Arc<Box<dyn Database>> = Arc::new(database);
    let helper = MooAdminHelper::new(database_arc.clone());
    let mut rl = Editor::with_config(rl_config)?;
    rl.set_helper(Some(helper));

    // Create scheduler
    let tasks_db = Box::new(NoopTasksDb {});
    let features = Arc::new(FeaturesConfig::default());
    let config = Config {
        features: features.clone(),
        ..Default::default()
    };

    // We need to clone the Box contents for the scheduler, not the Arc
    // The Database trait isn't Clone, so we need to open it again
    let (database_for_scheduler, _) =
        TxDB::open(Some(&resolved_db_path), DatabaseConfig::default());

    let scheduler = Scheduler::new(
        version,
        Box::new(database_for_scheduler),
        tasks_db,
        Arc::new(config),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );

    let scheduler_client = scheduler
        .client()
        .map_err(|e| eyre!("Failed to get scheduler client: {}", e))?;

    let session_factory = Arc::new(AdminSessionFactory);

    // Start scheduler thread
    let scheduler_session_factory = session_factory.clone();
    let scheduler_thread = std::thread::Builder::new()
        .name("moor-admin-scheduler".to_string())
        .spawn(move || scheduler.run(scheduler_session_factory))
        .map_err(|e| eyre!("Failed to spawn scheduler thread: {}", e))?;

    // Sleep a little to let the scheduler finish its start-up jobs.
    std::thread::sleep(Duration::from_secs(1));

    // Run REPL
    let repl_result = repl(
        scheduler_client.clone(),
        session_factory,
        features,
        database_arc.clone(),
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
