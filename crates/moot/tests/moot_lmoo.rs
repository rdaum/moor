//! Execute `.moot` tests against a MOO server listening over telnet.
//! Configured using ENV vars
//! * MOOT_MOO_PATH: path to the `moo` binary, defaults to `$HOME/MOO-1.8.1/moo`
//! * MOOT_DB_PATH: path to the textdump file, defaults to the `Test.db` next to this file
//! * MOOT_PORT: port the MOO server listens on, defaults to 7777

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use moor_moot::{execute_moot_test, test_db_path, ManagedChild, TelnetMootRunner};

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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to start moo server {}", moo_path().display())),
    )
}

fn test_moo(path: &Path) {
    let mut _moo = start_moo();
    execute_moot_test(TelnetMootRunner::new(moo_port()), path)
}

#[test]
#[ignore = "Useful for debugging; just run a single test against 'real' MOO"]
fn test_single() {
    test_moo(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/testsuite/moot/recycle.moot"),
    );
}
