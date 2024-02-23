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

    use moor_db::db_tx::DbTransaction;
    use moor_db::odb::{RelBoxTransaction, WorldStateRelation, WorldStateSequences};
    use moor_rdb::{relation_info_for, RelBox, RelationInfo};
    use moor_values::model::BinaryType;
    use moor_values::model::CommitResult;
    use moor_values::model::HasUuid;
    use moor_values::model::ObjAttrs;
    use moor_values::model::VerbArgsSpec;
    use moor_values::util::BitEnum;
    use moor_values::NOTHING;

    pub fn test_db(dir: PathBuf) -> Arc<RelBox> {
        let relations: Vec<RelationInfo> =
            WorldStateRelation::iter().map(relation_info_for).collect();

        RelBox::new(1 << 24, Some(dir), &relations, WorldStateSequences::COUNT)
    }

    #[test]
    fn open_reopen() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir_str = tmpdir.path().to_str().unwrap();

        let a = {
            let db = test_db(tmpdir.path().into());

            let tx = RelBoxTransaction::new(db.clone());

            let a = tx
                .create_object(
                    None,
                    ObjAttrs {
                        owner: Some(NOTHING),
                        name: Some("test".into()),
                        parent: Some(NOTHING),
                        location: Some(NOTHING),
                        flags: Some(BitEnum::new()),
                    },
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

            let tx = RelBoxTransaction::new(db.clone());

            let v_uuid = tx.resolve_verb(a, "test".into(), None).unwrap().uuid();
            assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);
            assert_eq!(tx.commit(), Ok(CommitResult::Success));

            db.shutdown();
        }
    }
}
