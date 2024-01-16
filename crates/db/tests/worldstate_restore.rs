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
    use moor_db::db_tx::DbTransaction;
    use moor_db::odb::RelBoxTransaction;
    use moor_db::odb::{WorldStateRelation, WorldStateSequences};
    use moor_db::rdb::{RelBox, RelationInfo};
    use moor_values::model::BinaryType;
    use moor_values::model::CommitResult;
    use moor_values::model::HasUuid;
    use moor_values::model::ObjAttrs;
    use moor_values::model::VerbArgsSpec;
    use moor_values::util::BitEnum;
    use moor_values::NOTHING;
    use std::path::PathBuf;
    use std::sync::Arc;
    use strum::{EnumCount, IntoEnumIterator};

    pub async fn test_db(dir: PathBuf) -> Arc<RelBox> {
        let mut relations: Vec<RelationInfo> = WorldStateRelation::iter()
            .map(|wsr| {
                RelationInfo {
                    name: wsr.to_string(),
                    domain_type_id: 0, /* tbd */
                    codomain_type_id: 0,
                    secondary_indexed: false,
                }
            })
            .collect();
        relations[WorldStateRelation::ObjectParent as usize].secondary_indexed = true;
        relations[WorldStateRelation::ObjectLocation as usize].secondary_indexed = true;

        RelBox::new(1 << 24, Some(dir), &relations, WorldStateSequences::COUNT).await
    }

    #[tokio::test]
    async fn open_reopen() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir_str = tmpdir.path().to_str().unwrap();

        let a = {
            let db = test_db(tmpdir.path().into()).await;

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
                .await
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
            .await
            .unwrap();

            tx.commit().await.unwrap();
            db.shutdown().await;

            // TODO: this should not be necessary, but seems to be to pass the test (!?).
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            a
        };
        // Verify the WAL directory is not empty.
        assert!(std::fs::read_dir(format!("{}/wal", tmpdir_str))
            .unwrap()
            .next()
            .is_some());
        {
            let db = test_db(tmpdir.path().into()).await;

            // Verify the pages directory is not empty after recovery.
            assert!(std::fs::read_dir(format!("{}/pages", tmpdir_str))
                .unwrap()
                .next()
                .is_some());

            let tx = RelBoxTransaction::new(db.clone());

            let v_uuid = tx
                .resolve_verb(a, "test".into(), None)
                .await
                .unwrap()
                .uuid();
            assert_eq!(tx.get_verb_binary(a, v_uuid).await.unwrap(), vec![]);
            assert_eq!(tx.commit().await, Ok(CommitResult::Success));

            db.shutdown().await;
        }
    }
}
