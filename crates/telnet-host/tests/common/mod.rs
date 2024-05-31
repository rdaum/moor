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
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::OnceLock,
    thread::{self},
    time::{Duration, Instant},
};

/// The current DB implementation reserves this much RAM. Default is 1TB, and
/// we rely on `vm.overcommit_memory` to allow this to be allocated. Instead of
/// trying to set `vm.overcommit_memory` on GitHub Actions test envs,
/// limit the DB size. This is plenty for the tests and, unlike the default,
/// allocation succeeds.
const MAX_BUFFER_POOL_BYTES: usize = 1 << 24;

struct ManagedChild {
    child: Child,
}
impl ManagedChild {
    fn new(name: &'static str, mut child: Child) -> Self {
        // Rust tests capture output, and hide it if the test passes unless `--nocapture` is passed to `cargo test`.
        // This does *not* automatically apply to subprocesses, so: start threads to send subprocess output through
        // `print!` / `eprintln!` to get the same behavior.
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let stderr = child.stderr.take().expect("Failed to get stderr");
        thread::spawn(|| {
            let name = name.to_string();
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                println!("[{name}]: {}", line.expect("Failed to read line"));
            }
        });
        thread::spawn(|| {
            let name = name.to_string();
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                eprintln!("[{name}]: {}", line.expect("Failed to read line"));
            }
        });
        Self { child }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.child.kill().expect("Failed to kill child process");
    }
}

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

pub struct Client {
    stream: TcpStream,
}
impl Client {
    fn new(port: u16) -> Result<Self, std::io::Error> {
        TcpStream::connect(format!("localhost:{port}")).and_then(|stream| {
            stream.set_read_timeout(Some(Duration::from_secs(1)))?;
            Ok(Self { stream })
        })
    }

    fn send_string<S>(&mut self, s: S) -> Result<(), std::io::Error>
    where
        S: AsRef<str>,
    {
        eprintln!(">> {}", s.as_ref());
        self.stream.write_all(s.as_ref().as_bytes())?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()
    }

    pub fn command<S>(&mut self, s: S) -> Result<String, std::io::Error>
    where
        S: AsRef<str>,
    {
        self.send_string(s)?;

        let mut lines = Vec::new();
        let mut reader = BufReader::new(&self.stream);

        // Wait for prefix
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line == "-=!-^-!=-\n" {
                break;
            }
        }

        // Read until suffix
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line == "-=!-v-!=-\n" {
                break;
            }
            eprintln!("<< {}", line.strip_suffix('\n').unwrap());
            lines.push(line);
        }
        Ok(lines.join(""))
    }
}

pub fn run_test_as<F>(connect_params: &[&str], f: F) -> eyre::Result<()>
where
    F: FnOnce(Client) -> eyre::Result<()>,
{
    let daemon_workdir = tempfile::TempDir::new()?;
    let _daemon = start_daemon(daemon_workdir.path());
    let _telnet_host = start_telnet_host();

    let start = Instant::now();
    loop {
        if let Ok(mut client) = Client::new(8080) {
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
