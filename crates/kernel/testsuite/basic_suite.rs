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
use common::{create_db, eval, testsuite_dir, WIZARD};
use pretty_assertions::assert_eq;

fn run_basic_test(test_dir: &str) {
    let abs_test_dir = testsuite_dir().join("basic").join(test_dir);

    let test_in = abs_test_dir.join("test.in");
    let test_out = abs_test_dir.join("test.out");

    // Read the lines from both files, the first is an input expression, the second the
    // expected output. Both as MOO expressions. # of lines must be identical in each.
    let input = std::fs::read_to_string(test_in).unwrap();
    let in_lines = input.lines();
    let output = std::fs::read_to_string(test_out).unwrap();
    let out_lines = output.lines();
    assert_eq!(in_lines.clone().count(), out_lines.clone().count());

    let db = create_db();

    // Zip
    let zipped = in_lines.zip(out_lines);
    for (line_num, (input, expected_str)) in zipped.enumerate() {
        let actual = eval(db.clone(), WIZARD, &format!("return {};", input.trim()));
        let expected = eval(
            db.clone(),
            WIZARD,
            &format!("return {};", expected_str.trim()),
        );
        assert_eq!(actual, expected, "{test_dir}: line {line_num}: {input}")
    }
}

fn main() {}
#[test]
fn basic_arithmetic() {
    run_basic_test("arithmetic");
}

#[test]
fn basic_value() {
    run_basic_test("value");
}

#[test]
fn basic_string() {
    run_basic_test("string");
}

#[test]
fn basic_list() {
    run_basic_test("list");
}

#[test]
fn basic_property() {
    run_basic_test("property");
}

#[test]
fn basic_object() {
    run_basic_test("object");
}
