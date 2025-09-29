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
use moor_kernel::{SchedulerClient, tasks::TaskResult};
use moor_var::{Obj, SYSTEM_OBJECT, Variant, v_str};
use rpc_common::{
    AuthToken, ClientToken, HostToken, HostType, MOOR_AUTH_TOKEN_FOOTER, MOOR_HOST_TOKEN_FOOTER,
    MOOR_SESSION_TOKEN_FOOTER, RpcMessageError, auth_token_from_ref, client_token_from_ref,
    flatbuffers_generated::{
        moor_rpc,
        moor_rpc::{DaemonToClientReply, DaemonToClientReplyUnion, LoginResult},
    },
    obj_to_flatbuffer_struct,
};
use rusty_paseto::core::{
    Footer, Paseto, PasetoAsymmetricPrivateKey, PasetoAsymmetricPublicKey, Payload, Public, V4,
};
use serde_json::json;
use std::{sync::Arc, time::Instant};
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

    pub(crate) fn perform_login(
        &self,
        handler_object: &Obj,
        scheduler_client: SchedulerClient,
        client_id: Uuid,
        connection: &Obj,
        args: Vec<String>,
        attach: bool,
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
        let mut task_handle = match scheduler_client.submit_verb_task(
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
        let player = loop {
            let receiver = task_handle.into_receiver();
            match receiver.recv() {
                Ok((_, Ok(TaskResult::Replaced(th)))) => {
                    task_handle = th;
                    continue;
                }
                Ok((_, Ok(TaskResult::Result(v)))) => {
                    // If v is an objid, we have a successful login and we need to rewrite this
                    // client id to use the player objid and then return a result to the client.
                    // with its new player objid and login result.
                    // If it's not an objid, that's considered an auth failure.
                    match v.variant() {
                        Variant::Obj(o) => break *o,
                        _ => {
                            return Ok(DaemonToClientReply {
                                reply: DaemonToClientReplyUnion::LoginResult(Box::new(
                                    LoginResult {
                                        success: false,
                                        auth_token: None,
                                        connect_type: moor_rpc::ConnectType::Connected,
                                        player: None,
                                    },
                                )),
                            });
                        }
                    }
                }
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

        if attach
            && let Err(e) = self.submit_connected_task(
                handler_object,
                scheduler_client,
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

        Ok(DaemonToClientReply {
            reply: DaemonToClientReplyUnion::LoginResult(Box::new(LoginResult {
                success: true,
                auth_token: Some(Box::new(moor_rpc::AuthToken {
                    token: auth_token.0.clone(),
                })),
                connect_type,
                player: Some(Box::new(obj_to_flatbuffer_struct(&player))),
            })),
        })
    }

    pub(crate) fn validate_host_token_impl(
        &self,
        token: &HostToken,
    ) -> Result<HostType, RpcMessageError> {
        // Check cache first.
        {
            let guard = self.host_token_cache.pin();
            if let Some((t, host_type)) = guard.get(token)
                && t.elapsed().as_secs() <= 60
            {
                return Ok(*host_type);
            }
        }
        let pk: PasetoAsymmetricPublicKey<V4, Public> =
            PasetoAsymmetricPublicKey::from(&self.public_key);
        let host_type = Paseto::<V4, Public>::try_verify(
            token.0.as_str(),
            &pk,
            Footer::from(MOOR_HOST_TOKEN_FOOTER),
            None,
        )
        .map_err(|e| {
            warn!(error = ?e, "Unable to parse/validate token");
            RpcMessageError::PermissionDenied
        })?;

        let Some(host_type) = HostType::parse_id_str(host_type.as_str()) else {
            warn!("Unable to parse/validate host type in token");
            return Err(RpcMessageError::PermissionDenied);
        };

        // Cache the result.
        let guard = self.host_token_cache.pin();
        guard.insert(token.clone(), (Instant::now(), host_type));

        Ok(host_type)
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
        let token_ref = get_token(msg)
            .map_err(|_| RpcMessageError::InvalidRequest("Missing client_token".to_string()))?;
        let token = client_token_from_ref(token_ref)
            .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
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
        let token_ref = get_client_token(msg)
            .map_err(|_| RpcMessageError::InvalidRequest("Missing client_token".to_string()))?;
        let _token = client_token_from_ref(token_ref)
            .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
        let connection = self.client_auth(_token, client_id)?;

        // Extract and validate auth token
        let auth_token_ref = get_auth_token(msg)
            .map_err(|_| RpcMessageError::InvalidRequest("Missing auth_token".to_string()))?;
        let auth_token = auth_token_from_ref(auth_token_ref)
            .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
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
        let auth_token_ref = get_token(msg)
            .map_err(|_| RpcMessageError::InvalidRequest("Missing auth_token".to_string()))?;
        let auth_token = auth_token_from_ref(auth_token_ref)
            .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
        self.validate_auth_token(auth_token, None)
    }

    pub fn client_auth(&self, token: ClientToken, client_id: Uuid) -> Result<Obj, RpcMessageError> {
        let Some(connection) = self.connections.connection_object_for_client(client_id) else {
            return Err(RpcMessageError::NoConnection);
        };

        self.validate_client_token_impl(token, client_id)?;
        Ok(connection)
    }
}
