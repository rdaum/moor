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

use crate::rpc_server::RpcServer;
use moor_common::tasks::SessionError::DeliveryError;
use moor_common::tasks::SystemControl;
use moor_var::Obj;
use rpc_common::{HOST_BROADCAST_TOPIC, HostBroadcastEvent, HostType};
use std::sync::atomic::Ordering;
use tracing::{error, warn};

impl SystemControl for RpcServer {
    fn shutdown(&self, msg: Option<String>) -> Result<(), moor_var::Error> {
        warn!("Shutting down server: {}", msg.unwrap_or_default());
        self.kill_switch.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn listen(
        &self,
        handler_object: Obj,
        host_type: &str,
        port: u16,
        print_messages: bool,
    ) -> Result<(), moor_var::Error> {
        let host_type = match host_type {
            "tcp" => HostType::TCP,
            _ => {
                return Err(
                    moor_var::E_INVARG.with_msg(|| format!("Unhandled host type: {host_type}"))
                );
            }
        };

        let event = HostBroadcastEvent::Listen {
            handler_object,
            host_type,
            port,
            print_messages,
        };

        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish
                .send_multipart(payload, 0)
                .map_err(|e| {
                    error!(error = ?e, "Unable to send Listen to client");
                    DeliveryError
                })
                .map_err(|e| {
                    error!("Could not send Listen event: {}", e);
                    moor_var::E_INVARG.msg("Unable to send Listen event")
                })?;
        }

        Ok(())
    }

    fn unlisten(&self, port: u16, host_type: &str) -> Result<(), moor_var::Error> {
        let host_type = match host_type {
            "tcp" => HostType::TCP,
            _ => return Err(moor_var::E_INVARG.msg("Invalid host type")),
        };

        let event = HostBroadcastEvent::Unlisten { host_type, port };

        let event_bytes = bincode::encode_to_vec(event, bincode::config::standard()).unwrap();

        // We want responses from all clients, so send on this broadcast "topic"
        let payload = vec![HOST_BROADCAST_TOPIC.to_vec(), event_bytes];
        {
            let publish = self.events_publish.lock().unwrap();
            publish
                .send_multipart(payload, 0)
                .map_err(|e| {
                    error!(error = ?e, "Unable to send Unlisten to client");
                    DeliveryError
                })
                .map_err(|e| {
                    error!("Could not send Unlisten event: {}", e);
                    moor_var::E_INVARG.msg("Unable to send Unlisten event")
                })?;
        }
        Ok(())
    }

    fn listeners(&self) -> Result<Vec<(Obj, String, u16, bool)>, moor_var::Error> {
        let hosts = self.hosts.lock().unwrap();
        let listeners = hosts
            .listeners()
            .iter()
            .map(|(o, t, h)| (o.clone(), t.id_str().to_string(), h.port(), true))
            .collect();
        Ok(listeners)
    }
}
