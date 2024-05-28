// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

mod common;
use common::{create_db, eval, WIZARD};
use moor_values::{
    var::{
        v_empty_list, v_empty_str,
        Error::{E_ARGS, E_PERM, E_TYPE},
    },
    NOTHING,
};
use pretty_assertions::assert_eq;

#[test]
fn test_that_eval_cannot_be_called_by_non_programmers() {
    let db = create_db();
    eval(db.clone(), WIZARD, "player.programmer = 0;");
    assert_eq!(eval(db.clone(), WIZARD, r#"return 5;"#), E_PERM.into());
}

#[test]
fn test_bf_eval_cannot_be_called_by_non_programmers() {
    let db = create_db();
    assert_eq!(
        eval(
            db.clone(),
            WIZARD,
            r#"player.programmer = 0; return eval("return 5;");"#
        )
        .unwrap_err()
        .code,
        E_PERM
    );
}

#[test]
fn test_that_bf_eval_requires_at_least_one_argument() {
    let db = create_db();
    assert_eq!(eval(db, WIZARD, "return eval();").unwrap_err().code, E_ARGS);
}

#[test]
fn test_that_eval_requires_string_arguments() {
    let db = create_db();
    assert_eq!(
        eval(db.clone(), WIZARD, "return eval(1);")
            .unwrap_err()
            .code,
        E_TYPE
    );
    // TODO uncomment when `eval()` gets support for multiple arguments
    // assert_eq!(eval(db, WIZARD, "return eval(1, 2);"), E_ARGS.into());
    assert_eq!(
        eval(db, WIZARD, "return eval({});").unwrap_err().code,
        E_TYPE
    );
}

#[test]
#[ignore = "We don't currently support multiple args to eval()"]
fn test_that_eval_evaluates_multiple_strings() {
    let db = create_db();
    assert_eq!(
        eval(
            db,
            WIZARD,
            r#"return eval("x = 0;", "for i in [1..5]", "x = x + i;", "endfor", "return x;");"#
        ),
        [1, 15].into()
    );
}

#[test]
fn test_that_eval_evaluates_a_single_string() {
    let db = create_db();
    assert_eq!(
        eval(db, WIZARD, r#"return eval("return 5;");"#),
        [1, 5].into()
    );
}

#[test]
fn test_eval_builtin_variables() {
    // As seen on https://stunt.io/ProgrammersManual.html#Language
    let db = create_db();
    assert_eq!(eval(db.clone(), WIZARD, "return player;"), WIZARD.into());
    assert_eq!(eval(db.clone(), WIZARD, "return this;"), NOTHING.into());
    assert_eq!(eval(db.clone(), WIZARD, "return caller;"), WIZARD.into());
    assert_eq!(eval(db.clone(), WIZARD, "return args;"), v_empty_list());
    assert_eq!(eval(db.clone(), WIZARD, "return argstr;"), v_empty_str());
    assert_eq!(eval(db.clone(), WIZARD, "return verb;"), v_empty_str());
    assert_eq!(eval(db.clone(), WIZARD, "return dobjstr;"), v_empty_str());
    assert_eq!(eval(db.clone(), WIZARD, "return dobj;"), NOTHING.into());
    assert_eq!(eval(db.clone(), WIZARD, "return prepstr;"), v_empty_str());
    assert_eq!(eval(db.clone(), WIZARD, "return iobjstr;"), v_empty_str());
    assert_eq!(eval(db.clone(), WIZARD, "return iobj;"), NOTHING.into());
}
