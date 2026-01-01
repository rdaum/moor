// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Host message builders for HostToDaemon messages

use crate::helpers::uuid_fb;
use moor_schema::rpc;
use uuid::Uuid;

/// Build a RequestPerformanceCounters message
#[inline]
pub fn mk_request_performance_counters_msg() -> rpc::HostToDaemonMessage {
    rpc::HostToDaemonMessage {
        message: rpc::HostToDaemonMessageUnion::RequestPerformanceCounters(Box::new(
            rpc::RequestPerformanceCounters {},
        )),
    }
}

/// Build a GetServerFeatures message
#[inline]
pub fn mk_get_server_features_msg() -> rpc::HostToDaemonMessage {
    rpc::HostToDaemonMessage {
        message: rpc::HostToDaemonMessageUnion::GetServerFeatures(Box::new(
            rpc::GetServerFeatures {},
        )),
    }
}

/// Build a RegisterHost message
#[inline]
pub fn mk_register_host_msg(
    host_id: Uuid,
    timestamp: u64,
    host_type: rpc::HostType,
    listeners: Vec<rpc::Listener>,
) -> rpc::HostToDaemonMessage {
    rpc::HostToDaemonMessage {
        message: rpc::HostToDaemonMessageUnion::RegisterHost(Box::new(rpc::RegisterHost {
            host_id: uuid_fb(host_id),
            timestamp,
            host_type,
            listeners,
        })),
    }
}

/// Build a DetachHost message
#[inline]
pub fn mk_detach_host_msg(host_id: Uuid) -> rpc::HostToDaemonMessage {
    rpc::HostToDaemonMessage {
        message: rpc::HostToDaemonMessageUnion::DetachHost(Box::new(rpc::DetachHost {
            host_id: uuid_fb(host_id),
        })),
    }
}

/// Build a HostPong message
#[inline]
pub fn mk_host_pong_msg(
    host_id: Uuid,
    timestamp: u64,
    host_type: rpc::HostType,
    listeners: Vec<rpc::Listener>,
) -> rpc::HostToDaemonMessage {
    rpc::HostToDaemonMessage {
        message: rpc::HostToDaemonMessageUnion::HostPong(Box::new(rpc::HostPong {
            host_id: uuid_fb(host_id),
            timestamp,
            host_type,
            listeners,
        })),
    }
}
