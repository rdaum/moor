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
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::Child,
    sync::Once,
    thread,
    time::{Duration, Instant},
};

use eyre::WrapErr;
use moor_values::var::Objid;

use pretty_assertions::assert_eq;

#[allow(dead_code)]
pub const WIZARD: Objid = Objid(3);
#[allow(dead_code)]
pub const PROGRAMMER: Objid = Objid(4);
#[allow(dead_code)]
pub const NONPROGRAMMER: Objid = Objid(5);

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
            .expect("Unable to set configure logging");
    });
}
/// Look up the path to Test.db from any crate under the `moor` workspace
pub fn test_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../moot/Test.db")
}

pub trait MootRunner {
    type Value: PartialEq + std::fmt::Debug;
    type Error: std::error::Error + Send + Sync + 'static;

    fn eval<S: Into<String>>(&mut self, player: Objid, command: S) -> Result<(), Self::Error>;
    fn command<S: AsRef<str>>(&mut self, player: Objid, command: S) -> Result<(), Self::Error>;

    fn read_line(&mut self, player: Objid) -> Result<Option<String>, Self::Error>;
    fn read_eval_result(&mut self, player: Objid) -> Result<Option<Self::Value>, Self::Error>;

    fn none(&self) -> Self::Value;
}

#[derive(Clone, Copy, Debug)]
pub enum CommandKind {
    Eval,
    Command,
}
impl From<char> for CommandKind {
    fn from(c: char) -> Self {
        match c {
            ';' => CommandKind::Eval,
            '%' => CommandKind::Command,
            _ => panic!("Unknown command kind: {}", c),
        }
    }
}

pub enum MootState<R: MootRunner> {
    Ready {
        runner: R,
        player: Objid,
    },
    ReadingCommand {
        runner: R,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
    },
    ReadingEvalAssertion {
        runner: R,
        player: Objid,
        line_no: usize,
        expectation: String,
    },
}
impl<R: MootRunner> MootState<R> {
    pub fn new(runner: R, player: Objid) -> Self {
        MootState::Ready { runner, player }
    }

    // Could implement this with `nom` I guess, but this seems simple enough, and it's probably easier to read.
    pub fn process_line(self, new_line_no: usize, line: &str) -> eyre::Result<Self> {
        let line = line.trim_end_matches('\n');
        match self {
            MootState::Ready { mut runner, player } => {
                if line.starts_with([';', '%']) {
                    Ok(MootState::ReadingCommand {
                        runner,
                        player,
                        line_no: new_line_no,
                        command: line[1..].trim_start().to_string(),
                        command_kind: line.chars().next().unwrap().into(),
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(runner, player))
                } else if let Some(expectation) = line.strip_prefix('=') {
                    Self::assert_raw_line(&mut runner, player, Some(expectation), new_line_no)?;
                    Ok(MootState::new(runner, player))
                } else {
                    Err(eyre::eyre!(
                        "Expected a command (starting `;`), a comment (starting `//`), a player switch (starting `@`), a command (starting `%`), or an empty line"
                    ))
                }
            }
            MootState::ReadingCommand {
                mut runner,
                player,
                line_no,
                mut command,
                command_kind,
            } => {
                if let Some(rest) = line.strip_prefix('>') {
                    command.push_str(rest);
                    Ok(MootState::ReadingCommand {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                    })
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Self::execute_command(&mut runner, player, &command, command_kind, line_no)?;
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.is_empty()
                    || line.starts_with("//")
                    || line.starts_with([';', '%', '='])
                {
                    Self::execute_command(&mut runner, player, &command, command_kind, line_no)?;
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    Self::execute_command(&mut runner, player, &command, command_kind, line_no)?;
                    let line = line.strip_prefix('<').unwrap_or(line);
                    Ok(MootState::ReadingEvalAssertion {
                        runner,
                        player,
                        line_no: new_line_no,
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingEvalAssertion {
                mut runner,
                player,
                line_no,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with("//") || line.starts_with([';', '%', '=']) {
                    Self::assert_eval_result(&mut runner, player, Some(&expectation), line_no)?;
                }
                if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(runner, player))
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.starts_with([';', '%', '=']) {
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    expectation.push('\n');
                    let line = line.strip_prefix('<').unwrap_or(line);
                    expectation.push_str(line);
                    Ok(MootState::ReadingEvalAssertion {
                        runner,
                        player,
                        line_no,
                        expectation,
                    })
                }
            }
        }
    }

    pub fn finalize(self) -> eyre::Result<()> {
        match self {
            MootState::Ready { mut runner, player } => {
                Self::assert_raw_line(&mut runner, player, None, 0)
            }
            MootState::ReadingCommand {
                mut runner,
                player,
                command,
                line_no,
                command_kind,
            } => {
                Self::execute_command(&mut runner, player, &command, command_kind, line_no)?;
                Self::assert_eval_result(&mut runner, player, None, line_no)
            }
            MootState::ReadingEvalAssertion {
                mut runner,
                player,
                line_no,
                expectation,
            } => Self::assert_eval_result(&mut runner, player, Some(&expectation), line_no),
        }
    }

    fn player(s: &str) -> eyre::Result<Objid> {
        match s {
            "wizard" => Ok(WIZARD),
            "programmer" => Ok(PROGRAMMER),
            "nonprogrammer" => Ok(NONPROGRAMMER),
            _ => Err(eyre::eyre!("Unknown player: {s}")),
        }
    }

    fn execute_command(
        runner: &mut R,
        player: Objid,
        command: &str,
        command_kind: CommandKind,
        line_no: usize,
    ) -> eyre::Result<()> {
        match command_kind {
            CommandKind::Eval => {
                runner.eval(player, &format!("{command} \"moot-line:{line_no}\";"))
            }
            CommandKind::Command => runner.command(player, command),
        }?;
        Ok(())
    }

    fn assert_eval_result(
        runner: &mut R,
        player: Objid,
        expectation: Option<&str>,
        line_no: usize, // for the assertion message
    ) -> eyre::Result<()> {
        let actual = runner
            .read_eval_result(player)?
            .expect("No eval result received");

        let expected = if let Some(expectation) = expectation {
            runner
                .eval(player, format!("return {expectation};"))
                .wrap_err(format!("Failed to compile expected output: {expectation}"))?;
            runner
                .read_eval_result(player)?
                .expect("No eval result received")
        } else {
            runner.none()
        };

        assert_eq!(actual, expected, "Line {line_no}");
        Ok(())
    }

    fn assert_raw_line(
        runner: &mut R,
        player: Objid,
        expectation: Option<&str>,
        line_no: usize,
    ) -> eyre::Result<()> {
        let actual = runner.read_line(player)?;
        assert_eq!(actual.as_deref(), expectation, "Line {line_no}");
        Ok(())
    }
}

pub struct ManagedChild {
    pub child: Child,
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
        Self { child }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.child.kill().expect("Failed to kill child process");
    }
}

pub struct MootClient {
    stream: TcpStream,
}
impl MootClient {
    pub fn new(port: u16) -> Result<Self, std::io::Error> {
        TcpStream::connect(format!("localhost:{port}")).and_then(|stream| {
            stream.set_read_timeout(Some(Duration::from_secs(1)))?;
            stream.set_write_timeout(Some(Duration::from_secs(1)))?;
            Ok(Self { stream })
        })
    }

    fn port(&self) -> u16 {
        self.stream
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_default()
    }

    pub fn write_line<S>(&mut self, s: S) -> Result<(), std::io::Error>
    where
        S: AsRef<str>,
    {
        eprintln!("{} >> {}", self.port(), s.as_ref());
        let mut writer = BufWriter::new(&mut self.stream);
        writer.write_all(s.as_ref().as_bytes()).unwrap();
        writer.write_all(b"\n").unwrap();
        Ok(())
    }

    fn read_line(&self) -> Result<Option<String>, std::io::Error> {
        let mut buf = String::new();
        match BufReader::new(&self.stream).read_line(&mut buf) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
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
    clients: HashMap<Objid, MootClient>,
}
impl TelnetMootRunner {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            clients: HashMap::new(),
        }
    }

    fn client(&mut self, player: Objid) -> &mut MootClient {
        self.clients.entry(player).or_insert_with(|| {
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
                    panic!("Failed to connect to daemon");
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
    }

    fn resolve_response(
        &mut self,
        player: Objid,
        response: String,
    ) -> Result<String, std::io::Error> {
        let client = self.client(player);
        // Resolve the response; for example, the test assertion may be `$object`; resolve it to the object's specific number.
        client.write_line(format!(
            "; return {response}; \"TelnetMootRunner::resolve_response\";"
        ))?;
        client.read_line().map(Option::unwrap)
    }
}
impl MootRunner for TelnetMootRunner {
    type Value = String;
    type Error = std::io::Error;

    fn eval<S: Into<String>>(&mut self, player: Objid, command: S) -> Result<(), Self::Error> {
        self.client(player)
            .write_line(format!("; {} \"TelnetMootRunner::eval\";", command.into()))?;
        Ok(())
    }

    fn command<S: AsRef<str>>(&mut self, player: Objid, command: S) -> Result<(), Self::Error> {
        self.client(player).write_line(command)
    }

    fn none(&self) -> Self::Value {
        "0".to_string()
    }

    fn read_line(&mut self, player: Objid) -> Result<Option<String>, Self::Error> {
        Ok(self.client(player).read_line().expect("Reading raw line"))
    }

    fn read_eval_result(&mut self, player: Objid) -> Result<Option<Self::Value>, Self::Error> {
        let raw = self
            .client(player)
            .read_line()
            .expect("Reading raw eval response");
        if let Some(raw) = raw {
            Ok(self
                .resolve_response(player, raw)
                .map(Some)
                .expect("Reading resolved eval response"))
        } else {
            Ok(None)
        }
    }
}

pub fn execute_moot_test<R: MootRunner>(runner: R, path: &Path) {
    init_logging();
    eprintln!("Test definition: {}", path.display());

    let f = BufReader::new(
        File::open(path)
            .wrap_err(format!("{}", path.display()))
            .unwrap(),
    );

    let mut state = MootState::new(runner, WIZARD);
    for (line_no, line) in f.lines().enumerate() {
        let line = line.unwrap();
        let line_no = line_no + 1;
        state = state
            .process_line(line_no, &line)
            .wrap_err(format!("line {}", line_no))
            .unwrap();
        //eprintln!("[{line_no}] {line}");
    }
    state.finalize().unwrap();
}
