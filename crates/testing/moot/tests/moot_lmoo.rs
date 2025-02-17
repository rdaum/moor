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

//! Execute `.moot` tests against a MOO server listening over telnet.
//! Configured using ENV vars
//! * MOOT_MOO_PATH: path to the `moo` binary, defaults to `$HOME/MOO-1.8.1/moo`
//! * MOOT_DB_PATH: path to the textdump file, defaults to the `Test.db` next to this file
//! * MOOT_PORT: port the MOO server listens on, defaults to 7777

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use moor_moot::{
    execute_moot_test, telnet::ManagedChild, telnet::TelnetMootRunner, test_db_path, MootOptions,
};

fn moo_path() -> PathBuf {
    env::var("MOOT_MOO_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let mut path = PathBuf::from(env::var("HOME").unwrap());
            path.push("MOO-1.8.1/moo");
            path
        })
}

fn db_path() -> PathBuf {
    env::var("MOOT_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| test_db_path())
}

fn moo_port() -> u16 {
    env::var("MOOT_PORT")
        .map(|s| s.parse().unwrap())
        .unwrap_or(7777)
}

fn start_moo() -> ManagedChild {
    ManagedChild::new(
        "moo",
        Command::new(moo_path())
            .arg(db_path())
            .arg("/dev/null")
            .arg(moo_port().to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to start moo server {}", moo_path().display())),
    )
}

fn test_moo(path: &Path) {
    let moo = Arc::new(Mutex::new(start_moo()));
    let moo_clone = moo.clone();
    let validate_state = move || moo_clone.lock().unwrap().assert_running();

    let moot_options = MootOptions::default();
    execute_moot_test(
        TelnetMootRunner::new(moo_port()),
        &moot_options,
        path,
        validate_state,
    );

    drop(moo);
}

#[test]
#[ignore = "Useful for debugging; just run a single test against 'real' MOO"]
fn test_single() {
    test_moo(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../kernel/testsuite/moot/objects/test_parent_chparent.moot"),
    );
}
