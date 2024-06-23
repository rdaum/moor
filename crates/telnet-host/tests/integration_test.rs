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

use moor_moot::{test_db_path, ManagedChild};
use serial_test::serial;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

/// The current DB implementation reserves this much RAM. Default is 1TB, and
/// we rely on `vm.overcommit_memory` to allow this to be allocated. Instead of
/// trying to set `vm.overcommit_memory` on GitHub Actions test envs,
/// limit the DB size. This is plenty for the tests and, unlike the default,
/// allocation succeeds.
const MAX_BUFFER_POOL_BYTES: usize = 1 << 24;

static DAEMON_HOST_BIN: OnceLock<PathBuf> = OnceLock::new();
fn daemon_host_bin() -> &'static PathBuf {
    DAEMON_HOST_BIN.get_or_init(|| {
        escargot::CargoBuild::new()
            .bin("moor-daemon")
            .manifest_path("../daemon/Cargo.toml")
            .current_release()
            .run()
            .expect("Failed to build moor-daemon")
            .path()
            .to_owned()
    })
}

fn start_daemon(workdir: &Path) -> ManagedChild {
    ManagedChild::new(
        "daemon",
        Command::new(daemon_host_bin())
            .arg("--textdump")
            .arg(test_db_path())
            .arg("--generate-keypair")
            .arg("--max-buffer-pool-bytes")
            .arg(MAX_BUFFER_POOL_BYTES.to_string())
            .arg("test.db")
            .current_dir(workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start daemon"),
    )
}

static TELNET_HOST_BIN: OnceLock<PathBuf> = OnceLock::new();
fn telnet_host_bin() -> &'static PathBuf {
    TELNET_HOST_BIN.get_or_init(|| {
        escargot::CargoBuild::new()
            .bin("moor-telnet-host")
            .manifest_path("../telnet-host/Cargo.toml")
            .current_release()
            .run()
            .expect("Failed to build moor-telnet-host")
            .path()
            .to_owned()
    })
}

fn start_telnet_host() -> ManagedChild {
    ManagedChild::new(
        "telnet-host",
        Command::new(telnet_host_bin())
            .arg("--debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start telnet host"),
    )
}

// These tests all listen on the same port, so we need to make sure
// only one runs at a time.

fn test_moot_with_telnet_host<P: AsRef<Path>>(moot_file: P) {
    use moor_moot::{execute_moot_test, TelnetMootRunner};

    let daemon_workdir = tempfile::TempDir::new().expect("Failed to create temporary directory");
    let _daemon = start_daemon(daemon_workdir.path());
    let _telnet_host = start_telnet_host();

    execute_moot_test(
        TelnetMootRunner::new(8080),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/moot")
            .join(moot_file)
            .with_extension("moot"),
    );
}

#[cfg(target_os = "linux")]
#[test]
#[serial(telnet_host)]
fn test_echo() {
    test_moot_with_telnet_host("echo");
}

#[cfg(target_os = "linux")]
#[test]
#[serial(telnet_host)]
fn test_suspend_read_notify() {
    test_moot_with_telnet_host("suspend_read_notify");
}
