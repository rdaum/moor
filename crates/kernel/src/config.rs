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

//! Config is created by the host daemon, and passed through the scheduler, whereupon it is
//! available to all components. Used to hold things typically configured by CLI flags, etc.

use crate::textdump::EncodingMode;
use moor_compiler::CompileOptions;
use moor_db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database_config: DatabaseConfig,
    pub features_config: FeaturesConfig,
    pub textdump_config: TextdumpConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeaturesConfig {
    /// Whether to allow notify() to send arbitrary MOO common to players. The interpretation of
    /// the common varies depending on host/client.
    /// If this is false, only strings are allowed, as in LambdaMOO.
    pub rich_notify: bool,
    /// Whether to support block-level lexical scoping, and the 'begin', 'let' and 'global'
    /// keywords.
    pub lexical_scopes: bool,
    /// Whether to support a Map datatype ([ k -> v, .. ]) compatible with Stunt/ToastStunt
    pub map_type: bool,
    /// Whether to support primitive-type verb dispatching. E.g. "test":reverse() becomes
    ///   $string:reverse("test")
    pub type_dispatch: bool,
    /// Whether to support flyweight types. Flyweights are a lightweight, non-persistent thingy
    pub flyweight_type: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            rich_notify: true,
            lexical_scopes: true,
            map_type: true,
            type_dispatch: true,
            flyweight_type: true,
        }
    }
}

impl FeaturesConfig {
    pub fn compile_options(&self) -> CompileOptions {
        CompileOptions {
            lexical_scopes: self.lexical_scopes,
            map_type: self.map_type,
            flyweight_type: self.flyweight_type,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TextdumpConfig {
    /// Where to read the initial textdump from, if any.
    pub input_path: Option<PathBuf>,
    /// Where to write periodic textdumps of the database, if any.
    pub output_path: Option<PathBuf>,
    /// What encoding to use for textdumps (ISO-8859-1 or UTF-8).
    pub encoding: EncodingMode,
    /// Interval between database checkpoints.
    /// If None, no checkpoints will be made.
    pub checkpoint_interval: Option<Duration>,
}
