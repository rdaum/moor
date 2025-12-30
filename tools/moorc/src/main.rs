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
#![recursion_limit = "256"]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
mod feature_args;
#[cfg_attr(coverage_nightly, coverage(off))]
mod testrun;

use crate::{feature_args::FeatureArgs, testrun::run_test};
use clap::Parser;
use clap_derive::Parser;
use moor_common::{
    build,
    model::{CompileError, Named, ObjectRef, PropFlag, ValSet, WorldStateSource},
    tasks::{NoopSystemControl, SchedulerError, SessionFactory},
};
use moor_compiler::emit_compile_error;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::{
    SchedulerClient,
    config::{Config, FeaturesConfig},
    tasks::{NoopTasksDb, TaskNotification, scheduler::Scheduler},
};
use moor_moot::MootOptions;
use moor_objdef::{ObjectDefinitionLoader, collect_object_definitions, dump_object_definitions};
use moor_textdump::{TextdumpImportOptions, textdump_load};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol};
use once_cell::sync::Lazy;
use std::{
    fs,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tracing::{error, info, warn};

static VERSION_STRING: Lazy<String> = Lazy::new(|| {
    format!(
        "{} (commit: {})",
        env!("CARGO_PKG_VERSION"),
        moor_common::build::short_commit()
    )
});
#[derive(Parser, Debug)]
#[command(version = VERSION_STRING.as_str())]
pub struct Args {
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
        help = "Run the set of integration `moot` tests defined in the defined directory"
    )]
    test_directory: Option<PathBuf>,

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

    #[clap(long, help = "Enable debug logging")]
    debug: bool,

    #[clap(
        long,
        help = "Path to database directory (if not specified, uses a temporary directory)"
    )]
    db_path: Option<PathBuf>,

    #[clap(
        long,
        help = "Parse legacy type constant names (INT, OBJ, STR, etc.) as type literals. \
                Use this when importing code that uses the old-style type constants. \
                The code will be migrated to the new TYPE_* format on output."
    )]
    legacy_type_constants: Option<bool>,
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

fn run_tests(
    test_directory: &PathBuf,
    player: Obj,
    programmer: Obj,
    wizard: Obj,
    scheduler_client: SchedulerClient,
) -> Result<(), eyre::Report> {
    let moot_options = MootOptions::default()
        .wizard_object(wizard)
        .nonprogrammer_object(player)
        .programmer_object(programmer)
        .init_logging(false);

    // Iterate all the .moot tests and run them in the context of the current database.
    warn!("Running integration tests in {}", test_directory.display());
    let Ok(dir) = std::fs::read_dir(test_directory) else {
        error!(
            "Failed to read test directory: {}",
            test_directory.display()
        );
        return Ok(());
    };
    for entry in dir {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.path();
        let Some(extension) = path.extension() else {
            continue;
        };

        if extension != "moot" {
            continue;
        }

        run_test(&moot_options, scheduler_client.clone(), &path);
    }

    Ok(())
}

fn main() -> Result<(), eyre::Report> {
    color_eyre::install().unwrap();
    let args: Args = Args::parse();

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

    // Actual binary database is either in a specified path or tmpdir.
    // Keep the TempDir alive for the entire scope if we're using a temp directory.
    let _temp_dir_guard;
    let db_path = if let Some(ref path) = args.db_path {
        info!("Using specified database path: {}", path.display());
        path.as_path()
    } else {
        _temp_dir_guard = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                error!("Failed to create temporary directory: {}", e);
                std::process::exit(1);
            }
        };
        info!(
            "Using temporary database at {}",
            _temp_dir_guard.path().display()
        );
        _temp_dir_guard.path()
    };

    let (database, _) = TxDB::open(Some(db_path), DatabaseConfig::default());
    let mut loader_interface = match database.loader_client() {
        Ok(loader) => loader,
        Err(e) => {
            error!("Unable to open database at {}: {}", db_path.display(), e);
            std::process::exit(1);
        }
    };

    let mut features = FeaturesConfig::default();
    if let Some(fa) = args.feature_args.as_ref() {
        fa.merge_config(&mut features)?;
    }
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
        let commit =
            match od.load_objdef_directory(make_compile_options(), objdef_dir.as_ref(), options) {
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
                        return Ok(());
                    }
                    error!("Object load failure @ {}", e.source());
                    error!("{:#}", e);
                    return Ok(());
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
        "Database loaded. out_objdef_dir?: {:?} test_directory?: {:?} run_tests?: {:?}",
        args.out_objdef_dir, args.test_directory, args.run_tests
    );

    // Dump phase.
    if let Some(dirdump_path) = args.out_objdef_dir {
        let Ok(loader_interface) = database.create_snapshot() else {
            error!("Unable to open database at {}", db_path.display());
            return Ok(());
        };

        info!("Collecting objects for dump...");
        let objects = collect_object_definitions(loader_interface.as_ref())?;
        info!("Dumping objects to {dirdump_path:?}");
        dump_object_definitions(&objects, &dirdump_path)?;

        info!(?dirdump_path, "Objdefdump written.");
    }

    if args.run_tests != Some(true) && args.test_directory.is_none() {
        info!("No tests to run. Exiting.");
        info!("Dropping database to trigger shutdown...");
        // Explicitly drop database to ensure clean shutdown with barrier waiting
        drop(database);
        info!("Database dropped, moorc exiting");
        return Ok(());
    }

    let wizard = Obj::mk_id(args.test_wizard.expect("Must specify wizard object"));

    let tasks_db = Box::new(NoopTasksDb {});
    let test_version = semver::Version::new(0, 1, 0);
    let db = Box::new(database);

    // If running integration tests, we need to create a scratch property on #0 that is used for tests to stick transient
    // values in
    if args.test_directory.is_some() {
        let mut tx = db.new_world_state().unwrap();
        tx.define_property(
            &wizard,
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
        let mo = tx.max_object(&wizard).unwrap().as_u64();
        info!("Scanning objects 0..{} for tests", mo);
        for o in 0..=mo {
            let o = Obj::mk_id(o as i32);
            if let Ok(verbs) = tx.verbs(&wizard, &o) {
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
    let scheduler_loop_jh = std::thread::Builder::new()
        .name("moor-scheduler".to_string())
        .spawn(move || scheduler.run(session_factory.clone()))
        .expect("Failed to spawn scheduler");

    // Run unit tests
    if args.run_tests == Some(true) && !unit_tests.is_empty() {
        'outer: for (o, verb) in unit_tests {
            info!("Running {}:{}....", o, verb);
            let session = test_session_factory
                .clone()
                .mk_background_session(&wizard)
                .expect("Failed to create session");
            let handle = scheduler_client
                .submit_verb_task(
                    &wizard,
                    &ObjectRef::Id(o),
                    verb,
                    List::mk_list(&[]),
                    "".to_string(),
                    &wizard,
                    session,
                )
                .expect("Failed to submit task");
            let result_value = loop {
                let result = handle
                    .receiver()
                    .recv_timeout(Duration::from_secs(4))
                    .expect("Test timed out");
                match result {
                    (_, Ok(TaskNotification::Result(rv))) => break rv,
                    (_, Ok(TaskNotification::Suspended)) => continue,
                    (_, Err(e)) => match e {
                        SchedulerError::TaskAbortedException(e) => {
                            error!("Test {}:{} aborted: {}", o, verb, e.error);
                            for l in e.backtrace {
                                let Some(s) = l.as_string() else {
                                    continue;
                                };
                                error!("{s}");
                            }
                            continue 'outer;
                        }
                        _ => {
                            error!("Test {}:{} failed: {:?}", o, verb, e);
                            continue 'outer;
                        }
                    },
                }
            };
            // Result must be non-Error
            if let Some(e) = result_value.as_error() {
                error!("Test {}:{} failed: {:?}", o, verb, e);
                continue;
            }
            info!("Test {}:{} passed", o, verb);
        }
    }

    // Perform integration test run.
    if let Some(test_directory) = args.test_directory {
        let player = Obj::mk_id(args.test_player.expect("Must specify player object"));
        let programmer = Obj::mk_id(
            args.test_programmer
                .expect("Must specify programmer object"),
        );
        run_tests(
            &test_directory,
            player,
            programmer,
            wizard,
            scheduler_client.clone(),
        )?;
    }

    scheduler_client
        .submit_shutdown("Test runs are done")
        .expect("Failed to shut down scheduler");
    scheduler_loop_jh
        .join()
        .expect("Failed to join() scheduler");

    Ok(())
}
