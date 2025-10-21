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

use ed25519_dalek::{
    SigningKey, VerifyingKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
use moor_schema::rpc;
use rusty_paseto::core::Key;
use std::path::Path;
use thiserror::Error;

pub const MOOR_SESSION_TOKEN_FOOTER: &str = "key-id:moor_client";
pub const MOOR_AUTH_TOKEN_FOOTER: &str = "key-id:moor_player";

/// PASETO public token for a connection, used for the validation of RPC requests after the initial
/// connection is established.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ClientToken(pub String);

/// PASTEO public token for an authenticated player, encoding the player's identity.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AuthToken(pub String);

#[derive(Error, Debug)]
pub enum KeyError {
    #[error("Could not parse PEM-encoded key")]
    KeyParseError,
    #[error("Incorrect key format for key: {0}")]
    IncorrectKeyFormat(String),
    #[error("Could not read key from file: {0}")]
    ReadError(std::io::Error),
}

/// Parse a public and private key from the given PEM strings.
pub fn parse_keypair(public_key: &str, private_key: &str) -> Result<(Key<64>, Key<32>), KeyError> {
    let private_key =
        SigningKey::from_pkcs8_pem(private_key).map_err(|_| KeyError::KeyParseError)?;
    let public_key =
        VerifyingKey::from_public_key_pem(public_key).map_err(|_| KeyError::KeyParseError)?;

    let priv_key: Key<64> = Key::from(private_key.to_keypair_bytes());
    let pub_key: Key<32> = Key::from(public_key.to_bytes());
    Ok((priv_key, pub_key))
}

/// Load a keypair from the given public and private key (PEM) files.
pub fn load_keypair(public_key: &Path, private_key: &Path) -> Result<(Key<64>, Key<32>), KeyError> {
    let (Some(pubkey_pem), Some(privkey_pem)) = (
        std::fs::read_to_string(public_key).ok(),
        std::fs::read_to_string(private_key).ok(),
    ) else {
        return Err(KeyError::ReadError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not read key from file",
        )));
    };

    parse_keypair(&pubkey_pem, &privkey_pem)
}

/// Convert from FlatBuffer ClientTokenRef to ClientToken
pub fn client_token_from_ref(
    token_ref: rpc::ClientTokenRef<'_>,
) -> Result<crate::ClientToken, String> {
    let token_string = token_ref
        .token()
        .map_err(|_| "Missing client token string")?;
    Ok(crate::ClientToken(token_string.to_string()))
}

/// Convert from FlatBuffer AuthTokenRef to AuthToken
pub fn auth_token_from_ref(token_ref: rpc::AuthTokenRef<'_>) -> Result<crate::AuthToken, String> {
    let token_string = token_ref.token().map_err(|_| "Missing auth token string")?;
    Ok(crate::AuthToken(token_string.to_string()))
}
