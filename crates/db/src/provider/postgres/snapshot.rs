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

//! Owned read sessions. A bounded FIFO pool limits snapshots independently of the writer.
use super::{
    PostgresConnection, PostgresError, PostgresParam, PostgresShutdown, PostgresStorageConfig,
    codec::invalid,
    rows,
    state::{self, Progress},
};
use crate::provider::logical::PublicationId;
use flume::{Receiver, Sender};
use parking_lot::{Condvar, Mutex};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(10);
// Each fetch owns at most this many rows, each bounded by max_row_bytes.
const FETCH_ROWS: usize = 8;

pub(super) struct ExportPool {
    limit: usize,
    state: Mutex<PoolState>,
    changed: Condvar,
}
#[derive(Default)]
struct PoolState {
    next: u64,
    waiting: VecDeque<u64>,
    active: BTreeMap<u64, Instant>,
}
struct ExportLease {
    pool: Arc<ExportPool>,
    id: u64,
}
impl Drop for ExportLease {
    fn drop(&mut self) {
        self.pool.state.lock().active.remove(&self.id);
        self.pool.changed.notify_all();
    }
}
impl ExportPool {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            state: Mutex::new(PoolState::default()),
            changed: Condvar::new(),
        })
    }
    pub fn diagnostics(&self) -> (u64, u64) {
        let state = self.state.lock();
        (
            state.active.len() as u64,
            state
                .active
                .values()
                .map(|t| super::metrics::nanos(t.elapsed()) / 1000)
                .max()
                .unwrap_or(0),
        )
    }
    fn acquire(
        self: &Arc<Self>,
        deadline: Instant,
        running: &impl Fn() -> bool,
    ) -> Result<ExportLease, PostgresError> {
        let mut state = self.state.lock();
        let id = state.next;
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| invalid("snapshot", "export ticket exhausted"))?;
        state.waiting.push_back(id);
        loop {
            let error = if !running() {
                Some(PostgresError::Shutdown)
            } else if Instant::now() >= deadline {
                Some(PostgresError::Timeout)
            } else {
                None
            };
            if let Some(error) = error {
                state.waiting.retain(|ticket| *ticket != id);
                self.changed.notify_all();
                return Err(error);
            }
            if state.active.len() < self.limit && state.waiting.front() == Some(&id) {
                state.waiting.pop_front();
                state.active.insert(id, Instant::now());
                self.changed.notify_all();
                return Ok(ExportLease {
                    pool: self.clone(),
                    id,
                });
            }
            self.changed.wait_for(
                &mut state,
                POLL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
}

pub(super) enum Command {
    Open {
        relation: &'static str,
        predicate: String,
        parameters: Vec<String>,
        order: String,
    },
    Fetch(u64),
    Close(u64),
}
struct Request {
    command: Command,
    deadline: Instant,
    reply: Sender<Result<Reply, PostgresError>>,
}
pub(super) enum Reply {
    Cursor(u64),
    Rows(Vec<Value>),
    Closed,
}

pub(super) struct ReadSession {
    requests: Sender<Request>,
    shutdown: PostgresShutdown,
    worker: Mutex<Option<JoinHandle<()>>>,
    pub config: PostgresStorageConfig,
    pub progress: Progress,
}
impl Drop for ReadSession {
    fn drop(&mut self) {
        // Closing the connection ends the transaction. Do not wait for a ROLLBACK round trip.
        self.shutdown.request();
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.join();
        }
    }
}
impl ReadSession {
    pub fn open(
        config: PostgresStorageConfig,
        pool: &Arc<ExportPool>,
        token: PublicationId,
        deadline: Instant,
        running: impl Fn() -> bool,
    ) -> Result<Arc<Self>, PostgresError> {
        let lease = pool.acquire(deadline, &running)?;
        let (requests, receiver) = flume::bounded(32);
        let (opened, opening) = flume::bounded(1);
        let shutdown = PostgresShutdown::default();
        let io_shutdown = shutdown.clone();
        let worker_config = config.clone();
        let worker = thread::Builder::new()
            .name("pg-snapshot".into())
            .spawn(move || {
                let _lease = lease;
                let result = (|| {
                    let mut connection = PostgresConnection::connect(
                        &worker_config.connection,
                        deadline.min(Instant::now() + worker_config.connect_timeout),
                        io_shutdown.clone(),
                    )?;
                    connection.query(
                        "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
                        &[],
                        deadline,
                        |_| unreachable!(),
                    )?;
                    state::validate_metadata(&mut connection, &worker_config, deadline)?;
                    let progress = state::read_progress(&mut connection, &worker_config, deadline)?;
                    if progress.epoch != token.epoch().as_u64()
                        || progress.applied < token.version()
                    {
                        return Err(PostgresError::OwnershipLost);
                    }
                    Ok((connection, progress))
                })();
                match result {
                    Ok((connection, progress)) => {
                        if opened.send(Ok(progress)).is_ok() {
                            serve(connection, &worker_config, receiver, io_shutdown);
                        }
                    }
                    Err(error) => {
                        let _ = opened.send(Err(error));
                    }
                }
            })
            .map_err(|_| PostgresError::Configuration("cannot spawn PostgreSQL snapshot reader"))?;
        let progress = loop {
            if !running() || Instant::now() >= deadline {
                shutdown.request();
                let _ = worker.join();
                return Err(if running() {
                    PostgresError::Timeout
                } else {
                    PostgresError::Shutdown
                });
            }
            match opening.recv_timeout(POLL.min(deadline.saturating_duration_since(Instant::now())))
            {
                Ok(Ok(progress)) => break progress,
                Ok(Err(error)) => {
                    let _ = worker.join();
                    return Err(error);
                }
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => {
                    let _ = worker.join();
                    return Err(PostgresError::Closed);
                }
            }
        };
        Ok(Arc::new(Self {
            requests,
            shutdown,
            worker: Mutex::new(Some(worker)),
            config,
            progress,
        }))
    }
    pub fn request(&self, command: Command) -> Result<Reply, PostgresError> {
        let deadline = Instant::now() + self.config.query_timeout;
        let (reply, response) = flume::bounded(1);
        self.requests
            .send_deadline(
                Request {
                    command,
                    deadline,
                    reply,
                },
                deadline,
            )
            .map_err(|error| match error {
                flume::SendTimeoutError::Timeout(_) => PostgresError::Timeout,
                _ => PostgresError::Closed,
            })?;
        match response.recv_deadline(deadline) {
            Ok(result) => result,
            Err(flume::RecvTimeoutError::Timeout) => {
                self.shutdown.request();
                Err(PostgresError::Timeout)
            }
            Err(flume::RecvTimeoutError::Disconnected) => Err(PostgresError::Closed),
        }
    }
}

fn serve(
    mut connection: PostgresConnection,
    config: &PostgresStorageConfig,
    requests: Receiver<Request>,
    shutdown: PostgresShutdown,
) {
    let mut next = 0_u64;
    let mut cursors = BTreeMap::new();
    loop {
        if shutdown.is_requested() {
            return;
        }
        let request = match requests.recv_timeout(POLL) {
            Ok(request) => request,
            Err(flume::RecvTimeoutError::Timeout) => continue,
            Err(_) => return,
        };
        let relation = match &request.command {
            Command::Open { relation, .. } => *relation,
            Command::Fetch(id) | Command::Close(id) => {
                cursors.get(id).copied().unwrap_or("snapshot")
            }
        };
        let result = (|| {
            match request.command {
                Command::Open { relation, predicate, parameters, order } => {
                    let id = next;
                    next = next.checked_add(1).ok_or_else(|| invalid("snapshot", "cursor identity exhausted"))?;
                    let table = config.schema.qualify(relation)?;
                    let sql = format!("DECLARE moor_export_{id} NO SCROLL CURSOR FOR SELECT {} FROM {table} t {predicate} ORDER BY {order}", rows::select_row(relation));
                    let parameters: Vec<_> = parameters.iter().map(|p| PostgresParam::Text(25, p)).collect();
                    connection.query(&sql, &parameters, request.deadline, |_| unreachable!())?;
                    cursors.insert(id, relation);
                    Ok(Reply::Cursor(id))
                }
                Command::Fetch(id) => {
                    if !cursors.contains_key(&id) { return Err(invalid("snapshot", "unknown cursor")); }
                    let mut result = Vec::new();
                    connection.query(&format!("FETCH FORWARD {FETCH_ROWS} FROM moor_export_{id}"), &[], request.deadline, |row| { result.push(rows::parse_row(row)?); Ok(()) })?;
                    Ok(Reply::Rows(result))
                }
                Command::Close(id) => {
                    if cursors.remove(&id).is_some() { connection.query(&format!("CLOSE moor_export_{id}"), &[], request.deadline, |_| unreachable!())?; }
                    Ok(Reply::Closed)
                }
            }
        })().map_err(|source| PostgresError::Operation { relation, operation: "snapshot read", source: Box::new(source) });
        let failed = result.is_err();
        let _ = request.reply.send(result);
        // A disconnect or SQL error invalidates this read transaction. Never reconnect it.
        if failed {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_slots_are_fifo_and_timeouts_do_not_hold_capacity() {
        let pool = ExportPool::new(1);
        let held = pool
            .acquire(Instant::now() + Duration::from_secs(1), &|| true)
            .unwrap();
        assert!(matches!(
            pool.acquire(Instant::now(), &|| true),
            Err(PostgresError::Timeout)
        ));
        assert!(matches!(
            pool.acquire(Instant::now() + Duration::from_secs(1), &|| false),
            Err(PostgresError::Shutdown)
        ));
        let (sent, received) = flume::unbounded();
        std::thread::scope(|scope| {
            for expected in 0..3 {
                let pool = pool.clone();
                let sent = sent.clone();
                let observed = pool.clone();
                scope.spawn(move || {
                    let _lease = pool
                        .acquire(Instant::now() + Duration::from_secs(3), &|| true)
                        .unwrap();
                    sent.send(expected).unwrap();
                });
                let deadline = Instant::now() + Duration::from_secs(2);
                while observed.state.lock().waiting.len() != expected + 1 {
                    assert!(Instant::now() < deadline);
                    std::thread::yield_now();
                }
            }
            drop(held);
            for expected in 0..3 {
                assert_eq!(
                    received.recv_timeout(Duration::from_secs(3)).unwrap(),
                    expected
                );
            }
        });
        assert_eq!(pool.diagnostics(), (0, 0));
        assert!(pool.state.lock().waiting.is_empty());
    }
}
