// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use strum::{EnumCount, IntoEnumIterator};

    use moor_db::worldstate_transaction::WorldStateTransaction;
    use moor_db::{RelationalWorldStateTransaction, WorldStateSequence, WorldStateTable};
    use moor_db_relbox::RelboxTransaction;
    use moor_values::model::BinaryType;
    use moor_values::model::CommitResult;
    use moor_values::model::HasUuid;
    use moor_values::model::ObjAttrs;
    use moor_values::model::VerbArgsSpec;
    use moor_values::util::BitEnum;
    use moor_values::NOTHING;
    use relbox::{relation_info_for, RelBox, RelationInfo};

    pub fn test_db(dir: PathBuf) -> Arc<RelBox> {
        let relations: Vec<RelationInfo> = WorldStateTable::iter().map(relation_info_for).collect();

        RelBox::new(1 << 24, Some(dir), &relations, WorldStateSequence::COUNT)
    }

    pub fn begin_tx(
        db: Arc<RelBox>,
    ) -> RelationalWorldStateTransaction<RelboxTransaction<WorldStateTable>> {
        let tx = RelboxTransaction::new(db.clone().start_tx());
        RelationalWorldStateTransaction { tx: Some(tx) }
    }

    #[test]
    fn open_reopen() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir_str = tmpdir.path().to_str().unwrap();

        let a = {
            let db = test_db(tmpdir.path().into());
            let mut tx = begin_tx(db.clone());

            let a = tx
                .create_object(
                    None,
                    ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
                )
                .unwrap();

            tx.add_object_verb(
                a,
                a,
                vec!["test".into()],
                vec![],
                BinaryType::LambdaMoo18X,
                BitEnum::new(),
                VerbArgsSpec::this_none_this(),
            )
            .unwrap();

            tx.commit().unwrap();
            db.shutdown();

            // TODO: Sleep in "worldstate_restore" test should not be necessary.
            //   code smell.
            std::thread::sleep(Duration::from_millis(100));
            a
        };
        // Verify the WAL directory is not empty.
        assert!(std::fs::read_dir(format!("{}/wal", tmpdir_str))
            .unwrap()
            .next()
            .is_some());
        {
            let db = test_db(tmpdir.path().into());

            // Verify the pages directory is not empty after recovery.
            assert!(std::fs::read_dir(format!("{}/pages", tmpdir_str))
                .unwrap()
                .next()
                .is_some());

            let mut tx = begin_tx(db.clone());

            let v_uuid = tx.resolve_verb(a, "test".into(), None).unwrap().uuid();
            assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);
            assert_eq!(tx.commit(), Ok(CommitResult::Success));

            db.shutdown();
        }
    }
}
