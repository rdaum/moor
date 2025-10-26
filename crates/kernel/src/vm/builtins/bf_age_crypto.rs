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

//! Builtin functions for Age encryption (modern file encryption).

use std::{io::Write, str::FromStr};

use age::{
    Decryptor, Encryptor, Recipient as AgeRecipient,
    secrecy::ExposeSecret,
    x25519::{Identity, Recipient},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use ssh_key::public::PublicKey;
use std::io::Read;
use tracing::{error, warn};

use crate::vm::builtins::{
    BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction, world_state_bf_err,
};
use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_TYPE, Sequence, Variant, v_binary, v_list, v_string};

/// MOO: `list age_generate_keypair([bool as_bytes])`
/// Generates a new X25519 keypair for age encryption. Programmer-only function.
/// If as_bytes is true, returns {public_key, private_key} as bytes. Otherwise returns Bech32-encoded strings.
fn bf_age_generate_keypair(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("age_generate_keypair() takes at most one argument"),
        ));
    }

    // Check for programmer permissions
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;

    // Get the optional as_bytes argument (defaults to false)
    let as_bytes = if bf_args.args.is_empty() {
        false
    } else {
        // Validate type before using is_true()
        match bf_args.args[0].variant() {
            Variant::Int(_) | Variant::Bool(_) => bf_args.args[0].is_true(),
            _ => {
                return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                    format!(
                        "age_generate_keypair() argument must be a boolean or integer, was {}",
                        bf_args.args[0].type_code().to_literal()
                    )
                })));
            }
        }
    };

    // Generate a new X25519 identity (keypair)
    let identity = Identity::generate();
    let public_key = identity.to_public().to_string();
    let private_key = identity.to_string().expose_secret().to_string();

    if as_bytes {
        // Return as bytes
        Ok(Ret(v_list(&[
            v_binary(public_key.into_bytes()),
            v_binary(private_key.into_bytes()),
        ])))
    } else {
        // Return as strings (default)
        Ok(Ret(v_list(&[v_string(public_key), v_string(private_key)])))
    }
}

/// MOO: `bytes age_encrypt(str message, list recipients)`
/// Encrypts message using age encryption for one or more recipients. Programmer-only function.
/// Recipients can be age X25519 keys or SSH public keys (strings or bytes). Returns encrypted data as bytes.
fn bf_age_encrypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("age_encrypt() requires exactly two arguments"),
        ));
    }

    // Check for programmer permissions
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;

    // Get the message to encrypt
    let message = match bf_args.args[0].variant() {
        Variant::Str(s) => s.as_str(),
        _ => {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "age_encrypt() first argument must be a string, was {}",
                    bf_args.args[0].type_code().to_literal()
                )
            })));
        }
    };

    // Get the recipients list
    let recipients_list = match bf_args.args[1].variant() {
        Variant::List(l) => l,
        _ => {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "age_encrypt() second argument must be a list, was {}",
                    bf_args.args[1].type_code().to_literal()
                )
            })));
        }
    };

    if recipients_list.is_empty() {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("age_encrypt() requires at least one recipient"),
        ));
    }

    // Process each recipient - accept both strings and bytes
    let mut recipients = Vec::new();
    for recipient_var in recipients_list.iter() {
        let recipient_str = match recipient_var.variant() {
            Variant::Str(s) => s.as_str().to_string(),
            Variant::Binary(b) => {
                // Convert bytes to string for key parsing
                let Some(s) = std::str::from_utf8(b.as_bytes()).ok() else {
                    warn!("Invalid UTF-8 in binary recipient key");
                    return Err(BfErr::ErrValue(
                        E_INVARG.msg("age_encrypt() binary recipient key must be UTF-8 encoded text (Bech32 or OpenSSH format)"),
                    ));
                };
                s.to_string()
            }
            _ => {
                return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                    format!(
                        "age_encrypt() recipient must be a string or bytes, was {}",
                        recipient_var.type_code().to_literal()
                    )
                })));
            }
        };

        if let Ok(x25519_recipient) = recipient_str.parse::<Recipient>() {
            recipients.push(Box::new(x25519_recipient) as Box<dyn AgeRecipient>);
            continue;
        }
        let ssh_key = PublicKey::from_openssh(&recipient_str).map_err(|_| {
            warn!("Failed to parse SSH key");
            BfErr::ErrValue(E_INVARG.msg("age_encrypt() failed to parse SSH key"))
        })?;

        let ssh_recipient = age::ssh::Recipient::from_str(&ssh_key.to_string()).map_err(|_| {
            warn!("Failed to create age recipient from SSH key");
            BfErr::ErrValue(
                E_INVARG.msg("age_encrypt() failed to create age recipient from SSH key"),
            )
        })?;
        recipients.push(Box::new(ssh_recipient) as Box<dyn AgeRecipient>);
    }

    // Create an encryptor with the recipients
    let encryptor =
        Encryptor::with_recipients(recipients.iter().map(|r| r.as_ref())).map_err(|_| {
            error!("Failed to create encryptor");
            BfErr::ErrValue(E_INVARG.msg("age_encrypt() failed to create encryptor"))
        })?;

    // Encrypt the message
    let mut encrypted = Vec::new();
    let mut writer = encryptor.wrap_output(&mut encrypted).map_err(|e| {
        error!("Failed to create encryption writer: {}", e);
        BfErr::ErrValue(E_INVARG.msg("age_encrypt() failed to create encryption writer"))
    })?;

    writer
        .write_all(message.as_bytes())
        .and_then(|_| writer.finish())
        .map_err(|e| {
            error!("Failed to write message for encryption: {}", e);
            BfErr::ErrValue(E_INVARG.msg("age_encrypt() failed to write message for encryption"))
        })?;

    // Return the encrypted data as bytes
    Ok(Ret(v_binary(encrypted)))
}

/// MOO: `str age_decrypt(bytes|str encrypted_message, list private_keys)`
/// Decrypts age-encrypted message using one or more private keys. Programmer-only function.
/// Encrypted message can be bytes or base64-encoded string. Private keys can be strings or bytes. Returns decrypted plaintext string.
fn bf_age_decrypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("age_decrypt() requires exactly two arguments"),
        ));
    }

    // Check for programmer permissions
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;

    // Get the encrypted message - accept both bytes and base64 string for compatibility
    let encrypted = match bf_args.args[0].variant() {
        Variant::Binary(b) => b.as_bytes().to_vec(),
        Variant::Str(s) => {
            let Some(data) = BASE64.decode(s.as_str()).ok() else {
                warn!("Invalid base64 data for decryption");
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("age_decrypt() failed to decode base64 data"),
                ));
            };
            data
        }
        _ => {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "age_decrypt() first argument must be bytes or string, was {}",
                    bf_args.args[0].type_code().to_literal()
                )
            })));
        }
    };

    // Get the private keys list
    let private_keys_list = match bf_args.args[1].variant() {
        Variant::List(l) => l,
        _ => {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "age_decrypt() second argument must be a list, was {}",
                    bf_args.args[1].type_code().to_literal()
                )
            })));
        }
    };

    if private_keys_list.is_empty() {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("age_decrypt() requires at least one private key"),
        ));
    }

    // Process each private key - accept both strings and bytes
    let mut identities = Vec::new();
    for key_var in private_keys_list.iter() {
        let key_str = match key_var.variant() {
            Variant::Str(s) => s.as_str().to_string(),
            Variant::Binary(b) => {
                // Convert bytes to string for key parsing
                let Some(s) = std::str::from_utf8(b.as_bytes()).ok() else {
                    warn!("Invalid UTF-8 in binary private key");
                    return Err(BfErr::ErrValue(
                        E_INVARG.msg("age_decrypt() binary private key must be UTF-8 encoded text (Bech32 or OpenSSH format)"),
                    ));
                };
                s.to_string()
            }
            _ => {
                return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                    format!(
                        "age_decrypt() private key must be a string or bytes, was {}",
                        key_var.type_code().to_literal()
                    )
                })));
            }
        };

        // Try to parse as an age identity
        match key_str.parse::<Identity>() {
            Ok(identity) => {
                identities.push(Box::new(identity) as Box<dyn age::Identity>);
            }
            Err(_) => {
                // Try to parse as an SSH private key
                match ssh_key::PrivateKey::from_openssh(&key_str) {
                    Ok(ssh_key) => {
                        // Convert to OpenSSH format string
                        let ssh_key_str = ssh_key
                            .to_openssh(ssh_key::LineEnding::LF)
                            .unwrap_or_default();
                        // Use from_buffer to create the identity
                        match age::ssh::Identity::from_buffer(
                            std::io::Cursor::new(ssh_key_str),
                            None,
                        ) {
                            Ok(ssh_identity) => {
                                identities.push(Box::new(ssh_identity) as Box<dyn age::Identity>);
                            }
                            Err(_) => {
                                warn!("Failed to create age identity from SSH key");
                                // Continue to try other keys
                            }
                        }
                    }
                    Err(_) => {
                        warn!("Invalid private key format");
                        // Continue to try other keys
                    }
                }
            }
        }
    }

    if identities.is_empty() {
        warn!("No valid identities found for decryption");
        return Err(BfErr::ErrValue(
            E_INVARG.msg("age_decrypt() no valid identities found for decryption"),
        ));
    }
    let decryptor = Decryptor::new_buffered(&encrypted[..]).map_err(|_| {
        error!("Failed to create decryptor");
        BfErr::ErrValue(E_INVARG.msg("age_decrypt() failed to create decryptor"))
    })?;

    // Create a decryptor
    let mut reader = decryptor
        .decrypt(identities.iter().map(|i| i.as_ref()))
        .map_err(|_| {
            warn!("Failed to create decryptor");
            BfErr::ErrValue(E_INVARG.msg("age_decrypt() failed to create decryptor"))
        })?;
    let mut decrypted = String::new();
    match reader.read_to_string(&mut decrypted) {
        Ok(_) => Ok(Ret(v_string(decrypted))),
        Err(_) => {
            warn!("Decrypted data is not valid UTF-8");
            Err(BfErr::ErrValue(
                E_INVARG.msg("age_decrypt() decrypted data is not valid UTF-8"),
            ))
        }
    }
}

pub(crate) fn register_bf_age_crypto(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("age_generate_keypair")] = Box::new(bf_age_generate_keypair);
    builtins[offset_for_builtin("age_encrypt")] = Box::new(bf_age_encrypt);
    builtins[offset_for_builtin("age_decrypt")] = Box::new(bf_age_decrypt);
}
