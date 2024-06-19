//! Execute `.moot` tests against a MOO server listening over telnet.
//! Configured using ENV vars
//! * MOOT_MOO_PATH: path to the `moo` binary, defaults to `$HOME/MOO-1.8.1/moo`
//! * MOOT_DB_PATH: path to the textdump file, defaults to the `Test.db` next to this file
//! * MOOT_PORT: port the MOO server listens on, defaults to 7777

use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use eyre::Context;
use moor_moot::{test_db_path, ManagedChild, MootClient, MootRunner, MootState, WIZARD};
use moor_values::var::Objid;

fn moo_path() -> PathBuf {
    env::var("MOOT_MOO_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let mut path = PathBuf::from(env::var("HOME").unwrap());
            path.push("MOO-1.8.1/moo");
            path
        })
}

fn db_path() -> PathBuf {
    env::var("MOOT_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| test_db_path())
}

fn moo_port() -> u16 {
    env::var("MOOT_PORT")
        .map(|s| s.parse().unwrap())
        .unwrap_or(7777)
}

fn start_moo() -> ManagedChild {
    ManagedChild::new(
        "moo",
        Command::new(moo_path())
            .arg(db_path())
            .arg("/dev/null")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to start moo server {}", moo_path().display())),
    )
}

struct TelnetMootRunner {
    clients: HashMap<Objid, MootClient>,
}
impl TelnetMootRunner {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    fn client(&mut self, player: Objid) -> &mut MootClient {
        self.clients.entry(player).or_insert_with(|| {
            let start = Instant::now();
            loop {
                if let Ok(mut client) = MootClient::new(moo_port()) {
                    client
                        .send_string(std::format!("connect {}", player))
                        .unwrap();
                    return client;
                } else if start.elapsed() > Duration::from_secs(5) {
                    panic!("Failed to connect to daemon");
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
    }

    fn resolve_response(&mut self, response: String) -> Result<String, std::io::Error> {
        // Resolve the response; for example, the test assertion may be `$object`; resolve it to the object's specific number.
        self.client(WIZARD).command(format!(
            "; return {response}; \"TelnetMootRunner::resolve_response\";"
        ))
    }
}
impl MootRunner for TelnetMootRunner {
    type Value = String;
    type Error = std::io::Error;

    fn eval<S: Into<String>>(
        &mut self,
        player: Objid,
        command: S,
    ) -> Result<String, std::io::Error> {
        let response = self
            .client(player)
            .command(format!("; {} \"TelnetMootRunner::eval\";", command.into()))?;
        self.resolve_response(response)
    }

    fn command<S: AsRef<str>>(
        &mut self,
        player: Objid,
        command: S,
    ) -> Result<String, std::io::Error> {
        let response = self.client(player).command(command)?;
        self.resolve_response(response)
    }

    fn none(&self) -> Self::Value {
        "0".to_string()
    }
}

fn test_moo(path: &Path) {
    let mut _moo = start_moo();

    let f = BufReader::new(
        File::open(path)
            .wrap_err(format!("{}", path.display()))
            .unwrap(),
    );

    let mut state = MootState::new(TelnetMootRunner::new(), WIZARD);
    for (line_no, line) in f.lines().enumerate() {
        let line = line.unwrap();
        let line_no = line_no + 1;
        state = state
            .process_line(line_no, &line)
            .wrap_err(format!("line {}", line_no))
            .unwrap();
        //eprintln!("[{line_no}] {line} {state:?}");
    }
    state.finalize().unwrap();
}

#[test]
#[ignore = "Useful for debugging; just run a single test against 'real' MOO"]
fn test_single() {
    test_moo(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/testsuite/moot/recycle.moot"),
    );
}
