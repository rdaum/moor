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
use common::{create_db, eval, AssertEval, AssertRunAsVerb, WIZARD};
use moor_values::var::{v_empty_list, v_none, Var, Variant};

#[test]
fn test_changing_programmer_and_wizard_flags() {
    let db = create_db();

    // Create an object we can work with
    let obj = eval(db.clone(), WIZARD, "return create(#2);").unwrap();

    // Start: it's neither a programmer nor a wizard
    db.assert_eval(
        WIZARD,
        format!("return {{ {obj}.programmer, {obj}.wizard }};"),
        [0, 0],
    );

    // Set both, verify
    db.assert_eval(
        WIZARD,
        format!("{obj}.programmer = 1; {obj}.wizard = 1;"),
        v_none(),
    );
    db.assert_eval(
        WIZARD,
        format!("return {{ {obj}.programmer, {obj}.wizard }};"),
        [1, 1],
    );

    // Clear both, verify
    db.assert_eval(
        WIZARD,
        format!("{obj}.programmer = 0; {obj}.wizard = 0;"),
        v_none(),
    );
    db.assert_eval(
        WIZARD,
        format!("return {{ {obj}.programmer, {obj}.wizard }};"),
        [0, 0],
    );
}

#[test]
fn test_testhelper_verb_redefinition() {
    let db = create_db();
    db.assert_run_as_verb("return 42;", 42);
    db.assert_run_as_verb("return create(#2).name;", "");
    db.assert_run_as_verb("return 200;", 200);
}

#[test]
#[ignore = "Currently broken"]
fn test_properties_does_not_list_parent_props() {
    let db = create_db();

    let obj_var = eval(db.clone(), WIZARD, "return create(#2);").unwrap();
    let objid = match obj_var.variant() {
        Variant::Obj(objid) => objid,
        _ => panic!("Expected an object"),
    };

    db.assert_eval(
        WIZARD,
        r#"add_property(#2, "prop1", 0, {player, "rwc" });"#,
        v_none(),
    );
    db.assert_eval(WIZARD, "#2.prop1 = 1;;", v_none());
    db.assert_eval(WIZARD, "return properties(#2);", vec![Var::from("prop1")]);
    db.assert_eval(
        WIZARD,
        format!("return properties({objid});"),
        v_empty_list(),
    );
}
