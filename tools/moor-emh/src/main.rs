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

//! Emergency administration tool for mooR databases.
//! Provides direct database access when normal logins are unavailable.

mod commands;
mod parse;
mod repl;
mod wizard;

use clap::Parser;
use clap_derive::Parser as DeriveParser;
use crossterm::style::Stylize;
use eyre::{Report, bail, eyre};
use fs2::FileExt;
use moor_common::{
    build,
    model::{Named, ValSet},
    tasks::{
        ConnectionDetails, Event, NarrativeEvent, NoopSystemControl, Session, SessionError,
        SessionFactory,
    },
};
use moor_compiler::to_literal;
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, scheduler::Scheduler},
};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var};
use repl::repl;
use rustyline::ExternalPrinter;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::time::Duration;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};
use tracing::{error, info};
use tracing_subscriber::fmt::MakeWriter;
use uuid::Uuid;
use wizard::find_wizard;

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

static VERSION_STRING: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});

#[derive(DeriveParser, Debug)]
#[command(name = "moor-emh")]
#[command(
    version = VERSION_STRING.as_str(),
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
        help = "Object reference to use as wizard for commands (defaults to deterministic wizard scan)"
    )]
    wizard: Option<String>,

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

/// A session that outputs to the console (stdout).
/// Buffers narrative events and prints them on commit.
pub(crate) struct ConsoleSession {
    buffer: Mutex<Vec<NarrativeEvent>>,
}

impl ConsoleSession {
    pub(crate) fn new() -> Self {
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
                    eprintln!("{} {}", "error:".red().bold(), exception.error);
                    for line in &exception.stack {
                        eprintln!("  {}", to_literal(line));
                    }
                }
                Event::Present(_)
                | Event::Unpresent(_)
                | Event::Data { .. }
                | Event::SetConnectionOption { .. } => {
                    // Ignore presentation and connection option events in console mode
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

    fn log_event(&self, _player: Obj, _event: Box<NarrativeEvent>) -> Result<(), SessionError> {
        // Log-only events are ignored in console mode - no persistent event log
        Ok(())
    }

    fn send_system_msg(&self, _player: Obj, msg: &str) -> Result<(), SessionError> {
        println!("{} {}", "system:".blue().bold(), msg);
        Ok(())
    }

    fn notify_shutdown(&self, msg: Option<String>) -> Result<(), SessionError> {
        if let Some(msg) = msg {
            println!("{} Server shutting down: {msg}", "system:".blue().bold());
        } else {
            println!("{} Server shutting down", "system:".blue().bold());
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
pub(crate) struct AdminSessionFactory;

impl SessionFactory for AdminSessionFactory {
    fn mk_background_session(
        self: Arc<Self>,
        _player: &Obj,
    ) -> Result<Arc<dyn Session>, SessionError> {
        Ok(Arc::new(ConsoleSession::new()))
    }
}

/// Helper for rustyline with tab completion
pub(crate) struct MooAdminHelper {
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

            let Ok(obj) = parse::parse_objref_with_scheduler(
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
            let Ok(obj) = parse::parse_objref_with_scheduler(
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
    let wizard = find_wizard(database.as_ref(), args.wizard.as_deref())?;

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

    // Start scheduler (spawns timer + worker threads internally)
    let scheduler_thread = scheduler.start(session_factory.clone());

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
