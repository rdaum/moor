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

#![recursion_limit = "256"]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
mod feature_args;
mod persistence_probe;
#[cfg_attr(coverage_nightly, coverage(off))]
mod testrun;

use crate::{feature_args::FeatureArgs, testrun::run_test};
use clap::Parser;
use clap_derive::Parser;
use eyre::{bail, eyre};
use moor_common::{
    build,
    model::{CompileError, Named, ObjectRef, PropFlag, TaskPermissions, ValSet, WorldStateSource},
    tasks::{NoopSystemControl, SchedulerError, SessionFactory},
    util::BitEnum,
};
use moor_compiler::emit_compile_error;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_moot::MootOptions;
use moor_objdef::{
    ObjDefSet, ObjectDefinitionLoader, ObjectDumpOptions,
    dump_snapshot_object_definitions_with_options,
};
use moor_textdump::{TextdumpImportOptions, textdump_load};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Var, v_float, v_int};
use std::{
    fs,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tabled::{Table, Tabled, settings::Style};
use tracing::{error, info, warn};

static VERSION_STRING: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});
#[derive(Parser, Debug)]
#[command(version = VERSION_STRING.as_str())]
pub struct Args {
    #[command(flatten)]
    storage_args: moor_db::StorageArgs,

    #[clap(
        long,
        help = "If set, the source to compile lives in an objdef directory, and the compiler should run over the files contained in there."
    )]
    src_objdef_dir: Option<PathBuf>,

    #[clap(
        long,
        help = "If set, output form should be an 'objdef' style directory written to this path."
    )]
    out_objdef_dir: Option<PathBuf>,

    /// Include accepted program baselines in objdef output. Disable for source exports.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    include_baselines: bool,

    /// Prepare upstream baselines from the common ancestor of HEAD and a local remote-tracking ref.
    /// Uses local Git history only; fetch the remote first when needed.
    #[arg(
        long,
        requires = "src_objdef_dir",
        conflicts_with = "baseline_objdef_dir"
    )]
    git_upstream: Option<String>,

    /// Use these objdefs as the accepted baseline while installing src-objdef-dir as live content.
    #[arg(long, requires = "src_objdef_dir")]
    baseline_objdef_dir: Option<PathBuf>,

    #[clap(
        long,
        help = "If set, the source to compile lives in a textdump file, and the compiler should run over the files contained in there."
    )]
    src_textdump: Option<PathBuf>,

    #[clap(
        long,
        help = "Continue importing even when verbs fail to compile. \
                Failed verbs will be created with empty programs. \
                Useful for importing legacy LambdaMOO/ToastStunt databases."
    )]
    continue_on_errors: bool,

    #[command(flatten)]
    feature_args: Option<FeatureArgs>,

    #[clap(
        long,
        help = "Do a test run by executing all verbs prefixed with `test_` in all imported objects"
    )]
    run_tests: Option<bool>,

    #[clap(
        long,
        help = "Filter which unit tests to run. Format: 'objectid:verb_name' (e.g., '#123:test_foo'). \
                The objectid is parsed with Obj::try_from (supports #N or UUID format). \
                If only objectid is given, runs all test_ verbs on that object."
    )]
    test_filter: Option<String>,

    #[clap(
        long = "test-files",
        visible_alias = "test-directory",
        help = "Run integration `moot` tests from directory (recursive) or glob pattern (e.g., 'tests/**/*.moot')"
    )]
    test_files: Option<String>,

    #[clap(
        long,
        help = "The hardcoded object number to use for the wizard character in integration tests."
    )]
    test_wizard: Option<i32>,

    #[clap(
        long,
        help = "The hardcoded object number to use for the programmer character in integration tests."
    )]
    test_programmer: Option<i32>,

    #[clap(
        long,
        help = "The hardcoded object number to use for the non-programmer player character in integration tests."
    )]
    test_player: Option<i32>,

    #[clap(
        long,
        help = "The hardcoded object number to use as the task player for unit-style test_ verbs. Defaults to test-wizard."
    )]
    test_task_player: Option<i32>,

    #[clap(long, help = "Enable debug logging")]
    debug: bool,

    #[clap(
        long = "db",
        visible_alias = "db-path",
        help = "Fjall world directory (default: a temporary directory)"
    )]
    db_path: Option<PathBuf>,

    #[clap(
        long,
        help = "Parse legacy type constant names (INT, OBJ, STR, etc.) as type literals. \
                Use this when importing code that uses the old-style type constants. \
                The code will be migrated to the new TYPE_* format on output."
    )]
    legacy_type_constants: Option<bool>,

    #[clap(
        long,
        default_value = "4",
        help = "Timeout in seconds for individual test verbs (default: 4)"
    )]
    test_timeout: u64,

    #[clap(
        long,
        help = "Arguments to pass to test verbs, as a MOO literal (e.g., '{300}' or '300' for a single value)"
    )]
    test_args: Option<String>,

    #[clap(long, value_parser = clap::value_parser!(u16).range(1..),
        help = "Run each test verb in N phases, appending phase number and previous result to its arguments; wait for application and durability between phases")]
    test_phases: Option<u16>,
}

fn emit_objdef_compile_error(
    path: &str,
    compile_error: &CompileError,
    inline_source: Option<&str>,
) {
    let (source, source_name) = if let Some(source) = inline_source {
        (Some(source.to_string()), format!("{} (verb body)", path))
    } else if path != "<string>" {
        match fs::read_to_string(path) {
            Ok(text) => (Some(text), path.to_string()),
            Err(err) => {
                error!("Failed to read {path} for diagnostic rendering: {err}");
                (None, path.to_string())
            }
        }
    } else {
        (None, path.to_string())
    };

    let use_color = io::stderr().is_terminal();
    eprintln!();
    emit_compile_error(compile_error, source.as_deref(), &source_name, use_color);
}

/// Check if a path has the .moot extension.
fn is_moot_file(path: &std::path::Path) -> bool {
    path.extension().map(|ext| ext == "moot").unwrap_or(false)
}

/// Recursively collect all .moot test files from a directory, skipping hidden directories.
fn collect_moot_files_recursive(path: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !path.is_dir() {
        return Ok(files);
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            // Skip hidden directories (starting with '.')
            if let Some(name) = entry_path.file_name()
                && name.to_string_lossy().starts_with('.')
            {
                continue;
            }
            // Recursively collect from subdirectories
            files.extend(collect_moot_files_recursive(&entry_path)?);
        } else if entry_path.is_file() && is_moot_file(&entry_path) {
            files.push(entry_path);
        }
    }

    Ok(files)
}

/// Collect test files from either a directory path or glob pattern.
/// Returns files sorted depth-first, then alphabetically.
fn collect_moot_files(input: &str) -> Result<Vec<PathBuf>, eyre::Report> {
    let mut files = Vec::new();

    // Check if input contains glob characters
    if input.contains('*') || input.contains('?') || input.contains('[') {
        // Treat as glob pattern
        for entry in glob::glob(input)? {
            let path = entry?;
            if path.is_file() && is_moot_file(&path) {
                files.push(path);
            }
        }
    } else {
        // Treat as directory path
        let path = PathBuf::from(input);
        if !path.exists() {
            // Path doesn't exist, just return empty list (caller will warn)
            return Ok(files);
        }
        if path.is_file() {
            // Single file specified
            if is_moot_file(&path) {
                files.push(path);
            } else {
                return Err(eyre::eyre!("File is not a .moot file: {}", input));
            }
        } else {
            files = collect_moot_files_recursive(&path)?;
        }
    }

    // Sort: depth-first (fewer components first), then alphabetically
    files.sort_by(|a, b| {
        let a_depth = a.components().count();
        let b_depth = b.components().count();
        a_depth.cmp(&b_depth).then_with(|| a.cmp(b))
    });

    Ok(files)
}

#[derive(Tabled)]
struct TestFailure {
    #[tabled(rename = "Test")]
    test: String,
    #[tabled(rename = "Failure")]
    failure: String,
}

fn print_test_failure_summary(failures: &[TestFailure]) {
    if failures.is_empty() {
        return;
    }

    error!("Test failure summary ({}):", failures.len());
    eprintln!("\n{}", Table::new(failures).with(Style::sharp()));
}

fn run_tests(
    test_input: &str,
    player: Obj,
    programmer: Obj,
    wizard: Obj,
    scheduler_client: SchedulerClient,
) -> Result<Vec<TestFailure>, eyre::Report> {
    let moot_options = MootOptions::default()
        .wizard_object(wizard)
        .nonprogrammer_object(player)
        .programmer_object(programmer)
        .init_logging(false);

    let test_files = collect_moot_files(test_input)?;

    if test_files.is_empty() {
        warn!("No .moot test files found matching: {}", test_input);
        return Ok(Vec::new());
    }

    warn!(
        "Running {} integration tests from {}",
        test_files.len(),
        test_input
    );

    let mut failures = Vec::new();
    for path in test_files {
        if let Err(err) = run_test(&moot_options, scheduler_client.clone(), &path) {
            failures.push(TestFailure {
                test: path.display().to_string(),
                failure: err.to_string(),
            });
        }
    }

    Ok(failures)
}

fn main() -> Result<(), eyre::Report> {
    color_eyre::install().unwrap();
    let args: Args = Args::parse();
    let storage = args.storage_args.resolve(
        &moor_db::StorageSettings::default(),
        || args.db_path.clone(),
        args.db_path.is_some(),
        false,
    )?;
    if args.storage_args.install_views(&storage)? {
        println!("Installed PostgreSQL inspection views");
        return Ok(());
    }
    if let Some(report) = args.storage_args.validate(&storage)? {
        println!("{report}");
        return Ok(());
    }
    if let Some(identity) = args.storage_args.initialize(&storage)? {
        println!("Initialized PostgreSQL database {identity}");
        return Ok(());
    }

    moor_common::tracing::init_tracing_simple(args.debug).unwrap_or_else(|e| {
        eprintln!("Unable to configure logging: {e}");
        std::process::exit(1);
    });

    let version = build::PKG_VERSION;
    let commit = build::short_commit();
    info!("mooRc {version}+{commit}");

    // Valid argument scenarios require 1 src and 1 out, no more.
    if args.src_objdef_dir.is_some() && args.src_textdump.is_some() {
        error!("Cannot specify both src-objdef-dir and src-textdump");
        std::process::exit(1);
    }
    if args.src_objdef_dir.is_none() && args.src_textdump.is_none() {
        error!("Must specify either src-objdef_dir or src-textdump");
        std::process::exit(1);
    }

    if (args.git_upstream.is_some() || args.baseline_objdef_dir.is_some())
        && args.out_objdef_dir.is_some()
        && !args.include_baselines
    {
        bail!("baseline preparation requires --include-baselines=true for objdef output");
    }

    info!(storage_backend = %storage.kind(), "Opening world database");
    let (database, _) = TxDB::try_open(
        storage,
        DatabaseConfig::default(),
        args.storage_args
            .merge(&moor_db::StorageSettings::default())
            .persistence_config()?,
    )?;
    // Retain a handle for persistence boundaries around test/benchmark runs.
    let persistence_handle = database.clone();
    let mut loader_interface = match database.loader_client() {
        Ok(loader) => loader,
        Err(e) => {
            error!("Unable to open world database: {e}");
            std::process::exit(1);
        }
    };

    let mut features = FeaturesConfig::default();
    if let Some(fa) = args.feature_args.as_ref() {
        fa.merge_config(&mut features)?;
    }
    features.normalize_deprecated_flags();
    info!("Importing with features: {features:?}");

    // Create compile options from features, then apply legacy flag if set
    let make_compile_options = || {
        let mut opts = features.compile_options();
        opts.legacy_type_constants = args.legacy_type_constants.unwrap_or(false);
        opts
    };

    // Compile phase.
    if let Some(textdump) = args.src_textdump {
        info!("Loading textdump from {:?}", textdump);
        let start = std::time::Instant::now();
        let version = semver::Version::parse(build::PKG_VERSION).expect("Invalid moor version");

        let import_options = TextdumpImportOptions {
            continue_on_compile_errors: args.continue_on_errors,
        };

        if let Err(e) = textdump_load(
            loader_interface.as_mut(),
            textdump.clone(),
            version.clone(),
            make_compile_options(),
            import_options,
        ) {
            error!("Failed to load textdump: {e}");
            std::process::exit(1);
        }

        info!("Loaded textdump in {:?}", start.elapsed());
        loader_interface
            .commit()
            .expect("Failure to commit loaded database...");
        info!("Committed. Total time: {:?}", start.elapsed());
    } else if let Some(objdef_dir) = args.src_objdef_dir {
        let start = std::time::Instant::now();
        let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());

        let options = moor_objdef::ObjDefLoaderOptions::default();
        let compile_options = make_compile_options();
        let prepared = ObjDefSet::read_directory_with_baseline(
            &compile_options,
            &objdef_dir,
            args.git_upstream.as_deref(),
            args.baseline_objdef_dir.as_deref(),
        );
        let commit = match prepared.and_then(|set| od.load_objdef_set(set, options)) {
            Ok(results) => {
                info!(
                    "Imported {} objects w/ {} verbs, {} properties and {} property overrides",
                    results.loaded_objects.len(),
                    results.num_loaded_verbs,
                    results.num_loaded_property_definitions,
                    results.num_loaded_property_overrides
                );

                results.commit
            }
            Err(e) => {
                if let Some((file_path, compile_error, verb_source)) = e.compile_error() {
                    let source_to_use = if !verb_source.is_empty() {
                        Some(verb_source)
                    } else {
                        None
                    };
                    emit_objdef_compile_error(file_path, compile_error, source_to_use);
                    error!("Object load failed");
                    return Err(e.into());
                }
                error!("Object load failure @ {}", e.source());
                error!("{:#}", e);
                return Err(e.into());
            }
        };
        info!("Loaded objdef directory in {:?}", start.elapsed());
        if commit {
            loader_interface
                .commit()
                .expect("Failure to commit loaded database...");
            info!("Committed. Total time: {:?}", start.elapsed());
        } else {
            info!("Object loader requested rollback (dry-run).")
        }
    }

    info!(
        "Database loaded. out_objdef_dir?: {:?} test_files?: {:?} run_tests?: {:?}",
        args.out_objdef_dir, args.test_files, args.run_tests
    );

    // Dump phase.
    if let Some(dirdump_path) = args.out_objdef_dir {
        let Some(output_name) = dirdump_path.file_name() else {
            bail!("Objdef output path must name a directory");
        };
        if dirdump_path.exists() {
            bail!(
                "Objdef output path already exists: {}",
                dirdump_path.display()
            );
        }
        let mut in_progress_name = output_name.to_os_string();
        in_progress_name.push(".in-progress");
        let in_progress_path = dirdump_path.with_file_name(in_progress_name);
        if in_progress_path.exists() {
            bail!(
                "Incomplete objdef output path already exists: {}",
                in_progress_path.display()
            );
        };

        let loader_interface = database
            .create_snapshot()
            .map_err(|e| eyre!("Unable to create database snapshot: {e}"))?;
        info!(path = ?in_progress_path, "Dumping objects");
        let stats = dump_snapshot_object_definitions_with_options(
            loader_interface.as_ref(),
            &in_progress_path,
            ObjectDumpOptions {
                include_baselines: args.include_baselines,
            },
        )?;
        fs::rename(&in_progress_path, &dirdump_path).map_err(|e| {
            eyre!(
                "Failed to finalize objdef export from {} to {}: {e}",
                in_progress_path.display(),
                dirdump_path.display()
            )
        })?;

        info!(?dirdump_path, ?stats, "Objdefdump written.");
    }

    if args.run_tests != Some(true) && args.test_files.is_none() {
        info!("No tests to run. Exiting.");
        info!("Dropping database to trigger shutdown...");
        // Explicitly drop database to ensure clean shutdown with barrier waiting
        drop(database);
        info!("Database dropped, moorc exiting");
        return Ok(());
    }

    if let Err(e) =
        moor_kernel::initialize_server_symmetric_key(*b"moor-test-server-symmetric-key!!")
    {
        warn!("Server symmetric key already initialized for test run: {e}");
    }

    let wizard = Obj::mk_id(args.test_wizard.expect("Must specify wizard object"));
    let wizard_permissions = TaskPermissions::new(wizard, BitEnum::new());

    let tasks_db = Box::new(NoopTasksDb {});
    let test_version = semver::Version::new(0, 1, 0);

    // Qualification boundary: ensure the imported world has applied to storage before tests or
    // benchmark producers run, so seed work cannot bleed into the measured section.
    persistence_handle
        .wait_for_persistence_with_deadline(Duration::from_secs(args.test_timeout.max(60)))
        .map_err(|e| eyre!("Initial world state did not apply to storage: {e}"))?;
    info!("Initial world state applied before test run");

    let db = Box::new(database);

    // If running integration tests, we need to create a scratch property on #0 that is used for tests to stick transient
    // values in
    if args.test_files.is_some() {
        let mut tx = db.new_world_state().unwrap();
        tx.define_property(
            &wizard_permissions,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            Symbol::mk("scratch"),
            &wizard,
            PropFlag::rw(),
            None,
        )
        .expect("Could not create scratch object for integration test execution");
        tx.commit().unwrap();
    }

    // Before handing off the DB ot the scheduler, we need to find a list of all potential tests
    // to run.
    let mut unit_tests = vec![];
    if args.run_tests == Some(true) {
        let tx = db.new_world_state().unwrap();
        let mo = tx.max_object(&wizard_permissions).unwrap().as_u64();
        info!("Scanning objects 0..{} for tests", mo);
        for o in 0..=mo {
            let o = Obj::mk_id(o as i32);
            if let Ok(verbs) = tx.verbs(&wizard_permissions, &o) {
                for verb in verbs.iter() {
                    for name in verb.names() {
                        if name.as_arc_str().starts_with("test_") {
                            unit_tests.push((o, *name));
                        }
                    }
                }
            }
        }
        info!("Found {} tests", unit_tests.len());

        // Apply test filter if specified
        if let Some(ref filter) = args.test_filter {
            let (filter_obj, filter_verb): (Option<Obj>, Option<&str>) =
                if let Some((obj_part, verb_part)) = filter.split_once(':') {
                    let obj = Obj::try_from(obj_part).unwrap_or_else(|e| {
                        error!("Failed to parse object id '{}': {}", obj_part, e);
                        std::process::exit(1);
                    });
                    (Some(obj), Some(verb_part))
                } else {
                    // Only object id specified, run all test_ verbs on that object
                    let obj = Obj::try_from(filter.as_str()).unwrap_or_else(|e| {
                        error!("Failed to parse object id '{}': {}", filter, e);
                        std::process::exit(1);
                    });
                    (Some(obj), None)
                };

            let before_count = unit_tests.len();
            unit_tests.retain(|(o, verb)| {
                let obj_matches = filter_obj.as_ref().is_none_or(|fo| fo == o);
                let verb_matches = filter_verb.is_none_or(|fv| verb.as_arc_str().as_str() == fv);
                obj_matches && verb_matches
            });
            info!(
                "Filtered tests from {} to {} (filter: {:?})",
                before_count,
                unit_tests.len(),
                filter
            );
        }
    }

    let config = Config {
        features: Arc::new(features),
        ..Default::default()
    };
    let scheduler = Scheduler::new(
        test_version,
        db,
        tasks_db,
        Arc::new(config),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let scheduler_client = scheduler.client().unwrap();
    let session_factory = Arc::new(crate::testrun::NoopSessionFactory {});
    let test_session_factory = session_factory.clone();
    let scheduler_loop_jh = scheduler
        .start(session_factory)
        .expect("Failed to start scheduler");

    // Parse test args if provided
    let test_args_list = if let Some(ref args_str) = args.test_args {
        // Try parsing as a float first, then int, then as a MOO list literal
        if let Ok(f) = args_str.parse::<f64>() {
            List::mk_list(&[v_float(f)])
        } else if let Ok(i) = args_str.parse::<i64>() {
            List::mk_list(&[v_int(i)])
        } else if args_str.starts_with('{') && args_str.ends_with('}') {
            // Simple list of numbers: {300} or {1, 2, 3}
            let inner = &args_str[1..args_str.len() - 1];
            let vals: Vec<Var> = inner
                .split(',')
                .filter_map(|s| {
                    let s = s.trim();
                    if let Ok(f) = s.parse::<f64>() {
                        Some(v_float(f))
                    } else if let Ok(i) = s.parse::<i64>() {
                        Some(v_int(i))
                    } else {
                        warn!("Could not parse test arg: {}", s);
                        None
                    }
                })
                .collect();
            List::mk_list(&vals)
        } else {
            warn!("Could not parse test-args: {}", args_str);
            List::mk_list(&[])
        }
    } else {
        List::mk_list(&[])
    };

    let mut test_failures = Vec::new();

    // Run unit tests
    if args.run_tests == Some(true) && !unit_tests.is_empty() {
        let test_task_player = args.test_task_player.map(Obj::mk_id).unwrap_or(wizard);
        'outer: for (o, verb) in unit_tests {
            let test_name = format!("{o}:{verb}");
            info!("Running {}:{}....", o, verb);
            let mut previous_result = moor_var::v_none();
            for phase in 1..=args.test_phases.unwrap_or(1) {
                let _probe = args.test_phases.map(|_| {
                    persistence_probe::PersistenceProbe::start(
                        persistence_handle.clone(),
                        test_name.clone(),
                        phase,
                    )
                });
                let phase_args = if args.test_phases.is_some() {
                    let mut values: Vec<_> = test_args_list.iter().collect();
                    values.extend([v_int(i64::from(phase)), previous_result.clone()]);
                    List::mk_list(&values)
                } else {
                    test_args_list.clone()
                };
                let session = test_session_factory
                    .clone()
                    .mk_background_session(&test_task_player)
                    .expect("Failed to create session");
                let handle = scheduler_client
                    .submit_verb_task(
                        &test_task_player,
                        &ObjectRef::Id(o),
                        verb,
                        phase_args,
                        "".into(),
                        &wizard,
                        session,
                    )
                    .expect("Failed to submit task");
                let result_value = loop {
                    let result = match handle
                        .receiver()
                        .recv_timeout(Duration::from_secs(args.test_timeout))
                    {
                        Ok(result) => result,
                        Err(e) => {
                            let failure = format!("timed out after {}s: {e}", args.test_timeout);
                            error!("Test {test_name} {failure}");
                            test_failures.push(TestFailure {
                                test: test_name,
                                failure,
                            });
                            continue 'outer;
                        }
                    };
                    match result {
                        (_, Ok(TaskNotification::Result(rv))) => break rv,
                        (_, Ok(TaskNotification::Suspended)) => continue,
                        (_, Err(e)) => match e {
                            SchedulerError::TaskAbortedException(e) => {
                                let failure = format!("aborted: {}", e.error);
                                error!("Test {test_name} {failure}");
                                for l in e.backtrace {
                                    let Some(s) = l.as_string() else {
                                        continue;
                                    };
                                    error!("{s}");
                                }
                                test_failures.push(TestFailure {
                                    test: test_name,
                                    failure,
                                });
                                continue 'outer;
                            }
                            _ => {
                                let failure = format!("failed: {e:?}");
                                error!("Test {test_name} {failure}");
                                test_failures.push(TestFailure {
                                    test: test_name,
                                    failure,
                                });
                                continue 'outer;
                            }
                        },
                    }
                };
                // Result must be non-Error
                if let Some(e) = result_value.as_error() {
                    let failure = format!("returned error: {e:?}");
                    error!("Test {test_name} {failure}");
                    test_failures.push(TestFailure {
                        test: test_name,
                        failure,
                    });
                    continue 'outer;
                }
                previous_result = result_value;
                if args.test_phases.is_some() {
                    let started = std::time::Instant::now();
                    let applied = persistence_handle
                        .wait_for_persistence_with_deadline(Duration::from_secs(args.test_timeout));
                    let applied_seconds = started.elapsed().as_secs_f64();
                    let started = std::time::Instant::now();
                    let durable = applied.as_ref().map_err(|e| e.to_string()).and_then(|_| {
                        persistence_handle
                            .wait_for_durability(Duration::from_secs(args.test_timeout))
                            .map_err(|e| e.to_string())
                    });
                    let durable_seconds = started.elapsed().as_secs_f64();
                    info!(%test_name, phase, applied_seconds, durable_seconds, "PERSISTENCE_BOUNDARY");
                    if let Err(error) = durable {
                        test_failures.push(TestFailure {
                            test: test_name,
                            failure: format!("phase {phase} persistence: {error}"),
                        });
                        continue 'outer;
                    }
                }
            }
            info!("Test {}:{} passed", o, verb);
        }
    }

    // Perform integration test run.
    if let Some(ref test_files) = args.test_files {
        let player = Obj::mk_id(args.test_player.expect("Must specify player object"));
        let programmer = Obj::mk_id(
            args.test_programmer
                .expect("Must specify programmer object"),
        );
        run_tests(
            test_files,
            player,
            programmer,
            wizard,
            scheduler_client.clone(),
        )?
        .into_iter()
        .for_each(|failure| test_failures.push(failure));
    }

    scheduler_client
        .submit_shutdown("Test runs are done")
        .expect("Failed to shut down scheduler");
    scheduler_loop_jh
        .join()
        .expect("Failed to join() scheduler");

    if !test_failures.is_empty() {
        print_test_failure_summary(&test_failures);
        std::process::exit(1);
    }

    Ok(())
}
