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

#![cfg(target_os = "linux")]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg_attr(coverage_nightly, coverage(off))]
use moor_moot::{MootOptions, telnet::ManagedChild, test_db_path};
use serial_test::serial;
use std::net::TcpListener;
use std::sync::OnceLock;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

static DAEMON_HOST_BIN: OnceLock<PathBuf> = OnceLock::new();
fn daemon_host_bin() -> &'static PathBuf {
    DAEMON_HOST_BIN.get_or_init(|| {
        // If `moor-daemon` already exists, use it.
        // This is potentially misleading: if you change daemon code, then re-run an integration test,
        // then you won't automatically get a new build. You'll need to manually build the daemon binary.
        // This is not a common workflow, and instead rebuilding the daemon all the time is a problem.
        //
        // This horribleness can be dropped whenever https://rust-lang.github.io/rfcs/3028-cargo-binary-dependencies.html
        // is implemented.
        //
        // Approach adopted from snapbox: https://docs.rs/snapbox/0.6.21/src/snapbox/cmd.rs.html#853-872
        let moor_daemon_path = std::env::current_exe()
            .ok()
            .map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
            .unwrap()
            .join(format!("moor-daemon{}", std::env::consts::EXE_SUFFIX));
        if moor_daemon_path.exists() {
            eprintln!("Using existing moor-daemon binary: {:?}", moor_daemon_path);
            return moor_daemon_path;
        }

        // If `moor-daemon` doesn't exist, build it.
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
            .arg("--events-listen")
            .arg(format!("{}{}", NARRATIVE_PATH_ROOT, uuid))
            .arg("--rpc-listen")
            .arg(format!("{}{}", RPC_PATH_ROOT, uuid))
            .arg("test.db")
            .current_dir(workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start daemon"),
    )
}

fn telnet_host_bin() -> &'static str {
    env!("CARGO_BIN_EXE_moor-telnet-host")
}

fn start_telnet_host(workdir: &Path, uuid: Uuid, port: u16) -> ManagedChild {
    ManagedChild::new(
        "telnet-host",
        Command::new(telnet_host_bin())
            .arg("--events-address")
            .arg(format!("{}{}", NARRATIVE_PATH_ROOT, uuid))
            .arg("--rpc-address")
            .arg(format!("{}{}", RPC_PATH_ROOT, uuid))
            .arg("--telnet-address")
            .arg("0.0.0.0")
            .arg("--telnet-port")
            .arg(format!("{}", port))
            .arg("--debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(workdir)
            .spawn()
            .expect("Failed to start telnet host"),
    )
}

// Just a keypair generated with openssl to satisfy the daemon for running unit tests...

const SIGNING_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEILrkKmddHFUDZqRCnbQsPoW/Wsp0fLqhnv5KNYbcQXtk
-----END PRIVATE KEY-----
"#;

const VERIFYING_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAZQUxGvw8u9CcUHUGLttWFZJaoroXAmQgUGINgbBlVYw=
-----END PUBLIC KEY-----
"#;

// These tests all listen on the same port, so we need to make sure
// only one runs at a time.

fn test_moot_with_telnet_host<P: AsRef<Path>>(moot_file: P) {
    use moor_moot::{execute_moot_test, telnet::TelnetMootRunner};

    // Assign our unique identifier for this test run to be used in the paths for the IPC sockets.
    let uuid = Uuid::new_v4();

    let test_workdir = tempfile::TempDir::new().expect("Failed to create temporary directory");

    // Write the private and public key files in the test workdir
    let signing_key_file = test_workdir.path().join("moor-signing-key.pem");
    std::fs::write(&signing_key_file, SIGNING_KEY).expect("Failed to write signing key file");
    let verifying_key_file = test_workdir.path().join("moor-verifying-key.pem");
    std::fs::write(&verifying_key_file, VERIFYING_KEY).expect("Failed to write verifying key file");

    let daemon = Arc::new(Mutex::new(start_daemon(test_workdir.path(), uuid)));
    daemon.lock().unwrap().assert_running().unwrap();

    // Ask the OS for a random unused port. Then immediately drop the listener and use the port
    // for the telnet host.
    let listener = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let telnet_host = Arc::new(Mutex::new(start_telnet_host(
        test_workdir.path(),
        uuid,
        port,
    )));

    let daemon_clone = daemon.clone();
    let telnet_host_clone = telnet_host.clone();
    let validate_state = move || {
        daemon_clone.lock().unwrap().assert_running()?;
        telnet_host_clone.lock().unwrap().assert_running()
    };

    let moot_options = MootOptions::default();
    execute_moot_test(
        TelnetMootRunner::new(port),
        &moot_options,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/moot")
            .join(moot_file)
            .with_extension("moot"),
        validate_state,
    );

    drop(daemon);
    drop(telnet_host);
}

// TODO: Disables these for now because of ongoing issues where these run with out of date daemon
//  binaries unless done from a clean build.

#[ignore]
#[test]
#[serial(telnet_host)]
fn test_echo() {
    test_moot_with_telnet_host("echo");
}

#[ignore]
#[test]
#[serial(telnet_host)]
fn test_suspend_read_notify() {
    test_moot_with_telnet_host("suspend_read_notify");
}

#[ignore]
#[test]
#[serial(telnet_host)]
fn test_huh() {
    test_moot_with_telnet_host("huh");
}
