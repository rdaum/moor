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

use moor_var::Obj;
use rpc_common::HostType;
use std::{collections::HashMap, net::SocketAddr, time::SystemTime};
use tracing::warn;
use uuid::Uuid;

/// Manages the set of known hosts and the listeners they have registered.
struct HostRecord {
    last_seen: SystemTime,
    host_type: HostType,
    listeners: Vec<(Obj, SocketAddr)>,
}

#[derive(Default)]
pub struct Hosts(HashMap<Uuid, HostRecord>);

impl Hosts {
    pub(crate) fn receive_ping(
        &mut self,
        host_id: Uuid,
        host_type: HostType,
        listeners: Vec<(Obj, SocketAddr)>,
    ) -> bool {
        let now = SystemTime::now();
        self.0
            .insert(
                host_id,
                HostRecord {
                    last_seen: now,
                    host_type,
                    listeners,
                },
            )
            .is_none()
    }

    pub(crate) fn ping_check(&mut self, timeout: std::time::Duration) {
        let now = SystemTime::now();
        let mut expired = vec![];
        for (host_id, HostRecord { last_seen, .. }) in self.0.iter() {
            if now.duration_since(*last_seen).unwrap() > timeout {
                warn!(
                    "Host {} has not responded in time: {:?}, removing its listeners from the list",
                    host_id,
                    now.duration_since(*last_seen).unwrap()
                );
                expired.push(*host_id);
            }
        }
        for host_id in expired {
            self.unregister_host(&host_id);
        }
    }

    pub(crate) fn listeners(&self) -> Vec<(Obj, HostType, SocketAddr)> {
        self.0
            .values()
            .flat_map(
                |HostRecord {
                     host_type,
                     listeners,
                     ..
                 }| {
                    listeners
                        .iter()
                        .map(move |(oid, addr)| (*oid, *host_type, *addr))
                },
            )
            .collect()
    }

    pub(crate) fn unregister_host(&mut self, host_id: &Uuid) {
        self.0.remove(host_id);
    }
}
