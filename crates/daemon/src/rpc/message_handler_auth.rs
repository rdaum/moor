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

use crate::rpc::{message_handler::RpcMessageHandler, session::RpcSession};
use moor_common::model::ObjectRef;
use moor_kernel::{SchedulerClient, tasks::TaskNotification};
use moor_schema::{
    convert::{symbol_from_ref, var_from_ref},
    rpc as moor_rpc,
    rpc::DaemonToClientReply,
};
use moor_var::{Obj, SYSTEM_OBJECT, Symbol, Var, Variant, v_str};
use rpc_common::{
    AuthToken, ClientToken, MOOR_AUTH_TOKEN_FOOTER, MOOR_SESSION_TOKEN_FOOTER, RpcErr,
    RpcMessageError, auth_token_from_ref, client_token_from_ref, obj_fb,
};
use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

impl RpcMessageHandler {
    pub(crate) fn validate_auth_token(
        &self,
        token: AuthToken,
        objid: Option<&Obj>,
    ) -> Result<Obj, RpcMessageError> {
        {
            let guard = self.auth_token_cache.pin();
            if let Some((t, o)) = guard.get(&token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(*o);
            }
        }
        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_AUTH_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcMessageError::PermissionDenied
            })
            .unwrap();

        let Some(token_player) = verified_token.get("player") else {
            debug!("Token does not contain player");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Some(token_player) = token_player.as_str() else {
            debug!("Token player is not valid (expected string, found: {token_player:?})");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Ok(token_player) = Obj::try_from(token_player) else {
            debug!("Token player is not valid");
            return Err(RpcMessageError::PermissionDenied);
        };
        if !token_player.is_positive() {
            debug!("Token player is not a valid objid");
            return Err(RpcMessageError::PermissionDenied);
        }
        if let Some(objid) = objid {
            // Does the 'player' match objid? If not, reject it.
            if objid.ne(&token_player) {
                debug!(?objid, ?token_player, "Token player does not match objid");
                return Err(RpcMessageError::PermissionDenied);
            }
        }

        // TODO: we will need to verify that the player object id inside the token is valid inside
        //   moor itself. And really only something with a WorldState can do that. So it's not
        //   enough to have validated the auth token here, we will need to pepper the scheduler/task
        //   code with checks to make sure that the player objid is valid before letting it go
        //   forwards.

        let guard = self.auth_token_cache.pin();
        guard.insert(token.clone(), (Instant::now(), token_player));
        Ok(token_player)
    }

    pub(crate) fn make_client_token(&self, client_id: Uuid) -> ClientToken {
        let privkey: PasetoAsymmetricPrivateKey<V4, Public> =
            PasetoAsymmetricPrivateKey::from(self.private_key.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_SESSION_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "client_id": client_id.to_string(),
                    "iss": "moor",
                    "aud": "moor_connection",
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");

        ClientToken(token)
    }

    pub(crate) fn make_auth_token(&self, oid: &Obj) -> AuthToken {
        let privkey = PasetoAsymmetricPrivateKey::from(self.private_key.as_ref());
        let token = Paseto::<V4, Public>::default()
            .set_footer(Footer::from(MOOR_AUTH_TOKEN_FOOTER))
            .set_payload(Payload::from(
                json!({
                    "player": oid.to_string(),
                })
                .to_string()
                .as_str(),
            ))
            .try_sign(&privkey)
            .expect("Unable to build Paseto token");
        AuthToken(token)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn perform_login(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        args: Vec<String>,
        attach: bool,
        event_log_pubkey: Option<String>,
    ) -> Result<DaemonToClientReply, RpcMessageError> {
        // TODO: change result of login to return this information, rather than just Objid, so
        //   we're not dependent on this.
        let connect_type = if args.first() == Some(&"create".to_string()) {
            moor_rpc::ConnectType::Created
        } else {
            moor_rpc::ConnectType::Connected
        };

        info!(
            "Performing {:?} login for client: {}",
            connect_type, client_id
        );
        let session = Arc::new(RpcSession::new(
            client_id,
            *connection,
            self.event_log.clone(),
            self.mailbox_sender.clone(),
        ));
        let task_handle = match scheduler_client.submit_verb_task(
            connection,
            &ObjectRef::Id(*handler_object),
            *crate::rpc::message_handler::DO_LOGIN_COMMAND,
            args.iter().map(|s| v_str(s)).collect(),
            args.join(" "),
            &SYSTEM_OBJECT,
            session,
        ) {
            Ok(t) => t,
            Err(e) => {
                error!(error = ?e, "Error submitting login task");

                return Err(RpcMessageError::InternalError(e.to_string()));
            }
        };
        let receiver = task_handle.into_receiver();
        let player = loop {
            match receiver.recv() {
                Ok((_, Ok(TaskNotification::Result(v)))) => {
                    // If v is an objid, we have a successful login and we need to rewrite this
                    // client id to use the player objid and then return a result to the client.
                    // with its new player objid and login result.
                    // If it's not an objid, that's considered an auth failure.
                    match v.variant() {
                        Variant::Obj(o) => break o,
                        _ => {
                            return Ok(DaemonToClientReply {
                                reply: moor_rpc::DaemonToClientReplyUnion::LoginResult(Box::new(
                                    moor_rpc::LoginResult {
                                        success: false,
                                        auth_token: None,
                                        connect_type: moor_rpc::ConnectType::Connected,
                                        player: None,
                                        player_flags: 0,
                                    },
                                )),
                            });
                        }
                    }
                }
                Ok((_, Ok(TaskNotification::Suspended))) => continue,
                Ok((_, Err(e))) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::LoginTaskFailed(e.to_string()));
                }
                Err(e) => {
                    error!(error = ?e, "Error waiting for login results");

                    return Err(RpcMessageError::InternalError(e.to_string()));
                }
            }
        };

        let Ok(_) = self
            .connections
            .associate_player_object(*connection, player)
        else {
            return Err(RpcMessageError::InternalError(
                "Unable to update client connection".to_string(),
            ));
        };

        // Set event log pubkey BEFORE triggering connected events, ensuring encryption
        // is active from the very start of the session
        if let Some(pubkey) = event_log_pubkey {
            debug!(player = ?player, "Setting event log pubkey during login");
            self.event_log.set_pubkey(player, pubkey);
        }

        if attach
            && let Err(e) = self.submit_connected_task(
                handler_object,
                scheduler_client.clone(),
                client_id,
                &player,
                connection,
                connect_type,
            )
        {
            error!(error = ?e, "Error submitting user_connected task");

            // Note we still continue to return a successful login result here, hoping for the best
            // but we do log the error.
        }

        let auth_token = self.make_auth_token(&player);

        // Get player flags for client-side permission checks
        let player_flags = scheduler_client.get_object_flags(&player).unwrap_or(0);

        Ok(DaemonToClientReply {
            reply: moor_rpc::DaemonToClientReplyUnion::LoginResult(Box::new(
                moor_rpc::LoginResult {
                    success: true,
                    auth_token: Some(Box::new(moor_rpc::AuthToken {
                        token: auth_token.0.clone(),
                    })),
                    connect_type,
                    player: Some(obj_fb(&player)),
                    player_flags,
                },
            )),
        })
    }

    pub(crate) fn validate_client_token_impl(
        &self,
        token: ClientToken,
        client_id: Uuid,
    ) -> Result<(), RpcMessageError> {
        {
            let guard = self.client_token_cache.pin();
            if let Some(t) = guard.get(&token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(());
            }
        }

        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let verified_token = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_SESSION_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let verified_token = serde_json::from_str::<serde_json::Value>(verified_token.as_str())
            .map_err(|e| {
                warn!(error = ?e, "Unable to parse/validate token");
                RpcMessageError::PermissionDenied
            })?;

        // Does the token match the client it came from? If not, reject it.
        let Some(token_client_id) = verified_token.get("client_id") else {
            debug!("Token does not contain client_id");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Some(token_client_id) = token_client_id.as_str() else {
            debug!("Token client_id is null");
            return Err(RpcMessageError::PermissionDenied);
        };
        let Ok(token_client_id) = Uuid::parse_str(token_client_id) else {
            debug!("Token client_id is not a valid UUID");
            return Err(RpcMessageError::PermissionDenied);
        };
        if client_id != token_client_id {
            debug!(
                ?client_id,
                ?token_client_id,
                "Token client_id does not match client_id"
            );
            return Err(RpcMessageError::PermissionDenied);
        }

        let guard = self.client_token_cache.pin();
        guard.insert(token.clone(), Instant::now());

        Ok(())
    }

    /// Extract and validate client token from a FlatBuffer message.
    /// Returns the connection object for this client.
    pub(crate) fn extract_client_token<T>(
        &self,
        msg: &T,
        client_id: Uuid,
        get_token: impl FnOnce(&T) -> Result<moor_rpc::ClientTokenRef, planus::Error>,
    ) -> Result<Obj, RpcMessageError> {
        let token = get_token(msg)
            .rpc_err()
            .and_then(|r| client_token_from_ref(r).rpc_err())?;
        self.client_auth(token, client_id)
    }

    /// Extract and validate both client token and auth token.
    /// Verifies the player from auth token matches the logged-in player.
    /// Returns (connection_obj, player).
    pub(crate) fn extract_and_verify_tokens<T>(
        &self,
        msg: &T,
        client_id: Uuid,
        get_client_token: impl FnOnce(&T) -> Result<moor_rpc::ClientTokenRef, planus::Error>,
        get_auth_token: impl FnOnce(&T) -> Result<moor_rpc::AuthTokenRef, planus::Error>,
    ) -> Result<(Obj, Obj), RpcMessageError> {
        // Extract and validate client token
        let client_token = get_client_token(msg)
            .rpc_err()
            .and_then(|r| client_token_from_ref(r).rpc_err())?;
        let connection = self.client_auth(client_token, client_id)?;

        // Extract and validate auth token
        let auth_token = get_auth_token(msg)
            .rpc_err()
            .and_then(|r| auth_token_from_ref(r).rpc_err())?;
        let player = self.validate_auth_token(auth_token, None)?;

        // Verify player matches logged-in player
        let Some(logged_in_player) = self.connections.player_object_for_client(client_id) else {
            return Err(RpcMessageError::PermissionDenied);
        };
        if player != logged_in_player {
            return Err(RpcMessageError::PermissionDenied);
        }

        Ok((connection, player))
    }

    /// Extract and validate auth token from a FlatBuffer message.
    /// Returns the player object.
    pub(crate) fn extract_auth_token<T>(
        &self,
        msg: &T,
        get_token: impl FnOnce(&T) -> Result<moor_rpc::AuthTokenRef, planus::Error>,
    ) -> Result<Obj, RpcMessageError> {
        let auth_token = get_token(msg)
            .rpc_err()
            .and_then(|r| auth_token_from_ref(r).rpc_err())?;
        self.validate_auth_token(auth_token, None)
    }

    pub fn client_auth(&self, token: ClientToken, client_id: Uuid) -> Result<Obj, RpcMessageError> {
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(RpcMessageError::NoConnection);
        };

        self.validate_client_token_impl(token, client_id)?;
        Ok(connection)
    }

    /// Extract connection parameters (hostname, ports) from a FlatBuffer message
    pub(crate) fn extract_connection_params<T>(
        &self,
        msg: &T,
        get_peer_addr: impl FnOnce(&T) -> Result<&str, planus::Error>,
        get_local_port: impl FnOnce(&T) -> Result<u16, planus::Error>,
        get_remote_port: impl FnOnce(&T) -> Result<u16, planus::Error>,
    ) -> Result<(String, u16, u16), RpcMessageError> {
        let hostname = get_peer_addr(msg).rpc_err()?.to_string();
        let local_port = get_local_port(msg).rpc_err()?;
        let remote_port = get_remote_port(msg).rpc_err()?;
        Ok((hostname, local_port, remote_port))
    }

    /// Extract optional acceptable_content_types from a FlatBuffer message
    pub(crate) fn extract_acceptable_content_types<T>(
        &self,
        msg: &T,
        get_types: impl FnOnce(
            &T,
        ) -> Result<
            Option<planus::Vector<'_, Result<moor_rpc::SymbolRef<'_>, planus::Error>>>,
            planus::Error,
        >,
    ) -> Option<Vec<Symbol>> {
        get_types(msg).ok()?.map(|types| {
            types
                .iter()
                .filter_map(|s| s.ok().and_then(|s| symbol_from_ref(s).ok()))
                .collect()
        })
    }

    /// Extract optional connection_attributes from a FlatBuffer message
    pub(crate) fn extract_connection_attributes<T>(
        &self,
        msg: &T,
        get_attrs: impl FnOnce(
            &T,
        ) -> Result<
            Option<planus::Vector<'_, Result<moor_rpc::ConnectionAttributeRef<'_>, planus::Error>>>,
            planus::Error,
        >,
    ) -> Option<HashMap<Symbol, Var>> {
        get_attrs(msg).ok()?.map(|attrs| {
            attrs
                .iter()
                .filter_map(|attr| {
                    let attr = attr.ok()?;
                    let key = symbol_from_ref(attr.key().ok()?).ok()?;
                    let value = var_from_ref(attr.value().ok()?).ok()?;
                    Some((key, value))
                })
                .collect()
        })
    }
}
