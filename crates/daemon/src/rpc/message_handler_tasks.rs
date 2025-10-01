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

use crate::rpc::{
    message_handler::{
        RpcMessageHandler, USER_CONNECTED_SYM, USER_CREATED_SYM, USER_DISCONNECTED_SYM,
        USER_RECONNECTED_SYM,
    },
    session::RpcSession,
};
use eyre::{Context, Error};
use moor_common::{model::ObjectRef, schema::rpc as moor_rpc, util::parse_into_words};
use moor_kernel::{SchedulerClient, tasks::TaskResult};
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Var, v_obj};
use rpc_common::{RpcMessageError, var_to_flatbuffer_bytes};
use std::sync::Arc;
use tracing::{debug, error, warn};
use uuid::Uuid;

impl RpcMessageHandler {
    pub(crate) fn submit_connected_task(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        connection: &Obj,
        initiation_type: moor_rpc::ConnectType,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let connected_verb = match initiation_type {
            moor_rpc::ConnectType::Connected => *USER_CONNECTED_SYM,
            moor_rpc::ConnectType::Reconnected => *USER_RECONNECTED_SYM,
            moor_rpc::ConnectType::Created => *USER_CREATED_SYM,
            moor_rpc::ConnectType::NoConnect => {
                unreachable!("NoConnect should never call submit_connected_task")
            }
        };
        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(*handler_object),
                connected_verb,
                List::mk_list(&[v_obj(*player)]),
                "".to_string(),
                &SYSTEM_OBJECT,
                session,
            )
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    pub(crate) fn submit_disconnected_task(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        connection: &Obj,
    ) -> Result<(), Error> {
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        scheduler_client
            .submit_verb_task(
                player,
                &ObjectRef::Id(*handler_object),
                *USER_DISCONNECTED_SYM,
                List::mk_list(&[v_obj(*player)]),
                "".to_string(),
                &SYSTEM_OBJECT,
                session,
            )
            .with_context(|| "could not submit 'connected' task")?;
        Ok(())
    }

    pub(crate) fn submit_command_task(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        handler_object: &Obj,
        player: &Obj,
        command: String,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Get the connection object for activity tracking and session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        debug!(command, ?client_id, ?player, "Invoking submit_command_task");
        let parse_command_task_handle = match scheduler_client.submit_command_task(
            handler_object,
            player,
            command.as_str(),
            session,
        ) {
            Ok(t) => t,
            Err(e) => return Err(RpcMessageError::TaskError(e)),
        };

        let task_id = parse_command_task_handle.task_id();
        if let Err(e) = self
            .task_monitor
            .add_task(task_id, client_id, parse_command_task_handle)
        {
            error!(error = ?e, "Error adding task to monitor");
        }
        Ok(moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::TaskSubmitted(Box::new(
                moor_rpc::TaskSubmitted {
                    task_id: task_id as u64,
                },
            )),
        })
    }

    pub(crate) fn respond_input(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        input_request_id: Uuid,
        input: Var,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Get the connection object for activity tracking
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        if let Err(e) = self
            .connections
            .record_client_activity(client_id, connection)
        {
            warn!("Unable to update client connection activity: {}", e);
        };

        // Pass this back over to the scheduler to handle using the player object.
        if let Err(e) = scheduler_client.submit_requested_input(player, input_request_id, input) {
            error!(error = ?e, "Error submitting requested input");
            return Err(RpcMessageError::InternalError(e.to_string()));
        }

        // TODO: do we need a new response for this? Maybe just a "Thanks"?
        Ok(moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::InputThanks(Box::new(
                moor_rpc::InputThanks {},
            )),
        })
    }

    pub(crate) fn submit_out_of_bound_task(
        &self,
        scheduler_client: SchedulerClient,
        handler_object: &Obj,
        client_id: Uuid,
        player: &Obj,
        command: String,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let command_components = parse_into_words(command.as_str());
        let task_handle = match scheduler_client.submit_out_of_band_task(
            handler_object,
            player,
            command_components,
            command,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting command task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };

        // Just return immediately with success, we do not wait for the task to complete, we'll
        // let the session run to completion on its own and output back to the client.
        // Maybe we should be returning a value from this for the future, but the way clients are
        // written right now, there's little point.
        Ok(moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::TaskSubmitted(Box::new(
                moor_rpc::TaskSubmitted {
                    task_id: task_handle.task_id() as u64,
                },
            )),
        })
    }

    pub(crate) fn submit_eval_task(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        expression: String,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let mut task_handle = match scheduler_client.submit_eval_task(
            player,
            player,
            expression,
            session,
            self.config.features.clone(),
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting eval task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };
        loop {
            match task_handle.into_receiver().recv() {
                Ok((_, Ok(TaskResult::Replaced(th)))) => {
                    task_handle = th;
                    continue;
                }
                Ok((_, Ok(TaskResult::Result(v)))) => {
                    let result_bytes = var_to_flatbuffer_bytes(&v).map_err(|e| {
                        RpcMessageError::InternalError(format!("Failed to encode result: {}", e))
                    })?;
                    break Ok(moor_rpc::DaemonToClientReply {
                        reply: moor_rpc::DaemonToClientReplyUnion::EvalResult(Box::new(
                            moor_rpc::EvalResult {
                                result: Box::new(moor_rpc::VarBytes { data: result_bytes }),
                            },
                        )),
                    });
                }
                Ok((_, Err(e))) => break Err(RpcMessageError::TaskError(e)),
                Err(e) => {
                    error!(error = ?e, "Error processing eval");

                    break Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        }
    }

    pub(crate) fn submit_invoke_verb_task(
        &self,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        player: &Obj,
        object: &ObjectRef,
        verb: Symbol,
        args: Vec<Var>,
    ) -> Result<moor_rpc::DaemonToClientReply, RpcMessageError> {
        // Get the connection object for session management
        let connection = self
            .connections
            .connection_object_for_client(client_id)
            .ok_or(RpcMessageError::InternalError(
                "Connection not found".to_string(),
            ))?;

        let session = Arc::new(RpcSession::new(
            client_id,
            connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));

        let task_handle = match scheduler_client.submit_verb_task(
            player,
            object,
            verb,
            List::mk_list(&args),
            "".to_string(),
            &SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting verb task");
                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };

        let task_id = task_handle.task_id();
        if let Err(e) = self.task_monitor.add_task(task_id, client_id, task_handle) {
            error!(error = ?e, "Error adding task to monitor");
            return Err(RpcMessageError::InternalError(e.to_string()));
        }
        Ok(moor_rpc::DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::TaskSubmitted(Box::new(
                moor_rpc::TaskSubmitted {
                    task_id: task_id as u64,
                },
            )),
        })
    }
}
