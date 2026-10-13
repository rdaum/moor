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

//! Structured actions keep client-supplied context intact for the core to validate.

use moor_common::model::ObjectRef;
use moor_runtime_api::{
    AuthToken, ClientToken,
    api::{ClientRequest, InvocationMode},
};
use moor_var::{Obj, Symbol, json::json_to_var};

/// Binary WebSocket frame: this byte followed by a JSON object.
pub(crate) const ACTION_PREFIX: u8 = 0x04;
const MAX_ACTION_BYTES: usize = 16 * 1024;

pub(crate) fn is_action_frame(bytes: &[u8]) -> bool {
    bytes.starts_with(&[ACTION_PREFIX, b'{'])
}

/// The host authenticates the connection; the core owns all action semantics.
pub(crate) fn action_request(
    body: &[u8],
    handler: Obj,
    auth_token: AuthToken,
    client_token: ClientToken,
) -> Result<ClientRequest, &'static str> {
    if body.len() > MAX_ACTION_BYTES {
        return Err("Action is too large");
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| "Invalid action JSON")?;
    if !value.is_object() {
        return Err("Action must be a JSON object");
    }
    Ok(ClientRequest::InvokeVerb {
        auth_token,
        object: ObjectRef::Id(handler),
        verb: Symbol::mk("do_action"),
        args: vec![json_to_var(&value, false)],
        mode: InvocationMode::Connected { client_token },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{Variant, v_str};

    #[test]
    fn retains_opaque_context_and_connected_credentials() {
        let body = br#"{"command":"go east","exit":{"source":"oid:12","passage":"door"}}"#;
        let request = action_request(
            body,
            Obj::mk_id(7),
            AuthToken("actor".into()),
            ClientToken("connection".into()),
        )
        .unwrap();
        let ClientRequest::InvokeVerb {
            auth_token,
            object,
            verb,
            args,
            mode,
        } = request
        else {
            panic!("expected verb call")
        };
        assert_eq!(auth_token, AuthToken("actor".into()));
        assert_eq!(object, ObjectRef::Id(Obj::mk_id(7)));
        assert_eq!(verb, Symbol::mk("do_action"));
        assert_eq!(args.len(), 1);
        assert_eq!(
            args[0],
            json_to_var(&serde_json::from_slice(body).unwrap(), false)
        );
        let InvocationMode::Connected { client_token } = mode else {
            panic!("expected connected call")
        };
        assert_eq!(client_token, ClientToken("connection".into()));
        assert!(matches!(args[0].variant(), Variant::Map(_)));
        assert_ne!(args[0], v_str("go east"));
    }

    #[test]
    fn rejects_malformed_and_oversized_actions() {
        for body in [
            b"[]".as_slice(),
            b"null",
            b"{",
            &vec![b' '; MAX_ACTION_BYTES + 1],
        ] {
            assert!(
                action_request(
                    body,
                    Obj::mk_id(0),
                    AuthToken("actor".into()),
                    ClientToken("connection".into())
                )
                .is_err()
            );
        }
        assert!(is_action_frame(b"\x04{\"kind\":\"custom\"}"));
        assert!(!is_action_frame(b"\x04\x00\x00\x00"));
        assert!(!is_action_frame(b"\x03{\"type\":\"offer\"}"));
    }
}
