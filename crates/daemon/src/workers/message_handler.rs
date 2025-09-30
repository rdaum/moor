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

//! Message handler for workers business logic, separated from transport concerns

use eyre::Context;
use moor_common::{schema::rpc as moor_rpc, tasks::WorkerError};
use moor_kernel::tasks::workers::{WorkerRequest, WorkerResponse};
use moor_var::{Obj, Symbol, Var};
use planus::Builder;
use rpc_common::{
    MOOR_WORKER_TOKEN_FOOTER, RpcMessageError, WORKER_BROADCAST_TOPIC, WorkerToken,
    mk_ping_workers_msg, mk_worker_ack_reply, mk_worker_attached_reply, mk_worker_rejected_reply,
    mk_worker_request_msg, var_from_flatbuffer_bytes, var_to_flatbuffer_bytes,
    worker_error_from_flatbuffer_struct,
};
use rusty_paseto::core::{Footer, Key, Paseto, PasetoAsymmetricPublicKey, Public, V4};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use tracing::{error, info, warn};
use uuid::Uuid;
use zmq::{Socket, SocketType};

pub const WORKER_TIMEOUT: Duration = Duration::from_secs(10);
pub const PING_FREQUENCY: Duration = Duration::from_secs(5);

struct Worker {
    id: Uuid,
    last_ping_time: Instant,
    worker_type: Symbol,
    token: WorkerToken,
    /// The set of pending requests for this worker, that we are waiting on responses for.
    requests: Vec<(Uuid, Obj, Vec<Var>)>,
}

/// Trait for handling workers message business logic
pub trait WorkersMessageHandler: Send + Sync {
    /// Process a worker-to-daemon message (flatbuffer format)
    fn handle_worker_message(
        &self,
        worker_token: &[u8],
        worker_id: &[u8],
        message: &moor_rpc::WorkerToDaemonMessageRef,
    ) -> moor_rpc::DaemonToWorkerReply;

    /// Check for expired workers and handle pending requests
    fn check_expired_workers(&self);

    /// Send ping to all workers
    fn ping_workers(&self) -> Result<(), eyre::Error>;

    /// Process a worker request from the scheduler
    fn process_worker_request(&self, request: WorkerRequest) -> Result<(), eyre::Error>;

    /// Get information about all workers
    fn get_workers_info(&self) -> Vec<moor_kernel::tasks::task_scheduler_client::WorkerInfo>;
}

/// Implementation of message handler that contains the actual business logic
pub struct WorkersMessageHandlerImpl {
    public_key: Key<32>,
    #[allow(dead_code)]
    private_key: Key<64>,
    workers: Arc<RwLock<HashMap<Uuid, Worker>>>,
    scheduler_send: flume::Sender<WorkerResponse>,
    token_cache: Arc<RwLock<HashMap<WorkerToken, (Instant, Uuid)>>>,
    workers_publish: Arc<Mutex<Socket>>,
}

impl WorkersMessageHandlerImpl {
    pub fn new(
        zmq_context: zmq::Context,
        workers_broadcast: &str,
        public_key: Key<32>,
        private_key: Key<64>,
        scheduler_send: flume::Sender<WorkerResponse>,
    ) -> Result<Self, eyre::Error> {
        // Create the publish socket for broadcasting to workers
        let publish = zmq_context
            .socket(SocketType::PUB)
            .context("Unable to create ZMQ PUB socket")?;
        publish
            .bind(workers_broadcast)
            .context("Unable to bind ZMQ PUB socket")?;

        let workers_publish = Arc::new(Mutex::new(publish));

        Ok(Self {
            public_key,
            private_key,
            workers: Arc::new(RwLock::new(HashMap::new())),
            scheduler_send,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
            workers_publish,
        })
    }

    fn validate_worker_token(&self, token: &WorkerToken) -> Result<Uuid, RpcMessageError> {
        // Check cache first
        {
            let worker_tokens = self.token_cache.read().unwrap();
            if let Some((t, worker_id)) = worker_tokens.get(token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(*worker_id);
            }
        }

        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let worker_id = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_WORKER_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let worker_id = Uuid::parse_str(worker_id.as_str()).map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        // Cache the result
        let mut tokens = self.token_cache.write().unwrap();
        tokens.insert(token.clone(), (Instant::now(), worker_id));

        Ok(worker_id)
    }
}

impl WorkersMessageHandler for WorkersMessageHandlerImpl {
    fn handle_worker_message(
        &self,
        worker_token: &[u8],
        worker_id: &[u8],
        message: &moor_rpc::WorkerToDaemonMessageRef,
    ) -> moor_rpc::DaemonToWorkerReply {
        // Worker token has to be a valid UTF8 string
        let Ok(worker_token_str) = std::str::from_utf8(worker_token) else {
            error!("Worker token is not valid UTF8");
            return mk_worker_invalid_payload_reply("Worker token is not valid UTF8");
        };

        // Verify the token
        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);

        let Ok(token_worker_id) = Paseto::<V4, Public>::try_verify(
            worker_token_str,
            &pk,
            Footer::from(MOOR_WORKER_TOKEN_FOOTER),
            None,
        ) else {
            error!("Unable to verify worker token; ignoring");
            return mk_worker_auth_failed_reply("Unable to verify worker token");
        };

        let Ok(worker_id) = Uuid::from_slice(worker_id) else {
            error!("Unable to parse worker id {worker_id:?} from message");
            return mk_worker_invalid_payload_reply("Invalid worker ID format");
        };

        // Worker ID must match the one in the token
        if token_worker_id != worker_id.to_string() {
            error!("Worker ID does not match token");
            return mk_worker_auth_failed_reply("Worker ID does not match token");
        }

        // Now handle the message using flatbuffer types directly
        let Ok(message_union) = message.message() else {
            error!("Failed to read message union from WorkerToDaemonMessage");
            return mk_worker_invalid_payload_reply("Missing or invalid message union");
        };

        let result = match message_union {
            moor_rpc::WorkerToDaemonMessageUnionRef::AttachWorker(attach) => {
                self.handle_attach_worker(worker_id, attach)
            }
            moor_rpc::WorkerToDaemonMessageUnionRef::WorkerPong(pong) => {
                self.handle_worker_pong(worker_id, pong)
            }
            moor_rpc::WorkerToDaemonMessageUnionRef::DetachWorker(_detach) => {
                self.handle_detach_worker(worker_id)
            }
            moor_rpc::WorkerToDaemonMessageUnionRef::RequestResult(result) => {
                self.handle_request_result(result)
            }
            moor_rpc::WorkerToDaemonMessageUnionRef::RequestError(error) => {
                self.handle_request_error(worker_id, error)
            }
        }
    }

    fn check_expired_workers(&self) {
        let mut workers = self.workers.write().unwrap();
        let now = Instant::now();
        workers.retain(|_, worker| {
            if now.duration_since(worker.last_ping_time) > WORKER_TIMEOUT {
                error!(
                    "Worker {} of type {} has expired",
                    worker.id, worker.worker_type
                );
                // Abort all requests for this worker
                for (id, _, _) in &worker.requests {
                    self.scheduler_send
                        .send(WorkerResponse::Error {
                            request_id: *id,
                            error: WorkerError::WorkerDetached(format!(
                                "{} worker {} detached",
                                worker.worker_type, worker.id
                            )),
                        })
                        .ok();
                }
                false
            } else {
                true
            }
        });
    }

    fn ping_workers(&self) -> Result<(), eyre::Error> {
        // Create flatbuffer message
        let fb_message = mk_ping_workers_msg();

        let mut builder = Builder::new();
        let event_bytes = builder.finish(&fb_message, None);
        let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes.to_vec()];

        let publish = self.workers_publish.lock().unwrap();
        publish
            .send_multipart(payload, 0)
            .context("Unable to send ping to workers")?;
        Ok(())
    }

    fn process_worker_request(&self, request: WorkerRequest) -> Result<(), eyre::Error> {
        match request {
            WorkerRequest::Request {
                request_id,
                request_type,
                perms,
                request,
                timeout,
            } => {
                // Pick a worker of the given type to send the request to,
                // preferably one with the lowest # of requests already queued up
                let mut workers = self.workers.write().unwrap();
                let mut found_worker = None;
                let mut min_requests = usize::MAX;
                for (worker_id, worker) in workers.iter_mut() {
                    if worker.worker_type == request_type && worker.requests.len() < min_requests {
                        min_requests = worker.requests.len();
                        found_worker = Some((worker_id, worker));
                    }
                }

                let Some((_, worker)) = found_worker else {
                    error!("No workers available for request type {}", request_type);
                    self.scheduler_send
                        .send(WorkerResponse::Error {
                            request_id,
                            error: WorkerError::NoWorkerAvailable(request_type),
                        })
                        .ok();
                    return Ok(());
                };

                // Then send the message out on the workers broadcast channel
                // Convert request parameters to flatbuffer VarBytes
                let request_var_bytes: Result<Vec<Vec<u8>>, _> =
                    request.iter().map(var_to_flatbuffer_bytes).collect();
                let request_bytes = request_var_bytes
                    .context("Failed to serialize request variables to flatbuffer")?;

                let timeout_ms = timeout.map(|d| d.as_millis() as u64).unwrap_or(0);

                // Create flatbuffer WorkerRequest message using builder
                let request_varbytes: Vec<moor_rpc::VarBytes> = request_bytes
                    .into_iter()
                    .map(|bytes| moor_rpc::VarBytes { data: bytes })
                    .collect();

                let fb_message = mk_worker_request_msg(
                    worker.id,
                    &worker.token,
                    request_id,
                    &perms,
                    request_varbytes,
                    timeout_ms,
                );

                let mut builder = Builder::new();
                let event_bytes = builder.finish(&fb_message, None);
                let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes.to_vec()];

                {
                    let publish = self.workers_publish.lock().unwrap();
                    publish
                        .send_multipart(payload, 0)
                        .context("Unable to send request to worker")?;
                }

                info!(
                    "Sending request to worker {} of type {}",
                    worker.id, worker.worker_type
                );

                // Then shove it into the queue for the given worker
                worker.requests.push((request_id, perms, request));
                Ok(())
            }
            WorkerRequest::GetWorkersInfo { request_id } => {
                let workers_info = self.get_workers_info();
                self.scheduler_send
                    .send(WorkerResponse::WorkersInfo {
                        request_id,
                        workers_info,
                    })
                    .map_err(|e| eyre::eyre!("Failed to send workers info response: {e}"))?;
                Ok(())
            }
        }
    }

    fn get_workers_info(&self) -> Vec<moor_kernel::tasks::task_scheduler_client::WorkerInfo> {
        use moor_kernel::tasks::task_scheduler_client::WorkerInfo;
        use std::collections::HashMap;

        let workers = self.workers.read().unwrap();
        let now = std::time::Instant::now();

        // Group workers by type and calculate statistics
        let mut worker_stats: HashMap<moor_var::Symbol, (usize, usize, Vec<f64>)> = HashMap::new();

        for worker in workers.values() {
            let entry = worker_stats
                .entry(worker.worker_type)
                .or_insert((0, 0, Vec::new()));
            entry.0 += 1; // worker_count
            entry.1 += worker.requests.len(); // total_queue_size

            // Calculate time since last ping in seconds
            let last_ping_ago_secs = now.duration_since(worker.last_ping_time).as_secs_f64();
            entry.2.push(last_ping_ago_secs);
        }

        // Convert to WorkerInfo structs
        worker_stats
            .into_iter()
            .map(
                |(worker_type, (worker_count, total_queue_size, ping_times))| {
                    let last_ping_ago_secs = ping_times.iter().copied().fold(0.0f64, f64::max);
                    WorkerInfo {
                        worker_type,
                        worker_count,
                        total_queue_size,
                        avg_response_time_ms: 0.0, // TODO: Implement response time tracking
                        last_ping_ago_secs,
                    }
                },
            )
            .collect()
    }
}
