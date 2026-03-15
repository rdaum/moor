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

use moor_compiler::CompileOptions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FeaturesConfig {
    /// Whether to host a tasks DB and persist the state of suspended/forked tasks between restarts.
    /// Note that this is the default behaviour in LambdaMOO.
    pub persistent_tasks: bool,
    /// Whether to allow notify() to send arbitrary MOO common to players. The interpretation of
    /// the common varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    pub rich_notify: bool,
    /// Whether to support block-level lexical scoping, and the 'begin', 'let' and 'global'
    /// keywords.
    pub lexical_scopes: bool,
    /// Whether to support primitive-type verb dispatching. E.g. "test":reverse() becomes
    ///   $string:reverse("test")
    pub type_dispatch: bool,
    /// Whether to support flyweight types. Flyweights are a lightweight, non-persistent thingy
    pub flyweight_type: bool,
    /// Whether to support list/range comprehensions in the language
    pub list_comprehensions: bool,
    /// Whether to support a boolean literal type in the compiler
    pub bool_type: bool,
    /// Whether to have builtins that return truth values return boolean types instead of integer
    /// 1 or 0. Same goes for binary value operators like <, !, ==, <= etc.
    ///
    /// This can break backwards compatibility with existing cores, so is off by default.
    pub use_boolean_returns: bool,
    /// Whether to support any arbitrary "custom" errors beyond the builtin set.
    /// These errors cannot be converted to/from integers, and using them in existing cores can
    /// cause problems.  Example  `return E_EXAMPLE;`
    pub custom_errors: bool,
    /// Whether to support a symbol literal type in the compiler
    pub symbol_type: bool,
    /// Whether to have certain builtins use or return symbols instead of strings for things like property
    /// names, etc.
    ///
    /// This can break backwards compatibility with existing cores, so is off by default.
    pub use_symbols_in_builtins: bool,
    /// Whether to create objects using uuobjids (UUID-based object IDs) instead of objids (integer-based object IDs).
    /// This provides better uniqueness guarantees and avoids integer overflow issues.
    pub use_uuobjids: bool,
    /// Whether to enable persistent event logging. When disabled, events are not persisted to disk
    /// and history features are unavailable.
    pub enable_eventlog: bool,
    /// Whether to enable anonymous objects. Anonymous objects are lightweight objects that can be
    /// created without assigned object IDs. Requires garbage collection to clean up unreferenced objects.
    /// Defaults to false due to GC overhead.
    pub anonymous_objects: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            persistent_tasks: true,
            rich_notify: true,
            lexical_scopes: true,
            bool_type: true,
            symbol_type: true,
            type_dispatch: true,
            flyweight_type: true,
            list_comprehensions: true,
            use_boolean_returns: false,
            use_symbols_in_builtins: false,
            custom_errors: false,
            use_uuobjids: false,
            enable_eventlog: false,
            anonymous_objects: false,
        }
    }
}

impl FeaturesConfig {
    pub fn compile_options(&self) -> CompileOptions {
        CompileOptions {
            lexical_scopes: self.lexical_scopes,
            flyweight_type: self.flyweight_type,
            list_comprehensions: self.list_comprehensions,
            bool_type: self.bool_type,
            symbol_type: self.symbol_type,
            custom_errors: self.custom_errors,
            call_unsupported_builtins: false,
            legacy_type_constants: false,
        }
    }

    /// Returns true if the configuration is backwards compatible with LambdaMOO 1.8 features
    pub fn is_lambdamoo_compatible(&self) -> bool {
        !self.lexical_scopes
            && !self.type_dispatch
            && !self.flyweight_type
            && !self.rich_notify
            && !self.bool_type
            && !self.list_comprehensions
            && !self.use_boolean_returns
            && !self.symbol_type
            && !self.custom_errors
            && self.persistent_tasks
    }
}
