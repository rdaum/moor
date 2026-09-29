// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::tasks::scheduler::*;
use moor_common::{
    model::{ObjFlag, ObjectKind, PropFlag, TaskPermissions, WorldStateSource},
    tasks::NoopSystemControl,
    util::BitEnum,
};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_float, v_obj};

#[test]
fn commit_queue_policy_loads_from_server_options() {
    let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let mut tx = database.new_world_state().unwrap();
    let system = tx
        .create_object(
            &TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new()),
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert_eq!(system, SYSTEM_OBJECT);
    let server_options = tx
        .create_object(
            &TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new()),
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    for (object, name, value) in [
        (SYSTEM_OBJECT, "server_options", v_obj(server_options)),
        (
            server_options,
            "db_commit_queue_warn_seconds",
            v_float(0.25),
        ),
        (
            server_options,
            "db_commit_queue_timeout_seconds",
            v_float(1.5),
        ),
    ] {
        tx.define_property(
            &TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new()),
            &object,
            &object,
            Symbol::mk(name),
            &SYSTEM_OBJECT,
            PropFlag::all_flags(),
            Some(value),
        )
        .unwrap();
    }
    tx.commit().unwrap();

    let scheduler = Scheduler::new(
        semver::Version::new(0, 0, 0),
        Box::new(database),
        Box::new(crate::tasks::NoopTasksDb {}),
        Arc::new(Config::default()),
        Arc::new(NoopSystemControl::default()),
        None,
        None,
    );
    let options = scheduler.server_options.load();
    assert_eq!(options.db_commit_queue_warn, Duration::from_millis(250));
    assert_eq!(options.db_commit_queue_timeout, Duration::from_millis(1500));
}
