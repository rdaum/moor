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
use moor_common::tasks::WorkerError;
use moor_kernel::tasks::workers::{WorkerRequest, WorkerResponse};
use moor_var::{Obj, Symbol, Var};
use rpc_common::{
    DaemonToWorkerMessage, DaemonToWorkerReply, MOOR_WORKER_TOKEN_FOOTER, RpcMessageError,
    WORKER_BROADCAST_TOPIC, WorkerToDaemonMessage, WorkerToken,
};
use rusty_paseto::core::{Footer, Key, Paseto, PasetoAsymmetricPublicKey, Public, V4};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
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
    /// Process a worker-to-daemon message
    fn handle_worker_message(
        &self,
        worker_token: &[u8],
        worker_id: &[u8],
        message: WorkerToDaemonMessage,
    ) -> DaemonToWorkerReply;

    /// Check for expired workers and handle pending requests
    fn check_expired_workers(&self);

    /// Send ping to all workers
    fn ping_workers(&self) -> Result<(), eyre::Error>;

    /// Process a worker request from the scheduler
    fn process_worker_request(&self, request: WorkerRequest) -> Result<(), eyre::Error>;
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
            if let Some((t, worker_id)) = worker_tokens.get(token) {
                if t.elapsed().as_secs() <= 60 {
                    return Ok(*worker_id);
                }
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
        msg: WorkerToDaemonMessage,
    ) -> DaemonToWorkerReply {
        // Worker token has to be a valid UTF8 string
        let Ok(worker_token_str) = std::str::from_utf8(worker_token) else {
            error!("Worker token is not valid UTF8");
            return DaemonToWorkerReply::Rejected;
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
            return DaemonToWorkerReply::Rejected;
        };

        let Ok(worker_id) = Uuid::from_slice(worker_id) else {
            error!("Unable to parse worker id {worker_id:?} from message");
            return DaemonToWorkerReply::Rejected;
        };

        // Worker ID must match the one in the token
        if token_worker_id != worker_id.to_string() {
            error!("Worker ID does not match token");
            return DaemonToWorkerReply::Rejected;
        }

        // Now handle the message
        match msg {
            WorkerToDaemonMessage::AttachWorker { token, worker_type } => {
                let mut workers = self.workers.write().unwrap();
                workers.insert(
                    worker_id,
                    Worker {
                        token: token.clone(),
                        last_ping_time: Instant::now(),
                        worker_type,
                        id: worker_id,
                        requests: vec![],
                    },
                );
                info!("Worker {} of type {} attached", worker_id, worker_type);
                DaemonToWorkerReply::Attached(token, worker_id)
            }
            WorkerToDaemonMessage::Pong(token, worker_type) => {
                // Update the last ping time for this worker
                let mut workers = self.workers.write().unwrap();
                if let Some(worker) = workers.get_mut(&worker_id) {
                    worker.last_ping_time = Instant::now();
                    DaemonToWorkerReply::Ack
                } else {
                    warn!(
                        "Received pong from unknown or old worker (did we restart?); re-establishing..."
                    );
                    workers.insert(
                        worker_id,
                        Worker {
                            token: token.clone(),
                            last_ping_time: Instant::now(),
                            worker_type,
                            id: worker_id,
                            requests: vec![],
                        },
                    );
                    info!("Worker {} of type {} re-attached", worker_id, worker_type);
                    DaemonToWorkerReply::Attached(token, worker_id)
                }
            }
            WorkerToDaemonMessage::DetachWorker(_) => {
                // Remove the worker from the list of workers, discarding all its requests
                let mut workers = self.workers.write().unwrap();
                if let Some(worker) = workers.remove(&worker_id) {
                    for (id, _, _) in worker.requests {
                        self.scheduler_send
                            .send(WorkerResponse::Error {
                                request_id: id,
                                error: WorkerError::WorkerDetached(format!(
                                    "{} worker {} detached",
                                    worker.worker_type, worker_id
                                )),
                            })
                            .ok();
                    }
                } else {
                    error!("Received detach from unknown or old worker");
                }
                DaemonToWorkerReply::Ack
            }
            WorkerToDaemonMessage::RequestResult(token, request_id, r) => {
                let worker_id = match self.validate_worker_token(&token) {
                    Ok(id) => id,
                    Err(e) => {
                        error!(error = ?e, "Unable to validate worker token");
                        return DaemonToWorkerReply::Rejected;
                    }
                };

                let mut workers = self.workers.write().unwrap();
                if let Some(worker) = workers.get_mut(&worker_id) {
                    let mut found_request_idx = None;
                    for (idx, (pending_request_id, _, _)) in worker.requests.iter().enumerate() {
                        if request_id == *pending_request_id {
                            found_request_idx = Some(idx);
                            break;
                        }
                    }

                    if let Some(found_request_idx) = found_request_idx {
                        worker.requests.remove(found_request_idx);

                        // Send back the result
                        self.scheduler_send
                            .send(WorkerResponse::Response {
                                request_id,
                                response: r,
                            })
                            .ok();
                    } else {
                        error!("Received result from unknown or old request");
                    }
                } else {
                    error!(
                        "Received result from unknown or old request ({}), ignoring",
                        request_id
                    );
                }
                DaemonToWorkerReply::Ack
            }
            WorkerToDaemonMessage::RequestError(_, request_id, e) => {
                // Same as RequestResult, but dispatch an error
                let mut workers = self.workers.write().unwrap();
                if let Some(worker) = workers.get_mut(&worker_id) {
                    let mut found_request_idx = None;

                    // Search for the request corresponding to the given request_id
                    for (idx, (worker_request_id, _, _)) in worker.requests.iter().enumerate() {
                        if request_id == *worker_request_id {
                            found_request_idx = Some(idx);
                            break;
                        }
                    }

                    if let Some(found_request_idx) = found_request_idx {
                        worker.requests.remove(found_request_idx);

                        self.scheduler_send
                            .send(WorkerResponse::Error {
                                request_id,
                                error: e,
                            })
                            .ok();
                    } else {
                        error!("Received error for an unknown or old request");
                    }
                } else {
                    error!("Received error from unknown or old worker");
                }
                DaemonToWorkerReply::Ack
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
        let event = DaemonToWorkerMessage::PingWorkers;
        let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())
            .context("Unable to encode ping event")?;
        let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes];

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
                let event = DaemonToWorkerMessage::WorkerRequest {
                    worker_id: worker.id,
                    token: worker.token.clone(),
                    id: request_id,
                    perms,
                    request: request.clone(),
                    timeout,
                };
                let event_bytes = bincode::encode_to_vec(&event, bincode::config::standard())
                    .context("Unable to encode worker request")?;
                let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes];

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
        }
    }
}
