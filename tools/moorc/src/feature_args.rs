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

use clap_derive::Parser;
use moor_kernel::config::FeaturesConfig;

#[derive(Parser, Debug)]
pub struct FeatureArgs {
    /// Whether to allow notify() to send arbitrary MOO common to players. The interpretation of
    /// the common varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    #[arg(
        long,
        help = "Enable rich_notify, allowing notify() to send arbitrary MOO common to players. \
                The interpretation of the common varies depending on host/client. \
                If this is false, only strings are allowed, as in LambdaMOO."
    )]
    pub rich_notify: Option<bool>,

    #[arg(
        long,
        help = "Enable block-level lexical scoping in programs. \
                Adds the `begin`/`end` syntax for creating lexical scopes, and `let` and `global`
                for declaring variables. \
                This is a feature that is not present in LambdaMOO, so if you need backwards compatibility, turn this off."
    )]
    pub lexical_scopes: Option<bool>,

    #[arg(
        long,
        help = "Enable primitive-type verb dispatching. E.g. \"test\":reverse() becomes $string:reverse(\"test\")"
    )]
    pub type_dispatch: Option<bool>,

    #[arg(
        long,
        help = "Enable flyweight types. Flyweights are a lightweight, object delegate"
    )]
    pub flyweight_type: Option<bool>,

    #[arg(long, help = "Enable boolean true/false literals and a boolean type")]
    pub bool_type: Option<bool>,

    #[arg(
        long,
        help = "Whether to have builtins that return truth values return boolean types instead of integer 1 or 0. Same goes for binary value operators like <, !, ==, <= etc."
    )]
    pub use_boolean_returns: Option<bool>,

    #[arg(long, help = "Enable 'symbol literals")]
    pub symbol_type: Option<bool>,

    #[arg(
        long,
        help = "Enable error symbols beyond the standard builtin set, with no integer conversions for them."
    )]
    pub custom_errors: Option<bool>,

    #[arg(
        long,
        help = "Whether to have certain builtins use or return symbols instead of strings for things like property names, etc."
    )]
    pub use_symbols_in_builtins: Option<bool>,

    #[arg(
        long,
        help = "Enable support for list / range comprehensions in the language"
    )]
    pub list_comprehensions: Option<bool>,

    #[arg(
        long,
        help = "Enable persistent tasks, which persist the state of suspended/forked tasks between restarts. \
                Note that this is the default behaviour in LambdaMOO."
    )]
    pub persistent_tasks: Option<bool>,

    #[arg(
        long,
        help = "Create objects using uuobjids (UUID-based object IDs) instead of objids (integer-based object IDs). \
                This provides better uniqueness guarantees and avoids integer overflow issues."
    )]
    pub use_uuobjids: Option<bool>,

    #[arg(
        long,
        help = "Enable anonymous objects, which are garbage-collected when no longer referenced."
    )]
    pub anonymous_objects: Option<bool>,
}

impl FeatureArgs {
    pub fn merge_config(&self, config: &mut FeaturesConfig) -> Result<(), eyre::Report> {
        if let Some(args) = self.rich_notify {
            config.rich_notify = args;
        }
        if let Some(args) = self.lexical_scopes {
            config.lexical_scopes = args;
        }
        if let Some(args) = self.type_dispatch {
            config.type_dispatch = args;
        }
        if let Some(args) = self.flyweight_type {
            config.flyweight_type = args;
        }
        if let Some(args) = self.bool_type {
            config.bool_type = args;
        }
        if let Some(args) = self.use_boolean_returns {
            config.use_boolean_returns = args;
        }
        if let Some(args) = self.custom_errors {
            config.custom_errors = args;
        }
        if let Some(args) = self.symbol_type {
            config.symbol_type = args;
        }
        if let Some(args) = self.use_symbols_in_builtins {
            config.use_symbols_in_builtins = args;
        }
        if let Some(args) = self.persistent_tasks {
            config.persistent_tasks = args;
        }
        if let Some(args) = self.list_comprehensions {
            config.list_comprehensions = args;
        }
        if let Some(args) = self.use_uuobjids {
            config.use_uuobjids = args;
        }
        if let Some(args) = self.anonymous_objects {
            config.anonymous_objects = args;
        }
        Ok(())
    }
}
