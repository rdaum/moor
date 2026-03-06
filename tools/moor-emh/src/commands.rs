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

use crate::{
    AdminSessionFactory, ConsoleSession, MooAdminHelper,
    parse::{parse_flags, parse_objref_with_scheduler, parse_propref, parse_verbref},
};
use crossterm::style::Stylize;
use eyre::{Report, bail, eyre};
use moor_common::model::{Named, ObjectKind, ValSet};
use moor_compiler::to_literal;
use moor_common::tasks::{SchedulerError, SessionFactory};
use moor_kernel::{
    SchedulerClient,
    config::FeaturesConfig,
    tasks::TaskNotification,
};
use moor_var::{Obj, Symbol};
use rustyline::Editor;
use rustyline::error::ReadlineError;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tabled::{Table, Tabled, settings::Style};
use tracing::info;

pub(crate) fn print_help() {
    println!("{}", "Emergency Medical Hologram - Available Procedures".bold());
    println!();
    println!("Database Operations");
    println!("  ;EXPR                              Evaluate MOO expression, print result");
    println!("  ;;CODE                             Execute MOO code block, print result");
    println!("  get #OBJ.PROP                      Read property value");
    println!("  set #OBJ.PROP VALUE                Write property value");
    println!("  props #OBJ                         List all properties on object");
    println!("  verbs #OBJ                         List all verbs on object");
    println!("  prog #OBJ:VERB                     Program a verb (multi-line)");
    println!("  list #OBJ:VERB                     Show verb code");
    println!("  dump #OBJ [--file PATH]            Dump object to file or console");
    println!("  load [--file PATH] [options]       Load object from file or console");
    println!("  reload [#OBJ] [--file PATH]        Replace object contents from file or console");
    println!("  su #OBJ                            Switch to different player object");
    println!("  help, ?                            Show this help");
    println!("  quit, exit                         Save and exit");
    println!();
    println!("Load options");
    println!("  --file PATH                        Load from file instead of stdin");
    println!("  --constants PATH                   MOO file with constant definitions");
    println!("  --dry-run                          Validate without making changes");
    println!("  --conflict-mode MODE               clobber | skip | detect");
    println!("  --as SPEC                          new | anonymous | uuid | #OBJ");
    println!("  --return-conflicts                 Return detailed conflict information");
}

pub(crate) fn print_success(message: impl AsRef<str>) {
    println!("{} {}", "✓".green().bold(), message.as_ref());
}

pub(crate) fn print_warning(message: impl AsRef<str>) {
    println!("{} {}", "!".yellow().bold(), message.as_ref());
}

/// Format a SchedulerError for user-friendly display
pub(crate) fn format_scheduler_error(err: &SchedulerError) -> String {
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

pub(crate) fn exec_code_block(
    scheduler_client: &SchedulerClient,
    session_factory: &Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    wizard: &Obj,
    code: &str,
) -> Result<(), Report> {
    let session = session_factory
        .clone()
        .mk_background_session(wizard)
        .map_err(|e| eyre!("Failed to create session: {:?}", e))?;

    let handle = scheduler_client
        .submit_eval_task(
            wizard,
            wizard,
            code.trim().to_string(),
            None,
            session,
            features,
        )
        .map_err(|e| eyre!("Failed to submit eval task: {:?}", e))?;

    let result = loop {
        match handle.receiver().recv_timeout(Duration::from_secs(30)) {
            Ok((_task_id, Ok(TaskNotification::Suspended))) => continue,
            other => break other,
        }
    };

    match result {
        Ok((_task_id, Ok(TaskNotification::Result(value)))) => {
            println!("=> {}", to_literal(&value));
            Ok(())
        }
        Ok((_task_id, Err(e))) => bail!("Execution failed: {}", format_scheduler_error(&e)),
        Err(_) => bail!("Task timed out after 30 seconds"),
        _ => bail!("Task was replaced by scheduler"),
    }
}

pub(crate) fn eval_expression(
    scheduler_client: &SchedulerClient,
    session_factory: &Arc<AdminSessionFactory>,
    features: Arc<FeaturesConfig>,
    wizard: &Obj,
    expr: &str,
) -> Result<(), Report> {
    let code = format!("return {expr};");
    exec_code_block(scheduler_client, session_factory, features, wizard, &code)
}

/// Get a property value directly from the database
pub(crate) fn cmd_get(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<(), Report> {
    let args = args.trim();

    if args.starts_with('$') && !args.contains('.') {
        let prop_name = &args[1..];
        if prop_name.is_empty() {
            bail!("Usage: get $property or get #OBJ.PROP");
        }

        let prop = Symbol::mk(prop_name);
        let value = scheduler_client
            .request_system_property(
                wizard,
                &moor_common::model::ObjectRef::Id(Obj::mk_id(0)),
                prop,
            )
            .map_err(|e| eyre!("Failed to retrieve ${}: {:?}", prop_name, e))?;

        println!("${} = {}", prop_name, to_literal(&value));
        return Ok(());
    }

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
        .submit_eval_task(wizard, wizard, eval_code, None, session, features)
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

    println!(
        "{}.{} = {}",
        obj.to_literal(),
        prop.as_string(),
        to_literal(&value)
    );
    Ok(())
}

/// Set a property value
pub(crate) fn cmd_set(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
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
        .submit_eval_task(wizard, wizard, code, None, session, features)
        .map_err(|e| eyre!("Failed to submit task: {:?}", e))?;

    let (_task_id, result) = handle
        .receiver()
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| eyre!("Task timed out"))?;

    let Err(e) = result else {
        print_success(format!(
            "Property {}.{} set successfully",
            obj.to_literal(),
            prop.as_string()
        ));
        return Ok(());
    };

    eprintln!("{}", format_scheduler_error(&e));
    bail!("Failed to set property");
}

/// List all properties on an object
pub(crate) fn cmd_props(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<(), Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Listing properties on {}", obj.to_literal());

    use moor_common::model::ObjectRef;
    let obj_ref = ObjectRef::Id(obj);
    let props = scheduler_client
        .request_properties(wizard, wizard, &obj_ref, false)
        .map_err(|e| eyre!("Failed to get properties: {:?}", e))?;

    #[derive(Tabled)]
    struct PropertyRow {
        property: String,
    }

    let rows: Vec<PropertyRow> = props
        .iter()
        .map(|(prop, _)| PropertyRow {
            property: prop.name().as_string().to_string(),
        })
        .collect();
    println!("{}", format!("Properties on {}", obj.to_literal()).bold());
    println!("{}", Table::new(rows).with(Style::rounded()));
    println!("{} properties", props.len());
    Ok(())
}

/// List all verbs on an object
pub(crate) fn cmd_verbs(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<(), Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Listing verbs on {}", obj.to_literal());

    use moor_common::model::ObjectRef;
    let obj_ref = ObjectRef::Id(obj);
    let verbs = scheduler_client
        .request_verbs(wizard, wizard, &obj_ref, false)
        .map_err(|e| eyre!("Failed to get verbs: {:?}", e))?;

    #[derive(Tabled)]
    struct VerbRow {
        verb: String,
    }

    let rows: Vec<VerbRow> = verbs
        .iter()
        .filter_map(|verb| {
            verb.names().first().map(|name| VerbRow {
                verb: name.as_string().to_string(),
            })
        })
        .collect();
    println!("{}", format!("Verbs on {}", obj.to_literal()).bold());
    println!("{}", Table::new(rows).with(Style::rounded()));
    println!("{} verbs", verbs.len());
    Ok(())
}

/// Program a verb with multi-line input
pub(crate) fn cmd_prog(
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

    let intro = format!(
        "Programming {}:{}\n\nEnter code (type `.` on a line by itself to finish):",
        obj.to_literal(),
        verb.as_string()
    );
    println!("{intro}");

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
        .submit_eval_task(wizard, wizard, moo_code, None, session, features)
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
        print_success(format!(
            "Verb {}:{} programmed successfully ({} lines)",
            obj.to_literal(),
            verb.as_string(),
            code_lines.len()
        ));
        return Ok(());
    };

    eprintln!("{}", format_scheduler_error(&e));
    bail!("Failed to program verb");
}

/// Switch to a different player object
pub(crate) fn cmd_su(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<Obj, Report> {
    let obj = parse_objref_with_scheduler(args.trim(), Some(scheduler_client), Some(wizard))?;
    info!("Attempting to switch to object {}", obj.to_literal());

    // Check if object is valid and has User flag by evaluating MOO code
    let check_code = format!("return {{valid({0}), is_player({0})}};", obj.to_literal());

    let session = Arc::new(ConsoleSession::new());
    let features = Arc::new(FeaturesConfig::default());

    let handle = scheduler_client
        .submit_eval_task(wizard, wizard, check_code, None, session, features)
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

    print_success(format!("Switched to player {}", obj.to_literal()));
    Ok(obj)
}

/// List a verb's code
pub(crate) fn cmd_list(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<(), Report> {
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
        .submit_eval_task(wizard, wizard, code, None, session, features)
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

    println!(
        "{} {}:{}",
        "Verb".bold(),
        obj.to_literal(),
        verb.as_string()
    );
    println!();

    for line in lines.iter() {
        let Some(s) = line.as_string() else {
            continue;
        };
        println!("{s}");
    }
    println!();
    println!("{} lines", lines.len());
    Ok(())
}

/// Dump an object definition
pub(crate) fn cmd_dump(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
) -> Result<(), Report> {
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
        .submit_eval_task(wizard, wizard, code, None, session, features)
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

    // If filename provided, write to file; otherwise print to console
    if let Some(path) = filename {
        let mut file =
            std::fs::File::create(&path).map_err(|e| eyre!("Failed to create file {:?}: {}", path, e))?;

        for line in lines.iter() {
            let Some(s) = line.as_string() else {
                continue;
            };
            writeln!(file, "{s}")
                .map_err(|e| eyre!("Failed to write to file {:?}: {}", path, e))?;
        }

        print_success(format!(
            "Object {} dumped to {}",
            obj.to_literal(),
            path.display()
        ));
        println!("{} lines written", lines.len());
    } else {
        println!("{}", format!("Object Definition: {}", obj.to_literal()).bold());
        println!();

        // Print the definition lines
        for line in lines.iter() {
            let Some(s) = line.as_string() else {
                continue;
            };
            println!("{s}");
        }

        println!();
        println!("{} lines dumped", lines.len());
    }

    Ok(())
}

/// Load an object definition
pub(crate) fn cmd_load(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
    use moor_objdef::{ConflictMode, ObjDefLoaderOptions};

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
        println!("Loading object definition");
        println!("Paste object definition (type `.` on a line by itself to finish):");

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
        if result.commit {
            print_success("Load completed successfully");
            println!("Loaded objects: {}", result.loaded_objects.len());
            println!("Conflicts: {}", result.conflicts.len());
            println!("{} lines processed", definition_lines.len());
        } else {
            print_warning("Load would have conflicts (dry-run or detect mode)");
            println!("Would load: {}", result.loaded_objects.len());
            println!("Conflicts: {}", result.conflicts.len());
            println!("{} lines processed", definition_lines.len());
        }
    } else if result.loaded_objects.is_empty() {
        bail!("No objects were loaded");
    } else {
        let obj = result.loaded_objects[0];
        print_success(format!("Object {} loaded successfully", obj.to_literal()));
        println!("{} lines processed", definition_lines.len());
    }

    Ok(())
}

/// Reload an object definition, completely replacing its contents
pub(crate) fn cmd_reload(
    scheduler_client: &SchedulerClient,
    wizard: &Obj,
    args: &str,
    rl: &mut Editor<MooAdminHelper, rustyline::history::DefaultHistory>,
) -> Result<(), Report> {
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
                "Reloading object {}\n\nPaste object definition (type `.` on a line by itself to finish):",
                obj.to_literal()
            )
        } else {
            "Reloading object\n\nPaste object definition (type `.` on a line by itself to finish):"
                .to_string()
        };
        println!("{intro}");

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
    print_success(format!("Object {} reloaded successfully", obj.to_literal()));
    println!("{} lines processed", definition_lines.len());

    Ok(())
}
