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

//! Exact structural conformance for the versioned persistence literal codec.

use moor_compiler::{SourceProfile, read_persistent_literal, write_persistent_literal};
fn to_literal(value: &moor_var::Var) -> String {
    let mut text = String::new();
    write_persistent_literal(value, &SourceProfile::default(), &mut text).unwrap();
    text
}
fn parse_literal_value(text: &str) -> Result<moor_var::Var, moor_compiler::LiteralDecodeError> {
    read_persistent_literal(text, &SourceProfile::default())
}
use moor_var::{
    AnonymousObjid, Error, ErrorCode, List, NOTHING, Obj, Symbol, Var, Variant, v_binary, v_bool,
    v_err, v_error, v_float, v_flyweight, v_int, v_list, v_map, v_none, v_obj, v_str, v_symbol_str,
};

/// Compare storage-visible data recursively, including case, payloads, and float bits.
fn structurally_equal(left: &Var, right: &Var) -> bool {
    fn sequence_equal<'a>(
        a: impl IntoIterator<Item = &'a Var>,
        b: impl IntoIterator<Item = &'a Var>,
    ) -> bool {
        let mut a = a.into_iter();
        let mut b = b.into_iter();
        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (Some(a), Some(b)) if structurally_equal(a, b) => {}
                _ => return false,
            }
        }
    }
    match (left.variant(), right.variant()) {
        (Variant::None, Variant::None) => true,
        (Variant::Bool(a), Variant::Bool(b)) => a == b,
        (Variant::Int(a), Variant::Int(b)) => a == b,
        (Variant::Float(a), Variant::Float(b)) => a.to_bits() == b.to_bits(),
        (Variant::Obj(a), Variant::Obj(b)) => a == b,
        (Variant::Sym(a), Variant::Sym(b)) => a.as_str() == b.as_str(),
        (Variant::Str(a), Variant::Str(b)) => a.as_str() == b.as_str(),
        (Variant::Binary(a), Variant::Binary(b)) => a.as_bytes() == b.as_bytes(),
        (Variant::List(a), Variant::List(b)) => sequence_equal(a.iter_ref(), b.iter_ref()),
        (Variant::Map(a), Variant::Map(b)) => sequence_equal(
            a.iter_ref().flat_map(|(key, value)| [key, value]),
            b.iter_ref().flat_map(|(key, value)| [key, value]),
        ),
        (Variant::Err(a), Variant::Err(b)) => {
            let code_equal = match (a.err_type(), b.err_type()) {
                (ErrorCode::ErrCustom(a), ErrorCode::ErrCustom(b)) => a.as_str() == b.as_str(),
                (a, b) => a == b,
            };
            code_equal && a.msg() == b.msg() && sequence_equal(a.value(), b.value())
        }
        (Variant::Flyweight(a), Variant::Flyweight(b)) => {
            a.delegate() == b.delegate()
                && a.slots_storage().len() == b.slots_storage().len()
                && a.slots_storage()
                    .iter()
                    .zip(b.slots_storage())
                    .all(|((ak, av), (bk, bv))| {
                        ak.as_str() == bk.as_str() && structurally_equal(av, bv)
                    })
                && sequence_equal(a.contents().iter_ref(), b.contents().iter_ref())
        }
        // Executable lambda equivalence is checked by the runtime corpus, not bytecode equality.
        (Variant::Lambda(_), Variant::Lambda(_)) => panic!("use the behavioral lambda corpus"),
        _ => false,
    }
}

#[test]
fn exact_comparison_detects_nested_storage_differences() {
    assert!(!structurally_equal(
        &v_list(&[v_float(-0.0)]),
        &v_list(&[v_float(0.0)])
    ));
    assert!(!structurally_equal(
        &v_list(&[v_str("A")]),
        &v_list(&[v_str("a")])
    ));
    assert!(!structurally_equal(
        &v_map(&[(v_int(1), v_str("A"))]),
        &v_map(&[(v_int(1), v_str("a"))])
    ));
    assert!(!structurally_equal(&v_bool(true), &v_int(1)));
    assert!(!structurally_equal(
        &v_error(Error::new(ErrorCode::E_INVARG, None, Some(v_str("A")))),
        &v_error(Error::new(ErrorCode::E_INVARG, None, Some(v_str("a")))),
    ));
}

/// Validate through the persistence API in both directions.
fn round_trip(value: &Var) -> Result<Var, String> {
    let text = to_literal(value);
    parse_literal_value(&text).map_err(|error| format!("{error:?}"))
}

#[track_caller]
fn assert_round_trip(value: &Var) {
    let parsed = round_trip(value).unwrap_or_else(|error| {
        panic!("literal for {value:?} did not parse: {error}");
    });
    assert!(
        structurally_equal(&parsed, value),
        "literal {:?} parsed as {parsed:?}, expected {value:?}",
        to_literal(value)
    );
}

#[test]
fn scalars_round_trip() {
    let values = [
        v_bool(true),
        v_bool(false),
        v_int(0),
        v_int(42),
        v_int(-42),
        v_int(i64::MIN),
        v_int(i64::MAX),
        v_float(-0.0),
        v_float(f64::from_bits(1)),
        v_float(0.5),
        v_float(-1.25),
        v_float(f64::MIN),
        v_float(f64::MAX),
        v_str(""),
        v_str("hello world"),
        v_str("quote \" backslash \\"),
        v_str("line\nbreak"),
        v_str("tab\tseparated"),
        v_str("\u{1}\u{7f}"),
        v_str("nul \0 inside"),
        v_str("literal \\x00 inside"),
        v_str("unicode \u{3b1}\u{3b2}\u{3c2} \u{1f404}"),
        v_symbol_str(Symbol::mk("foo")),
        v_symbol_str(Symbol::mk("MixedCase")),
        v_binary(vec![0x00, 0x01, 0x7f, 0x80, 0xff]),
        v_binary((0..=255).collect()),
    ];

    for value in &values {
        assert_round_trip(value);
    }
}

#[test]
fn objects_round_trip() {
    let values = [
        v_obj(NOTHING),
        v_obj(Obj::mk_id(0)),
        v_obj(Obj::mk_id(42)),
        v_obj(Obj::mk_id(-42)),
        v_obj(Obj::mk_uuobjid_generated()),
    ];

    for value in &values {
        assert_round_trip(value);
    }
}

#[test]
fn containers_round_trip() {
    let values = [
        v_list(&[]),
        v_list(&[v_int(1), v_str("two"), v_obj(Obj::mk_id(3))]),
        v_list(&[v_list(&[v_int(1)]), v_list(&[])]),
        v_map(&[]),
        v_map(&[
            (v_int(1), v_str("one")),
            (v_str("key"), v_obj(Obj::mk_id(2))),
            (v_bool(true), v_list(&[v_int(0)])),
        ]),
    ];

    for value in &values {
        assert_round_trip(value);
    }
}

#[test]
fn errors_round_trip() {
    let values = [
        v_err(ErrorCode::E_INVARG),
        v_error(Error::new(
            ErrorCode::E_INVARG,
            Some("bad".to_string()),
            None,
        )),
        v_error(Error::new(
            ErrorCode::ErrCustom(Symbol::mk("E_CUSTOM")),
            Some("custom".to_string()),
            None,
        )),
    ];

    for value in &values {
        assert_round_trip(value);
    }
}

#[test]
fn flyweights_round_trip() {
    let delegate = Obj::mk_id(0);
    let slots = [(Symbol::mk("base"), v_int(42))];
    let contents = List::mk_list(&[v_int(1), v_str("two")]);
    let values = [
        v_flyweight(delegate, &[], List::mk_list(&[])),
        v_flyweight(delegate, &slots, contents),
    ];

    for value in &values {
        assert_round_trip(value);
    }
}

/// None remains a distinct value inside containers.
#[test]
fn none_round_trips() {
    assert_round_trip(&v_none());
    assert_round_trip(&v_list(&[v_none()]));
    assert_round_trip(&v_map(&[(v_bool(true), v_list(&[v_none()]))]));
}

/// §8.2: ordinary object display hides anonymous identity, so anonymous objects need an explicit
/// identity-preserving literal spelling. §8.3: the value table requires complete identity.
#[test]
fn anonymous_objects_round_trip() {
    let anonymous = Obj::mk_anonymous(AnonymousObjid::generate(1));
    assert_round_trip(&v_obj(anonymous));
    assert_round_trip(&v_list(&[v_obj(anonymous)]));
}

/// Flyweight delegates retain their complete object identity.
#[test]
fn flyweight_delegates_preserve_identity() {
    for delegate in [
        Obj::mk_uuobjid_generated(),
        Obj::mk_anonymous(AnonymousObjid::generate(2)),
    ] {
        assert_round_trip(&v_flyweight(delegate, &[], List::mk_list(&[])));
    }
}

/// §8.3: non-finite floats require an explicit scalar literal grammar extension preserving
/// infinity sign and NaN sign/payload bits.
#[test]
fn non_finite_floats_round_trip_exactly() {
    for value in [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0xfff8_0000_0000_0001),
    ] {
        assert_round_trip(&v_float(value));
    }
}

/// Rich errors retain their optional attached value.
#[test]
fn rich_error_attached_values_round_trip() {
    let value = v_error(Error::new(
        ErrorCode::E_INVARG,
        None,
        Some(v_list(&[v_int(1), v_str("detail")])),
    ));
    assert_round_trip(&value);
}

/// §8.3: canonical escaping writes NUL as the four characters `\x00`, not `\0`.
#[test]
fn nul_uses_canonical_hex_escape() {
    let text = to_literal(&v_str("a\0b"));
    assert!(text.contains("\\x00"), "expected \\x00 in {text:?}");
    assert!(!text.contains("\\0"), "unexpected \\0 in {text:?}");
}

/// §8.3: ordinary Unicode remains UTF-8; hex escapes cover C0 controls and DEL.
#[test]
fn unicode_is_emitted_as_utf8() {
    let text = to_literal(&v_str("\u{3b1}\u{1f404}"));
    assert!(text.contains('\u{3b1}'), "expected raw UTF-8 in {text:?}");
    assert!(text.contains('\u{1f404}'), "expected raw UTF-8 in {text:?}");
}

/// §8.3: the lexer recognizes `\U` escapes but the unquoter has no eight-digit decode branch.
/// The codec must reject unsupported escapes instead of silently passing them through.
#[test]
fn unsupported_unicode_escape_is_rejected() {
    assert!(parse_literal_value("\"\\U0001F404\"").is_err());
}

#[test]
fn symbols_errors_and_slots_preserve_unrestricted_spelling() {
    for spelling in [
        "",
        "with space",
        "MixedCASE",
        "E_TYPE",
        "α\0🐄",
        "none",
        "x\"\\y",
    ] {
        let symbol = Symbol::mk(spelling);
        assert_round_trip(&v_symbol_str(symbol));
        for message in [None, Some("\0 detail 🐄".to_owned())] {
            for value in [None, Some(v_none()), Some(v_list(&[v_float(-0.0)]))] {
                assert_round_trip(&v_error(Error::new(
                    ErrorCode::ErrCustom(symbol),
                    message.clone(),
                    value,
                )));
            }
        }
        assert_round_trip(&v_flyweight(
            Obj::mk_id(1),
            &[(symbol, v_none())],
            List::mk_list(&[]),
        ));
    }
    for spelling in ["delegate", "slots"] {
        assert_round_trip(&v_flyweight(
            Obj::mk_id(1),
            &[(Symbol::mk(spelling), v_int(2))],
            List::mk_list(&[]),
        ));
    }
}

#[test]
fn deterministic_float_bit_corpus() {
    let mut bits = 0x1234_5678_9abc_def0_u64;
    for _ in 0..4096 {
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        assert_round_trip(&v_float(f64::from_bits(bits)));
    }
    for bits in [
        0x7ff0_0000_0000_0001,
        0xfff0_0000_0000_0001,
        0x7fff_ffff_ffff_ffff,
    ] {
        assert_round_trip(&v_float(f64::from_bits(bits)));
    }
}

#[test]
fn profile_is_checked_before_decoding_and_output() {
    use moor_compiler::{CompileOptions, LiteralDecodeError, PersistentProgram, compile};
    use moor_var::program::ProgramType;
    let profile = SourceProfile::default();
    let mut profiles = vec![];
    let mut p = profile.clone();
    p.language = "other".into();
    profiles.push(p);
    let mut p = profile.clone();
    p.literal_version += 1;
    profiles.push(p);
    let mut p = profile.clone();
    p.source_version += 1;
    profiles.push(p);
    let mut p = profile.clone();
    p.compiler_profile_version += 1;
    profiles.push(p);
    let mut p = profile.clone();
    p.options.bool_type = false;
    profiles.push(p);
    for p in profiles {
        assert!(matches!(
            read_persistent_literal("invalid", &p),
            Err(LiteralDecodeError::Profile(_))
        ));
        let mut output = "prefix".to_owned();
        assert!(write_persistent_literal(&v_int(1), &p, &mut output).is_err());
        assert_eq!(output, "prefix");
        assert!(moor_compiler::read_persistent_source("invalid", &p).is_err());
    }
    let program = ProgramType::MooR(compile("return 42;", CompileOptions::default()).unwrap());
    let mut stored = PersistentProgram::encode(&program, &profile).unwrap();
    stored.originating_compiler = "another-build".into();
    assert!(stored.decode().is_ok());
}

#[test]
fn persistence_rejects_external_inputs_malformed_bits_and_duplicate_captures() {
    for text in [
        "$external",
        "EXTERNAL",
        "include!(\"secret\")",
        "include_bin!(\"secret\")",
        "1; return 2;",
        "f\"0000000000000000\"",
        "f\"7FF\"",
        "f\"GGGGGGGGGGGGGGGG\"",
        "1e9999",
        "#FFFFFF-1234567890",
        "#anon_FFFFFF-1234567890",
        "#2147483648",
        "{} => x with captured [{x: None, x: 1}]",
        "{} => 1 with self 1",
        "\"\\q\"",
        "\"raw\0nul\"",
        "\"\\x0\"",
    ] {
        assert!(parse_literal_value(text).is_err(), "accepted {text}");
    }
    let nested = format!("{}None{}", "E_TYPE(None, ".repeat(100), ")".repeat(100));
    assert!(parse_literal_value(&nested).is_err());
}

#[test]
fn rich_error_nesting_limit_is_case_insensitive() {
    for code in ["E_TYPE", "e_type", "e_TyPe", "E_tYpE", "e\"custom\""] {
        for depth in [64, 65] {
            let text = format!(
                "{}None{}",
                format!("{code}(None, ").repeat(depth),
                ")".repeat(depth)
            );
            assert_eq!(
                parse_literal_value(&text).is_ok(),
                depth == 64,
                "{code}: depth {depth}"
            );
        }
    }
}

#[test]
fn empty_frames_none_captures_and_invalid_lambda_metadata() {
    let value = parse_literal_value("{} => 7 with captured [{}, {}, {}]").unwrap();
    let loaded = parse_literal_value(&to_literal(&value)).unwrap();
    assert_eq!(
        loaded.as_lambda().unwrap().0.captured_env,
        vec![vec![], vec![], vec![]]
    );
    let value = parse_literal_value("{} => x with captured [{x: None}]").unwrap();
    let loaded = parse_literal_value(&to_literal(&value)).unwrap();
    let lambda = loaded.as_lambda().unwrap();
    let binding = lambda
        .0
        .body
        .var_names()
        .name_for_ident(Symbol::mk("x"))
        .unwrap();
    assert!(lambda.0.captured_env[binding.1 as usize][binding.0 as usize].is_none());
    let body =
        moor_compiler::compile("return 1;", moor_compiler::CompileOptions::default()).unwrap();
    let params = moor_var::program::opcode::ScatterArgs {
        labels: vec![moor_var::program::opcode::ScatterLabel::Required(
            moor_var::program::names::Name(65535, 0, 0),
        )],
        done: moor_var::program::labels::Label(0),
    };
    let value = Var::mk_lambda(params, body, vec![], None);
    let mut output = "prefix".to_owned();
    assert!(write_persistent_literal(&value, &SourceProfile::default(), &mut output).is_err());
    assert_eq!(output, "prefix");
}

#[test]
fn source_accepts_literal_keywords_as_property_names_and_quoted_slots() {
    let profile = SourceProfile::default();
    for source in [
        "return #0.none;",
        "return $none;",
        "f = 1; e = 2; inf = 3; nan = 4; return f + e + inf + nan;",
    ] {
        assert!(
            moor_compiler::read_persistent_source(source, &profile).is_ok(),
            "{source}"
        );
    }
    for slot in [
        "none", "if", "global", "false", "delegate", "slots", "α name",
    ] {
        let value = v_flyweight(
            Obj::mk_id(1),
            &[(Symbol::mk(slot), v_int(1))],
            List::mk_list(&[]),
        );
        let source = format!("return {};", to_literal(&value));
        assert!(
            moor_compiler::read_persistent_source(&source, &profile).is_ok(),
            "{source}"
        );
    }
}

fn value_strategy() -> impl proptest::strategy::Strategy<Value = Var> {
    use proptest::prelude::*;
    let scalar = prop_oneof![
        Just(v_none()),
        any::<bool>().prop_map(v_bool),
        any::<i64>().prop_map(v_int),
        any::<u64>().prop_map(|bits| v_float(f64::from_bits(bits))),
        any::<String>().prop_map(|s| v_str(&s)),
        any::<String>().prop_map(|s| v_symbol_str(Symbol::mk(&s))),
        proptest::collection::vec(any::<u8>(), 0..64).prop_map(v_binary),
        any::<i32>().prop_map(|id| v_obj(Obj::mk_id(id))),
        (any::<u16>(), any::<u8>(), any::<u64>()).prop_map(|(counter, random, time)| v_obj(
            Obj::mk_anonymous(AnonymousObjid::new(counter, random, time))
        )),
        (any::<u16>(), any::<u8>(), any::<u64>()).prop_map(|(counter, random, time)| v_obj(
            Obj::mk_uuobjid(moor_var::UuObjid::new(counter, random, time))
        )),
        Just(v_err(ErrorCode::E_TYPE)),
    ];
    scalar.prop_recursive(5, 128, 8, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..8).prop_map(|values| v_list(&values)),
            proptest::collection::vec((any::<i64>(), inner.clone()), 0..8).prop_map(|pairs| v_map(
                &pairs
                    .into_iter()
                    .map(|(key, value)| (v_int(key), value))
                    .collect::<Vec<_>>()
            )),
            (
                any::<String>(),
                proptest::option::of(any::<String>()),
                proptest::option::of(inner.clone())
            )
                .prop_map(|(code, message, value)| v_error(Error::new(
                    ErrorCode::ErrCustom(Symbol::mk(&code)),
                    message,
                    value
                ))),
            (
                proptest::collection::vec((any::<String>(), inner.clone()), 0..4),
                proptest::collection::vec(inner, 0..4)
            )
                .prop_map(|(slots, contents)| v_flyweight(
                    Obj::mk_id(1),
                    &slots
                        .into_iter()
                        .map(|(name, value)| (Symbol::mk(&name), value))
                        .collect::<Vec<_>>(),
                    List::mk_list(&contents)
                )),
        ]
    })
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn nested_values_round_trip_exactly(value in value_strategy()) {
        assert_round_trip(&value);
    }
}
