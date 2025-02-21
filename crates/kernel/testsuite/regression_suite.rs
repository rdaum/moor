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

use crate::common::AssertRunAsVerb;
use crate::common::create_db;

mod common;

#[test]
fn test_testhelper_verb_redefinition() {
    let db = create_db();
    db.assert_run_as_verb("return 42;", Ok(42.into()));
    db.assert_run_as_verb("return create(#2).name;", Ok("".into()));
    db.assert_run_as_verb("return 200;", Ok(200.into()));
}
