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

use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use md5::Digest;
use moor_compiler::offset_for_builtin;
use moor_values::Error::{E_ARGS, E_INVARG, E_TYPE};
use moor_values::{v_bool, v_int, v_str, v_string};
use moor_values::{Sequence, Variant};
use rand::distributions::Alphanumeric;
use rand::Rng;
use tracing::warn;

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{world_state_bf_err, BfCallState, BfErr, BfRet, BuiltinFunction};

fn strsub(subject: &str, what: &str, with: &str, case_matters: bool) -> String {
    let mut result = String::new();
    let mut source = subject;

    if what.is_empty() {
        return subject.to_string();
    }

    while let Some(index) = if case_matters {
        source.find(what)
    } else {
        source.to_lowercase().find(&what.to_lowercase())
    } {
        result.push_str(&source[..index]);
        result.push_str(with);
        let next = index + what.len();
        source = &source[next..];
    }

    result.push_str(source);

    result
}

//Function: str strsub (str subject, str what, str with [, case-matters])
fn bf_strsub(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 3 {
        false
    } else if bf_args.args.len() == 4 {
        let Variant::Int(case_matters) = bf_args.args[3].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };
    let (subject, what, with) = (
        bf_args.args[0].variant(),
        bf_args.args[1].variant(),
        bf_args.args[2].variant(),
    );
    match (subject, what, with) {
        (Variant::Str(subject), Variant::Str(what), Variant::Str(with)) => Ok(Ret(v_str(
            strsub(
                subject.as_string().as_str(),
                what.as_string().as_str(),
                with.as_string().as_str(),
                case_matters,
            )
            .as_str(),
        ))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(strsub, bf_strsub);

fn str_index(subject: &str, what: &str, case_matters: bool) -> i64 {
    if case_matters {
        subject.find(what).map(|i| i as i64 + 1).unwrap_or(0)
    } else {
        subject
            .to_lowercase()
            .find(&what.to_lowercase())
            .map(|i| i as i64 + 1)
            .unwrap_or(0)
    }
}

fn str_rindex(subject: &str, what: &str, case_matters: bool) -> i64 {
    if case_matters {
        subject.rfind(what).map(|i| i as i64 + 1).unwrap_or(0)
    } else {
        subject
            .to_lowercase()
            .rfind(&what.to_lowercase())
            .map(|i| i as i64 + 1)
            .unwrap_or(0)
    }
}

fn bf_index(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        let Variant::Int(case_matters) = bf_args.args[2].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_index(
            subject.as_string().as_str(),
            what.as_string().as_str(),
            case_matters,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(index, bf_index);

fn bf_rindex(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let case_matters = if bf_args.args.len() == 2 {
        false
    } else if bf_args.args.len() == 3 {
        let Variant::Int(case_matters) = bf_args.args[2].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *case_matters == 1
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    let (subject, what) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (subject, what) {
        (Variant::Str(subject), Variant::Str(what)) => Ok(Ret(v_int(str_rindex(
            subject.as_string().as_str(),
            what.as_string().as_str(),
            case_matters,
        )))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(rindex, bf_rindex);

fn bf_strcmp(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (str1, str2) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    match (str1, str2) {
        (Variant::Str(str1), Variant::Str(str2)) => Ok(Ret(v_int(
            str1.as_string().as_str().cmp(str2.as_string().as_str()) as i64,
        ))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(strcmp, bf_strcmp);

/*
str crypt (str text [, str salt])

Encrypts the given text using the standard UNIX encryption method. If provided, salt should be a
string at least two characters long, the first two characters of which will be used as the extra
encryption "salt" in the algorithm. If salt is not provided, a random pair of characters is used.
 In any case, the salt used is also returned as the first two characters of the resulting encrypted
 string.
*/
fn bf_crypt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let salt = if bf_args.args.len() == 1 {
        // Provide a random 2-letter salt.
        let mut rng = rand::thread_rng();
        let mut salt = String::new();

        salt.push(char::from(rng.sample(Alphanumeric)));
        salt.push(char::from(rng.sample(Alphanumeric)));
        salt
    } else {
        let Variant::Str(salt) = bf_args.args[1].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        String::from(salt.as_string().as_str())
    };
    if let Variant::Str(text) = bf_args.args[0].variant() {
        let crypted = pwhash::unix::crypt(text.as_string().as_str(), salt.as_str()).unwrap();
        Ok(Ret(v_string(crypted)))
    } else {
        Err(BfErr::Code(E_TYPE))
    }
}
bf_declare!(crypt, bf_crypt);

fn bf_string_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Str(s) => {
            let hash_digest = md5::Md5::digest(s.as_string().as_bytes());
            Ok(Ret(v_str(
                format!("{:x}", hash_digest).to_uppercase().as_str(),
            )))
        }
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(string_hash, bf_string_hash);

fn bf_binary_hash(_bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    return Err(BfErr::Code(E_INVARG));
}
bf_declare!(binary_hash, bf_binary_hash);

bf_declare!(argon2, bf_argon2);
// password (string), salt (string), iterations, memory, parallelism
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
    let Variant::Str(password) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::Str(salt) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let iterations = if bf_args.args.len() > 2 {
        let Variant::Int(iterations) = bf_args.args[2].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *iterations as u32
    } else {
        3
    };
    let memory = if bf_args.args.len() > 3 {
        let Variant::Int(memory) = bf_args.args[3].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *memory as u32
    } else {
        4096
    };

    let parallelism = if bf_args.args.len() > 4 {
        let Variant::Int(parallelism) = bf_args.args[4].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *parallelism as u32
    } else {
        1
    };

    let params = Params::new(memory, iterations, parallelism, None).map_err(|e| {
        warn!("Failed to create argon2 params: {}", e);
        BfErr::Code(E_INVARG)
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let salt_string =
        SaltString::encode_b64(salt.as_string().as_str().as_bytes()).map_err(|e| {
            warn!("Failed to encode salt: {}", e);
            BfErr::Code(E_INVARG)
        })?;

    let hash = argon2
        .hash_password(password.as_string().as_bytes(), &salt_string)
        .map_err(|e| {
            warn!("Failed to hash password: {}", e);
            BfErr::Code(E_INVARG)
        })?;

    Ok(Ret(v_string(hash.to_string())))
}

// password, salt
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
    let Variant::Str(hashed_password) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::Str(password) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let Ok(hashed_password) = argon2::PasswordHash::new(hashed_password.as_string()) else {
        return Err(BfErr::Code(E_INVARG));
    };

    let validated = argon2
        .verify_password(password.as_string().as_bytes(), &hashed_password)
        .is_ok();
    Ok(Ret(v_bool(validated)))
}
bf_declare!(argon2_verify, bf_argon2_verify);

pub(crate) fn register_bf_strings(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("strsub")] = Box::new(BfStrsub {});
    builtins[offset_for_builtin("index")] = Box::new(BfIndex {});
    builtins[offset_for_builtin("rindex")] = Box::new(BfRindex {});
    builtins[offset_for_builtin("strcmp")] = Box::new(BfStrcmp {});
    builtins[offset_for_builtin("crypt")] = Box::new(BfCrypt {});
    builtins[offset_for_builtin("argon2")] = Box::new(BfArgon2 {});
    builtins[offset_for_builtin("argon2_verify")] = Box::new(BfArgon2Verify {});
    builtins[offset_for_builtin("string_hash")] = Box::new(BfStringHash {});
    builtins[offset_for_builtin("binary_hash")] = Box::new(BfBinaryHash {});
}

#[cfg(test)]
mod tests {
    use crate::builtins::bf_strings::strsub;

    #[test]
    fn test_strsub_remove_piece() {
        let subject = "empty_message_integrate_room";
        assert_eq!(
            strsub(subject, "empty_message_", "", false),
            "integrate_room"
        );
    }

    #[test]
    fn test_strsub_case_insensitive_substitution() {
        let subject = "foo bar baz";
        let expected = "fizz bar baz";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_case_sensitive_substitution() {
        let subject = "foo bar baz";
        let expected = "foo bar fizz";
        assert_eq!(strsub(subject, "baz", "fizz", true), expected);
    }

    #[test]
    fn test_strsub_empty_subject() {
        let subject = "";
        let expected = "";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_empty_what() {
        let subject = "foo bar baz";
        let expected = "foo bar baz";
        assert_eq!(strsub(subject, "", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_multiple_occurrences() {
        let subject = "foo foo foo";
        let expected = "fizz fizz fizz";
        assert_eq!(strsub(subject, "foo", "fizz", false), expected);
    }

    #[test]
    fn test_strsub_no_occurrences() {
        let subject = "foo bar baz";
        let expected = "foo bar baz";
        assert_eq!(strsub(subject, "fizz", "buzz", false), expected);
    }
}
