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
use common::{create_db, AssertEval, WIZARD};
use moor_values::{
    var::{
        v_empty_list, v_empty_str, v_none,
        Error::{E_ARGS, E_PERM, E_TYPE},
    },
    NOTHING,
};

#[test]
fn test_that_eval_cannot_be_called_by_non_programmers() {
    let db = create_db();
    db.assert_eval(WIZARD, "player.programmer = 0;", v_none());
    db.assert_eval(WIZARD, "return 5;", E_PERM);
}

#[test]
fn test_bf_eval_cannot_be_called_by_non_programmers() {
    let db = create_db();
    db.assert_eval_exception(
        WIZARD,
        r#"player.programmer = 0; return eval("return 5;");"#,
        E_PERM,
    );
}

#[test]
fn test_that_bf_eval_requires_at_least_one_argument() {
    let db = create_db();
    db.assert_eval_exception(WIZARD, "return eval();", E_ARGS);
}

#[test]
fn test_that_eval_requires_string_arguments() {
    let db = create_db();
    db.assert_eval_exception(WIZARD, "return eval(1);", E_TYPE);
    // TODO uncomment when `eval()` gets support for multiple arguments
    // db.assert_eval_exception(WIZARD, "return eval(1, 2);", E_TYPE);
    db.assert_eval_exception(WIZARD, "return eval({});", E_TYPE);
}

#[test]
#[ignore = "We don't currently support multiple args to eval()"]
fn test_that_eval_evaluates_multiple_strings() {
    let db = create_db();
    db.assert_eval(
        WIZARD,
        r#"return eval("x = 0;", "for i in [1..5]", "x = x + i;", "endfor", "return x;");"#,
        [1, 15],
    );
}

#[test]
fn test_that_eval_evaluates_a_single_string() {
    let db = create_db();
    db.assert_eval(WIZARD, r#"return eval("return 5;");"#, [1, 5]);
}

#[test]
fn test_eval_builtin_variables() {
    // As seen on https://stunt.io/ProgrammersManual.html#Language
    let db = create_db();
    db.assert_eval(WIZARD, "return player;", WIZARD);
    db.assert_eval(WIZARD, "return this;", NOTHING);
    db.assert_eval(WIZARD, "return caller;", WIZARD);
    db.assert_eval(WIZARD, "return args;", v_empty_list());
    db.assert_eval(WIZARD, "return argstr;", v_empty_str());
    db.assert_eval(WIZARD, "return verb;", v_empty_str());
    db.assert_eval(WIZARD, "return dobjstr;", v_empty_str());
    db.assert_eval(WIZARD, "return dobj;", NOTHING);
    db.assert_eval(WIZARD, "return prepstr;", v_empty_str());
    db.assert_eval(WIZARD, "return iobjstr;", v_empty_str());
    db.assert_eval(WIZARD, "return iobj;", NOTHING);
}
