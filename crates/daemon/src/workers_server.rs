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
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;
use zmq::SocketType;

pub const WORKER_TIMEOUT: Duration = Duration::from_secs(10);
pub const PING_FREQUENCY: Duration = Duration::from_secs(5);

pub struct WorkersServer {
    zmq_context: zmq::Context,
    workers: Arc<RwLock<HashMap<Uuid, Worker>>>,
    kill_switch: Arc<AtomicBool>,
    jh: Option<std::thread::JoinHandle<()>>,
    public_key: Key<32>,
    #[allow(dead_code)]
    private_key: Key<64>,
    scheduler_send: crossbeam_channel::Sender<WorkerResponse>,
    token_cache: Arc<RwLock<HashMap<WorkerToken, (Instant, Uuid)>>>,
}

struct Worker {
    #[allow(dead_code)]
    id: Uuid,
    last_ping_time: Instant,
    worker_type: Symbol,
    token: WorkerToken,
    /// The set of pending requests for this worker, that we are waiting on responses for.
    requests: Vec<(Uuid, Obj, Vec<Var>)>,
}

fn reject(rpc_socket: &zmq::Socket) {
    let response = DaemonToWorkerReply::Rejected;
    let Ok(response) = bincode::encode_to_vec(&response, bincode::config::standard()) else {
        error!("Unable to encode response");
        return;
    };

    let Ok(_) = rpc_socket.send_multipart([&response], 0) else {
        error!("Unable to send response");
        return;
    };
}

fn ack(rpc_socket: &zmq::Socket) {
    let response = DaemonToWorkerReply::Ack;
    let Ok(response) = bincode::encode_to_vec(&response, bincode::config::standard()) else {
        error!("Unable to encode response");
        return;
    };

    let Ok(_) = rpc_socket.send_multipart([&response], 0) else {
        error!("Unable to send response");
        return;
    };
}

fn process(
    publish: zmq::Socket,
    ks: Arc<AtomicBool>,
    recv: crossbeam_channel::Receiver<WorkerRequest>,
    send: crossbeam_channel::Sender<WorkerResponse>,
    workers: Arc<RwLock<HashMap<Uuid, Worker>>>,
) {
    let mut last_ping_out = Instant::now();
    loop {
        if ks.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Workers server thread exiting.");
            break;
        }

        // Check for expired workers and remove them.
        {
            let mut workers = workers.write().unwrap();
            let now = Instant::now();
            workers.retain(|_, worker| {
                if now.duration_since(worker.last_ping_time) > WORKER_TIMEOUT {
                    error!(
                        "Worker {} of type {} has expired",
                        worker.id, worker.worker_type
                    );
                    // Abort all requests for this worker.
                    for (id, _, _) in &worker.requests {
                        send.send(WorkerResponse::Error {
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

        // If it's been a while since we sent a ping, send one out to all workers.
        if last_ping_out.elapsed() > PING_FREQUENCY {
            let event = DaemonToWorkerMessage::PingWorkers;
            let Ok(event_bytes) = bincode::encode_to_vec(&event, bincode::config::standard())
            else {
                error!("Unable to encode event");
                continue;
            };
            let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes];
            if let Err(e) = publish.send_multipart(payload, 0) {
                error!("Unable to send message to workers: {}", e);
            }
            last_ping_out = Instant::now();
        }

        match recv.recv_timeout(Duration::from_millis(200)) {
            Ok(WorkerRequest::Request {
                request_id,
                request_type,
                perms,
                request,
                timeout,
            }) => {
                // Pick a worker of the given type to send the request to, preferably one
                // with the lowest # of requests already queueud up.
                // If we can't find one, send an error back on the response channel.
                let mut workers = workers.write().unwrap();
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
                    send.send(WorkerResponse::Error {
                        request_id,
                        error: WorkerError::NoWorkerAvailable(request_type),
                    })
                    .ok();

                    continue;
                };

                // Then send the message out on the workers broadcast channel.
                let event = DaemonToWorkerMessage::WorkerRequest {
                    worker_id: worker.id,
                    token: worker.token.clone(),
                    id: request_id,
                    perms,
                    request: request.clone(),
                    timeout,
                };
                let Ok(event_bytes) = bincode::encode_to_vec(&event, bincode::config::standard())
                else {
                    error!("Unable to encode event");
                    continue;
                };
                let payload = vec![WORKER_BROADCAST_TOPIC.to_vec(), event_bytes];
                {
                    if let Err(e) = publish.send_multipart(payload, 0) {
                        error!("Unable to send message to workers: {}", e);

                        send.send(WorkerResponse::Error {
                            request_id,
                            error: WorkerError::RequestError(format!(
                                "Unable to send message to workers: {}",
                                e
                            )),
                        })
                        .ok();

                        continue;
                    }

                    info!(
                        "Sending request to worker {} of type {}",
                        worker.id, worker.worker_type
                    );
                }
                // Then shove it into the queue for the given worker, with timeout info.
                worker.requests.push((request_id, perms, request)); // TODO: update to store timeout if needed for enforcement
            }
            Err(_) => continue,
        }
    }
}

impl WorkersServer {
    /// Start up and listen for messages from workers on the given endpoint.
    pub fn new(
        kill_switch: Arc<AtomicBool>,
        public_key: Key<32>,
        private_key: Key<64>,
        zmq_context: zmq::Context,
        scheduler_send: crossbeam_channel::Sender<WorkerResponse>,
    ) -> Self {
        Self {
            public_key,
            private_key,
            workers: Arc::new(RwLock::new(HashMap::new())),
            zmq_context,
            jh: None,
            kill_switch,
            scheduler_send,
            token_cache: Arc::new(Default::default()),
        }
    }

    /// Start the background thread which listens for messages from the mooR side and queues
    /// them up for processing.
    pub fn start(
        &mut self,
        workers_broadcast: &str,
    ) -> Result<crossbeam_channel::Sender<WorkerRequest>, eyre::Report> {
        let (send, recv) = crossbeam_channel::unbounded::<WorkerRequest>();
        let b = std::thread::Builder::new().name("moor-workers-out".into());
        let ks = self.kill_switch.clone();

        let publish = self
            .zmq_context
            .socket(SocketType::PUB)
            .expect("Unable to create ZMQ PUB socket");
        publish
            .bind(workers_broadcast)
            .expect("Unable to bind ZMQ PUB socket");

        let response_send = self.scheduler_send.clone();
        let workers = self.workers.clone();
        let jh = b
            .spawn(move || process(publish, ks.clone(), recv, response_send, workers))
            .with_context(|| "Error starting workers server thread")?;
        self.jh = Some(jh);
        Ok(send)
    }

    pub fn listen(&mut self, workers_endpoint: &str) -> Result<(), eyre::Report> {
        let rpc_socket = self.zmq_context.socket(zmq::REP)?;
        rpc_socket.bind(workers_endpoint)?;

        info!(
            "Workers 0mq server listening on {} with {} IO threads",
            workers_endpoint,
            self.zmq_context.get_io_threads().unwrap()
        );

        loop {
            let poll_result = rpc_socket
                .poll(zmq::POLLIN, 100)
                .with_context(|| "Error polling ZMQ socket. Bailing out.")?;
            if poll_result == 0 {
                continue;
            }

            let msg = rpc_socket
                .recv_multipart(0)
                .with_context(|| "Error receiving message from ZMQ socket. Bailing out.")?;
            if msg.len() != 3 {
                warn!(
                    "Received message with {} parts, expected 3; rejecting",
                    msg.len()
                );
                reject(&rpc_socket);
                continue;
            }

            // First argument should be a WorkerToken, or we don't like it.
            // Second argument is a Uuid.
            // Third argument is a bincoded WorkerToDaemonMessage. Or we don't like it.
            let (worker_token, worker_id, request) = (&msg[0], &msg[1], &msg[2]);

            // Worker token has to be a valid UTF8 string.
            let Ok(worker_token) = std::str::from_utf8(worker_token) else {
                error!("Worker token is not valid UTF8");
                reject(&rpc_socket);
                continue;
            };

            let pk: PasetoAsymmetricPublicKey<V4, Public> =
                PasetoAsymmetricPublicKey::from(&self.public_key);

            let Ok(token_worker_id) = Paseto::<V4, Public>::try_verify(
                worker_token,
                &pk,
                Footer::from(MOOR_WORKER_TOKEN_FOOTER),
                None,
            ) else {
                error!("Unable to verify worker token; ignoring");
                reject(&rpc_socket);
                continue;
            };

            let Ok(worker_id) = Uuid::from_slice(worker_id) else {
                error!("Unable to parse worker id {worker_id:?} from message");
                reject(&rpc_socket);
                continue;
            };

            // Worker ID must match the one in the token.
            if token_worker_id != worker_id.to_string() {
                error!("Worker ID does not match token");
                reject(&rpc_socket);
                continue;
            }

            let Ok((msg, _)) = bincode::decode_from_slice::<WorkerToDaemonMessage, _>(
                request,
                bincode::config::standard(),
            ) else {
                error!("Unable to decode WorkerToDaemonMessage message");
                reject(&rpc_socket);
                continue;
            };

            // Now handle the message.
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
                    let response = DaemonToWorkerReply::Attached(token, worker_id);
                    let Ok(response) =
                        bincode::encode_to_vec(&response, bincode::config::standard())
                    else {
                        error!("Unable to encode response");
                        reject(&rpc_socket);
                        continue;
                    };

                    info!("Attaching worker {} of type {}", worker_id, worker_type);
                    let Ok(_) = rpc_socket.send_multipart([&response], 0) else {
                        error!("Unable to send response");
                        continue;
                    };

                    info!("Worker {} attached", worker_id);
                }
                WorkerToDaemonMessage::Pong(token, worker_type) => {
                    // Update the last ping time for this worker.
                    let mut workers = self.workers.write().unwrap();
                    if let Some(worker) = workers.get_mut(&worker_id) {
                        worker.last_ping_time = Instant::now();
                        ack(&rpc_socket);
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

                        let response = DaemonToWorkerReply::Attached(token, worker_id);
                        let Ok(response) =
                            bincode::encode_to_vec(&response, bincode::config::standard())
                        else {
                            error!("Unable to encode response");
                            reject(&rpc_socket);
                            continue;
                        };

                        info!("Attaching worker {} of type {}", worker_id, worker_type);
                        let Ok(_) = rpc_socket.send_multipart([&response], 0) else {
                            error!("Unable to send response");
                            continue;
                        };

                        info!("Worker {} attached", worker_id);
                        continue;
                    }
                }
                WorkerToDaemonMessage::DetachWorker(_) => {
                    ack(&rpc_socket);
                    // Remove the worker from the list of workers, discarding all its requests, and
                    // if there's a response channel, send an error.
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
                        continue;
                    }
                }
                WorkerToDaemonMessage::RequestResult(token, request_id, r) => {
                    ack(&rpc_socket);

                    let worker_id = self
                        .validate_worker_token(&token)
                        .map_err(|e| {
                            error!(error = ?e, "Unable to validate worker token");
                            reject(&rpc_socket);
                        })
                        .ok();

                    let Some(worker_id) = worker_id else {
                        error!("Unable to validate worker token");
                        continue;
                    };

                    let mut workers = self.workers.write().unwrap();
                    if let Some(worker) = workers.get_mut(&worker_id) {
                        let mut found_request_idx = None;
                        for (idx, (pending_request_id, _, _)) in worker.requests.iter().enumerate()
                        {
                            if request_id == *pending_request_id {
                                found_request_idx = Some(idx);
                                break;
                            }
                        }

                        let Some(found_request_idx) = found_request_idx else {
                            error!("Received result from unknown or old worker");
                            continue;
                        };

                        worker.requests.remove(found_request_idx);

                        // Send back the result
                        self.scheduler_send
                            .send(WorkerResponse::Response {
                                request_id,
                                response: r,
                            })
                            .ok();
                    } else {
                        error!(
                            "Received result from unknown or old request ({}), ignoring",
                            request_id
                        );
                        continue;
                    }
                }
                WorkerToDaemonMessage::RequestError(_, request_id, e) => {
                    ack(&rpc_socket);
                    // Same as RequestResult, but dispatch an error.

                    // Handle the RequestError message, which dispatches an error response to the response channel
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

                        let Some(found_request_idx) = found_request_idx else {
                            error!("Received error for an unknown or old request");
                            continue;
                        };

                        worker.requests.remove(found_request_idx);

                        self.scheduler_send
                            .send(WorkerResponse::Error {
                                request_id,
                                error: e,
                            })
                            .ok();
                    } else {
                        error!("Received error from unknown or old worker");
                        reject(&rpc_socket);
                        continue;
                    }
                }
            }
        }
    }

    fn validate_worker_token(&mut self, token: &WorkerToken) -> Result<Uuid, RpcMessageError> {
        // Check cache first.
        {
            let worker_tokens = self.token_cache.read().unwrap();

            if let Some((t, host_type)) = worker_tokens.get(token) {
                if t.elapsed().as_secs() <= 60 {
                    return Ok(*host_type);
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

        // Cache the result.
        let mut tokens = self.token_cache.write().unwrap();
        tokens.insert(token.clone(), (Instant::now(), worker_id));

        Ok(worker_id)
    }
}
