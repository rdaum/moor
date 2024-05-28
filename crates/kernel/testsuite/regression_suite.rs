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
use pretty_assertions::assert_eq;

#[test]
fn test_changing_programmer_and_wizard_flags() {
    let db = create_db();

    // Create an object we can work with
    let obj = eval(db.clone(), WIZARD, "return create(#2);");

    // Start: it's neither a programmer nor a wizard
    assert_eq!(
        eval(
            db.clone(),
            WIZARD,
            &format!("return {{ {obj}.programmer, {obj}.wizard }};")
        ),
        [0, 0].into()
    );

    // Set both, verify
    eval(
        db.clone(),
        WIZARD,
        &format!("{obj}.programmer = 1; {obj}.wizard = 1;"),
    );
    assert_eq!(
        eval(
            db.clone(),
            WIZARD,
            &format!("return {{ {obj}.programmer, {obj}.wizard }};")
        ),
        [1, 1].into()
    );

    // Clear both, verify
    eval(
        db.clone(),
        WIZARD,
        &format!("{obj}.programmer = 0; {obj}.wizard = 0;"),
    );
    assert_eq!(
        eval(
            db.clone(),
            WIZARD,
            &format!("return {{ {obj}.programmer, {obj}.wizard }};")
        ),
        [0, 0].into()
    );
}
