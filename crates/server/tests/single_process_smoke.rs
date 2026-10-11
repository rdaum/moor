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

#![cfg(target_family = "unix")]

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, ExitStatus, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn moor_bin() -> &'static str {
    env!("CARGO_BIN_EXE_moor")
}

#[test]
fn single_process_binary_starts_hosts_accepts_telnet_and_shuts_down() {
    let test_dir = tempfile::tempdir().expect("create temp dir");
    let data_dir = test_dir.path().join("data");
    let config_dir = test_dir.path().join("config");
    let xdg_data_dir = test_dir.path().join("xdg-data");
    let export_dir = test_dir.path().join("export");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    std::fs::create_dir_all(&xdg_data_dir).expect("create xdg data dir");
    std::fs::create_dir_all(&export_dir).expect("create export dir");

    let telnet_port = unused_port();
    let telnet_health_port = unused_port();
    let web_port = unused_port();
    let config_path = test_dir.path().join("moor-smoke.yaml");
    std::fs::write(
        &config_path,
        format!(
            r#"
services:
  telnet:
    address: "127.0.0.1"
    port: {telnet_port}
    health_check_port: {telnet_health_port}
  web:
    listen_address: "127.0.0.1:{web_port}"
  curl_worker:
    enabled: false
  git_worker:
    enabled: true
"#
        ),
    )
    .expect("write config");

    let child = Command::new(moor_bin())
        .arg(&data_dir)
        .arg("--config-file")
        .arg(&config_path)
        .arg("--db")
        .arg("world.db")
        .arg("--import")
        .arg(minimal_core_path())
        .arg("--import-format")
        .arg("objdef")
        .arg("--generate-keypair")
        .arg("--public-key")
        .arg(config_dir.join("moor-verifying-key.pem"))
        .arg("--private-key")
        .arg(config_dir.join("moor-signing-key.pem"))
        .arg("--enrollment-token-file")
        .arg(config_dir.join("enrollment-token"))
        .arg("--export")
        .arg(&export_dir)
        .env("XDG_CONFIG_HOME", &config_dir)
        .env("XDG_DATA_HOME", &xdg_data_dir)
        .env("HOME", test_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start moor");

    let result = run_smoke_flow(telnet_port, web_port);
    let output = terminate_and_wait(child);

    if let Err(error) = result {
        panic_with_output(error, &output);
    }

    let status = output.status;
    if !status.success() {
        panic_with_output(format!("moor exited with {status}"), &output);
    }
}

fn run_smoke_flow(telnet_port: u16, web_port: u16) -> Result<(), String> {
    wait_for_http_ok(web_port, "/health")?;
    wait_for_tcp_port(telnet_port)?;

    let mut stream = TcpStream::connect(("127.0.0.1", telnet_port))
        .map_err(|e| format!("connect to telnet listener: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|e| format!("set telnet read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("set telnet write timeout: {e}"))?;

    stream
        .write_all(b"connect Wizard\n")
        .map_err(|e| format!("write login command: {e}"))?;
    read_until(&mut stream, "*** Connected ***", Duration::from_secs(10))?;

    stream
        .write_all(b"eval 1 + 1\n")
        .map_err(|e| format!("write eval command: {e}"))?;
    read_until(&mut stream, "=> 2", Duration::from_secs(10))?;

    stream
        .write_all(
            b"eval worker_request(\"git\", {\"capabilities\", [\"schema\" -> 1]})[\"schema\"]\n",
        )
        .map_err(|e| format!("write Git capabilities request: {e}"))?;
    read_until(&mut stream, "=> 1", Duration::from_secs(10))?;

    Ok(())
}

fn wait_for_http_ok(port: u16, path: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");

    loop {
        let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => stream,
            Err(e) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    continue;
                }
                continue;
            }
            Err(e) => return Err(format!("connect to web listener: {e}")),
        };

        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| format!("set web read timeout: {e}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| format!("set web write timeout: {e}"))?;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("write web health request: {e}"))?;

        let mut response = String::new();
        match stream.read_to_string(&mut response) {
            Ok(_) if response.starts_with("HTTP/1.1 200") => return Ok(()),
            Ok(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            Ok(_) => return Err(format!("web health did not return 200: {response:?}")),
            Err(e) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    continue;
                }
            }
            Err(e) => return Err(format!("read web health response: {e}")),
        }
    }
}

fn wait_for_tcp_port(port: u16) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(_) => return Ok(()),
            Err(e) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
                if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    continue;
                }
            }
            Err(e) => return Err(format!("connect to telnet listener: {e}")),
        }
    }
}

fn read_until(stream: &mut TcpStream, needle: &str, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];

    loop {
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for {needle:?}; received {:?}",
                String::from_utf8_lossy(&bytes)
            ));
        }

        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(format!(
                    "telnet connection closed waiting for {needle:?}; received {:?}",
                    String::from_utf8_lossy(&bytes)
                ));
            }
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&bytes);
                if text.contains(needle) {
                    return Ok(text.into_owned());
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(format!("read telnet output: {e}")),
        }
    }
}

fn terminate_and_wait(mut child: Child) -> Output {
    terminate(&mut child);
    wait_for_exit(&mut child, Duration::from_secs(20)).unwrap_or_else(|| {
        let _ = child.kill();
        child.wait_with_output().expect("wait for killed moor")
    })
}

fn terminate(child: &mut Child) {
    let pid = child.id() as libc::pid_t;
    // SAFETY: Sending SIGTERM to the child process id returned by std::process::Child.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<Output> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(output_after_status(child, status)),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            Ok(None) => return None,
            Err(e) => panic!("poll moor exit status: {e}"),
        }
    }
}

fn output_after_status(child: &mut Child, status: ExitStatus) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut child_stdout) = child.stdout.take() {
        child_stdout
            .read_to_end(&mut stdout)
            .expect("read moor stdout");
    }
    if let Some(mut child_stderr) = child.stderr.take() {
        child_stderr
            .read_to_end(&mut stderr)
            .expect("read moor stderr");
    }
    Output {
        status,
        stdout,
        stderr,
    }
}

fn panic_with_output(message: String, output: &Output) -> ! {
    panic!(
        "{message}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("ephemeral local addr").port();
    drop(listener);
    port
}

fn minimal_core_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("cores/minimal-core")
        .join("src")
}
