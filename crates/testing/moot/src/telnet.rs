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

use eyre::{WrapErr, eyre};
use moor_values::Obj;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    process::Child,
    thread,
    time::{Duration, Instant},
};

use crate::MootRunner;

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

    fn read_command_result(&mut self, player: &Obj) -> eyre::Result<Option<Self::Value>> {
        self.client(player)
            .read_line()
            .map(|maybe_line| maybe_line.map(|line| format!("{line:?}")))
            .with_context(|| format!("TelnetMootRunner::read_command_result({player}) / read raw"))
    }
}
