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

//! Process isolation for blocking Git operations and bounded IPC.

use crate::protocol::{self, Error, Limits, Operation, Request};
use moor_common::tasks::WorkerError;
use moor_var::{Var, decode_var_cbor, encode_var_cbor};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::{Notify, Semaphore},
};

/// A worker-local executor. Jobs share no repository state and do not queue without bounds.
pub struct Executor {
    executable: PathBuf,
    directory: PathBuf,
    limits: Limits,
    permits: Arc<Semaphore>,
    concurrency: u32,
    stopping: AtomicBool,
    shutdown: Notify,
}
impl Executor {
    pub fn new(
        executable: PathBuf,
        directory: PathBuf,
        limits: Limits,
        concurrency: usize,
    ) -> eyre::Result<Self> {
        limits.validate().map_err(eyre::Report::msg)?;
        if !(1..=64).contains(&concurrency) {
            eyre::bail!("Concurrency must be 1 to 64");
        }
        std::fs::create_dir_all(&directory)?;
        Ok(Self {
            executable,
            directory,
            limits,
            permits: Arc::new(Semaphore::new(concurrency)),
            concurrency: concurrency as u32,
            stopping: AtomicBool::new(false),
            shutdown: Notify::new(),
        })
    }

    /// Stop admission, terminate active children, and wait for their cleanup.
    pub async fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        self.shutdown.notify_waiters();
        let _all_slots = self.permits.acquire_many(self.concurrency).await;
        self.permits.close();
    }

    /// Validate input and run one child, killing and reaping it before a timeout is returned.
    pub async fn perform(
        &self,
        args: Vec<Var>,
        timeout: Option<Duration>,
    ) -> Result<Var, WorkerError> {
        let cancelled = self.shutdown.notified();
        tokio::pin!(cancelled);
        cancelled.as_mut().enable();
        if self.stopping.load(Ordering::Acquire) {
            return Err(WorkerError::WorkerDetached(
                "Git worker is shutting down".into(),
            ));
        }
        let request = match protocol::parse(&args, &self.limits) {
            Ok(request) => request,
            Err(error) => return Ok(error.value()),
        };
        if request.operation == Operation::Capabilities {
            return Ok(protocol::capabilities(&self.limits, self.concurrency));
        }
        let Ok(_permit) = self.permits.try_acquire() else {
            return Ok(Error::new("busy", "All Git worker slots are occupied").value());
        };
        let duration = timeout
            .unwrap_or(Duration::from_secs(self.limits.max_seconds))
            .min(Duration::from_secs(self.limits.max_seconds));
        let directory = tempfile::Builder::new()
            .prefix("job-")
            .tempdir_in(&self.directory)
            .map_err(internal)?;
        let input = serde_json::to_vec(&request).map_err(internal)?;
        // Bound metadata as well as content in the IPC response.
        let max_response = self.limits.max_total_bytes + self.limits.max_entries * 4608 + 65536;
        let mut command = Command::new(&self.executable);
        command
            .arg("--git-job")
            .arg(directory.path())
            .env_clear()
            // The combined server uses mimalloc, whose default 1 GiB arena reservation
            // exhausts the default address-space ceiling before a job can do useful work.
            // Bound reservation chunks in children without changing the server allocator.
            .env("MIMALLOC_ARENA_RESERVE", "65536")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(internal)?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child
            .stdout
            .take()
            .expect("piped stdout")
            .take(max_response as u64 + 1);
        let work = async {
            stdin.write_all(&input).await?;
            drop(stdin);
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await?;
            if bytes.len() > max_response {
                return Err(std::io::Error::other("Git response exceeds IPC limit"));
            }
            let status = child.wait().await?;
            Ok::<_, std::io::Error>((status, bytes))
        };
        let outcome = tokio::select! {
            outcome = tokio::time::timeout(duration, work) => Some(outcome),
            _ = cancelled => None,
        };
        let Some(outcome) = outcome else {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(WorkerError::WorkerDetached(
                "Git worker is shutting down".into(),
            ));
        };
        match outcome {
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                Err(WorkerError::RequestTimedOut(
                    "Git request exceeded its deadline".into(),
                ))
            }
            Ok(Err(error)) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                Err(internal(error))
            }
            Ok(Ok((status, bytes))) => {
                if !status.success() {
                    use std::os::unix::process::ExitStatusExt;
                    if status.signal() == Some(libc::SIGXFSZ) {
                        return Ok(Error::new(
                            "limit_exceeded",
                            "Git job exceeded its temporary file limit",
                        )
                        .value());
                    }
                    // Resource-limit signals and allocation failures terminate only this job.
                    return Ok(Error::new(
                        "job_failed",
                        "Git job failed or exceeded an operating-system resource limit",
                    )
                    .value());
                }
                decode_var_cbor(&bytes).map_err(internal)
            }
        }
    }
}
fn internal(error: impl std::fmt::Display) -> WorkerError {
    tracing::error!(%error, "Git worker execution failed");
    WorkerError::InternalError("Git worker execution failed".into())
}

/// Internal child entry point. The parent exclusively supplies the request and directory.
/// This runs before Tokio starts, so resource limits cover all backend threads.
pub fn child_main(directory: &Path) -> eyre::Result<()> {
    let mut input = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut input)?;
    if input.len() > 65536 {
        eyre::bail!("Job input exceeds limit");
    }
    let request: Request = serde_json::from_slice(&input)?;
    request.limits.validate().map_err(eyre::Report::msg)?;
    set_limit(libc::RLIMIT_AS, request.limits.max_memory_bytes as u64)?;
    // gix receives one pack per request. Limit the pack and every other individual temporary file.
    set_limit(libc::RLIMIT_FSIZE, request.limits.max_fetch_bytes as u64)?;
    set_limit(libc::RLIMIT_CORE, 0)?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let result = crate::backend::execute(&request, &directory.join("repository"))
        .unwrap_or_else(|error| error.value());
    std::io::stdout().write_all(&encode_var_cbor(&result)?)?;
    Ok(())
}
fn set_limit(resource: libc::__rlimit_resource_t, value: u64) -> std::io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: value as libc::rlim_t,
        rlim_max: value as libc::rlim_t,
    };
    // SAFETY: the pointer is valid and the call affects only this dedicated child process.
    if unsafe { libc::setrlimit(resource, &limit) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
