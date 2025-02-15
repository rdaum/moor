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

use bincommon::FeatureArgs;
use clap::Parser;
use clap_derive::Parser;
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_kernel::config::{FeaturesConfig, TextdumpConfig};
use moor_kernel::objdef::{
    collect_object_definitions, dump_object_definitions, ObjectDefinitionLoader,
};
use moor_kernel::textdump::{make_textdump, textdump_load, EncodingMode, TextdumpWriter};
use moor_values::build;
use std::fs::File;
use std::path::PathBuf;
use tracing::{debug, error, info, trace};

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

    #[clap(long, help = "Enable debug logging")]
    debug: bool,
}

fn main() {
    let args: Args = Args::parse();

    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
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

    let (database, _) = TxDB::open(Some(db_dir.path()), DatabaseConfig::default());
    let Ok(mut loader_interface) = database.loader_client() else {
        error!(
            "Unable to open temporary database at {}",
            db_dir.path().display()
        );
        return;
    };

    let mut features = FeaturesConfig::default();
    args.feature_args
        .as_ref()
        .map(|fa| fa.merge_config(&mut features));

    // Compile phase.
    if let Some(textdump) = args.src_textdump {
        info!("Loading textdump from {:?}", textdump);
        let start = std::time::Instant::now();
        let version = semver::Version::parse(build::PKG_VERSION).expect("Invalid moor version");

        textdump_load(
            loader_interface.as_mut(),
            textdump.clone(),
            version.clone(),
            features.clone(),
        )
        .unwrap();

        let duration = start.elapsed();
        info!("Loaded textdump in {:?}", duration);
        loader_interface
            .commit()
            .expect("Failure to commit loaded database...");
    } else if let Some(objdef_dir) = args.src_objdef_dir {
        let start = std::time::Instant::now();
        let mut od = ObjectDefinitionLoader::new(loader_interface.as_mut());
        od.read_dirdump(features.clone(), objdef_dir.as_ref())
            .unwrap();
        let duration = start.elapsed();
        info!("Loaded objdef directory in {:?}", duration);
        loader_interface
            .commit()
            .expect("Failure to commit loaded database...");
    }

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

        let textdump_config = TextdumpConfig::default();
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
}
