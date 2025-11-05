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

//! Builtin functions used for encryption & cryptographic hashing.

use std::{io::Write, str::FromStr};

use age::{
    Decryptor, Encryptor, Recipient as AgeRecipient,
    secrecy::ExposeSecret,
    x25519::{Identity, Recipient},
};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use rand::Rng;
use rand::distr::Alphanumeric;
use sha1::Sha1;
use sha2::Sha256;
use ssh_key::public::PublicKey;
use std::io::Read;
use tracing::{error, warn};

use crate::vm::builtins::{
    BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction, world_state_bf_err,
};
use moor_compiler::{offset_for_builtin, to_literal};
use moor_var::{
    E_ARGS, E_INVARG, E_TYPE, Sequence, Symbol, Variant, v_binary, v_list, v_str, v_string,
};

lazy_static! {
    static ref SHA1_SYM: Symbol = Symbol::mk("sha1");
    static ref SHA256_SYM: Symbol = Symbol::mk("sha256");
}

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

/// MOO: `str argon2(str password, str salt [, int iterations] [, int memory] [, int parallelism])`
/// Generates Argon2 hash with specified parameters. Wizard-only function.
fn bf_argon2(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() > 5 || bf_args.args.len() < 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(password) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(salt) = bf_args.args[1].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let iterations = if bf_args.args.len() > 2 {
        let Some(iterations) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        iterations as u32
    } else {
        3
    };
    let memory = if bf_args.args.len() > 3 {
        let Some(memory) = bf_args.args[3].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        memory as u32
    } else {
        4096
    };

    let parallelism = if bf_args.args.len() > 4 {
        let Some(parallelism) = bf_args.args[4].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        parallelism as u32
    } else {
        1
    };

    let params = Params::new(memory, iterations, parallelism, None).map_err(|e| {
        warn!("Failed to create argon2 params: {}", e);
        BfErr::Code(E_INVARG)
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let salt_string = SaltString::encode_b64(salt.as_bytes()).map_err(|e| {
        warn!("Failed to encode salt: {}", e);
        BfErr::Code(E_INVARG)
    })?;

    let hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .map_err(|e| {
            warn!("Failed to hash password: {}", e);
            BfErr::Code(E_INVARG)
        })?;

    Ok(Ret(v_string(hash.to_string())))
}

/// MOO: `bool argon2_verify(str hashed_password, str password)`
/// Verifies a password against an Argon2 hash. Wizard-only function.
fn bf_argon2_verify(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(hashed_password) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(password) = bf_args.args[1].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let Ok(hashed_password) = argon2::PasswordHash::new(hashed_password) else {
        return Err(BfErr::Code(E_INVARG));
    };

    let validated = argon2
        .verify_password(password.as_bytes(), &hashed_password)
        .is_ok();
    Ok(Ret(bf_args.v_bool(validated)))
}

/// MOO: `str crypt(str text [, str salt])`
/// Encrypts text using standard UNIX encryption method.
/// If salt is provided, uses first two characters as encryption salt.
/// If no salt provided, uses random pair. Salt is returned as first two characters of result.
fn bf_crypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let salt = if bf_args.args.len() == 1 {
        // Provide a random 2-letter salt.
        let mut rng = rand::rng();
        let mut salt = String::new();

        salt.push(char::from(rng.sample(Alphanumeric)));
        salt.push(char::from(rng.sample(Alphanumeric)));
        salt
    } else {
        let Some(salt) = bf_args.args[1].as_string() else {
            return Err(BfErr::Code(E_TYPE));
        };
        String::from(salt)
    };
    if let Some(text) = bf_args.args[0].as_string() {
        let crypted = pwhash::unix::crypt(text, salt.as_str()).unwrap();
        Ok(Ret(v_string(crypted)))
    } else {
        Err(BfErr::Code(E_TYPE))
    }
}

/// MOO: `str salt()`
/// Generates a random cryptographically secure salt string for use with crypt & argon2.
/// Note: Not compatible with ToastStunt's salt function which takes two arguments.
fn bf_salt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let mut rng_core = OsRng;
    let salt = SaltString::generate(&mut rng_core);
    let salt = v_str(salt.as_str());
    Ok(Ret(salt))
}

/// MOO: `str string_hmac(str text, str key [, str algorithm] [, bool binary_output])`
/// Computes HMAC of text using key with specified algorithm (SHA1, SHA256).
fn bf_string_hmac(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg_count = bf_args.args.len();
    if !(2..=4).contains(&arg_count) {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(text) = bf_args.args[0].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for text argument for string_hmac",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Some(key) = bf_args.args[1].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for key argument for string_hmac",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let algo = if arg_count > 2 {
        let Ok(kind) = bf_args.args[2].as_symbol() else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "Invalid type for algorithm argument in string_hmac: {})",
                    to_literal(&bf_args.args[2])
                )
            })));
        };
        kind
    } else {
        *SHA256_SYM
    };

    let binary_output = arg_count > 3 && bf_args.args[3].is_true();

    let result_bytes = if algo == *SHA1_SYM {
        let mut mac =
            Hmac::<Sha1>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(text.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else if algo == *SHA256_SYM {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(text.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else {
        return Err(BfErr::ErrValue(
            E_INVARG.with_msg(|| format!("Invalid algorithm for string_hmac: {algo}")),
        ));
    };

    if binary_output {
        Ok(Ret(v_binary(result_bytes)))
    } else {
        let hex_string = result_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Ret(v_str(&hex_string)))
    }
}

/// MOO: `str|binary binary_hmac(binary data, str key [, symbol algorithm] [, bool binary_output])`
/// Computes HMAC of binary data using key with specified algorithm (SHA1, SHA256).
/// Note: Takes mooR's native Binary type, NOT ToastStunt's bin-string format.
fn bf_binary_hmac(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg_count = bf_args.args.len();
    if !(2..=4).contains(&arg_count) {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(data) = bf_args.args[0].as_binary() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for data argument for binary_hmac (requires mooR Binary type, not string)",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Some(key) = bf_args.args[1].as_string() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Invalid type {} for key argument for binary_hmac",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let algo = if arg_count > 2 {
        let Ok(kind) = bf_args.args[2].as_symbol() else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "Invalid type for algorithm argument in binary_hmac: {})",
                    to_literal(&bf_args.args[2])
                )
            })));
        };
        kind
    } else {
        *SHA256_SYM
    };

    let binary_output = arg_count > 3 && bf_args.args[3].is_true();

    let result_bytes = if algo == *SHA1_SYM {
        let mut mac =
            Hmac::<Sha1>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else if algo == *SHA256_SYM {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| BfErr::Code(E_INVARG))?;
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else {
        return Err(BfErr::ErrValue(
            E_INVARG.with_msg(|| format!("Invalid algorithm for binary_hmac: {algo}")),
        ));
    };

    if binary_output {
        Ok(Ret(v_binary(result_bytes)))
    } else {
        let hex_string = result_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Ret(v_str(&hex_string)))
    }
}

pub(crate) fn register_bf_cryptography(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("age_generate_keypair")] = bf_age_generate_keypair;
    builtins[offset_for_builtin("age_encrypt")] = bf_age_encrypt;
    builtins[offset_for_builtin("age_decrypt")] = bf_age_decrypt;
    builtins[offset_for_builtin("argon2")] = bf_argon2;
    builtins[offset_for_builtin("argon2_verify")] = bf_argon2_verify;
    builtins[offset_for_builtin("crypt")] = bf_crypt;
    builtins[offset_for_builtin("salt")] = bf_salt;
    builtins[offset_for_builtin("string_hmac")] = bf_string_hmac;
    builtins[offset_for_builtin("binary_hmac")] = bf_binary_hmac;
}
