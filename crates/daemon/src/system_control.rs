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

//! SystemControl handle for the scheduler - minimal interface for system operations

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use moor_common::tasks::SystemControl;
use moor_var::{E_INVARG, E_QUOTA, Obj};
use rpc_common::HostType;
use tracing::warn;

use crate::{enrollment, rpc::MessageHandler};

/// Handle for system control operations - just what the scheduler needs
#[derive(Clone)]
pub struct SystemControlHandle {
    kill_switch: Arc<AtomicBool>,
    message_handler: Arc<dyn MessageHandler>,
    enrollment_token_path: Option<PathBuf>,
}

impl SystemControlHandle {
    pub fn new(
        kill_switch: Arc<AtomicBool>,
        message_handler: Arc<dyn MessageHandler>,
        enrollment_token_path: Option<PathBuf>,
    ) -> Self {
        Self {
            kill_switch,
            message_handler,
            enrollment_token_path,
        }
    }
}

impl SystemControl for SystemControlHandle {
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

        self.message_handler
            .broadcast_listen(handler_object, host_type, port, print_messages)
            .map_err(|_| moor_var::E_INVARG.msg("Unable to send Listen event"))
    }

    fn unlisten(&self, port: u16, host_type: &str) -> Result<(), moor_var::Error> {
        let host_type = match host_type {
            "tcp" => HostType::TCP,
            _ => return Err(moor_var::E_INVARG.msg("Invalid host type")),
        };

        self.message_handler
            .broadcast_unlisten(host_type, port)
            .map_err(|_| moor_var::E_INVARG.msg("Unable to send Unlisten event"))
    }

    fn listeners(&self) -> Result<Vec<(Obj, String, u16, bool)>, moor_var::Error> {
        let listeners = self
            .message_handler
            .get_listeners()
            .iter()
            .map(|(o, t, port)| (*o, t.id_str().to_string(), *port, true))
            .collect();
        Ok(listeners)
    }

    fn switch_player(&self, connection_obj: Obj, new_player: Obj) -> Result<(), moor_var::Error> {
        self.message_handler
            .switch_player(connection_obj, new_player)
            .map_err(|e| moor_var::E_QUOTA.with_msg(|| e.to_string()))
    }

    fn rotate_enrollment_token(&self) -> Result<String, moor_var::Error> {
        let path = self.enrollment_token_path.as_ref().ok_or_else(|| {
            E_INVARG.msg("Enrollment token rotation is not configured for this deployment")
        })?;

        enrollment::rotate_enrollment_token(path.as_path())
            .map_err(|e| E_QUOTA.with_msg(|| format!("Failed to rotate enrollment token: {e}")))
    }
}
