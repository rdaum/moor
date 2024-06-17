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

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

use moor_moot::{ManagedChild, MootClient};

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
    let mut minimal_db = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    minimal_db.push("tests/Test.db");

    ManagedChild::new(
        "daemon",
        Command::new(daemon_host_bin())
            .arg("--textdump")
            .arg(minimal_db)
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

pub fn run_test_as<F>(connect_params: &[&str], f: F) -> eyre::Result<()>
where
    F: FnOnce(MootClient) -> eyre::Result<()>,
{
    let daemon_workdir = tempfile::TempDir::new()?;
    let _daemon = start_daemon(daemon_workdir.path());
    let _telnet_host = start_telnet_host();

    let start = Instant::now();
    loop {
        if let Ok(mut client) = MootClient::new(8080) {
            client.send_string(format!("connect {}", connect_params.join(" ")))?;
            f(client)?;
            break;
        } else if start.elapsed() > Duration::from_secs(5) {
            panic!("Failed to connect to daemon");
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}
