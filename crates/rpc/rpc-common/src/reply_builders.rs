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

//! Helper functions for constructing RPC reply messages

use moor_schema::{convert::var_to_flatbuffer, rpc, var};
use moor_var::{Obj, Var};

use crate::{ClientToken, RpcMessageError, obj_fb};

// ============================================================================
// Host reply builders
// ============================================================================

/// Create a DaemonToHostAck reply
pub fn mk_daemon_to_host_ack() -> rpc::DaemonToHostReply {
    rpc::DaemonToHostReply {
        reply: rpc::DaemonToHostReplyUnion::DaemonToHostAck(Box::new(rpc::DaemonToHostAck {})),
    }
}

// ============================================================================
// Client reply builders
// ============================================================================

/// Create a NewConnection reply
pub fn mk_new_connection_reply(
    client_token: ClientToken,
    connection_obj: &Obj,
) -> rpc::DaemonToClientReply {
    rpc::DaemonToClientReply {
        reply: rpc::DaemonToClientReplyUnion::NewConnection(Box::new(rpc::NewConnection {
            client_token: Box::new(rpc::ClientToken {
                token: client_token.0,
            }),
            connection_obj: obj_fb(connection_obj),
        })),
    }
}

/// Create a Disconnected reply
pub fn mk_disconnected_reply() -> rpc::DaemonToClientReply {
    rpc::DaemonToClientReply {
        reply: rpc::DaemonToClientReplyUnion::Disconnected(Box::new(rpc::Disconnected {})),
    }
}

/// Create a ClientAttributeSet reply
pub fn mk_client_attribute_set_reply() -> rpc::DaemonToClientReply {
    rpc::DaemonToClientReply {
        reply: rpc::DaemonToClientReplyUnion::ClientAttributeSet(Box::new(
            rpc::ClientAttributeSet {},
        )),
    }
}

/// Create a PresentationDismissed reply
pub fn mk_presentation_dismissed_reply() -> rpc::DaemonToClientReply {
    rpc::DaemonToClientReply {
        reply: rpc::DaemonToClientReplyUnion::PresentationDismissed(Box::new(
            rpc::PresentationDismissed {},
        )),
    }
}

/// Create a ThanksPong reply
pub fn mk_thanks_pong_reply(timestamp: u64) -> rpc::DaemonToClientReply {
    rpc::DaemonToClientReply {
        reply: rpc::DaemonToClientReplyUnion::ThanksPong(Box::new(rpc::ThanksPong { timestamp })),
    }
}

// ============================================================================
// Conversion helpers with RpcMessageError
// ============================================================================

/// Convert Var to flatbuffer Var struct with RpcMessageError
pub fn var_to_flatbuffer_rpc(var: &Var) -> Result<var::Var, RpcMessageError> {
    var_to_flatbuffer(var)
        .map_err(|e| RpcMessageError::InternalError(format!("Failed to encode var: {e}")))
}
