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
use std::net::TcpListener;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

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

/// Base path for the PUB/SUB IPC sockets used by the daemon, a unique UUID is appended to this.
const NARRATIVE_PATH_ROOT: &str = "ipc:///tmp/narrative-moor-moot-daemon-";
/// Base path for the RPC IPC sockets used by the daemon, a unique UUID is appended to this.
const RPC_PATH_ROOT: &str = "ipc:///tmp/rpc-moor-moot-daemon.sock-";

fn start_daemon(workdir: &Path, uuid: Uuid) -> ManagedChild {
    ManagedChild::new(
        "daemon",
        Command::new(daemon_host_bin())
            .arg("--textdump")
            .arg(test_db_path())
            .arg("--generate-keypair")
            .arg("--narrative-listen")
            .arg(format!("{}{}", NARRATIVE_PATH_ROOT, uuid))
            .arg("--rpc-listen")
            .arg(format!("{}{}", RPC_PATH_ROOT, uuid))
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

fn start_telnet_host(uuid: Uuid, port: u16) -> ManagedChild {
    ManagedChild::new(
        "telnet-host",
        Command::new(telnet_host_bin())
            .arg("--narrative-server")
            .arg(format!("{}{}", NARRATIVE_PATH_ROOT, uuid))
            .arg("--rpc-server")
            .arg(format!("{}{}", RPC_PATH_ROOT, uuid))
            .arg("--telnet-address")
            .arg(format!("0.0.0.0:{}", port))
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

    // Assign our unique identifier for this test run to be used in the paths for the IPC sockets.
    let uuid = Uuid::new_v4();

    let daemon_workdir = tempfile::TempDir::new().expect("Failed to create temporary directory");

    let daemon = Arc::new(Mutex::new(start_daemon(daemon_workdir.path(), uuid)));

    // Ask the OS for a random unused port. Then immediately drop the listener and use the port
    // for the telnet host.
    let listener = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let telnet_host = Arc::new(Mutex::new(start_telnet_host(uuid, port)));

    let daemon_clone = daemon.clone();
    let telnet_host_clone = telnet_host.clone();
    let validate_state = move || {
        daemon_clone.lock().unwrap().assert_running()?;
        telnet_host_clone.lock().unwrap().assert_running()
    };

    execute_moot_test(
        TelnetMootRunner::new(port),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/moot")
            .join(moot_file)
            .with_extension("moot"),
        validate_state,
    );

    drop(daemon);
    drop(telnet_host);
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
