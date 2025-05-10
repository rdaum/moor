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

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod testrun;

use crate::testrun::run_test;
use bincommon::FeatureArgs;
use clap::Parser;
use clap_derive::Parser;
use moor_common::build;
use moor_common::model::{Named, ObjectRef, PropFlag, ValSet, WorldStateSource};
use moor_common::tasks::SchedulerError;
use moor_common::tasks::{NoopSystemControl, SessionFactory};
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::config::{Config, FeaturesConfig, ImportExportConfig};
use moor_kernel::tasks::scheduler::Scheduler;
use moor_kernel::tasks::{NoopTasksDb, TaskResult};
use moor_moot::MootOptions;
use moor_objdef::{ObjectDefinitionLoader, collect_object_definitions, dump_object_definitions};
use moor_textdump::{EncodingMode, TextdumpWriter, make_textdump, textdump_load};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Variant};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Parser, Debug)] // requires `derive` feature
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
        help = "The output should be a LambdaMOO style 'textdump' file located at this path."
    )]
    out_textdump: Option<PathBuf>,

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
}

fn main() {
    color_eyre::install().unwrap();
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_span_events(FmtSpan::NONE)
        .with_target(false)
        .with_file(false)
        .with_target(false)
        .with_line_number(false)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NONE)
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    let version = build::PKG_VERSION;
    let commit = build::SHORT_COMMIT;
    info!("mooRc {version}+{commit}");

    // Valid argument scenarios require 1 src and 1 out, no more.
    if args.src_objdef_dir.is_some() && args.src_textdump.is_some() {
        error!("Cannot specify both src-objdef-dir and src-textdump");
        return;
    }
    if args.src_objdef_dir.is_none() && args.src_textdump.is_none() {
        error!("Must specify either src-objdef_dir or src-textdump");
        return;
    }

    // Actual binary database is in a tmpdir.
    let db_dir = tempfile::tempdir().unwrap();

    info!("Opening temporary database at {}", db_dir.path().display());
    let (database, _) = TxDB::open(Some(db_dir.path()), DatabaseConfig::default());
    let Ok(mut loader_interface) = database.loader_client() else {
        error!(
            "Unable to open temporary database at {}",
            db_dir.path().display()
        );
        return;
    };

    let mut features = FeaturesConfig::default();
    if let Some(fa) = args.feature_args.as_ref() {
        fa.merge_config(&mut features)
    }
    info!("Importing with features: {features:?}");

    // Compile phase.
    if let Some(textdump) = args.src_textdump {
        info!("Loading textdump from {:?}", textdump);
        let start = std::time::Instant::now();
        let version = semver::Version::parse(build::PKG_VERSION).expect("Invalid moor version");

        textdump_load(
            loader_interface.as_mut(),
            textdump.clone(),
            version.clone(),
            features.compile_options(),
        )
        .unwrap();

        info!("Loaded textdump in {:?}", start.elapsed());
        loader_interface
            .commit()
            .expect("Failure to commit loaded database...");
        info!("Committed. Total time: {:?}", start.elapsed());
    } else if let Some(objdef_dir) = args.src_objdef_dir {
        let start = std::time::Instant::now();
        let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());

        if let Err(e) = od.read_dirdump(features.compile_options(), objdef_dir.as_ref()) {
            error!("Compilation failure @ {}", e.path().display());
            error!("{:#}", e);
            return;
        }
        info!("Loaded objdef directory in {:?}", start.elapsed());
        loader_interface
            .commit()
            .expect("Failure to commit loaded database...");
        info!("Committed. Total time: {:?}", start.elapsed());
    }

    info!(
        "Database loaded. out_textdump?: {:?} out_objdef_dir?: {:?} test_directory?: {:?} run_tests?: {:?}",
        args.out_textdump, args.out_objdef_dir, args.test_directory, args.run_tests
    );

    // Dump phase.
    if let Some(textdump_path) = args.out_textdump {
        let Ok(loader_interface) = database.loader_client() else {
            error!(
                "Unable to open temporary database at {}",
                db_dir.path().display()
            );
            return;
        };

        let version = semver::Version::parse(build::PKG_VERSION).expect("Invalid moor version");

        let textdump_config = ImportExportConfig::default();
        let encoding_mode = EncodingMode::UTF8;
        let version_string = textdump_config.version_string(&version, &features);

        let Ok(mut output) = File::create(&textdump_path) else {
            error!("Could not open textdump file for writing");
            return;
        };

        trace!("Creating textdump...");
        let textdump = make_textdump(loader_interface.as_ref(), version_string);

        debug!(?textdump_path, "Writing textdump..");
        let mut writer = TextdumpWriter::new(&mut output, encoding_mode);
        if let Err(e) = writer.write_textdump(&textdump) {
            error!(?e, "Could not write textdump");
            return;
        }

        // Now that the dump has been written, strip the in-progress suffix.
        let final_path = textdump_path.with_extension("moo-textdump");
        if let Err(e) = std::fs::rename(&textdump_path, &final_path) {
            error!(?e, "Could not rename textdump to final path");
        }
        info!(?final_path, "Textdump written.");
    }

    if let Some(dirdump_path) = args.out_objdef_dir {
        let Ok(loader_interface) = database.loader_client() else {
            error!(
                "Unable to open temporary database at {}",
                db_dir.path().display()
            );
            return;
        };

        info!("Collecting objects for dump...");
        let objects = collect_object_definitions(loader_interface.as_ref());
        info!("Dumping objects to {dirdump_path:?}");
        dump_object_definitions(&objects, &dirdump_path);

        info!(?dirdump_path, "Objdefdump written.");
    }

    if args.run_tests != Some(true) && args.test_directory.is_none() {
        info!("No tests to run. Exiting.");
        return;
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
        .unwrap();
        tx.commit().unwrap();
    }

    // Before handing off the DB ot the scheduler, we need to find a list of all potential tests
    // to run.
    let mut unit_tests = vec![];
    if args.run_tests == Some(true) {
        let tx = db.new_world_state().unwrap();
        let mo = tx.max_object(&wizard).unwrap().id().0;
        info!("Scanning objects 0..{} for tests", mo);
        for o in 0..=mo {
            let o = Obj::mk_id(o);
            if let Ok(verbs) = tx.verbs(&wizard, &o) {
                for verb in verbs.iter() {
                    for name in verb.names() {
                        if name.starts_with("test_") {
                            unit_tests.push((o.clone(), Symbol::mk(name)));
                        }
                    }
                }
            }
        }
        info!("Found {} tests", unit_tests.len());
    }

    let config = Config {
        features_config: features,
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
        for (o, verb) in unit_tests {
            let session = test_session_factory
                .clone()
                .mk_background_session(&wizard)
                .unwrap();
            info!("Running test {}:{}", o, verb);
            let handle = scheduler_client
                .submit_verb_task(
                    &wizard,
                    &ObjectRef::Id(o.clone()),
                    verb,
                    List::mk_list(&[]),
                    "".to_string(),
                    &wizard,
                    session,
                )
                .unwrap();
            let result = handle
                .receiver()
                .recv_timeout(Duration::from_secs(4))
                .expect("Test timed out");
            let result_value = match result {
                Ok(rv) => rv,
                Err(e) => match e {
                    SchedulerError::TaskAbortedException(e) => {
                        error!("Test {}:{} aborted: {}", o, verb, e.error);
                        for l in e.backtrace {
                            let Variant::Str(s) = l.variant() else {
                                continue;
                            };
                            error!("{s}");
                        }
                        return;
                    }
                    _ => {
                        panic!("Test {}:{} failed: {:?}", o, verb, e);
                    }
                },
            };
            let TaskResult::Result(result_value) = result_value else {
                panic!("Test failed to return a result");
            };
            // Result must be non-Error
            if let Variant::Err(e) = result_value.variant() {
                panic!("Test {}:{} failed: {:?}", o, verb, e);
            }
        }
    }

    // Perform integration test run.
    if let Some(test_directory) = args.test_directory {
        let player = Obj::mk_id(args.test_player.expect("Must specify player object"));
        let programmer = Obj::mk_id(
            args.test_programmer
                .expect("Must specify programmer object"),
        );
        let moot_options = MootOptions::default()
            .wizard_object(wizard.clone())
            .nonprogrammer_object(player.clone())
            .programmer_object(programmer)
            .init_logging(false);

        // Iterate all the .moot tests and run them in the context of the current database.
        warn!("Running integration tests in {}", test_directory.display());
        for entry in std::fs::read_dir(test_directory).unwrap() {
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
    }

    scheduler_client
        .submit_shutdown("Test runs are done")
        .expect("Failed to shut down scheduler");
    scheduler_loop_jh
        .join()
        .expect("Failed to join() scheduler");
}
