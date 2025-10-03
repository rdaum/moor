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

//! Client message builders for HostClientToDaemon messages

use crate::{
    AuthToken, ClientToken,
    helpers::{auth_token_fb, client_token_fb, obj_fb, objectref_fb, symbol_fb, uuid_fb, var_fb},
};
use moor_common::model::ObjectRef;
use moor_schema::{rpc, var};
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

/// Build a LoginCommand message
#[inline]
pub fn mk_login_command_msg(
    client_token: &ClientToken,
    handler_object: &Obj,
    connect_args: Vec<String>,
    do_attach: bool,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::LoginCommand(Box::new(rpc::LoginCommand {
            client_token: client_token_fb(client_token),
            handler_object: obj_fb(handler_object),
            connect_args,
            do_attach,
        })),
    }
}

/// Build a Command message
#[inline]
pub fn mk_command_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    handler_object: &Obj,
    command: String,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Command(Box::new(rpc::Command {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            handler_object: obj_fb(handler_object),
            command,
        })),
    }
}

/// Build an OutOfBand message
#[inline]
pub fn mk_out_of_band_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    handler_object: &Obj,
    command: String,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::OutOfBand(Box::new(rpc::OutOfBand {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            handler_object: obj_fb(handler_object),
            command,
        })),
    }
}

/// Build a ClientPong message
#[inline]
pub fn mk_client_pong_msg(
    client_token: &ClientToken,
    client_sys_time: u64,
    player: &Obj,
    host_type: rpc::HostType,
    socket_addr: String,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::ClientPong(Box::new(rpc::ClientPong {
            client_token: client_token_fb(client_token),
            client_sys_time,
            player: obj_fb(player),
            host_type,
            socket_addr,
        })),
    }
}

/// Build a Detach message
#[inline]
pub fn mk_detach_msg(
    client_token: &ClientToken,
    disconnected: bool,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Detach(Box::new(rpc::Detach {
            client_token: client_token_fb(client_token),
            disconnected,
        })),
    }
}

/// Build a RequestedInput message
#[inline]
pub fn mk_requested_input_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    request_id: Uuid,
    input: &Var,
) -> Option<rpc::HostClientToDaemonMessage> {
    Some(rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::RequestedInput(Box::new(
            rpc::RequestedInput {
                client_token: client_token_fb(client_token),
                auth_token: auth_token_fb(auth_token),
                request_id: uuid_fb(request_id),
                input: var_fb(input)?,
            },
        )),
    })
}

/// Build a Program message
#[inline]
pub fn mk_program_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    object: &ObjectRef,
    verb: &Symbol,
    code: Vec<String>,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Program(Box::new(rpc::Program {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            object: objectref_fb(object),
            verb: symbol_fb(verb),
            code,
        })),
    }
}

/// Build a SetClientAttribute message
#[inline]
pub fn mk_set_client_attribute_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    key: &Symbol,
    value: Option<&Var>,
) -> Option<rpc::HostClientToDaemonMessage> {
    let value_fb = value.and_then(var_fb);

    Some(rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::SetClientAttribute(Box::new(
            rpc::SetClientAttribute {
                client_token: client_token_fb(client_token),
                auth_token: auth_token_fb(auth_token),
                key: symbol_fb(key),
                value: value_fb,
            },
        )),
    })
}

/// Build a RequestSysProp message
#[inline]
pub fn mk_request_sys_prop_msg(
    client_token: &ClientToken,
    object: &ObjectRef,
    property: &Symbol,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::RequestSysProp(Box::new(
            rpc::RequestSysProp {
                client_token: client_token_fb(client_token),
                object: objectref_fb(object),
                property: symbol_fb(property),
            },
        )),
    }
}

/// Build an Eval message
#[inline]
pub fn mk_eval_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    expression: String,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Eval(Box::new(rpc::Eval {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            expression,
        })),
    }
}

/// Build a ConnectionEstablish message
#[inline]
pub fn mk_connection_establish_msg(
    peer_addr: String,
    local_port: u16,
    remote_port: u16,
    acceptable_content_types: Option<Vec<rpc::Symbol>>,
    connection_attributes: Option<Vec<rpc::ConnectionAttribute>>,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::ConnectionEstablish(Box::new(
            rpc::ConnectionEstablish {
                peer_addr,
                local_port,
                remote_port,
                acceptable_content_types,
                connection_attributes,
            },
        )),
    }
}

/// Build a Verbs message
#[inline]
pub fn mk_verbs_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    object: &ObjectRef,
    inherited: bool,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Verbs(Box::new(rpc::Verbs {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            object: objectref_fb(object),
            inherited,
        })),
    }
}

/// Build a Properties message
#[inline]
pub fn mk_properties_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    object: &ObjectRef,
    inherited: bool,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Properties(Box::new(rpc::Properties {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            object: objectref_fb(object),
            inherited,
        })),
    }
}

/// Build a Retrieve message
#[inline]
pub fn mk_retrieve_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    object: &ObjectRef,
    entity_type: rpc::EntityType,
    name: &Symbol,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Retrieve(Box::new(rpc::Retrieve {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            object: objectref_fb(object),
            entity_type,
            name: symbol_fb(name),
        })),
    }
}

/// Build an Attach message
#[inline]
pub fn mk_attach_msg(
    auth_token: &AuthToken,
    connect_type: Option<rpc::ConnectType>,
    handler_object: &Obj,
    peer_addr: String,
    local_port: u16,
    remote_port: u16,
    acceptable_content_types: Option<Vec<rpc::Symbol>>,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Attach(Box::new(rpc::Attach {
            auth_token: auth_token_fb(auth_token),
            connect_type: connect_type.unwrap_or(rpc::ConnectType::NoConnect),
            handler_object: obj_fb(handler_object),
            peer_addr,
            local_port,
            remote_port,
            acceptable_content_types,
        })),
    }
}

/// Build a Resolve message
#[inline]
pub fn mk_resolve_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    objref: &ObjectRef,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::Resolve(Box::new(rpc::Resolve {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            objref: objectref_fb(objref),
        })),
    }
}

/// Build a RequestHistory message
#[inline]
pub fn mk_request_history_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    history_recall: Box<rpc::HistoryRecall>,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::RequestHistory(Box::new(
            rpc::RequestHistory {
                client_token: client_token_fb(client_token),
                auth_token: auth_token_fb(auth_token),
                history_recall,
            },
        )),
    }
}

/// Build a RequestCurrentPresentations message
#[inline]
pub fn mk_request_current_presentations_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::RequestCurrentPresentations(Box::new(
            rpc::RequestCurrentPresentations {
                client_token: client_token_fb(client_token),
                auth_token: auth_token_fb(auth_token),
            },
        )),
    }
}

/// Build a DismissPresentation message
#[inline]
pub fn mk_dismiss_presentation_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    presentation_id: String,
) -> rpc::HostClientToDaemonMessage {
    rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::DismissPresentation(Box::new(
            rpc::DismissPresentation {
                client_token: client_token_fb(client_token),
                auth_token: auth_token_fb(auth_token),
                presentation_id,
            },
        )),
    }
}

/// Build an InvokeVerb message
#[inline]
pub fn mk_invoke_verb_msg(
    client_token: &ClientToken,
    auth_token: &AuthToken,
    object: &ObjectRef,
    verb_name: &Symbol,
    args: Vec<&Var>,
) -> Option<rpc::HostClientToDaemonMessage> {
    let args_fb: Vec<var::Var> = args.iter().filter_map(|v| var_fb(v).map(|b| *b)).collect();

    if args_fb.len() != args.len() {
        return None;
    }

    Some(rpc::HostClientToDaemonMessage {
        message: rpc::HostClientToDaemonMessageUnion::InvokeVerb(Box::new(rpc::InvokeVerb {
            client_token: client_token_fb(client_token),
            auth_token: auth_token_fb(auth_token),
            object: objectref_fb(object),
            verb: symbol_fb(verb_name),
            args: args_fb,
        })),
    })
}
