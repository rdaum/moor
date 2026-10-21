// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Socket-level tests of the telnet protocol layer.
//!
//! One daemon runs `Test.db` for the whole file, with four telnet hosts in front of it: passive
//! (defaults), offers (offer on connect), full (most protocols, from a config file), and capped
//! (GMCP with a 64 byte subnegotiation limit). Each test talks raw telnet on a TCP socket and
//! checks the bytes it receives.

#![cfg(target_os = "linux")]

mod common;

use flate2::{Decompress, FlushDecompress, Status};
use moor_moot::telnet::ManagedChild;
use serial_test::serial;
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::Command,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const GA: u8 = 249;
const SE: u8 = 240;
const EOR: u8 = 239;

const OPT_ECHO: u8 = 1;
const OPT_SGA: u8 = 3;
const OPT_TTYPE: u8 = 24;
const OPT_EOR: u8 = 25;
const OPT_NAWS: u8 = 31;
const OPT_CHARSET: u8 = 42;
const OPT_MSDP: u8 = 69;
const OPT_MSSP: u8 = 70;
const OPT_MCCP2: u8 = 86;
const OPT_GMCP: u8 = 201;
/// An option no host implements.
const OPT_UNKNOWN: u8 = 102;

/// How long to wait for something that should arrive.
const EXPECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long silence must last to count as "nothing was sent".
const QUIET: Duration = Duration::from_millis(400);

/// One element of the byte stream from the host.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ev {
    /// Text bytes, with `IAC IAC` already turned into one 0xFF.
    Text(Vec<u8>),
    /// `IAC <verb> <option>`.
    Negotiate(u8, u8),
    /// `IAC SB <option> <data> IAC SE`, data unescaped.
    Subneg(u8, Vec<u8>),
    Ga,
    Eor,
    /// Any other `IAC <cmd>`.
    Command(u8),
}

/// Parse one event from the front of `buf`. `None` when `buf` holds an incomplete sequence.
fn parse_one(buf: &[u8]) -> Option<(Ev, usize)> {
    let first = *buf.first()?;
    if first != IAC {
        let end = buf.iter().position(|&b| b == IAC).unwrap_or(buf.len());
        return Some((Ev::Text(buf[..end].to_vec()), end));
    }
    let cmd = *buf.get(1)?;
    match cmd {
        IAC => Some((Ev::Text(vec![IAC]), 2)),
        WILL | WONT | DO | DONT => Some((Ev::Negotiate(cmd, *buf.get(2)?), 3)),
        SB => {
            let option = *buf.get(2)?;
            let mut data = Vec::new();
            let mut i = 3;
            loop {
                let b = *buf.get(i)?;
                if b != IAC {
                    data.push(b);
                    i += 1;
                    continue;
                }
                match *buf.get(i + 1)? {
                    IAC => data.push(IAC),
                    SE => return Some((Ev::Subneg(option, data), i + 2)),
                    other => panic!("IAC {other} inside a subnegotiation from the host"),
                }
                i += 2;
            }
        }
        GA => Some((Ev::Ga, 2)),
        EOR => Some((Ev::Eor, 2)),
        other => Some((Ev::Command(other), 2)),
    }
}

/// Merge adjacent text events.
fn coalesce(events: Vec<Ev>) -> Vec<Ev> {
    let mut out: Vec<Ev> = Vec::with_capacity(events.len());
    for ev in events {
        if let (Some(Ev::Text(last)), Ev::Text(more)) = (out.last_mut(), &ev) {
            last.extend_from_slice(more);
            continue;
        }
        out.push(ev);
    }
    out
}

/// All text in `events`, concatenated.
fn text_of(events: &[Ev]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| match e {
            Ev::Text(t) => Some(t.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// IAC-escape `data`.
fn escape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        out.push(b);
        if b == IAC {
            out.push(IAC);
        }
    }
    out
}

/// Inflate all of `input` into `out`.
fn inflate(d: &mut Decompress, input: &[u8], out: &mut Vec<u8>) {
    let mut pos = 0;
    loop {
        let mut chunk = Vec::with_capacity(64 * 1024);
        let before = d.total_in();
        let status = d
            .decompress_vec(&input[pos..], &mut chunk, FlushDecompress::Sync)
            .expect("MCCP2 stream does not inflate");
        pos += (d.total_in() - before) as usize;
        let full = chunk.len() == chunk.capacity();
        let progressed = d.total_in() != before || !chunk.is_empty();
        out.extend_from_slice(&chunk);
        if status == Status::StreamEnd || !progressed || (pos >= input.len() && !full) {
            return;
        }
    }
}

static MARKER: AtomicUsize = AtomicUsize::new(0);

fn next_marker() -> String {
    format!("M{}", MARKER.fetch_add(1, Ordering::Relaxed))
}

/// A raw telnet client: bytes in, telnet events out.
struct Client {
    stream: TcpStream,
    /// Received bytes not yet parsed (after MCCP2 inflation).
    plain: Vec<u8>,
    queue: VecDeque<Ev>,
    /// Set once the host has started MCCP2.
    inflater: Option<Decompress>,
    /// Every byte read from the socket, before inflation.
    wire: Vec<u8>,
    closed: bool,
}

impl Client {
    fn connect(port: u16) -> Self {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to telnet host");
        stream.set_nodelay(true).unwrap();
        Self {
            stream,
            plain: Vec::new(),
            queue: VecDeque::new(),
            inflater: None,
            wire: Vec::new(),
            closed: false,
        }
    }

    // --- sending ---

    fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).expect("write to telnet host");
    }

    fn negotiate(&mut self, verb: u8, option: u8) {
        self.send(&[IAC, verb, option]);
    }

    fn will(&mut self, option: u8) {
        self.negotiate(WILL, option);
    }

    fn wont(&mut self, option: u8) {
        self.negotiate(WONT, option);
    }

    fn do_(&mut self, option: u8) {
        self.negotiate(DO, option);
    }

    fn dont(&mut self, option: u8) {
        self.negotiate(DONT, option);
    }

    /// `IAC SB <option> <data, escaped> IAC SE`.
    fn subneg(&mut self, option: u8, data: &[u8]) {
        let mut out = vec![IAC, SB, option];
        out.extend(escape(data));
        out.extend([IAC, SE]);
        self.send(&out);
    }

    fn gmcp(&mut self, message: &str) {
        self.subneg(OPT_GMCP, message.as_bytes());
    }

    fn line(&mut self, text: &str) {
        self.line_bytes(text.as_bytes());
    }

    /// Send `bytes` followed by CRLF, with no escaping.
    fn line_bytes(&mut self, bytes: &[u8]) {
        let mut out = bytes.to_vec();
        out.extend_from_slice(b"\r\n");
        self.send(&out);
    }

    // --- receiving ---

    /// Read once, waiting at most `timeout`. Returns false when nothing arrived.
    fn pump(&mut self, timeout: Duration) -> bool {
        if self.closed {
            return false;
        }
        let timeout = timeout.max(Duration::from_millis(1));
        self.stream.set_read_timeout(Some(timeout)).unwrap();
        let mut buf = [0u8; 65536];
        match self.stream.read(&mut buf) {
            Ok(0) => {
                self.closed = true;
                false
            }
            Ok(n) => {
                self.feed(&buf[..n]);
                true
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => false,
            Err(e) if e.kind() == ErrorKind::ConnectionReset => {
                self.closed = true;
                false
            }
            Err(e) => panic!("read from telnet host: {e}"),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.wire.extend_from_slice(bytes);
        match self.inflater.as_mut() {
            Some(d) => inflate(d, bytes, &mut self.plain),
            None => self.plain.extend_from_slice(bytes),
        }
        while let Some((ev, n)) = parse_one(&self.plain) {
            self.plain.drain(..n);
            let starts_mccp2 = ev == Ev::Subneg(OPT_MCCP2, vec![]) && self.inflater.is_none();
            self.queue.push_back(ev);
            if starts_mccp2 {
                // Everything after the start marker is zlib.
                let rest = std::mem::take(&mut self.plain);
                let mut d = Decompress::new(true);
                inflate(&mut d, &rest, &mut self.plain);
                self.inflater = Some(d);
            }
        }
    }

    /// Read events until `done` holds for the events so far; return them, text merged.
    fn read_until(&mut self, what: &str, done: impl Fn(&[Ev]) -> bool) -> Vec<Ev> {
        let deadline = Instant::now() + EXPECT_TIMEOUT;
        let mut got = Vec::new();
        loop {
            while let Some(ev) = self.queue.pop_front() {
                got.push(ev);
                if done(&got) {
                    return coalesce(got);
                }
            }
            let now = Instant::now();
            if now >= deadline || self.closed {
                panic!(
                    "timed out waiting for {what} (closed: {}); received {:?}",
                    self.closed,
                    show(&coalesce(got))
                );
            }
            self.pump(deadline - now);
        }
    }

    /// Wait for `IAC <verb> <option>`; return it and everything before it.
    fn expect_negotiation(&mut self, verb: u8, option: u8) -> Vec<Ev> {
        let want = Ev::Negotiate(verb, option);
        self.read_until(&format!("{want:?}"), |got| got.last() == Some(&want))
    }

    /// Wait for a subnegotiation of `option`; return its data.
    fn expect_subneg(&mut self, option: u8) -> Vec<u8> {
        let got = self.read_until(
            &format!("SB {option}"),
            |got| matches!(got.last(), Some(Ev::Subneg(o, _)) if *o == option),
        );
        match got.last() {
            Some(Ev::Subneg(_, data)) => data.clone(),
            _ => unreachable!(),
        }
    }

    /// Wait until the received text contains `needle`; return everything received.
    fn expect_text_contains(&mut self, needle: &[u8]) -> Vec<Ev> {
        let what = format!("text {:?}", String::from_utf8_lossy(needle));
        self.read_until(&what, |got| contains(&text_of(got), needle))
    }

    /// Read everything that arrives within `duration`.
    fn collect_for(&mut self, duration: Duration) -> Vec<Ev> {
        let deadline = Instant::now() + duration;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.pump(deadline - now);
        }
        coalesce(self.queue.drain(..).collect())
    }

    /// Fail if any byte arrives within `duration`.
    fn assert_no_bytes_for(&mut self, duration: Duration) {
        let before = self.wire.len();
        let got = self.collect_for(duration);
        assert!(
            got.is_empty() && self.wire.len() == before && self.plain.is_empty(),
            "expected no bytes, received {:?} (raw {:?})",
            show(&got),
            &self.wire[before..]
        );
    }

    // --- MOO helpers (Test.db `; <code>` runs `eval` as the logged-in player) ---

    /// `connect #3` and wait for the banner. Returns everything received up to it.
    fn login(&mut self) -> Vec<Ev> {
        self.line("connect #3");
        self.expect_text_contains(b"*** Connected ***\r\n")
    }

    /// Run MOO statements; return what arrived before the eval reply, text merged.
    fn run(&mut self, statements: &str) -> Vec<Ev> {
        self.run_bytes(statements.as_bytes())
    }

    fn run_bytes(&mut self, statements: &[u8]) -> Vec<Ev> {
        let marker = format!("\"{}\"\r\n", next_marker());
        let mut line = b"; ".to_vec();
        line.extend_from_slice(statements);
        line.extend_from_slice(format!(" return {};", marker.trim_end()).as_bytes());
        self.line_bytes(&line);
        let mut got = self.expect_text_contains(marker.as_bytes());
        let Some(Ev::Text(last)) = got.last_mut() else {
            unreachable!()
        };
        assert!(
            last.ends_with(marker.as_bytes()),
            "output after the eval reply: {:?}",
            show(&got)
        );
        last.truncate(last.len() - marker.len());
        if last.is_empty() {
            got.pop();
        }
        got
    }

    /// Evaluate a MOO expression; return its literal form.
    fn value(&mut self, expr: &str) -> String {
        self.value_bytes(expr.as_bytes())
    }

    fn value_bytes(&mut self, expr: &[u8]) -> String {
        let marker = next_marker();
        let prefix = format!("{{\"{marker}\", ");
        let mut line = format!("; return {prefix}").into_bytes();
        line.extend_from_slice(expr);
        line.extend_from_slice(b"};");
        self.line_bytes(&line);
        let got = self.read_until(&format!("value {marker}"), |got| {
            let text = text_of(got);
            text.windows(prefix.len())
                .position(|w| w == prefix.as_bytes())
                .is_some_and(|at| contains(&text[at..], b"\r\n"))
        });
        let text = text_of(&got);
        let at = text
            .windows(prefix.len())
            .position(|w| w == prefix.as_bytes())
            .unwrap();
        let rest = &text[at + prefix.len()..];
        let end = rest.windows(2).position(|w| w == b"\r\n").unwrap();
        let literal = rest[..end].strip_suffix(b"}").expect("value literal");
        String::from_utf8_lossy(literal).into_owned()
    }

    /// The connection object, e.g. `#-12`.
    fn conn(&mut self) -> String {
        self.value("connection()")
    }

    fn options(&mut self) -> String {
        self.value("connection_options(connection())")
    }
}

/// A readable form of events for failure messages.
fn show(events: &[Ev]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            Ev::Text(t) => format!("Text({:?})", String::from_utf8_lossy(t)),
            Ev::Subneg(o, d) => format!("Subneg({o}, {:?})", String::from_utf8_lossy(d)),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The daemon and hosts shared by every test in this file.
struct Fixture {
    /// Default configuration: every protocol off.
    passive: u16,
    /// GMCP, EOR, CHARSET, NAWS, TTYPE, offered at connect.
    offers: u16,
    /// Most protocols on, nothing offered at connect.
    full: u16,
    /// GMCP with `max_subneg` 64.
    capped: u16,
    workdir: PathBuf,
    children: Mutex<Vec<ManagedChild>>,
}

static FIXTURE: OnceLock<Fixture> = OnceLock::new();

/// Kill the processes and remove the work directory when the test binary exits.
extern "C" fn cleanup() {
    let Some(f) = FIXTURE.get() else {
        return;
    };
    if let Ok(mut children) = f.children.lock() {
        children.clear();
    }
    let _ = std::fs::remove_dir_all(&f.workdir);
}

/// Spawn `command` so that it is killed when this process dies, even without `cleanup`.
fn spawn(name: &'static str, mut command: Command) -> ManagedChild {
    // SAFETY: prctl is async-signal-safe and touches no memory of the parent.
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
    ManagedChild::new(name, command.spawn().expect("spawn"))
}

const FULL_CONFIG: &str = "\
protocols:
  gmcp: true
  msdp: true
  mssp: true
  naws: true
  ttype: true
  eor: true
  charset: true
  mccp2: true
  mxp: true
  client_data_rate: 0
  mssp_values:
    NAME: TestMOO
";

/// `#0:do_client_data` records `{player, args}` in `#0.cd_log`; `#0:client_data_for(conn)`
/// returns the entries for one connection. `#0:do_out_of_band_command` records
/// `{player, args}` in `#0.oob_log`.
const INSTALL_RECORDER: &str = r#"add_property(#0, "cd_log", {}, {player, "r"}); add_verb(#0, {player, "rxd", "do_client_data"}, {"this", "none", "this"}); set_verb_code(#0, "do_client_data", {"this.cd_log = {@this.cd_log, {player, args}};"}); add_verb(#0, {player, "rxd", "client_data_for"}, {"this", "none", "this"}); set_verb_code(#0, "client_data_for", {"r = {};", "for e in (this.cd_log)", "if (e[2][1] == args[1])", "r = {@r, e};", "endif", "endfor", "return r;"}); add_property(#0, "oob_log", {}, {player, "r"}); add_verb(#0, {player, "rxd", "do_out_of_band_command"}, {"this", "none", "this"}); set_verb_code(#0, "do_out_of_band_command", {"this.oob_log = {@this.oob_log, {player, args}};"});"#;

fn fixture() -> &'static Fixture {
    FIXTURE.get_or_init(|| {
        // PR_SET_PDEATHSIG fires when the spawning *thread* exits, so spawn from a thread that
        // lives as long as the process.
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            tx.send(start_fixture()).unwrap();
            loop {
                std::thread::park();
            }
        });
        let fixture = rx.recv().expect("fixture start");
        // SAFETY: registering a plain extern "C" function.
        unsafe {
            libc::atexit(cleanup);
        }
        fixture
    })
}

fn start_fixture() -> Fixture {
    let workdir = tempfile::TempDir::new().unwrap().keep();
    common::write_keys(&workdir);
    let uuid = Uuid::new_v4();
    let mut children = vec![spawn("daemon", common::daemon_command(&workdir, uuid))];
    std::fs::write(workdir.join("full.yaml"), FULL_CONFIG).unwrap();

    let host = |port: u16, args: &[&str]| {
        let mut command = common::telnet_host_command(&workdir, uuid, port);
        command.args(args);
        spawn("telnet-host", command)
    };
    let passive = common::free_port();
    let offers = common::free_port();
    let full = common::free_port();
    let capped = common::free_port();
    children.push(host(passive, &[]));
    children.push(host(
        offers,
        &[
            "--telnet-protocols-offer-on-connect",
            "--telnet-protocols-gmcp",
            "--telnet-protocols-eor",
            "--telnet-protocols-charset",
            "--telnet-protocols-naws",
            "--telnet-protocols-ttype",
        ],
    ));
    let full_yaml = workdir.join("full.yaml");
    children.push(host(full, &["--config-file", full_yaml.to_str().unwrap()]));
    children.push(host(
        capped,
        &[
            "--telnet-protocols-gmcp",
            "--telnet-protocols-max-subneg",
            "64",
            "--telnet-protocols-client-data-rate",
            "0",
        ],
    ));

    for port in [passive, offers, full, capped] {
        wait_until_login_works(port, &mut children);
    }
    let mut wizard = Client::connect(passive);
    wizard.login();
    wizard.run(INSTALL_RECORDER);
    assert_eq!(wizard.value("length(#0.cd_log)"), "0");

    Fixture {
        passive,
        offers,
        full,
        capped,
        workdir,
        children: Mutex::new(children),
    }
}

/// Retry `connect #3` until the host is registered with the daemon.
fn wait_until_login_works(port: u16, children: &mut [ManagedChild]) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        for child in children.iter_mut() {
            child.assert_running().unwrap();
        }
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            let mut client = Client {
                stream,
                plain: Vec::new(),
                queue: VecDeque::new(),
                inflater: None,
                wire: Vec::new(),
                closed: false,
            };
            client.line("connect #3");
            let got = client.collect_for(Duration::from_millis(500));
            if contains(&text_of(&got), b"*** Connected ***") {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "host on port {port} did not start"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Poll `#0:client_data_for(connection())` until it contains every one of `needles`.
fn wait_client_data(client: &mut Client, needles: &[&str]) -> String {
    let deadline = Instant::now() + EXPECT_TIMEOUT;
    loop {
        let log = client.value("#0:client_data_for(connection())");
        if needles.iter().all(|n| log.contains(n)) {
            return log;
        }
        assert!(
            Instant::now() < deadline,
            "do_client_data log is missing {needles:?}: {log}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll `connection_options(connection())` until it contains every one of `needles`.
fn wait_options(client: &mut Client, needles: &[&str]) -> String {
    let deadline = Instant::now() + EXPECT_TIMEOUT;
    loop {
        let options = client.options();
        if needles.iter().all(|n| options.contains(n)) {
            return options;
        }
        assert!(
            Instant::now() < deadline,
            "connection options are missing {needles:?}: {options}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `notify(connection(), <text>, 0, 1, 'text_plain, ["prompt" -> 1])`.
const PROMPT: &str = r#"notify(connection(), "HP> ", 0, 1, 'text_plain, ["prompt" -> 1]);"#;

fn text(s: &str) -> Ev {
    Ev::Text(s.as_bytes().to_vec())
}

/// Exercise the daemon notification path, session options, renderer, and wire codec together.
#[test]
#[serial(telnet_protocols)]
fn djot_rendering_with_moo_highlighting_and_mxp() {
    fn render(client: &mut Client, lines: &[&str]) -> String {
        let source = lines
            .iter()
            .map(|line| serde_json::to_string(line).unwrap())
            .collect::<Vec<_>>()
            .join(", ");
        let events = client.run(&format!(
            "notify(connection(), {{{source}}}, 0, 0, 'text_djot);"
        ));
        String::from_utf8(text_of(&events)).unwrap()
    }

    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();

    let code = render(&mut c, &["```MoO", "return 42;", "```"]);
    assert!(code.contains("\x1b["), "missing MOO highlighting: {code:?}");
    assert_eq!(ansi.replace_all(&code, ""), "return 42;\r\n");
    let fallback = render(&mut c, &["```rust", "let x = 1;", "```"]);
    assert_eq!(ansi.replace_all(&fallback, ""), "[rust]\r\nlet x = 1;\r\n");

    let ordered = render(&mut c, &["3. third", "4. fourth"]);
    let ordered = ansi.replace_all(&ordered, "");
    assert!(ordered.contains("3. third"), "{ordered:?}");
    assert!(ordered.contains("4. fourth"), "{ordered:?}");

    c.line(".A11Y");
    c.expect_text_contains(b"Screen reader mode is now on\r\n");
    assert_eq!(
        render(
            &mut c,
            &["| Name | Role |", "|---|---|", "| Alice | *Builder* |"]
        ),
        "Row 1: Name: Alice; Role: Builder\r\n"
    );
    assert_eq!(
        render(&mut c, &["```moo", "return 42;", "```"]),
        "[moo]\r\nreturn 42;\r\n"
    );

    const MXP: u8 = 91;
    c.do_(MXP);
    let negotiation = c.expect_text_contains(b"\x1b[7z");
    assert!(negotiation.contains(&Ev::Negotiate(WILL, MXP)));
    assert!(negotiation.contains(&Ev::Subneg(MXP, vec![])));
    wait_options(&mut c, &["{'mxp, true}"]);
    assert_eq!(
        render(&mut c, &["[look](moo://cmd/look) & <world>"]),
        "\x1b[1z<SEND href=\"look\">look</SEND> &amp; &lt;world&gt;\r\n"
    );
}

fn gmcp_ev(message: &str) -> Ev {
    Ev::Subneg(OPT_GMCP, message.as_bytes().to_vec())
}

// 1. Passive mode: the default configuration.

#[test]
#[serial(telnet_protocols)]
fn passive_sends_no_telnet_bytes_and_ignores_negotiation() {
    let f = fixture();
    let mut c = Client::connect(f.passive);
    c.assert_no_bytes_for(QUIET);
    // Test.db prints no welcome text; the first bytes are the login banner, with no IAC.
    assert_eq!(c.login(), vec![text("*** Connected ***\r\n")]);
    assert!(!c.wire.contains(&IAC), "IAC on a passive connection");

    // Passive mode neither answers nor refuses.
    c.do_(OPT_GMCP);
    c.will(OPT_UNKNOWN);
    c.do_(OPT_UNKNOWN);
    c.assert_no_bytes_for(QUIET);
    assert_eq!(c.value("1 + 1"), "2");
}

#[test]
#[serial(telnet_protocols)]
fn passive_forwards_raw_telnet_to_out_of_band_and_not_client_data() {
    let f = fixture();
    let mut c = Client::connect(f.passive);
    c.login();
    let conn = c.conn();
    c.will(OPT_UNKNOWN);
    let deadline = Instant::now() + EXPECT_TIMEOUT;
    loop {
        let oob = c.value("#0.oob_log");
        if oob.contains("{#3, {b\"__tm\"}}") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no raw WILL 102 in oob log: {oob}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(c.value(&format!("#0:client_data_for({conn})")), "{}");
}

#[test]
#[serial(telnet_protocols)]
fn passive_iac_iac_in_a_line_is_text() {
    let f = fixture();
    let mut c = Client::connect(f.passive);
    c.login();
    // A lone 0xFF is not UTF-8, so `IAC IAC` decodes to U+FFFD; the connection stays up.
    assert_eq!(c.value_bytes(b"\"a\xff\xffb\" == \"a\\uFFFDb\""), "1");
    assert_eq!(c.value_bytes(b"length(\"a\xff\xffb\")"), "3");
    assert_eq!(c.value("1 + 1"), "2");
}

#[test]
#[serial(telnet_protocols)]
fn passive_client_echo_resends_on_every_call() {
    let f = fixture();
    let mut c = Client::connect(f.passive);
    c.login();
    let off = r#"set_connection_option(connection(), "client-echo", 0);"#;
    let on = r#"set_connection_option(connection(), "client-echo", 1);"#;
    assert_eq!(c.run(off), vec![Ev::Negotiate(WILL, OPT_ECHO)]);
    assert_eq!(c.run(off), vec![Ev::Negotiate(WILL, OPT_ECHO)]);
    assert_eq!(c.run(on), vec![Ev::Negotiate(WONT, OPT_ECHO)]);
    assert_eq!(c.run(on), vec![Ev::Negotiate(WONT, OPT_ECHO)]);
}

// 2. Offers at connect.

#[test]
#[serial(telnet_protocols)]
fn offers_at_connect_match_the_configured_set() {
    let f = fixture();
    let mut c = Client::connect(f.offers);
    let expected = vec![
        Ev::Negotiate(WILL, OPT_GMCP),
        Ev::Negotiate(WILL, OPT_EOR),
        Ev::Negotiate(WILL, OPT_CHARSET),
        Ev::Negotiate(DO, OPT_NAWS),
        Ev::Negotiate(DO, OPT_TTYPE),
    ];
    let got = c.read_until("offers", |got| got.len() >= expected.len());
    assert_eq!(got, expected);
    c.assert_no_bytes_for(QUIET);
}

#[test]
#[serial(telnet_protocols)]
fn no_offers_without_offer_on_connect() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.assert_no_bytes_for(QUIET);
    assert_eq!(c.login(), vec![text("*** Connected ***\r\n")]);
}

// 3. Option toggling from MOO.

fn set_option(name: &str, value: i64) -> String {
    format!(r#"set_connection_option(connection(), "{name}", {value});"#)
}

#[test]
#[serial(telnet_protocols)]
fn toggling_gmcp_sends_one_verb_per_state_change() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    assert_eq!(
        c.run(&set_option("gmcp", 1)),
        vec![Ev::Negotiate(WILL, OPT_GMCP)]
    );
    c.do_(OPT_GMCP);
    c.assert_no_bytes_for(QUIET);
    wait_options(&mut c, &["{'gmcp, true}"]);
    assert_eq!(c.run(&set_option("gmcp", 1)), vec![]);

    assert_eq!(
        c.run(&set_option("gmcp", 0)),
        vec![Ev::Negotiate(WONT, OPT_GMCP)]
    );
    assert_eq!(c.run(&set_option("gmcp", 0)), vec![]);
    c.dont(OPT_GMCP);
    c.assert_no_bytes_for(QUIET);
    wait_options(&mut c, &["{'gmcp, false}"]);

    assert_eq!(
        c.run(&set_option("gmcp", 1)),
        vec![Ev::Negotiate(WILL, OPT_GMCP)]
    );
    c.do_(OPT_GMCP);
    c.assert_no_bytes_for(QUIET);
    wait_options(&mut c, &["{'gmcp, true}"]);
}

#[test]
#[serial(telnet_protocols)]
fn disabling_gmcp_shuts_down_the_client_direction() {
    let f = fixture();
    for bidirectional in [false, true] {
        let mut c = Client::connect(f.full);
        c.login();
        c.will(OPT_GMCP);
        assert_eq!(
            c.expect_negotiation(DO, OPT_GMCP),
            vec![Ev::Negotiate(DO, OPT_GMCP)]
        );
        if bidirectional {
            c.do_(OPT_GMCP);
            assert_eq!(
                c.expect_negotiation(WILL, OPT_GMCP),
                vec![Ev::Negotiate(WILL, OPT_GMCP)]
            );
        }
        wait_options(&mut c, &["{'gmcp, true}"]);
        let mut expected = vec![];
        if bidirectional {
            expected.push(Ev::Negotiate(WONT, OPT_GMCP));
        }
        expected.push(Ev::Negotiate(DONT, OPT_GMCP));
        assert_eq!(c.run(&set_option("gmcp", 0)), expected);
        c.wont(OPT_GMCP);
        c.dont(OPT_GMCP);
        c.assert_no_bytes_for(QUIET);
        wait_options(&mut c, &["{'gmcp, false}"]);
        assert_eq!(
            c.run(r#"emit_data(connection(), "gmcp", "Char.Vitals", 1);"#),
            vec![]
        );
        assert_eq!(c.run(&set_option("gmcp", 0)), vec![]);
    }
}

#[test]
#[serial(telnet_protocols)]
fn client_refusal_of_an_offer_does_not_loop() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    assert_eq!(
        c.run(&set_option("eor", 1)),
        vec![Ev::Negotiate(WILL, OPT_EOR)]
    );
    c.dont(OPT_EOR);
    c.assert_no_bytes_for(QUIET);
    // The option never came on, so the attribute does not claim it did.
    let options = c.options();
    assert!(!options.contains("'eor"), "{options}");
    // The refusal leaves the option off, so disabling it again sends nothing.
    assert_eq!(c.run(&set_option("eor", 0)), vec![]);
    // A client repeating DONT for an option that is off gets no reply.
    c.dont(OPT_EOR);
    c.wont(OPT_GMCP);
    c.assert_no_bytes_for(QUIET);
}

#[test]
#[serial(telnet_protocols)]
fn client_will_for_an_unimplemented_option_is_refused_once() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_UNKNOWN);
    assert_eq!(
        c.expect_negotiation(DONT, OPT_UNKNOWN),
        vec![Ev::Negotiate(DONT, OPT_UNKNOWN)]
    );
    c.wont(OPT_UNKNOWN);
    c.assert_no_bytes_for(QUIET);
    c.do_(OPT_UNKNOWN);
    assert_eq!(
        c.expect_negotiation(WONT, OPT_UNKNOWN),
        vec![Ev::Negotiate(WONT, OPT_UNKNOWN)]
    );
    c.assert_no_bytes_for(QUIET);
}

#[test]
#[serial(telnet_protocols)]
fn echo_and_client_echo_toggle_without_repeats() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    assert_eq!(
        c.run(&set_option("client-echo", 0)),
        vec![Ev::Negotiate(WILL, OPT_ECHO)]
    );
    assert_eq!(c.run(&set_option("client-echo", 0)), vec![]);
    c.do_(OPT_ECHO);
    c.assert_no_bytes_for(QUIET);
    // `echo` 1 is server echo, the state `client-echo` 0 already set.
    assert_eq!(c.run(&set_option("echo", 1)), vec![]);
    assert_eq!(
        c.run(&set_option("echo", 0)),
        vec![Ev::Negotiate(WONT, OPT_ECHO)]
    );
    c.dont(OPT_ECHO);
    c.assert_no_bytes_for(QUIET);
    assert_eq!(c.run(&set_option("client-echo", 1)), vec![]);
    assert_eq!(
        c.run(&set_option("echo", 1)),
        vec![Ev::Negotiate(WILL, OPT_ECHO)]
    );
    c.do_(OPT_ECHO);
    c.assert_no_bytes_for(QUIET);
}

// 4. UTF-8 and CHARSET.

/// `IAC SB CHARSET REQUEST ;UTF-8;ISO-8859-1 IAC SE`, the host's offer.
const CHARSET_REQUEST: &[u8] = b"\x01;UTF-8;ISO-8859-1";

/// Agree to CHARSET and wait for the host's REQUEST.
fn start_charset(c: &mut Client) {
    c.do_(OPT_CHARSET);
    let got = c.read_until("CHARSET REQUEST", |got| got.len() >= 2);
    assert_eq!(
        got,
        vec![
            Ev::Negotiate(WILL, OPT_CHARSET),
            Ev::Subneg(OPT_CHARSET, CHARSET_REQUEST.to_vec())
        ]
    );
}

#[test]
#[serial(telnet_protocols)]
fn charset_offered_at_connect_sends_request_on_agreement() {
    let f = fixture();
    let mut c = Client::connect(f.offers);
    c.read_until("offers", |got| got.len() >= 5);
    // The client agrees to the WILL CHARSET offer; the host follows with its REQUEST.
    c.do_(OPT_CHARSET);
    assert_eq!(c.expect_subneg(OPT_CHARSET), CHARSET_REQUEST);
    c.assert_no_bytes_for(QUIET);
}

#[test]
#[serial(telnet_protocols)]
fn client_initiated_charset_latin1_transcodes_before_login() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_CHARSET);
    assert_eq!(
        c.expect_negotiation(DO, OPT_CHARSET),
        vec![Ev::Negotiate(DO, OPT_CHARSET)]
    );
    c.assert_no_bytes_for(QUIET);
    c.subneg(OPT_CHARSET, b"\x01;ISO-8859-1");
    assert_eq!(c.expect_subneg(OPT_CHARSET), b"\x02ISO-8859-1");
    c.login();
    wait_options(&mut c, &["{'charset, \"ISO-8859-1\"}"]);
    assert_eq!(c.value_bytes(b"\"\xe9\" == \"\\u00E9\""), "1");
    assert_eq!(
        c.run(r#"notify(connection(), "\u00E9");"#),
        vec![Ev::Text(b"\xe9\r\n".to_vec())]
    );
}

#[test]
#[serial(telnet_protocols)]
fn charset_utf8_accepted_round_trips_non_ascii() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    start_charset(&mut c);
    c.subneg(OPT_CHARSET, b"\x02UTF-8");
    c.assert_no_bytes_for(QUIET);
    c.login();
    wait_options(&mut c, &["{'charset, \"UTF-8\"}", "{'utf8, true}"]);

    // UTF-8 in, UTF-8 out.
    let input = "h\u{e9}llo \u{2713}";
    assert_eq!(c.value(&format!("length(\"{input}\")")), "7");
    let got = c.run(&format!("notify(connection(), \"{input}\");"));
    assert_eq!(got, vec![Ev::Text(format!("{input}\r\n").into_bytes())]);
}

#[test]
#[serial(telnet_protocols)]
fn charset_latin1_transcodes_both_ways() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    start_charset(&mut c);
    c.subneg(OPT_CHARSET, b"\x02ISO-8859-1");
    c.assert_no_bytes_for(QUIET);
    c.login();
    wait_options(&mut c, &["{'charset, \"ISO-8859-1\"}", "{'utf8, false}"]);

    // 0xE9 in is é; IAC IAC in text is 0xFF, which is ÿ.
    assert_eq!(c.value_bytes(b"\"\xe9\" == \"\\u00E9\""), "1");
    assert_eq!(c.value_bytes(b"\"\xff\xff\" == \"\\u00FF\""), "1");
    assert_eq!(c.value_bytes(b"length(\"\xe9\xff\xffz\")"), "3");

    // é out is 0xE9, ÿ is IAC IAC, ✓ has no Latin-1 form and becomes '?'.
    let got = c.run(r#"notify(connection(), "\u00E9|\u00FF|\u2713");"#);
    assert_eq!(got, vec![Ev::Text(b"\xe9|\xff|?\r\n".to_vec())]);
    let raw_line: &[u8] = b"\xe9|\xff\xff|?\r\n";
    assert!(contains(&c.wire, raw_line), "ÿ was not written as IAC IAC");
}

#[test]
#[serial(telnet_protocols)]
fn charset_rejected_stays_utf8() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    start_charset(&mut c);
    c.subneg(OPT_CHARSET, b"\x03");
    c.assert_no_bytes_for(QUIET);
    c.login();
    let options = c.options();
    assert!(!options.contains("'charset"), "{options}");
    let got = c.run(r#"notify(connection(), "\u00E9");"#);
    assert_eq!(got, vec![Ev::Text("\u{e9}\r\n".as_bytes().to_vec())]);
}

// 5. NAWS.

#[test]
#[serial(telnet_protocols)]
fn naws_sets_columns_and_rows_before_and_after_login() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_NAWS);
    assert_eq!(
        c.expect_negotiation(DO, OPT_NAWS),
        vec![Ev::Negotiate(DO, OPT_NAWS)]
    );
    // 0x00FF columns exercises IAC escaping in the client's subnegotiation.
    c.subneg(OPT_NAWS, &[0x00, 0xFF, 0x00, 0x30]);
    c.assert_no_bytes_for(QUIET);
    c.login();
    let conn = c.conn();
    wait_options(&mut c, &["{'columns, 255}", "{'rows, 48}"]);
    // Delivered before login, with the connection as player.
    wait_client_data(
        &mut c,
        &[&format!(
            "{{{conn}, {{{conn}, 'client, \"attributes\", ['columns -> 255, 'rows -> 48]}}}}"
        )],
    );

    c.subneg(OPT_NAWS, &[0x00, 0x84, 0x00, 0x2A]);
    wait_options(&mut c, &["{'columns, 132}", "{'rows, 42}"]);
    wait_client_data(
        &mut c,
        &[&format!(
            "{{#3, {{{conn}, 'client, \"attributes\", ['columns -> 132, 'rows -> 42]}}}}"
        )],
    );
}

// 6. TTYPE and MTTS.

#[test]
#[serial(telnet_protocols)]
fn ttype_cycle_stops_after_three_and_sets_attributes() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_TTYPE);
    let send = Ev::Subneg(OPT_TTYPE, vec![1]);
    let got = c.read_until("TTYPE SEND", |got| got.len() >= 2);
    assert_eq!(got, vec![Ev::Negotiate(DO, OPT_TTYPE), send.clone()]);
    c.subneg(OPT_TTYPE, b"\x00MUDLET");
    assert_eq!(
        c.read_until("SEND 2", |g| !g.is_empty()),
        vec![send.clone()]
    );
    c.subneg(OPT_TTYPE, b"\x00XTERM-256COLOR");
    assert_eq!(c.read_until("SEND 3", |g| !g.is_empty()), vec![send]);
    // 137 = 128 | 8 | 1: the UTF-8 (4) and screen reader (64) bits are clear.
    c.subneg(OPT_TTYPE, b"\x00MTTS 137");
    c.assert_no_bytes_for(QUIET);
    // A fourth reply is ignored.
    c.subneg(OPT_TTYPE, b"\x00MTTS 137");
    c.assert_no_bytes_for(QUIET);
    c.login();
    let options = wait_options(
        &mut c,
        &[
            "{'terminal_type, \"MUDLET\"}",
            "{'client_name, \"MUDLET\"}",
            "{'mtts, 137}",
            "{'screen-reader, false}",
        ],
    );
    assert!(!options.contains("XTERM"), "{options}");
    assert!(!options.contains("'utf8"), "{options}");
}

#[test]
#[serial(telnet_protocols)]
fn ttype_repeat_ends_cycle_and_mtts_bits_set_utf8_and_screen_reader() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_TTYPE);
    c.read_until("TTYPE SEND", |got| got.len() >= 2);
    // 68 = 64 (screen reader) | 4 (UTF-8).
    c.subneg(OPT_TTYPE, b"\x00MTTS 68");
    assert_eq!(c.expect_subneg(OPT_TTYPE), vec![1]);
    // The same name again means the client has no more; the host stops asking.
    c.subneg(OPT_TTYPE, b"\x00MTTS 68");
    c.assert_no_bytes_for(QUIET);
    c.login();
    let options = wait_options(
        &mut c,
        &["{'mtts, 68}", "{'utf8, true}", "{'screen-reader, true}"],
    );
    // MTTS sets `utf8` but not the codec charset.
    assert!(!options.contains("'charset"), "{options}");
}

// 7. GMCP in.

/// Negotiate GMCP from the client side.
fn enable_gmcp(c: &mut Client) {
    c.do_(OPT_GMCP);
    assert_eq!(
        c.expect_negotiation(WILL, OPT_GMCP),
        vec![Ev::Negotiate(WILL, OPT_GMCP)]
    );
}

#[test]
#[serial(telnet_protocols)]
fn gmcp_in_reaches_do_client_data_before_and_after_login() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    enable_gmcp(&mut c);
    c.gmcp(r#"Core.Hello {"client":"Mudlet","version":"4.17"}"#);
    c.gmcp(r#"Char.Pre {"a":[1,true,null],"s":"x"}"#);
    c.login();
    let conn = c.conn();
    c.gmcp("Char.Post 7");
    c.gmcp("Core.Ping");
    let log = wait_client_data(
        &mut c,
        &[
            &format!(
                "{{{conn}, {{{conn}, 'gmcp, \"Core.Hello\", [\"client\" -> \"Mudlet\", \"version\" -> \"4.17\"]}}}}"
            ),
            &format!(
                "{{{conn}, {{{conn}, 'client, \"attributes\", ['client_name -> \"Mudlet\", 'client_version -> \"4.17\"]}}}}"
            ),
            // Before login `player` is the connection; JSON true is 1 without boolean returns.
            &format!(
                "{{{conn}, {{{conn}, 'gmcp, \"Char.Pre\", [\"a\" -> {{1, 1, #-1}}, \"s\" -> \"x\"]}}}}"
            ),
            // After login `player` is the logged-in player.
            &format!("{{#3, {{{conn}, 'gmcp, \"Char.Post\", 7}}}}"),
            // No body is delivered as the empty map.
            &format!("{{#3, {{{conn}, 'gmcp, \"Core.Ping\", []}}}}"),
        ],
    );
    assert!(
        !log.contains(&format!("{{#3, {{{conn}, 'gmcp, \"Char.Pre\"")),
        "pre-login message delivered as the player: {log}"
    );
    assert!(
        !log.contains(&format!("{{{conn}, {{{conn}, 'gmcp, \"Char.Post\"")),
        "post-login message delivered as the connection: {log}"
    );
    wait_options(
        &mut c,
        &["{'client_name, \"Mudlet\"}", "{'client_version, \"4.17\"}"],
    );
}

#[test]
#[serial(telnet_protocols)]
fn gmcp_core_supports_gates_outbound_packages() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    enable_gmcp(&mut c);
    c.login();
    c.gmcp(r#"Core.Supports.Set ["Char 1", "Room.Info 1"]"#);
    wait_options(&mut c, &["'gmcp_supports"]);
    let got = c.run(
        "emit_data(connection(), \"gmcp\", \"Comm.Channel\", [\"a\" -> 1]); \
         emit_data(connection(), \"gmcp\", \"Char.Vitals\", [\"hp\" -> 1]); \
         emit_data(connection(), \"gmcp\", \"Room.Exits\", [\"n\" -> 1]); \
         emit_data(connection(), \"gmcp\", \"Room.Info.Area\", [\"x\" -> 1]); \
         emit_data(connection(), \"gmcp\", \"Core.Goodbye\", []);",
    );
    assert_eq!(
        got,
        vec![
            gmcp_ev(r#"Char.Vitals {"hp":1}"#),
            gmcp_ev(r#"Room.Info.Area {"x":1}"#),
            // Core is always written.
            gmcp_ev("Core.Goodbye"),
        ]
    );
}

// 8. GMCP out.

#[test]
#[serial(telnet_protocols)]
fn gmcp_out_writes_exact_subnegotiation_bytes() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    // Not negotiated yet: nothing is written.
    c.login();
    assert_eq!(
        c.run("emit_data(connection(), \"gmcp\", \"Char.Vitals\", [\"hp\" -> 12]);"),
        vec![]
    );
    enable_gmcp(&mut c);
    let before = c.wire.len();
    let got = c.run(
        "emit_data(player, \"gmcp\", \"Char.Vitals\", [\"hp\" -> 12, \"name\" -> \"h\\u00E9\"]); \
         emit_data(connection(), \"gmcp\", \"Core.Ping\", []); \
         emit_data(connection(), \"gmcp\", \"Char.List\", {1, \"a\", #-1});",
    );
    assert_eq!(
        got,
        vec![
            gmcp_ev("Char.Vitals {\"hp\":12,\"name\":\"h\u{e9}\"}"),
            gmcp_ev("Core.Ping"),
            gmcp_ev(r#"Char.List [1,"a",null]"#),
        ]
    );
    let mut expected = vec![IAC, SB, OPT_GMCP];
    expected.extend_from_slice("Char.Vitals {\"hp\":12,\"name\":\"h\u{e9}\"}".as_bytes());
    expected.extend_from_slice(&[IAC, SE, IAC, SB, OPT_GMCP]);
    expected.extend_from_slice(b"Core.Ping");
    expected.extend_from_slice(&[IAC, SE]);
    assert_eq!(&c.wire[before..before + expected.len()], &expected[..]);
}

#[test]
#[serial(telnet_protocols)]
fn gmcp_out_drops_invalid_events_and_keeps_the_connection() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    enable_gmcp(&mut c);
    c.login();
    let got = c.run(
        "emit_data(connection(), \"gmcp\", \"Bad Name\", [\"a\" -> 1]); \
         emit_data(connection(), \"gmcp\", \"Char.Err\", [\"e\" -> E_PERM]); \
         emit_data(connection(), \"gmcp\", \"Char.Bin\", b\"AQI=\"); \
         emit_data(connection(), \"gmcp\", \"Char.Ok\", 1);",
    );
    assert_eq!(got, vec![gmcp_ev("Char.Ok 1")]);
    assert_eq!(c.value("1 + 1"), "2");
}

// 9. Unknown options.

#[test]
#[serial(telnet_protocols)]
fn unknown_option_is_refused_and_forwarded_to_client_data() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.will(OPT_UNKNOWN);
    c.expect_negotiation(DONT, OPT_UNKNOWN);
    // Data with 0xFF, escaped on the wire and unescaped in the delivered binary.
    c.subneg(OPT_UNKNOWN, &[0x01, 0xFF, 0x02]);
    c.assert_no_bytes_for(QUIET);
    c.login();
    let conn = c.conn();
    c.do_(OPT_UNKNOWN);
    c.expect_negotiation(WONT, OPT_UNKNOWN);
    wait_client_data(
        &mut c,
        &[
            &format!(
                "{{{conn}, {{{conn}, 'telnet, \"negotiate\", [\"option\" -> 102, \"verb\" -> 'will]}}}}"
            ),
            // b"Af8C" is base64url of 01 FF 02.
            &format!(
                "{{{conn}, {{{conn}, 'telnet, \"subneg\", [\"data\" -> b\"Af8C\", \"option\" -> 102]}}}}"
            ),
            &format!(
                "{{#3, {{{conn}, 'telnet, \"negotiate\", [\"option\" -> 102, \"verb\" -> 'do]}}}}"
            ),
        ],
    );
}

// 10. Prompt marks.

#[test]
#[serial(telnet_protocols)]
fn prompt_mark_is_ga_without_eor_and_none_after_sga() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    assert_eq!(c.run(PROMPT), vec![text("HP> "), Ev::Ga]);
    // SUPPRESS-GO-AHEAD on our side removes the GA.
    c.do_(OPT_SGA);
    assert_eq!(
        c.expect_negotiation(WILL, OPT_SGA),
        vec![Ev::Negotiate(WILL, OPT_SGA)]
    );
    assert_eq!(c.run(PROMPT), vec![text("HP> ")]);
    // EOR wins over SGA.
    c.do_(OPT_EOR);
    c.expect_negotiation(WILL, OPT_EOR);
    assert_eq!(c.run(PROMPT), vec![text("HP> "), Ev::Eor]);
}

#[test]
#[serial(telnet_protocols)]
fn prompt_mark_is_eor_and_ordered_with_gmcp() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.do_(OPT_EOR);
    c.expect_negotiation(WILL, OPT_EOR);
    enable_gmcp(&mut c);
    c.login();
    assert_eq!(c.run(PROMPT), vec![text("HP> "), Ev::Eor]);
    let got = c.run(&format!(
        "emit_data(connection(), \"gmcp\", \"A.B\", 1); {PROMPT} emit_data(connection(), \"gmcp\", \"C.D\", 2);"
    ));
    assert_eq!(
        got,
        vec![gmcp_ev("A.B 1"), text("HP> "), Ev::Eor, gmcp_ev("C.D 2")]
    );
    // No prompt metadata: no mark, with or without no_newline.
    assert_eq!(
        c.run("notify(connection(), \"plain> \", 0, 1);"),
        vec![text("plain> ")]
    );
    assert_eq!(
        c.run("notify(connection(), \"line\");"),
        vec![text("line\r\n")]
    );
    // A false prompt flag is not a prompt.
    assert_eq!(
        c.run(r#"notify(connection(), "p> ", 0, 1, 'text_plain, ["prompt" -> 0]);"#),
        vec![text("p> ")]
    );
}

#[test]
#[serial(telnet_protocols)]
fn passive_prompt_has_no_mark() {
    let f = fixture();
    let mut c = Client::connect(f.passive);
    c.login();
    assert_eq!(c.run(PROMPT), vec![text("HP> ")]);
    assert!(!c.wire.contains(&IAC));
}

// 11. MSSP.

#[test]
#[serial(telnet_protocols)]
fn mssp_answers_do_with_configured_and_computed_values() {
    let f = fixture();
    let mut player = Client::connect(f.passive);
    player.login();
    let expected_players = player.value("length(connected_players())");
    assert_ne!(expected_players, "0");
    let mut c = Client::connect(f.full);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    c.do_(OPT_MSSP);
    c.expect_negotiation(WILL, OPT_MSSP);
    let data = c.expect_subneg(OPT_MSSP);
    let mut vars = Vec::new();
    for pair in data.split(|&b| b == 1).skip(1) {
        let mut parts = pair.split(|&b| b == 2);
        let key = String::from_utf8(parts.next().unwrap().to_vec()).unwrap();
        let value = String::from_utf8(parts.next().unwrap().to_vec()).unwrap();
        assert!(parts.next().is_none());
        vars.push((key, value));
    }
    let keys: Vec<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["NAME", "PLAYERS", "UPTIME"]);
    assert_eq!(vars[0].1, "TestMOO");
    assert_eq!(vars[1].1, expected_players);
    let uptime: u64 = vars[2].1.parse().unwrap();
    assert!(
        uptime <= now && uptime + 600 > now,
        "UPTIME {uptime} is not the host start time (now {now})"
    );
    c.assert_no_bytes_for(QUIET);
}

// 12. MCCP2.

#[test]
#[serial(telnet_protocols)]
fn mccp2_compresses_everything_after_the_start_marker() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.login();
    c.do_(OPT_MCCP2);
    let got = c.read_until("MCCP2 start", |got| got.len() >= 2);
    assert_eq!(
        got,
        vec![
            Ev::Negotiate(WILL, OPT_MCCP2),
            Ev::Subneg(OPT_MCCP2, vec![])
        ]
    );
    // The marker itself is uncompressed on the wire.
    let marker = [IAC, WILL, OPT_MCCP2, IAC, SB, OPT_MCCP2, IAC, SE];
    let at = c
        .wire
        .windows(marker.len())
        .position(|w| w == marker)
        .expect("uncompressed MCCP2 start marker");
    let compressed_from = at + marker.len();
    let got = c.run(r#"notify(connection(), "compressed line");"#);
    assert_eq!(got, vec![text("compressed line\r\n")]);
    let after = &c.wire[compressed_from..];
    assert!(!after.is_empty());
    assert!(
        !contains(after, b"compressed line"),
        "output after MCCP2 start is not compressed"
    );
    assert_eq!(c.value("6 * 7"), "42");
}

// 13. Subnegotiation cap.

#[test]
#[serial(telnet_protocols)]
fn oversized_subnegotiation_is_discarded_and_the_stream_resyncs() {
    let f = fixture();
    let mut c = Client::connect(f.capped);
    enable_gmcp(&mut c);
    c.login();
    let conn = c.conn();
    let big = format!("Char.Big \"{}\"", "x".repeat(10 * 1024));
    c.gmcp(&big);
    c.gmcp("Char.Small 1");
    wait_client_data(
        &mut c,
        &[&format!("{{#3, {{{conn}, 'gmcp, \"Char.Small\", 1}}}}")],
    );
    let log = c.value(&format!("#0:client_data_for({conn})"));
    assert!(!log.contains("Char.Big"), "{log}");
    assert_eq!(c.value("1 + 1"), "2");
}

// 14. Binary notify.

#[test]
#[serial(telnet_protocols)]
fn binary_notify_after_login_writes_raw_bytes() {
    for port in [fixture().passive, fixture().full] {
        let mut c = Client::connect(port);
        c.login();
        // b"aGkA__tm" is 68 69 00 FF FB 66: "hi", NUL, then IAC WILL 102, written unchanged.
        let before = c.wire.len();
        let got = c.run(r#"notify(connection(), b"aGkA__tm");"#);
        assert_eq!(got, vec![text("hi\0"), Ev::Negotiate(WILL, OPT_UNKNOWN)]);
        assert_eq!(
            &c.wire[before..before + 6],
            &[0x68, 0x69, 0x00, 0xFF, 0xFB, 0x66]
        );
        assert_eq!(c.value("1 + 1"), "2");
    }
}

// 15. MSDP.

#[test]
#[serial(telnet_protocols)]
fn msdp_round_trip() {
    let f = fixture();
    let mut c = Client::connect(f.full);
    c.do_(OPT_MSDP);
    c.expect_negotiation(WILL, OPT_MSDP);
    c.login();
    let conn = c.conn();
    c.subneg(OPT_MSDP, b"\x01LIST\x02COMMANDS");
    wait_client_data(
        &mut c,
        &[&format!(
            "{{#3, {{{conn}, 'msdp, \"LIST\", \"COMMANDS\"}}}}"
        )],
    );
    let got = c.run(r#"emit_data(connection(), 'msdp, 'HEALTH, ["cur" -> 5, "l" -> {1, "a"}]);"#);
    assert_eq!(
        got,
        vec![Ev::Subneg(
            OPT_MSDP,
            b"\x01HEALTH\x02\x03\x01cur\x025\x01l\x02\x05\x021\x02a\x06\x04".to_vec()
        )]
    );
}
