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

#[cfg(test)]
mod test {
    use semver::Version;

    use std::collections::BTreeSet;
    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::path::PathBuf;
    use std::sync::Arc;

    use moor_common::model::VerbArgsSpec;
    use moor_common::model::VerbFlag;
    use moor_common::model::WorldStateSource;
    use moor_common::model::loader::LoaderInterface;
    use moor_common::model::{CommitResult, PropFlag, ValSet};
    use moor_common::model::{HasUuid, Named};
    use moor_common::util::BitEnum;
    use moor_compiler::CompileOptions;
    use moor_compiler::compile;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_textdump::{
        EncodingMode, LambdaMOODBVersion, TextdumpReader, TextdumpVersion, TextdumpWriter,
        make_textdump, read_textdump, textdump_load,
    };
    use moor_var::SYSTEM_OBJECT;
    use moor_var::Symbol;
    use moor_var::program::ProgramType;
    use moor_var::program::labels::Label;
    use moor_var::program::names::Name;
    use moor_var::program::opcode::{ScatterArgs, ScatterLabel};
    use moor_var::{NOTHING, Obj, Var, v_int};

    fn get_minimal_db() -> File {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");
        File::open(minimal_db.clone()).unwrap()
    }

    fn load_textdump_file(mut tx: Box<dyn LoaderInterface>, path: &str) {
        let compile_options = CompileOptions {
            // JHCore has an erroneous "E_PERMS" in it, which causes confusions.
            custom_errors: true,
            ..Default::default()
        };
        textdump_load(
            tx.as_mut(),
            PathBuf::from(path),
            Version::new(0, 1, 0),
            compile_options,
        )
        .expect("Could not load textdump");
        assert_eq!(tx.commit().unwrap(), CommitResult::Success);
    }

    fn write_textdump(db: Arc<TxDB>, version: &str) -> String {
        let mut output = Vec::new();
        let snapshot = db.create_snapshot().unwrap();
        let textdump = make_textdump(snapshot.as_ref(), version.to_string());

        let mut writer = TextdumpWriter::new(&mut output, EncodingMode::UTF8);
        writer
            .write_textdump(&textdump)
            .expect("Failed to write textdump");

        String::from_utf8(output).expect("Failed to convert output to string")
    }

    /// Load Minimal.db with the textdump reader and confirm that it has the expected contents.
    #[test]
    fn load_minimal() {
        let corefile = get_minimal_db();

        let br = BufReader::new(corefile);
        let mut tdr = TextdumpReader::new(br).unwrap();
        let td = tdr.read_textdump().expect("Failed to read textdump");

        // Version spec
        assert_eq!(
            td.version_string,
            "** LambdaMOO Database, Format Version 1 **"
        );
        assert_eq!(
            tdr.version,
            TextdumpVersion::LambdaMOO(LambdaMOODBVersion::DbvExceptions)
        );

        // Minimal DB has 1 user, #3,
        assert_eq!(td.users, vec![Obj::mk_id(3)]);

        // Minimal DB always has 4 objects in it.
        assert_eq!(td.objects.len(), 4);

        // The first object being the system object, whose owner is the "wizard" (#3), and whose parent is root (#1)
        let sysobj = td
            .objects
            .get(&SYSTEM_OBJECT)
            .expect("System object not found");
        assert_eq!(sysobj.name, "System Object");
        assert_eq!(sysobj.owner, Obj::mk_id(3));
        assert_eq!(sysobj.parent, Obj::mk_id(1));
        assert_eq!(sysobj.location, NOTHING);
        assert_eq!(sysobj.propdefs, Vec::<Symbol>::new());

        let rootobj = td
            .objects
            .get(&Obj::mk_id(1))
            .expect("Root object not found");
        assert_eq!(rootobj.name, "Root Class");
        assert_eq!(rootobj.owner, Obj::mk_id(3));
        assert_eq!(rootobj.parent, NOTHING);
        assert_eq!(rootobj.location, NOTHING);
        assert_eq!(rootobj.propdefs, Vec::<Symbol>::new());

        let first_room = td
            .objects
            .get(&Obj::mk_id(2))
            .expect("First room not found");
        assert_eq!(first_room.name, "The First Room");
        assert_eq!(first_room.owner, Obj::mk_id(3));
        assert_eq!(first_room.parent, Obj::mk_id(1));
        assert_eq!(first_room.location, NOTHING);
        assert_eq!(first_room.propdefs, Vec::<Symbol>::new());

        let wizard = td.objects.get(&Obj::mk_id(3)).expect("Wizard not found");
        assert_eq!(wizard.name, "Wizard");
        assert_eq!(wizard.owner, Obj::mk_id(3));
        assert_eq!(wizard.parent, Obj::mk_id(1));
        assert_eq!(wizard.location, Obj::mk_id(2));
        assert_eq!(wizard.propdefs, Vec::<Symbol>::new());

        assert_eq!(sysobj.verbdefs.len(), 1);
        let do_login_command_verb = &sysobj.verbdefs[0];
        assert_eq!(do_login_command_verb.name, "do_login_command");
        assert_eq!(do_login_command_verb.owner, Obj::mk_id(3));
        // this none this
        assert_eq!(do_login_command_verb.flags, 173);
        assert_eq!(do_login_command_verb.prep, -1);

        // Nothing on the root class
        assert_eq!(rootobj.verbdefs.len(), 0);

        // Eval on the first room (but this one seems unprogrammed?)
        assert_eq!(first_room.verbdefs.len(), 1);
        let eval_verb = &first_room.verbdefs[0];
        assert_eq!(eval_verb.name, "eval");
        assert_eq!(eval_verb.owner, Obj::mk_id(3));
        // any any any
        assert_eq!(eval_verb.flags, 88);
        assert_eq!(eval_verb.prep, -2);

        // Nothing on the wizard
        assert_eq!(wizard.verbdefs.len(), 0);

        // Look at the verb program
        assert_eq!(td.verbs.len(), 1);
        let do_login_verb = td.verbs.get(&(Obj::mk_id(0), 0)).expect("Verb not found");
        assert_eq!(do_login_verb.objid, Obj::mk_id(0));
        assert_eq!(do_login_verb.verbnum, 0);
        assert_eq!(do_login_verb.program.clone().unwrap(), "return #3;");
    }

    /// Load Minimal.db, then write it back out again and confirm that the output is the same as the input.
    #[test]
    fn load_then_write() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");
        let corefile = File::open(minimal_db.clone()).unwrap();
        let br = BufReader::new(corefile);
        let mut tdr = TextdumpReader::new(br).unwrap();
        let td = tdr.read_textdump().expect("Failed to read textdump");

        let mut output = Vec::new();
        let mut writer = TextdumpWriter::new(&mut output, EncodingMode::UTF8);
        writer
            .write_textdump(&td)
            .expect("Failed to write textdump");
        // Convert output to string.
        let output = String::from_utf8(output).expect("Failed to convert output to string");

        // Read input as string, and compare.
        let corefile = File::open(minimal_db).unwrap();
        let br = BufReader::new(corefile);
        let input = String::from_utf8(br.bytes().map(|b| b.unwrap()).collect())
            .expect("Failed to convert input to string");

        similar_asserts::assert_eq!(&input, &output, "");
    }

    /// Actually load a textdump into an actual *database* and confirm that it has the expected contents.
    #[test]
    fn load_into_db() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);
        let mut tx = db.clone().loader_client().unwrap();
        textdump_load(
            tx.as_mut(),
            minimal_db,
            Version::new(0, 1, 0),
            CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(tx.commit().unwrap(), CommitResult::Success);

        // Check a few things in a new transaction.
        let tx = db.new_world_state().unwrap();
        assert_eq!(
            tx.names_of(&Obj::mk_id(3), &Obj::mk_id(1)).unwrap(),
            ("Root Class".into(), vec![])
        );
        assert_eq!(
            tx.names_of(&Obj::mk_id(3), &Obj::mk_id(2)).unwrap(),
            ("The First Room".into(), vec![])
        );
        assert_eq!(
            tx.names_of(&Obj::mk_id(3), &Obj::mk_id(3)).unwrap(),
            ("Wizard".into(), vec![])
        );
        assert_eq!(
            tx.names_of(&Obj::mk_id(3), &SYSTEM_OBJECT).unwrap(),
            ("System Object".into(), vec![])
        );

        let dlc = tx
            .get_verb(
                &Obj::mk_id(3),
                &SYSTEM_OBJECT,
                Symbol::mk("do_login_command"),
            )
            .unwrap();
        assert_eq!(dlc.owner(), Obj::mk_id(3));
        assert_eq!(dlc.flags(), VerbFlag::rxd());
        assert_eq!(dlc.args(), VerbArgsSpec::this_none_this());
    }

    /// Load minimal into a db, then write a new textdump, and they should be the same-ish.
    #[test]
    fn load_minimal_into_db_then_compare() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);
        load_textdump_file(
            db.clone().loader_client().unwrap(),
            minimal_db.to_str().unwrap(),
        );

        // Read input as string, and compare.
        let corefile = File::open(minimal_db).unwrap();
        let br = BufReader::new(corefile);
        let input = String::from_utf8(br.bytes().map(|b| b.unwrap()).collect())
            .expect("Failed to convert input to string");

        let output = write_textdump(db, "** LambdaMOO Database, Format Version 1 **");

        similar_asserts::assert_eq!(&input, &output, "");
    }

    #[test]
    fn load_big_core() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let jhcore = manifest_dir.join("../../cores/JHCore-DEV-2.db");

        let (db1, _) = TxDB::open(None, DatabaseConfig::default());
        let db1 = Arc::new(db1);
        load_textdump_file(
            db1.clone().loader_client().unwrap(),
            jhcore.to_str().unwrap(),
        );
    }

    /// Load a big (JHCore-DEV-2.db) core into a db, then write a new textdump, and then reload
    /// the core to verify it can be loaded.
    #[test]
    // This is an expensive test, so it's not run by default.
    #[ignore]
    fn load_write_reload_big_core() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let jhcore = manifest_dir.join("../../cores/JHCore-DEV-2.db");

        let (db1, _) = TxDB::open(None, DatabaseConfig::default());
        let db1 = Arc::new(db1);
        load_textdump_file(
            db1.clone().loader_client().unwrap(),
            jhcore.to_str().unwrap(),
        );

        let textdump = write_textdump(db1.clone(), "** LambdaMOO Database, Format Version 4 **");

        // Now load that same core back in to a new DB, and hope we don't blow up anywhere.
        let (db2, _) = TxDB::open(None, DatabaseConfig::default());
        let db2 = Arc::new(db2);
        let buffered_string_reader = std::io::BufReader::new(textdump.as_bytes());
        let mut lc = db2.clone().loader_client().unwrap();
        read_textdump(
            lc.as_mut(),
            buffered_string_reader,
            Version::new(0, 1, 0),
            CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(lc.commit().unwrap(), CommitResult::Success);

        // Now go through the properties and verbs of all the objects on db1, and verify that
        // they're the same on db2.
        let tx1 = db1.create_snapshot().unwrap();
        let tx2 = db2.create_snapshot().unwrap();
        let objects1 = tx1.get_objects().unwrap();
        let objects2 = tx2.get_objects().unwrap();
        let objects1 = objects1.iter().collect::<BTreeSet<_>>();
        let objects2 = objects2.iter().collect::<BTreeSet<_>>();
        assert_eq!(objects1, objects2);

        for o in objects1 {
            // set of properties should be the same
            let o1_props = tx1.get_object_properties(&o).unwrap();
            let o2_props = tx2.get_object_properties(&o).unwrap();
            let mut o1_props = o1_props.iter().collect::<Vec<_>>();
            let mut o2_props = o2_props.iter().collect::<Vec<_>>();

            o1_props.sort_by_key(|a| a.name());
            o2_props.sort_by_key(|a| a.name());

            // We want to do equality testing, but ignore the UUID which can be different
            // between textdump loads...
            assert_eq!(o1_props.len(), o2_props.len());
            let zipped = o1_props.iter().zip(o2_props.iter());
            for (i, prop) in zipped.enumerate() {
                let (p1, p2) = prop;

                assert_eq!(
                    p1.name(),
                    p2.name(),
                    "{}.{}, name mismatch",
                    o.clone(),
                    p1.name(),
                );

                assert_eq!(
                    p1.definer(),
                    p2.definer(),
                    "{}.{}, definer mismatch ({} != {})",
                    o,
                    p1.name(),
                    p1.definer(),
                    p2.definer()
                );
                // location
                assert_eq!(
                    p1.location(),
                    p2.location(),
                    "{}.{}, location mismatch",
                    o,
                    p1.name()
                );

                let (value1, perms1) = tx1.get_property_value(&o, p1.uuid()).unwrap();
                let (value2, perms2) = tx2.get_property_value(&o, p2.uuid()).unwrap();

                assert_eq!(
                    perms1.flags(),
                    perms2.flags(),
                    "{}.{}, flags mismatch",
                    o,
                    p1.name(),
                );
                assert_eq!(
                    perms1.owner(),
                    perms2.owner(),
                    "{}.{}, owner mismatch",
                    o,
                    p1.name(),
                );

                assert_eq!(
                    value1,
                    value2,
                    "{}.{}, value mismatch ({}th value checked)",
                    o,
                    p1.name(),
                    i
                );
            }

            // Now compare verbdefs
            let o1_verbs = tx1.get_object_verbs(&o).unwrap();
            let o2_verbs = tx2.get_object_verbs(&o).unwrap();
            let o1_verbs = o1_verbs.iter().collect::<Vec<_>>();
            let o2_verbs = o2_verbs.iter().collect::<Vec<_>>();

            assert_eq!(o1_verbs.len(), o2_verbs.len());
            for (v1, v2) in o1_verbs.iter().zip(o2_verbs.iter()) {
                let v1_name = v1
                    .names()
                    .iter()
                    .map(|s| s.as_string())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_string();
                assert_eq!(v1.names(), v2.names(), "{o}:{v1_name}, name mismatch");

                assert_eq!(v1.owner(), v2.owner(), "{o}:{v1_name}, owner mismatch");
                assert_eq!(v1.flags(), v2.flags(), "{o}:{v1_name}, flags mismatch");
                assert_eq!(v1.args(), v2.args(), "{o}:{v1_name}, args mismatch");

                // We want to actually decode and compare the opcode streams rather than
                // the binary, so that we can make meaningful error reports.
                let prg1 = tx1.get_verb_program(&o, v1.uuid()).unwrap();
                let prg2 = tx2.get_verb_program(&o, v2.uuid()).unwrap();

                #[allow(irrefutable_let_patterns)]
                let ProgramType::MooR(program1) = &prg1 else {
                    panic!("ProgramType::Moo expected");
                };
                #[allow(irrefutable_let_patterns)]
                let ProgramType::MooR(program2) = &prg2 else {
                    panic!("ProgramType::Moo expected");
                };
                let program1 = moor_compiler::program_to_tree(program1).unwrap();
                let program2 = moor_compiler::program_to_tree(program2).unwrap();

                assert_eq!(
                    program1.variables, program2.variables,
                    "{o}:{v1_name}, variable names mismatch"
                );
                assert_eq!(
                    program1.stmts, program2.stmts,
                    "{o}:{v1_name}, statements mismatch"
                );
            }
        }
    }

    /// Test basic lambda database storage and retrieval
    #[test]
    fn test_lambda_database_storage() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        // Load minimal database
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);
        load_textdump_file(
            db.clone().loader_client().unwrap(),
            minimal_db.to_str().unwrap(),
        );

        // Test lambda storage in database without textdump
        {
            let mut tx = db.new_world_state().unwrap();
            let wizard = Obj::mk_id(3);
            let system_obj = SYSTEM_OBJECT;

            // Create a simple lambda
            let simple_source = "return x + 1;";
            let simple_program = compile(simple_source, CompileOptions::default()).unwrap();
            let simple_params = ScatterArgs {
                labels: vec![ScatterLabel::Required(Name(0, 0, 0))],
                done: Label(0),
            };
            let simple_lambda = Var::mk_lambda(simple_params, simple_program, vec![], None);

            // Store the lambda
            tx.define_property(
                &wizard,
                &wizard,
                &system_obj,
                Symbol::mk("test_lambda"),
                &wizard,
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                Some(simple_lambda.clone()),
            )
            .unwrap();

            tx.commit().unwrap();
        }

        // Test lambda retrieval from database
        {
            let tx = db.new_world_state().unwrap();
            let wizard = Obj::mk_id(3);
            let system_obj = SYSTEM_OBJECT;

            let retrieved = tx
                .retrieve_property(&wizard, &system_obj, Symbol::mk("test_lambda"))
                .unwrap();

            assert!(
                retrieved.as_lambda().is_some(),
                "Retrieved value should be a lambda"
            );

            if let Some(lambda) = retrieved.as_lambda() {
                assert_eq!(
                    lambda.0.params.labels.len(),
                    1,
                    "Lambda should have 1 parameter"
                );
                assert_eq!(
                    lambda.0.captured_env.len(),
                    0,
                    "Lambda should have no captured environment"
                );
                assert!(
                    lambda.0.self_var.is_none(),
                    "Lambda should have no self-reference"
                );
            }
        }
    }

    /// Test lambda serialization by adding lambdas to properties and doing a full round-trip
    #[test]
    fn test_lambda_textdump_integration() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        // Load minimal database into first DB
        let (db1, _) = TxDB::open(None, DatabaseConfig::default());
        let db1 = Arc::new(db1);
        load_textdump_file(
            db1.clone().loader_client().unwrap(),
            minimal_db.to_str().unwrap(),
        );

        // Add lambda properties to test different scenarios
        {
            let mut tx = db1.new_world_state().unwrap();
            let wizard = Obj::mk_id(3); // Wizard object from minimal db
            let system_obj = SYSTEM_OBJECT;

            // Create a simple lambda with no captured variables
            let simple_source = "return x + 1;";
            let simple_program = compile(simple_source, CompileOptions::default()).unwrap();
            let simple_params = ScatterArgs {
                labels: vec![ScatterLabel::Required(Name(0, 0, 0))],
                done: Label(0),
            };
            let simple_lambda = Var::mk_lambda(simple_params, simple_program, vec![], None);

            // Create a lambda with captured environment
            let captured_source = "return x + captured_var;";
            let captured_program = compile(captured_source, CompileOptions::default()).unwrap();
            let captured_params = ScatterArgs {
                labels: vec![ScatterLabel::Required(Name(0, 0, 0))],
                done: Label(0),
            };
            let captured_env = vec![vec![v_int(42), v_int(123)]];
            let captured_lambda =
                Var::mk_lambda(captured_params, captured_program, captured_env, None);

            // Create a lambda with self-reference (just test the structure, not actual recursion)
            let recursive_source = "return n + 1;";
            let recursive_program = compile(recursive_source, CompileOptions::default()).unwrap();
            let recursive_params = ScatterArgs {
                labels: vec![ScatterLabel::Required(Name(0, 0, 0))],
                done: Label(0),
            };
            let self_var = Some(Name(1, 0, 0)); // self-reference variable
            let recursive_lambda =
                Var::mk_lambda(recursive_params, recursive_program, vec![], self_var);

            // Define properties on system object for testing
            tx.define_property(
                &wizard,                                             // perms
                &system_obj,                                         // definer
                &system_obj,                                         // location
                Symbol::mk("simple_lambda"),                         // pname
                &wizard,                                             // owner
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write, // prop_flags
                Some(simple_lambda.clone()),                         // initial_value
            )
            .unwrap();

            tx.define_property(
                &wizard,                                             // perms
                &system_obj,                                         // definer
                &system_obj,                                         // location
                Symbol::mk("captured_lambda"),                       // pname
                &wizard,                                             // owner
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write, // prop_flags
                Some(captured_lambda.clone()),                       // initial_value
            )
            .unwrap();

            tx.define_property(
                &wizard,                                             // perms
                &system_obj,                                         // definer
                &system_obj,                                         // location
                Symbol::mk("recursive_lambda"),                      // pname
                &wizard,                                             // owner
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write, // prop_flags
                Some(recursive_lambda.clone()),                      // initial_value
            )
            .unwrap();

            tx.commit().unwrap();
        }

        // Force database checkpoint to ensure data is persisted before snapshot
        db1.checkpoint().unwrap();

        // Write database to textdump
        let textdump_output = write_textdump(db1, "** LambdaMOO Database, Format Version 4 **");

        // Load textdump into second database
        let (db2, _) = TxDB::open(None, DatabaseConfig::default());
        let db2 = Arc::new(db2);
        let buffered_reader = std::io::BufReader::new(textdump_output.as_bytes());
        let mut loader = db2.clone().loader_client().unwrap();
        read_textdump(
            loader.as_mut(),
            buffered_reader,
            Version::new(0, 1, 0),
            CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(loader.commit().unwrap(), CommitResult::Success);

        // Verify lambdas were preserved correctly
        {
            let tx = db2.new_world_state().unwrap();
            let wizard = Obj::mk_id(3); // Wizard object from minimal db
            let system_obj = SYSTEM_OBJECT;

            // Check simple lambda
            let simple_prop = tx
                .retrieve_property(&wizard, &system_obj, Symbol::mk("simple_lambda"))
                .unwrap();
            assert!(
                simple_prop.as_lambda().is_some(),
                "Simple lambda property should be a lambda"
            );

            if let Some(lambda) = simple_prop.as_lambda() {
                assert_eq!(
                    lambda.0.params.labels.len(),
                    1,
                    "Simple lambda should have 1 parameter"
                );
                assert_eq!(
                    lambda.0.captured_env.len(),
                    0,
                    "Simple lambda should have no captured environment"
                );
                assert!(
                    lambda.0.self_var.is_none(),
                    "Simple lambda should have no self-reference"
                );
            }

            // Check captured lambda
            let captured_prop = tx
                .retrieve_property(&wizard, &system_obj, Symbol::mk("captured_lambda"))
                .unwrap();
            assert!(
                captured_prop.as_lambda().is_some(),
                "Captured lambda property should be a lambda"
            );

            if let Some(lambda) = captured_prop.as_lambda() {
                assert_eq!(
                    lambda.0.params.labels.len(),
                    1,
                    "Captured lambda should have 1 parameter"
                );
                assert_eq!(
                    lambda.0.captured_env.len(),
                    1,
                    "Captured lambda should have 1 frame"
                );
                assert_eq!(
                    lambda.0.captured_env[0].len(),
                    2,
                    "Captured lambda frame should have 2 variables"
                );
                assert_eq!(
                    lambda.0.captured_env[0][0],
                    v_int(42),
                    "First captured var should be 42"
                );
                assert_eq!(
                    lambda.0.captured_env[0][1],
                    v_int(123),
                    "Second captured var should be 123"
                );
                assert!(
                    lambda.0.self_var.is_none(),
                    "Captured lambda should have no self-reference"
                );
            }

            // Check recursive lambda
            let recursive_prop = tx
                .retrieve_property(&wizard, &system_obj, Symbol::mk("recursive_lambda"))
                .unwrap();
            assert!(
                recursive_prop.as_lambda().is_some(),
                "Recursive lambda property should be a lambda"
            );

            if let Some(lambda) = recursive_prop.as_lambda() {
                assert_eq!(
                    lambda.0.params.labels.len(),
                    1,
                    "Recursive lambda should have 1 parameter"
                );
                assert_eq!(
                    lambda.0.captured_env.len(),
                    0,
                    "Recursive lambda should have no captured environment"
                );
                assert!(
                    lambda.0.self_var.is_some(),
                    "Recursive lambda should have self-reference"
                );

                if let Some(self_var) = lambda.0.self_var {
                    assert_eq!(self_var.0, 1, "Self-var should have offset 1");
                    assert_eq!(self_var.1, 0, "Self-var should have scope depth 0");
                    assert_eq!(self_var.2, 0, "Self-var should have scope id 0");
                }
            }
        }
    }
}
