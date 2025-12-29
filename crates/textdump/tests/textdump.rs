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

    use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};

    use moor_common::{
        model::{
            CommitResult, PropFlag, ValSet, VerbArgsSpec, VerbFlag, WorldStateSource,
            loader::LoaderInterface,
        },
        util::BitEnum,
    };
    use moor_compiler::{CompileOptions, compile};
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_textdump::{
        LambdaMOODBVersion, TextdumpImportOptions, TextdumpReader, TextdumpVersion, read_textdump,
        textdump_load,
    };
    use moor_var::{
        NOTHING, Obj, SYSTEM_OBJECT, Symbol, Var,
        program::{
            labels::Label,
            names::Name,
            opcode::{ScatterArgs, ScatterLabel},
        },
    };

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
            TextdumpImportOptions::default(),
        )
        .expect("Could not load textdump");
        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
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
            TextdumpImportOptions::default(),
        )
        .unwrap();
        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

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

    #[test]
    fn test_simple_anonymous_object_parsing() {
        use std::io::Cursor;

        // Create a minimal textdump with just one anonymous object
        let simple_textdump = r#"Moor 0.1.0, features: "flyweight_type=true lexical_scopes=true", encoding: UTF8
1
0
0
0
#0
*anonymous*

0
0
-1
-1
-1
-1
-1
-1
0
0
0
0 clocks
0 queued tasks
0 suspended tasks
"#;

        // Try to parse just this simple case
        let (db, _) = TxDB::open(None, DatabaseConfig::default());
        let db = Arc::new(db);

        {
            let mut loader = db.loader_client().unwrap();
            let cursor = std::io::BufReader::new(Cursor::new(simple_textdump.as_bytes()));

            // Add timeout or debug info to see where it hangs
            match read_textdump(
                loader.as_mut(),
                cursor,
                Version::new(0, 1, 0),
                CompileOptions::default(),
                TextdumpImportOptions::default(),
            ) {
                Ok(_) => {
                    assert!(matches!(loader.commit(), Ok(CommitResult::Success { .. })));
                }
                Err(e) => {
                    panic!("Textdump loading failed: {e:?}");
                }
            }
        }

        // Verify the anonymous object was loaded correctly
        {
            let snapshot = db.create_snapshot().unwrap();
            let objects = snapshot.get_objects().unwrap();

            // Should have loaded 1 anonymous object
            assert_eq!(objects.len(), 1, "Should have loaded 1 object");

            let obj = objects.iter().next().unwrap();
            assert!(obj.is_anonymous(), "Object should be anonymous");
        }
    }
}
