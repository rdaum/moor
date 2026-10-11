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

//! Versioned exact program content fingerprints.
//!
//! v1 encodes [schema, field kind, normalized program structure, typed literal projections]
//! with moor-var CBOR and SHA-256. Literal projections use explicit type tags. Symbols use
//! folded names, maps sort by encoded key, floats preserve finite IEEE bits (including -0),
//! and unsupported values fail closed. Object references are installation-local.

use moor_compiler::{program_to_tree, unparse_for_comparison};
use moor_var::{Var, Variant, encode_var_cbor, program::ProgramType, v_int, v_list, v_str};
use sha2::{Digest, Sha256};

/// Persistent schema identifier for program baselines. Changes require new vectors and a new version.
pub const PROGRAM_SCHEMA: &str = "objdef-v1:program:sha256";

/// Hash an exact typed projection with deterministic map and identifier normalization.
pub fn digest(value: &Var) -> Result<String, String> {
    let bytes = encode_var_cbor(&exact_projection(value, 0)?).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Encode values without MOO's case-insensitive and cross-type equality.
fn exact_projection(value: &Var, depth: usize) -> Result<Var, String> {
    if depth > 64 {
        return Err("fingerprint nesting exceeds 64".into());
    }
    let tagged = |tag: &str, payload: Var| v_list(&[v_str(tag), payload]);
    let recur = |v: &Var| exact_projection(v, depth + 1);
    Ok(match value.variant() {
        Variant::None => tagged("none", v_int(0)),
        Variant::Bool(_) => tagged("bool", value.clone()),
        Variant::Int(_) => tagged("int", value.clone()),
        Variant::Float(f) => {
            if !f.is_finite() {
                return Err("non-finite float has no objdef-v1 fingerprint".into());
            }
            tagged("float", v_str(&format!("{:016x}", f.to_bits())))
        }
        Variant::Str(_) => tagged("str", value.clone()),
        Variant::Binary(_) => tagged("binary", value.clone()),
        Variant::Obj(_) => tagged("object", value.clone()),
        Variant::Sym(s) => tagged("symbol", v_str(&s.to_folded_case())),
        Variant::List(l) => tagged(
            "list",
            v_list(&l.iter().map(|v| recur(&v)).collect::<Result<Vec<_>, _>>()?),
        ),
        Variant::Map(m) => {
            let mut entries = m
                .iter()
                .map(|(k, v)| {
                    let key = recur(&k)?;
                    let bytes = encode_var_cbor(&key).map_err(|e| e.to_string())?;
                    Ok((bytes, v_list(&[key, recur(&v)?])))
                })
                .collect::<Result<Vec<_>, String>>()?;
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            tagged(
                "map",
                v_list(&entries.into_iter().map(|(_, v)| v).collect::<Vec<_>>()),
            )
        }
        Variant::Flyweight(f) => {
            let mut slots = f
                .slots()
                .into_iter()
                .map(|(k, v)| Ok((k.to_folded_case(), recur(&v)?)))
                .collect::<Result<Vec<_>, String>>()?;
            slots.sort_by(|a, b| a.0.cmp(&b.0));
            tagged(
                "flyweight",
                v_list(&[
                    moor_var::v_obj(*f.delegate()),
                    v_list(
                        &slots
                            .into_iter()
                            .map(|(k, v)| v_list(&[v_str(&k), v]))
                            .collect::<Vec<_>>(),
                    ),
                    v_list(
                        &f.contents()
                            .iter()
                            .map(|v| recur(&v))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ]),
            )
        }
        Variant::Err(e) => {
            let code = match e.err_type() {
                moor_var::ErrorCode::ErrCustom(s) => tagged("custom", v_str(&s.to_folded_case())),
                _ => tagged(
                    "code",
                    v_int(e.to_int().ok_or("unsupported error code")? as i64),
                ),
            };
            tagged(
                "error",
                v_list(&[
                    code,
                    match e.msg() {
                        Some(s) => tagged("some", v_str(s)),
                        None => tagged("none", v_int(0)),
                    },
                    match e.value() {
                        Some(v) => tagged("some", recur(v)?),
                        None => tagged("none", v_int(0)),
                    },
                ]),
            )
        }
        Variant::Lambda(_) => {
            return Err("captured lambda values have no objdef-v1 fingerprint".into());
        }
    })
}

/// Hash normalized compiled structure and exact literals; unsupported representations fail.
pub fn program_fingerprint(program: &ProgramType) -> Result<String, String> {
    let ProgramType::MooR(program) = program;
    let tree = program_to_tree(program).map_err(|e| e.to_string())?;
    let (structure, literals) = unparse_for_comparison(&tree).map_err(|e| e.to_string())?;
    let projection = v_list(&[v_str(PROGRAM_SCHEMA), v_str(&structure), v_list(&literals)]);
    Ok(format!("{PROGRAM_SCHEMA}:{}", digest(&projection)?))
}

/// Conservatively identify identical compiled programs before normalizing both of them.
/// Program equality alone is insufficient: MOO literals compare strings without case
/// and can equate distinct numeric types, while float equality loses signed zero.
pub(crate) fn same_compiled_program(left: &ProgramType, right: &ProgramType) -> bool {
    use moor_var::program::{opcode::Op, program::Program};

    fn exact_payloads(left: &Program, right: &Program) -> bool {
        let left_literals = encode_var_cbor(&v_list(&left.0.literals));
        let right_literals = encode_var_cbor(&v_list(&right.0.literals));
        if !matches!((left_literals, right_literals), (Ok(a), Ok(b)) if a == b) {
            return false;
        }
        fn ops(program: &Program) -> impl Iterator<Item = &Op> {
            program
                .0
                .main_vector
                .iter()
                .chain(program.0.fork_vectors.iter().flat_map(|(_, ops)| ops))
        }
        if !ops(left).zip(ops(right)).all(|(a, b)| match (a, b) {
            (Op::ImmFloat(a), Op::ImmFloat(b)) => a.to_bits() == b.to_bits(),
            _ => true,
        }) {
            return false;
        }
        left.0
            .lambda_programs
            .iter()
            .zip(&right.0.lambda_programs)
            .all(|(a, b)| exact_payloads(a, b))
    }

    let (ProgramType::MooR(left), ProgramType::MooR(right)) = (left, right);
    left == right && exact_payloads(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(source: &str) -> String {
        program_fingerprint(&ProgramType::MooR(
            moor_compiler::compile(source, Default::default()).unwrap(),
        ))
        .unwrap()
    }
    #[test]
    fn layout_is_ignored_but_literals_are_exact() {
        assert_eq!(hash("return 1;"), hash("  return   1 ; "));
        assert_ne!(hash("return 1;"), hash("return 1.0;"));
        assert_ne!(hash("return \"Hello\";"), hash("return \"hello\";"));
        assert_ne!(hash("return 0.0;"), hash("return -0.0;"));
        assert_eq!(hash("return 'CaseProbe;"), hash("return 'caseprobe;"));
    }
    #[test]
    fn compiled_program_reuse_preserves_exact_literals_and_float_bits() {
        let program =
            |source| ProgramType::MooR(moor_compiler::compile(source, Default::default()).unwrap());
        for source in ["return 1;", "return fn() return {\"Case\", 1.0}; endfn;"] {
            assert!(same_compiled_program(&program(source), &program(source)));
        }
        for (left, right) in [
            ("return \"Case\";", "return \"case\";"),
            ("return 1;", "return 1.0;"),
            ("return 0.0;", "return -0.0;"),
            ("return {1};", "return {1.0};"),
            (
                "return fn() return \"Case\"; endfn;",
                "return fn() return \"case\"; endfn;",
            ),
            (
                "fork (0) return 0.0; endfork",
                "fork (0) return -0.0; endfork",
            ),
        ] {
            let left = program(left);
            let right = program(right);
            assert!(!same_compiled_program(&left, &right));
            assert_ne!(
                program_fingerprint(&left).unwrap(),
                program_fingerprint(&right).unwrap()
            );
        }
    }
    #[test]
    fn normalization_covers_empty_scopes_and_lambdas() {
        for source in [
            "",
            "begin let x = 1; return x; end",
            "return {x} => x + 1;",
            "return fn(x) return x; endfn;",
        ] {
            let program = moor_compiler::compile(source, Default::default()).unwrap();
            let tree = program_to_tree(&program).unwrap();
            let text = moor_compiler::unparse(&tree, true, false)
                .unwrap()
                .join("\n");
            assert_eq!(hash(source), hash(&text));
        }
    }
    #[test]
    fn frozen_program_vectors() {
        assert_eq!(
            hash("return 1;"),
            "objdef-v1:program:sha256:68d23569eafe766dc2751f763a36d5a486d24bd67dfc9933fcefe48ce89ab116"
        );
    }

    #[test]
    fn separate_process_symbol_order() {
        if let Ok(order) = std::env::var("MOOR_FINGERPRINT_TEST_ORDER") {
            for symbol in if order == "forward" {
                ["MiXeD", "another"]
            } else {
                ["ANOTHER", "mixed"]
            } {
                let _ = moor_var::Symbol::mk(symbol);
            }
            println!(
                "FINGERPRINT={}",
                hash("return ['mixed -> {\"Exact\", 1.0}];")
            );
            return;
        }
        let mut results = Vec::new();
        for order in ["forward", "reverse"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "fingerprint::tests::separate_process_symbol_order",
                    "--exact",
                    "--nocapture",
                ])
                .env("MOOR_FINGERPRINT_TEST_ORDER", order)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            results.push(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .lines()
                    .find(|line| line.starts_with("FINGERPRINT="))
                    .unwrap()
                    .to_owned(),
            );
        }
        assert_eq!(results[0], results[1]);
    }
}
