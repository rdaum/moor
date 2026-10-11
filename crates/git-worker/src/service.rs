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

//! Local runtime adapter for the same isolated executor used by the standalone worker.

use crate::job::Executor;
use eyre::{Result, eyre};
use moor_runtime_api::{DaemonToWorkerEvent, WorkerServices};
use moor_var::Symbol;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::task::JoinSet;
use uuid::Uuid;

/// Attach Git access to an in-process worker transport. Repository work still runs in children.
/// On shutdown or transport failure, terminate active jobs before detaching the worker.
pub async fn run_with_services(
    executor: Arc<Executor>,
    kill_switch: Arc<AtomicBool>,
    services: Arc<dyn WorkerServices>,
) -> Result<()> {
    let worker_id = Uuid::new_v4();
    let worker_type = Symbol::mk("git");
    let mut events = services
        .attach_worker(worker_id, worker_type)
        .await
        .map_err(|e| eyre!("Cannot attach local Git worker: {e}"))?;
    let mut jobs = JoinSet::new();
    let result = loop {
        if kill_switch.load(Ordering::Relaxed) {
            break Ok(());
        }
        let event = tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
            result = jobs.join_next(), if !jobs.is_empty() => {
                match result {
                    Some(Ok(Ok(()))) => continue,
                    Some(Ok(Err(error))) => break Err(error),
                    Some(Err(error)) => break Err(eyre!("Git job task failed: {error}")),
                    None => continue,
                }
            }
            event = events.recv() => match event {
                Ok(event) => event,
                Err(error) => break Err(eyre!("Local Git worker transport failed: {error}")),
            },
        };
        match event {
            DaemonToWorkerEvent::Ping => {
                if let Err(error) = services.worker_pong(worker_id, worker_type).await {
                    break Err(eyre!("Cannot send Git worker pong: {error}"));
                }
            }
            DaemonToWorkerEvent::Request {
                request_id,
                request,
                timeout,
                ..
            } => {
                let executor = executor.clone();
                let services = services.clone();
                jobs.spawn(async move {
                    let result = match executor.perform(request, timeout).await {
                        Ok(value) => services.request_result(worker_id, request_id, value).await,
                        Err(error) => services.request_error(worker_id, request_id, error).await,
                    };
                    result.map_err(|e| eyre!("Cannot deliver Git worker result: {e}"))
                });
            }
            DaemonToWorkerEvent::PleaseDie => break Ok(()),
        }
    };
    executor.shutdown().await;
    jobs.abort_all();
    while jobs.join_next().await.is_some() {}
    let detached = services
        .detach_worker(worker_id)
        .await
        .map_err(|e| eyre!("Cannot detach local Git worker: {e}"));
    result.and(detached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use moor_common::tasks::WorkerError;
    use moor_runtime_api::{RpcError, WorkerEventSubscription};
    use moor_var::{Obj, Var, v_int, v_map, v_str};
    use std::sync::Mutex;
    use tokio::sync::mpsc;

    #[derive(Debug)]
    enum Reply {
        Attached(Uuid, Symbol),
        Pong(Uuid, Symbol),
        Result(Uuid, Uuid, Var),
        Detached(Uuid),
    }

    struct Subscription(mpsc::UnboundedReceiver<DaemonToWorkerEvent>);

    #[async_trait]
    impl WorkerEventSubscription for Subscription {
        async fn recv(&mut self) -> Result<DaemonToWorkerEvent, RpcError> {
            self.0
                .recv()
                .await
                .ok_or_else(|| RpcError::ConnectionLost("test transport closed".into()))
        }
    }

    struct Services {
        events: Mutex<Option<Subscription>>,
        replies: mpsc::UnboundedSender<Reply>,
        fail_delivery: bool,
    }

    #[async_trait]
    impl WorkerServices for Services {
        async fn attach_worker(
            &self,
            id: Uuid,
            kind: Symbol,
        ) -> Result<Box<dyn WorkerEventSubscription>, RpcError> {
            self.replies.send(Reply::Attached(id, kind)).unwrap();
            Ok(Box::new(self.events.lock().unwrap().take().unwrap()))
        }
        async fn detach_worker(&self, id: Uuid) -> Result<(), RpcError> {
            self.replies.send(Reply::Detached(id)).unwrap();
            Ok(())
        }
        async fn worker_pong(&self, id: Uuid, kind: Symbol) -> Result<(), RpcError> {
            self.replies.send(Reply::Pong(id, kind)).unwrap();
            Ok(())
        }
        async fn request_result(
            &self,
            worker: Uuid,
            request: Uuid,
            value: Var,
        ) -> Result<(), RpcError> {
            if self.fail_delivery {
                return Err(RpcError::ConnectionLost(
                    "test result delivery failed".into(),
                ));
            }
            self.replies
                .send(Reply::Result(worker, request, value))
                .unwrap();
            Ok(())
        }
        async fn request_error(
            &self,
            _: Uuid,
            _: Uuid,
            error: WorkerError,
        ) -> Result<(), RpcError> {
            panic!("unexpected worker error: {error:?}");
        }
    }

    async fn receive(replies: &mut mpsc::UnboundedReceiver<Reply>) -> Reply {
        tokio::time::timeout(Duration::from_secs(5), replies.recv())
            .await
            .unwrap()
            .unwrap()
    }

    // Exercise all shutdown paths with the real executor, including delivery failure while
    // the subscription is still connected. Capabilities do not require network access.
    #[tokio::test]
    async fn routing_and_shutdown() {
        for shutdown in ["request", "signal", "transport", "delivery"] {
            let directory = tempfile::tempdir().unwrap();
            let executor = Arc::new(
                Executor::new(
                    std::env::current_exe().unwrap(),
                    directory.path().into(),
                    Default::default(),
                    2,
                )
                .unwrap(),
            );
            let (events, incoming) = mpsc::unbounded_channel();
            let (outgoing, mut replies) = mpsc::unbounded_channel();
            let services = Arc::new(Services {
                events: Mutex::new(Some(Subscription(incoming))),
                replies: outgoing,
                fail_delivery: shutdown == "delivery",
            });
            let kill = Arc::new(AtomicBool::new(false));
            let task = tokio::spawn(run_with_services(executor.clone(), kill.clone(), services));
            let Reply::Attached(worker, kind) = receive(&mut replies).await else {
                panic!("not attached")
            };
            assert_eq!(kind, Symbol::mk("git"));
            events.send(DaemonToWorkerEvent::Ping).unwrap();
            assert!(
                matches!(receive(&mut replies).await, Reply::Pong(id, kind) if id == worker && kind == Symbol::mk("git"))
            );
            let request = Uuid::new_v4();
            events
                .send(DaemonToWorkerEvent::Request {
                    request_id: request,
                    authority_principal: Obj::mk_id(3),
                    request: vec![v_str("capabilities"), v_map(&[(v_str("schema"), v_int(1))])],
                    timeout: Some(Duration::from_secs(1)),
                })
                .unwrap();
            if shutdown != "delivery" {
                let Reply::Result(id, received, value) = receive(&mut replies).await else {
                    panic!("missing result")
                };
                assert_eq!((id, received), (worker, request));
                assert_eq!(value, crate::protocol::capabilities(&Default::default(), 2));
            }
            match shutdown {
                "request" => events.send(DaemonToWorkerEvent::PleaseDie).unwrap(),
                "signal" => kill.store(true, Ordering::Relaxed),
                "transport" => drop(events),
                "delivery" => (),
                _ => unreachable!(),
            }
            assert!(matches!(receive(&mut replies).await, Reply::Detached(id) if id == worker));
            let result = tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                result.is_err(),
                matches!(shutdown, "transport" | "delivery")
            );
            assert!(matches!(
                executor.perform(vec![], None).await,
                Err(WorkerError::WorkerDetached(_))
            ));
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }
}
