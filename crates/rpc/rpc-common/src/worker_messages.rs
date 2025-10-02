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

//! Worker message builders for DaemonToWorker and WorkerToDaemon messages

use crate::{
    WorkerToken,
    helpers::{mk_worker_token, obj_fb, symbol_fb, uuid_fb},
};
use moor_common::schema::rpc;
use moor_var::{Obj, Symbol};
use uuid::Uuid;

// ============================================================================
// Worker Messages (Daemon -> Worker)
// ============================================================================

/// Build a PingWorkers broadcast message
#[inline]
pub fn mk_ping_workers_msg() -> rpc::DaemonToWorkerMessage {
    rpc::DaemonToWorkerMessage {
        message: rpc::DaemonToWorkerMessageUnion::PingWorkers(Box::new(rpc::PingWorkers {})),
    }
}

/// Build a WorkerRequest message
#[inline]
pub fn mk_worker_request_msg(
    worker_id: Uuid,
    worker_token: &WorkerToken,
    request_id: Uuid,
    perms: &Obj,
    request: Vec<moor_common::schema::var::Var>,
    timeout_ms: u64,
) -> rpc::DaemonToWorkerMessage {
    rpc::DaemonToWorkerMessage {
        message: rpc::DaemonToWorkerMessageUnion::WorkerRequest(Box::new(rpc::WorkerRequest {
            worker_id: uuid_fb(worker_id),
            token: mk_worker_token(worker_token),
            id: uuid_fb(request_id),
            perms: obj_fb(perms),
            request,
            timeout_ms,
        })),
    }
}

// ============================================================================
// Worker Reply Messages (Daemon -> Worker)
// ============================================================================

/// Build a WorkerAttached reply
#[inline]
pub fn mk_worker_attached_reply(
    worker_token: &WorkerToken,
    worker_id: Uuid,
) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerAttached(Box::new(rpc::WorkerAttached {
            token: mk_worker_token(worker_token),
            worker_id: uuid_fb(worker_id),
        })),
    }
}

/// Build a WorkerAck reply
#[inline]
pub fn mk_worker_ack_reply() -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerAck(Box::new(rpc::WorkerAck {})),
    }
}

/// Build a WorkerRejected reply
#[inline]
pub fn mk_worker_rejected_reply(reason: impl Into<String>) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerRejected(Box::new(rpc::WorkerRejected {
            reason: Some(reason.into()),
        })),
    }
}

/// Build a WorkerAuthFailed reply
#[inline]
pub fn mk_worker_auth_failed_reply(reason: impl Into<String>) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerAuthFailed(Box::new(rpc::WorkerAuthFailed {
            reason: reason.into(),
        })),
    }
}

/// Build a WorkerInvalidPayload reply
#[inline]
pub fn mk_worker_invalid_payload_reply(reason: impl Into<String>) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerInvalidPayload(Box::new(
            rpc::WorkerInvalidPayload {
                reason: reason.into(),
            },
        )),
    }
}

/// Build a WorkerUnknownRequest reply
#[inline]
pub fn mk_worker_unknown_request_reply(request_id: Uuid) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerUnknownRequest(Box::new(
            rpc::WorkerUnknownRequest {
                request_id: uuid_fb(request_id),
            },
        )),
    }
}

/// Build a WorkerNotRegistered reply
#[inline]
pub fn mk_worker_not_registered_reply(worker_id: Uuid) -> rpc::DaemonToWorkerReply {
    rpc::DaemonToWorkerReply {
        reply: rpc::DaemonToWorkerReplyUnion::WorkerNotRegistered(Box::new(
            rpc::WorkerNotRegistered {
                worker_id: uuid_fb(worker_id),
            },
        )),
    }
}

// ============================================================================
// Worker -> Daemon Messages
// ============================================================================

/// Build an AttachWorker message
#[inline]
pub fn mk_attach_worker_msg(
    worker_token: &WorkerToken,
    worker_type: &Symbol,
) -> rpc::WorkerToDaemonMessage {
    rpc::WorkerToDaemonMessage {
        message: rpc::WorkerToDaemonMessageUnion::AttachWorker(Box::new(rpc::AttachWorker {
            token: mk_worker_token(worker_token),
            worker_type: symbol_fb(worker_type),
        })),
    }
}

/// Build a WorkerPong message
#[inline]
pub fn mk_worker_pong_msg(
    worker_token: &WorkerToken,
    worker_type: &Symbol,
) -> rpc::WorkerToDaemonMessage {
    rpc::WorkerToDaemonMessage {
        message: rpc::WorkerToDaemonMessageUnion::WorkerPong(Box::new(rpc::WorkerPong {
            token: mk_worker_token(worker_token),
            worker_type: symbol_fb(worker_type),
        })),
    }
}

/// Build a DetachWorker message
#[inline]
pub fn mk_detach_worker_msg(worker_token: &WorkerToken) -> rpc::WorkerToDaemonMessage {
    rpc::WorkerToDaemonMessage {
        message: rpc::WorkerToDaemonMessageUnion::DetachWorker(Box::new(rpc::DetachWorker {
            token: mk_worker_token(worker_token),
        })),
    }
}

/// Build a RequestResult message
#[inline]
pub fn mk_request_result_msg(
    worker_token: &WorkerToken,
    request_id: Uuid,
    result: moor_common::schema::var::Var,
) -> rpc::WorkerToDaemonMessage {
    rpc::WorkerToDaemonMessage {
        message: rpc::WorkerToDaemonMessageUnion::RequestResult(Box::new(rpc::RequestResult {
            token: mk_worker_token(worker_token),
            id: uuid_fb(request_id),
            result: Box::new(result),
        })),
    }
}

/// Build a RequestError message
#[inline]
pub fn mk_request_error_msg(
    worker_token: &WorkerToken,
    request_id: Uuid,
    error: rpc::WorkerError,
) -> rpc::WorkerToDaemonMessage {
    rpc::WorkerToDaemonMessage {
        message: rpc::WorkerToDaemonMessageUnion::RequestError(Box::new(rpc::RequestError {
            token: mk_worker_token(worker_token),
            id: uuid_fb(request_id),
            error: Box::new(error),
        })),
    }
}
