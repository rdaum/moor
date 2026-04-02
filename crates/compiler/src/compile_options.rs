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

/// Options controlling MOO compilation behavior.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompileOptions {
    /// Whether we allow lexical scope blocks. begin/end blocks and 'let' and 'global' statements.
    pub lexical_scopes: bool,
    /// Whether to support the flyweight type (a delegate object with slots and contents).
    pub flyweight_type: bool,
    /// Whether to support list and range comprehensions in the compiler.
    pub list_comprehensions: bool,
    /// Whether to support boolean types in compilation.
    pub bool_type: bool,
    /// Whether to support symbol types ('sym) in compilation.
    pub symbol_type: bool,
    /// Whether to support non-standard custom error values.
    pub custom_errors: bool,
    /// Whether to turn unsupported builtins into `call_function` invocations.
    /// Useful for textdump imports from other MOO dialects.
    pub call_unsupported_builtins: bool,
    /// Whether to parse legacy type constant names (INT, OBJ, STR, etc.) as type literals.
    /// When false (default), these become valid variable identifiers.
    /// When true (textdump import mode), these are parsed as type literals.
    /// Note: The new TYPE_* forms (TYPE_INT, TYPE_OBJ, etc.) are always recognized.
    pub legacy_type_constants: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            lexical_scopes: true,
            flyweight_type: true,
            list_comprehensions: true,
            bool_type: true,
            symbol_type: true,
            custom_errors: true,
            call_unsupported_builtins: false,
            legacy_type_constants: false,
        }
    }
}
