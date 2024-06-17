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
    path::PathBuf,
    process::Child,
    thread,
    time::Duration,
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

/// Look up the path to Test.db from any crate under the `moor` workspace
pub fn test_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../moot/Test.db")
}

pub trait MootRunner {
    type Value: PartialEq + std::fmt::Debug;
    type Error: std::error::Error + Send + Sync + 'static;

    fn eval<S: Into<String>>(
        &mut self,
        player: Objid,
        command: S,
    ) -> Result<Self::Value, Self::Error>;

    fn command<S: AsRef<str>>(
        &mut self,
        player: Objid,
        command: S,
    ) -> Result<Self::Value, Self::Error>;

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
    ReadingExpectation {
        runner: R,
        player: Objid,
        line_no: usize,
        command: String,
        command_kind: CommandKind,
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
            MootState::Ready { runner, player } => {
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
                    Self::execute_test(&mut runner, player, &command, command_kind, None, line_no)?;
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) || line.is_empty() {
                    Self::execute_test(&mut runner, player, &command, command_kind, None, line_no)?;
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    let line = line.strip_prefix('<').unwrap_or(line);
                    Ok(MootState::ReadingExpectation {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                        expectation: line.to_string(),
                    })
                }
            }
            MootState::ReadingExpectation {
                mut runner,
                player,
                line_no,
                command,
                command_kind,
                mut expectation,
            } => {
                if line.is_empty() || line.starts_with("//") || line.starts_with([';', '%']) {
                    Self::execute_test(
                        &mut runner,
                        player,
                        &command,
                        command_kind,
                        Some(&expectation),
                        line_no,
                    )?;
                }
                if line.is_empty() || line.starts_with("//") {
                    Ok(MootState::new(runner, player))
                } else if let Some(new_player) = line.strip_prefix('@') {
                    Ok(MootState::new(runner, Self::player(new_player)?))
                } else if line.starts_with([';', '%']) {
                    MootState::new(runner, player).process_line(new_line_no, line)
                } else {
                    expectation.push('\n');
                    let line = line.strip_prefix('<').unwrap_or(line);
                    expectation.push_str(line);
                    Ok(MootState::ReadingExpectation {
                        runner,
                        player,
                        line_no,
                        command,
                        command_kind,
                        expectation,
                    })
                }
            }
        }
    }

    pub fn finalize(self) -> eyre::Result<()> {
        match self {
            MootState::Ready { .. } => Ok(()),
            MootState::ReadingCommand {
                mut runner,
                player,
                command,
                line_no,
                command_kind,
            } => Self::execute_test(&mut runner, player, &command, command_kind, None, line_no),
            MootState::ReadingExpectation {
                mut runner,
                player,
                line_no,
                command,
                command_kind,
                expectation,
            } => Self::execute_test(
                &mut runner,
                player,
                &command,
                command_kind,
                Some(&expectation),
                line_no,
            ),
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

    fn execute_test(
        runner: &mut R,
        player: Objid,
        command: &str,
        command_kind: CommandKind,
        expectation: Option<&str>,
        line_no: usize,
    ) -> eyre::Result<()> {
        let expected = if let Some(expectation) = expectation {
            runner
                .eval(WIZARD, format!("return {expectation};"))
                .wrap_err(format!("Failed to compile expected output: {expectation}"))?
        } else {
            runner.none()
        };

        let actual = match command_kind {
            CommandKind::Eval => {
                runner.eval(player, &format!("{command} \"moot-line:{line_no}\";"))
            }
            CommandKind::Command => runner.command(player, command),
        }?;
        assert_eq!(actual, expected, "Line {line_no}: {command}");
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

    pub fn send_string<S>(&mut self, s: S) -> Result<(), std::io::Error>
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
            if reader.read_line(&mut line)? == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "EOF while waiting for prefix",
                ));
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line == "-=!-^-!=-" {
                break;
            }
            eprintln!("[waiting for prefix] {}", line);
        }

        // Read until suffix
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line == "-=!-v-!=-" {
                break;
            }
            eprintln!("<< {line}");
            lines.push(line.to_string());
        }
        Ok(lines.join(""))
    }
}
