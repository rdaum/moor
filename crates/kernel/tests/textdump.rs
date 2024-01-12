#[cfg(test)]
mod test {
    use moor_db::loader::LoaderInterface;
    use moor_db::odb::RelBoxWorldState;
    use moor_db::Database;
    use moor_kernel::textdump::{make_textdump, read_textdump, textdump_load, TextdumpReader};
    use moor_values::model::r#match::VerbArgsSpec;
    use moor_values::model::verbs::VerbFlag;
    use moor_values::model::world_state::WorldStateSource;
    use moor_values::model::CommitResult;
    use moor_values::var::objid::Objid;
    use moor_values::SYSTEM_OBJECT;
    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::path::PathBuf;
    use std::sync::Arc;
    use text_diff::assert_diff;

    fn get_minimal_db() -> File {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");
        File::open(minimal_db.clone()).unwrap()
    }

    async fn load_textdump_file(tx: Arc<dyn LoaderInterface>, path: &str) {
        textdump_load(tx.clone(), PathBuf::from(path))
            .await
            .expect("Could not load textdump");
        assert_eq!(tx.commit().await.unwrap(), CommitResult::Success);
    }

    async fn write_textdump(db: Arc<RelBoxWorldState>) -> String {
        let tx = db.clone().loader_client().unwrap();
        let mut output = Vec::new();
        let textdump = make_textdump(
            tx.clone(),
            Some("** LambdaMOO Database, Format Version 4 **"),
        )
        .await;

        let mut writer = moor_kernel::textdump::TextdumpWriter::new(&mut output);
        writer
            .write_textdump(&textdump)
            .expect("Failed to write textdump");

        assert_eq!(tx.commit().await.unwrap(), CommitResult::Success);
        String::from_utf8(output).expect("Failed to convert output to string")
    }

    /// Load Minimal.db with the textdump reader and confirm that it has the expected contents.
    #[test]
    fn load_minimal() {
        let corefile = get_minimal_db();

        let br = BufReader::new(corefile);
        let mut tdr = TextdumpReader::new(br);
        let td = tdr.read_textdump().expect("Failed to read textdump");

        // Version spec
        assert_eq!(td.version, "** LambdaMOO Database, Format Version 4 **");

        // Minimal DB has 1 user, #3,
        assert_eq!(td.users, vec![Objid(3)]);

        // Minimal DB always has 4 objects in it.
        assert_eq!(td.objects.len(), 4);

        // The first object being the system object, whose owner is the "wizard" (#3), and whose parent is root (#1)
        let sysobj = td
            .objects
            .get(&SYSTEM_OBJECT)
            .expect("System object not found");
        assert_eq!(sysobj.name, "System Object");
        assert_eq!(sysobj.owner, Objid(3));
        assert_eq!(sysobj.parent, Objid(1));
        assert_eq!(sysobj.location, Objid(-1));
        assert_eq!(sysobj.propdefs, Vec::<String>::new());

        let rootobj = td.objects.get(&Objid(1)).expect("Root object not found");
        assert_eq!(rootobj.name, "Root Class");
        assert_eq!(rootobj.owner, Objid(3));
        assert_eq!(rootobj.parent, Objid(-1));
        assert_eq!(rootobj.location, Objid(-1));
        assert_eq!(rootobj.propdefs, Vec::<String>::new());

        let first_room = td.objects.get(&Objid(2)).expect("First room not found");
        assert_eq!(first_room.name, "The First Room");
        assert_eq!(first_room.owner, Objid(3));
        assert_eq!(first_room.parent, Objid(1));
        assert_eq!(first_room.location, Objid(-1));
        assert_eq!(first_room.propdefs, Vec::<String>::new());

        let wizard = td.objects.get(&Objid(3)).expect("Wizard not found");
        assert_eq!(wizard.name, "Wizard");
        assert_eq!(wizard.owner, Objid(3));
        assert_eq!(wizard.parent, Objid(1));
        assert_eq!(wizard.location, Objid(2));
        assert_eq!(wizard.propdefs, Vec::<String>::new());

        assert_eq!(sysobj.verbdefs.len(), 1);
        let do_login_command_verb = &sysobj.verbdefs[0];
        assert_eq!(do_login_command_verb.name, "do_login_command");
        assert_eq!(do_login_command_verb.owner, Objid(3));
        // this none this
        assert_eq!(do_login_command_verb.flags, 173);
        assert_eq!(do_login_command_verb.prep, -1);

        // Nothing on the root class
        assert_eq!(rootobj.verbdefs.len(), 0);

        // Eval on the first room (but this one seems unprogrammed?)
        assert_eq!(first_room.verbdefs.len(), 1);
        let eval_verb = &first_room.verbdefs[0];
        assert_eq!(eval_verb.name, "eval");
        assert_eq!(eval_verb.owner, Objid(3));
        // any any any
        assert_eq!(eval_verb.flags, 88);
        assert_eq!(eval_verb.prep, -2);

        // Nothing on the wizard
        assert_eq!(wizard.verbdefs.len(), 0);

        // Look at the verb program
        assert_eq!(td.verbs.len(), 1);
        let do_login_verb = td.verbs.get(&(Objid(0), 0)).expect("Verb not found");
        assert_eq!(do_login_verb.objid, Objid(0));
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
        let mut tdr = TextdumpReader::new(br);
        let td = tdr.read_textdump().expect("Failed to read textdump");

        let mut output = Vec::new();
        let mut writer = moor_kernel::textdump::TextdumpWriter::new(&mut output);
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

        assert_diff(&input, &output, "", 0);
    }

    /// Actually load a textdump into an actual *database* and confirm that it has the expected contents.
    #[tokio::test]
    async fn load_into_db() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        let (db, _) = RelBoxWorldState::open(None, 1 << 30).await;
        let db = Arc::new(db);
        let tx = db.clone().loader_client().unwrap();
        textdump_load(tx.clone(), PathBuf::from(minimal_db))
            .await
            .unwrap();
        assert_eq!(tx.commit().await.unwrap(), CommitResult::Success);

        // Check a few things in a new transaction.
        let tx = db.new_world_state().await.unwrap();
        assert_eq!(
            tx.names_of(Objid(3), Objid(1)).await.unwrap(),
            ("Root Class".into(), vec![])
        );
        assert_eq!(
            tx.names_of(Objid(3), Objid(2)).await.unwrap(),
            ("The First Room".into(), vec![])
        );
        assert_eq!(
            tx.names_of(Objid(3), Objid(3)).await.unwrap(),
            ("Wizard".into(), vec![])
        );
        assert_eq!(
            tx.names_of(Objid(3), SYSTEM_OBJECT).await.unwrap(),
            ("System Object".into(), vec![])
        );

        let dlc = tx
            .get_verb(Objid(3), SYSTEM_OBJECT, "do_login_command")
            .await
            .unwrap();
        assert_eq!(dlc.owner(), Objid(3));
        assert_eq!(dlc.flags(), VerbFlag::rxd());
        assert_eq!(dlc.args(), VerbArgsSpec::this_none_this());
    }

    /// Load minimal into a db, then write a new textdump, and they should be the same-ish.
    #[tokio::test]
    async fn load_minimal_into_db_then_compare() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("tests/Minimal.db");

        let (db, _) = RelBoxWorldState::open(None, 1 << 30).await;
        let db = Arc::new(db);
        load_textdump_file(
            db.clone().loader_client().unwrap(),
            minimal_db.to_str().unwrap(),
        )
        .await;

        // Read input as string, and compare.
        let corefile = File::open(minimal_db).unwrap();
        let br = BufReader::new(corefile);
        let input = String::from_utf8(br.bytes().map(|b| b.unwrap()).collect())
            .expect("Failed to convert input to string");

        let output = write_textdump(db).await;

        assert_diff(&input, &output, "", 0);
    }

    /// Load a big (JHCore-DEV-2.db) core into a db, then write a new textdump, and then reload
    /// the core to verify it can be loaded.
    #[tokio::test]
    // This is an expensive test, so it's not run by default.
    #[ignore]
    async fn load_write_reload_big_core() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let minimal_db = manifest_dir.join("../../JHCore-DEV-2.db");

        let textdump = {
            let (db, _) = RelBoxWorldState::open(None, 1 << 34).await;
            let db = Arc::new(db);
            load_textdump_file(
                db.clone().loader_client().unwrap(),
                minimal_db.to_str().unwrap(),
            )
            .await;

            write_textdump(db).await
        };

        // Now load that same core back in to a new DB, and hope we don't blow up anywhere.
        let (db, _) = RelBoxWorldState::open(None, 1 << 34).await;
        let db = Arc::new(db);
        let buffered_string_reader = std::io::BufReader::new(textdump.as_bytes());
        let _ = read_textdump(db.clone().loader_client().unwrap(), buffered_string_reader)
            .await
            .unwrap();
    }
}
