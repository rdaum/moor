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

//! Conformance corpus for the persistence-oriented MOO literal codec.
//!
//! The codec contract is defined in `pluggable-persistence-and-postgresql.md` §8 and §15.2. The
//! corpus is established now, before the codec is implemented, so the round-trip target is
//! executable rather than prose. Cases that the current objdef formatter cannot represent are
//! marked `#[ignore]` with the §8 item that must fix them; the fallible persistence codec (§16
//! step 4) un-ignores them.
//!
//! This file deliberately exercises only `to_literal`/`parse_literal_value`. Lambdas and
//! program source are covered by compiler unit tests when the source codec lands.

use moor_compiler::{parse_literal_value, to_literal};
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

/// Render with the current formatter and parse the result back.
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

/// §8.2: generic formatting emits `None`, but the objdef literal parser has no corresponding
/// keyword case and resolves it as a missing constant. `None` must become a built-in literal,
/// including inside captures and containers.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
fn none_round_trips() {
    assert_round_trip(&v_none());
    assert_round_trip(&v_list(&[v_none()]));
    assert_round_trip(&v_map(&[(v_bool(true), v_list(&[v_none()]))]));
}

/// §8.2: ordinary object display hides anonymous identity, so anonymous objects need an explicit
/// identity-preserving literal spelling. §8.3: the value table requires complete identity.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
fn anonymous_objects_round_trip() {
    let anonymous = Obj::mk_anonymous(AnonymousObjid::generate(1));
    assert_round_trip(&v_obj(anonymous));
    assert_round_trip(&v_list(&[v_obj(anonymous)]));
}

/// §8.2: the flyweight formatter resolves the delegate through `to_literal()`, which omits `#`
/// on UUID-style objects and hides anonymous identity.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
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
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
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

/// §8.2/§8.3: the formatter emits the error name and message but not the attached value.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
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
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
fn nul_uses_canonical_hex_escape() {
    let text = to_literal(&v_str("a\0b"));
    assert!(text.contains("\\x00"), "expected \\x00 in {text:?}");
    assert!(!text.contains("\\0"), "unexpected \\0 in {text:?}");
}

/// §8.3: ordinary Unicode remains UTF-8; hex escapes cover C0 controls and DEL.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
fn unicode_is_emitted_as_utf8() {
    let text = to_literal(&v_str("\u{3b1}\u{1f404}"));
    assert!(text.contains('\u{3b1}'), "expected raw UTF-8 in {text:?}");
    assert!(text.contains('\u{1f404}'), "expected raw UTF-8 in {text:?}");
}

/// §8.3: the lexer recognizes `\U` escapes but the unquoter has no eight-digit decode branch.
/// The codec must reject unsupported escapes instead of silently passing them through.
#[test]
#[ignore = "pending the fallible persistence codec (§8); un-ignore in §16 step 4"]
fn unsupported_unicode_escape_is_rejected() {
    assert!(parse_literal_value("\"\\U0001F404\"").is_err());
}
