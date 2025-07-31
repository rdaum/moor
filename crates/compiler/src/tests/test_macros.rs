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

//! General-purpose test macros for parser testing

/// Assert that a parse operation succeeds
#[macro_export]
macro_rules! assert_parse_ok {
    ($code:expr) => {
        match $crate::compile($code, $crate::CompileOptions::default()) {
            Ok(_) => {}
            Err(e) => panic!("Parse failed for code '{}': {:?}", $code, e),
        }
    };
    ($code:expr, $msg:expr) => {
        match $crate::compile($code, $crate::CompileOptions::default()) {
            Ok(_) => {}
            Err(e) => panic!("{}: Parse failed for code '{}': {:?}", $msg, $code, e),
        }
    };
}

/// Assert that a parse operation fails
#[macro_export]
macro_rules! assert_parse_fails {
    ($code:expr) => {
        match $crate::compile($code, $crate::CompileOptions::default()) {
            Ok(_) => panic!("Parse unexpectedly succeeded for code '{}'", $code),
            Err(_) => {}
        }
    };
    ($code:expr, $msg:expr) => {
        match $crate::compile($code, $crate::CompileOptions::default()) {
            Ok(_) => panic!(
                "{}: Parse unexpectedly succeeded for code '{}'",
                $msg, $code
            ),
            Err(_) => {}
        }
    };
}

/// Assert that parsing produces a specific error
#[macro_export]
macro_rules! assert_parse_error {
    ($code:expr, $expected_error:pat) => {
        match $crate::compile($code, $crate::CompileOptions::default()) {
            Ok(_) => panic!("Parse unexpectedly succeeded for code '{}'", $code),
            Err(e) => match e {
                $expected_error => {}
                _ => panic!(
                    "Expected error pattern {} but got {:?}",
                    stringify!($expected_error),
                    e
                ),
            },
        }
    };
}

/// Assert that two pieces of code compile to the same bytecode
#[macro_export]
macro_rules! assert_compiles_same {
    ($code1:expr, $code2:expr) => {
        let result1 = $crate::compile($code1, $crate::CompileOptions::default());
        let result2 = $crate::compile($code2, $crate::CompileOptions::default());
        match (result1, result2) {
            (Ok(prog1), Ok(prog2)) => {
                assert_eq!(
                    prog1.main_vector().to_vec(),
                    prog2.main_vector().to_vec(),
                    "Programs compiled differently:\nCode 1: {}\nCode 2: {}",
                    $code1,
                    $code2
                );
            }
            (Err(e1), _) => panic!("First program failed to compile: {:?}", e1),
            (_, Err(e2)) => panic!("Second program failed to compile: {:?}", e2),
        }
    };
}
