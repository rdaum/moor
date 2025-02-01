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

mod parser;

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::Child,
    sync::Once,
    thread,
    time::{Duration, Instant},
};

use eyre::{eyre, ContextCompat, WrapErr};
use moor_values::Obj;

use parser::{MootBlock, MootBlockSpan, MootBlockTestKind};
use pretty_assertions::assert_eq;

#[allow(dead_code)]
pub const WIZARD: Obj = Obj::mk_id(3);
#[allow(dead_code)]
pub const PROGRAMMER: Obj = Obj::mk_id(4);
#[allow(dead_code)]
pub const NONPROGRAMMER: Obj = Obj::mk_id(5);

#[allow(dead_code)]
static LOGGING_INIT: Once = Once::new();
#[allow(dead_code)]
fn init_logging() {
    LOGGING_INIT.call_once(|| {
        let main_subscriber = tracing_subscriber::fmt()
            .compact()
            .with_ansi(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_global_default(main_subscriber)
            .expect("Unable to configure logging");
    });
}
/// Look up the path to Test.db from any crate under the `moor` workspace
pub fn test_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../moot/Test.db")
}

pub trait MootRunner {
    type Value: PartialEq + std::fmt::Debug;

    fn eval<S: Into<String>>(&mut self, player: &Obj, command: S) -> eyre::Result<()>;
    fn command<S: AsRef<str>>(&mut self, player: &Obj, command: S) -> eyre::Result<()>;

    fn read_line(&mut self, player: &Obj) -> eyre::Result<Option<String>>;
    fn read_eval_result(&mut self, player: &Obj) -> eyre::Result<Option<Self::Value>>;

    fn none(&self) -> Self::Value;
}

fn run_moot_block<R: MootRunner>(
    runner: &mut R,
    player: &Obj,
    span: &MootBlockSpan,
) -> eyre::Result<Option<Obj>> {
    eprintln!("{:?}", span);
    let line_no = span.line_no;
    match &span.expr {
        MootBlock::ChangePlayer(change_player) => {
            return Ok(Some(match change_player.name {
                "wizard" => WIZARD,
                "programmer" => PROGRAMMER,
                "nonprogrammer" => NONPROGRAMMER,
                _ => return Err(eyre!("Unknown player: {}", change_player.name)),
            }));
        }

        MootBlock::Test(test) => {
            let prog = test.prog();
            match test.kind {
                MootBlockTestKind::Eval => {
                    runner.eval(player, format!("{prog} \"moot-line:{line_no}\";"))?;
                }
                MootBlockTestKind::Command => {
                    runner.command(player, prog)?;
                }
            }
            let expectation_line_no = test
                .expected_output
                .as_ref()
                .map(|e| e.line_no)
                .unwrap_or(line_no);
            if test.verbatim() {
                assert_raw_line(
                    runner,
                    player,
                    test.expected_output_str(),
                    expectation_line_no,
                )?;
            } else {
                assert_eval_result(
                    runner,
                    player,
                    test.expected_output_str(),
                    expectation_line_no,
                )?;
            }
        }
    };

    Ok(None)
}

fn assert_eval_result<R: MootRunner>(
    runner: &mut R,
    player: &Obj,
    expectation: Option<&str>,
    line_no: usize, // for the assertion message
) -> eyre::Result<()> {
    let err_prefix = || format!("assert_eval_result({player}, {expectation:?}, {line_no})");
    let actual = runner
        .read_eval_result(player)?
        .ok_or_else(|| eyre!("{}/actual: got no eval result", err_prefix()))?;

    let expected = if let Some(expectation) = expectation {
        runner
            .eval(
                player,
                format!("return {expectation}; \"moot-expect-line:{line_no}\";"),
            )
            .wrap_err_with(|| {
                format!(
                    "{}/expected: Failed to compile expected output: {expectation}",
                    err_prefix()
                )
            })?;
        runner
            .read_eval_result(player)?
            .wrap_err_with(|| format!("{}/expected: got no eval result", err_prefix()))?
    } else {
        runner.none()
    };

    // Send the common through the debug formatter, because MOO string comparison
    // is case-insensitive, but we want case-sensitive comparison in tests.
    assert_eq!(
        format!("{actual:?}"),
        format!("{expected:?}"),
        "Line {line_no}"
    );
    Ok(())
}

fn assert_raw_line<R: MootRunner>(
    runner: &mut R,
    player: &Obj,
    expectation: Option<&str>,
    line_no: usize,
) -> eyre::Result<()> {
    let actual = runner.read_line(player)?;
    assert_eq!(actual.as_deref(), expectation, "Line {line_no}");
    Ok(())
}

pub struct ManagedChild {
    name: &'static str,
    child: Child,
}
impl ManagedChild {
    pub fn new(name: &'static str, mut child: Child) -> Self {
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
        Self { name, child }
    }

    pub fn try_wait(&mut self) -> eyre::Result<Option<std::process::ExitStatus>> {
        self.child
            .try_wait()
            .wrap_err(format!("failed to wait: {}", self.name))
    }

    pub fn assert_running(&mut self) -> eyre::Result<()> {
        let status = self.try_wait()?;
        if status.is_some() {
            Err(eyre!("Unexpected exit: {}: {status:?}", self.name))
        } else {
            Ok(())
        }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        eprintln!("Killing {} (pid={})", self.name, self.child.id());
        self.child.kill().expect("Failed to kill child process");
    }
}

pub struct MootClient {
    stream: TcpStream,
}
impl MootClient {
    pub fn new(port: u16) -> eyre::Result<Self> {
        TcpStream::connect(format!("localhost:{port}"))
            .and_then(|stream| {
                stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                stream.set_write_timeout(Some(Duration::from_secs(1)))?;
                Ok(Self { stream })
            })
            .wrap_err_with(|| format!("MootClient::new({port})"))
    }

    fn port(&self) -> u16 {
        self.stream
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_default()
    }

    pub fn write_line<S>(&mut self, s: S) -> eyre::Result<()>
    where
        S: AsRef<str>,
    {
        let port = self.port();
        let mut writer = BufWriter::new(&mut self.stream);
        let result = writer
            .write_all(s.as_ref().as_bytes())
            .and_then(|_| writer.write_all(b"\n"))
            .wrap_err_with(|| format!("writing port={port}"));
        eprintln!("{} >> {}", port, s.as_ref());
        result
    }

    fn read_line(&self) -> eyre::Result<Option<String>> {
        let mut buf = String::new();
        match BufReader::new(&self.stream).read_line(&mut buf) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                eprintln!("{} read timeout", self.port());
                Ok(None)
            }
            Err(e) => {
                Err(e).wrap_err_with(|| format!("MootClient::read_line port={}", self.port()))
            }
            Ok(0) => Ok(None),
            Ok(_) => {
                let line = buf.trim_end_matches(['\r', '\n']).to_string();
                eprintln!("{} << {}", self.port(), line);
                Ok(Some(line))
            }
        }
    }
}

pub struct TelnetMootRunner {
    port: u16,
    clients: HashMap<Obj, MootClient>,
}
impl TelnetMootRunner {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            clients: HashMap::new(),
        }
    }

    fn client(&mut self, player: &Obj) -> &mut MootClient {
        self.clients.entry(player.clone()).or_insert_with(|| {
            let start = Instant::now();
            loop {
                if let Ok(mut client) = MootClient::new(self.port) {
                    client
                        .write_line(std::format!("connect {}", player))
                        .unwrap();
                    assert_eq!(
                        client.read_line().unwrap().as_deref(),
                        Some("*** Connected ***")
                    );
                    return client;
                } else if start.elapsed() > Duration::from_secs(5) {
                    panic!("Failed to connect to server @ {}", self.port);
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
    }

    fn resolve_response(&mut self, player: &Obj, response: String) -> eyre::Result<String> {
        let client = self.client(player);
        // Resolve the response; for example, the test assertion may be `$object`; resolve it to the object's specific number.
        client.write_line(format!(
            "; return {response}; \"TelnetMootRunner::resolve_response\";"
        ))?;
        client
            .read_line()
            .wrap_err_with(|| format!("TelnetMoorRunner::resolve_response({player}, {response:?})"))
            .and_then(|maybe_line| maybe_line.ok_or(eyre!("received no response from server")))
    }
}
impl MootRunner for TelnetMootRunner {
    type Value = String;

    fn eval<S: Into<String>>(&mut self, player: &Obj, command: S) -> eyre::Result<()> {
        let command: String = command.into();
        self.client(player)
            .write_line(format!("; {} \"TelnetMootRunner::eval\";", command))
            .with_context(|| format!("TelnetMootRunner::eval({player}, {:?})", command))
    }

    fn command<S: AsRef<str>>(&mut self, player: &Obj, command: S) -> eyre::Result<()> {
        let command: &str = command.as_ref();
        self.client(player)
            .write_line(command)
            .with_context(|| format!("TelnetMootRunner::command({player}, {:?}", command))
    }

    fn none(&self) -> Self::Value {
        "0".to_string()
    }

    fn read_line(&mut self, player: &Obj) -> eyre::Result<Option<String>> {
        self.client(player)
            .read_line()
            .with_context(|| format!("TelnetMootRunner::read_line({player})"))
    }

    fn read_eval_result(&mut self, player: &Obj) -> eyre::Result<Option<Self::Value>> {
        let raw = self
            .client(player)
            .read_line()
            .with_context(|| format!("TelnetMootRunner::read_eval_result({player}) / read raw"))?;
        if let Some(raw) = raw {
            self.resolve_response(player, raw)
                .map(Some)
                .with_context(|| format!("TelnetMootRunner::read_eval_result({player}) / resolve"))
        } else {
            Ok(None)
        }
    }
}

pub fn execute_moot_test<R: MootRunner, F: Fn() -> eyre::Result<()>>(
    mut runner: R,
    path: &Path,
    validate_state: F,
) {
    init_logging();
    eprintln!("Test definition: {}", path.display());

    let test = std::fs::read_to_string(path)
        .wrap_err(format!("{}", path.display()))
        .unwrap();

    let mut player = WIZARD;
    for span in parser::parse(&test).unwrap() {
        validate_state().unwrap_or_else(|e| {
            panic!(
                "Invalid state before processing line {}: {e:?}",
                span.line_no
            )
        });
        if let Some(new_player) = run_moot_block(&mut runner, &player, &span).unwrap() {
            player = new_player;
        }
    }
}
