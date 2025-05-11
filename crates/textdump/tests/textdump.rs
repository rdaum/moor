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
    use moor_common::model::{CommitResult, ValSet};
    use moor_common::model::{HasUuid, Named};
    use moor_common::program::ProgramType;
    use moor_compiler::CompileOptions;
    use moor_db::{Database, DatabaseConfig, TxDB};
    use moor_textdump::{
        EncodingMode, LambdaMOODBVersion, TextdumpReader, TextdumpVersion, TextdumpWriter,
        make_textdump, read_textdump, textdump_load,
    };
    use moor_var::SYSTEM_OBJECT;
    use moor_var::Symbol;
    use moor_var::{NOTHING, Obj};

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
        let tx = db.clone().loader_client().unwrap();
        let mut output = Vec::new();
        let textdump = make_textdump(tx.as_ref(), version.to_string());

        let mut writer = TextdumpWriter::new(&mut output, EncodingMode::UTF8);
        writer
            .write_textdump(&textdump)
            .expect("Failed to write textdump");

        assert_eq!(tx.commit().unwrap(), CommitResult::Success);
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
        assert_eq!(sysobj.propdefs, Vec::<String>::new());

        let rootobj = td
            .objects
            .get(&Obj::mk_id(1))
            .expect("Root object not found");
        assert_eq!(rootobj.name, "Root Class");
        assert_eq!(rootobj.owner, Obj::mk_id(3));
        assert_eq!(rootobj.parent, NOTHING);
        assert_eq!(rootobj.location, NOTHING);
        assert_eq!(rootobj.propdefs, Vec::<String>::new());

        let first_room = td
            .objects
            .get(&Obj::mk_id(2))
            .expect("First room not found");
        assert_eq!(first_room.name, "The First Room");
        assert_eq!(first_room.owner, Obj::mk_id(3));
        assert_eq!(first_room.parent, Obj::mk_id(1));
        assert_eq!(first_room.location, NOTHING);
        assert_eq!(first_room.propdefs, Vec::<String>::new());

        let wizard = td.objects.get(&Obj::mk_id(3)).expect("Wizard not found");
        assert_eq!(wizard.name, "Wizard");
        assert_eq!(wizard.owner, Obj::mk_id(3));
        assert_eq!(wizard.parent, Obj::mk_id(1));
        assert_eq!(wizard.location, Obj::mk_id(2));
        assert_eq!(wizard.propdefs, Vec::<String>::new());

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
        let jhcore = manifest_dir.join("../../JHCore-DEV-2.db");

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
        let jhcore = manifest_dir.join("../../JHCore-DEV-2.db");

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
        let tx1 = db1.loader_client().unwrap();
        let tx2 = db2.loader_client().unwrap();
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

            o1_props.sort_by(|a, b| a.name().cmp(b.name()));
            o2_props.sort_by(|a, b| a.name().cmp(b.name()));

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
                let v1_name = v1.names().join(" ").to_string();
                assert_eq!(v1.names(), v2.names(), "{}:{}, name mismatch", o, v1_name);

                assert_eq!(v1.owner(), v2.owner(), "{}:{}, owner mismatch", o, v1_name);
                assert_eq!(v1.flags(), v2.flags(), "{}:{}, flags mismatch", o, v1_name);
                assert_eq!(v1.args(), v2.args(), "{}:{}, args mismatch", o, v1_name);

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
                    program1.unbound_names, program2.unbound_names,
                    "{}:{}, variable names mismatch",
                    o, v1_name
                );
                assert_eq!(
                    program1.stmts, program2.stmts,
                    "{}:{}, statements mismatch",
                    o, v1_name
                );
            }
        }
    }
}
