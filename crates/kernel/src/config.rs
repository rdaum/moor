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

    /// Returns true if the configuration is backwards compatible with LambdaMOO 1.8 features.
    pub fn is_lambdammoo_compatible(&self) -> bool {
        !self.lexical_scopes && !self.map_type && !self.type_dispatch && !self.flyweight_type
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextdumpConfig {
    /// Where to read the initial textdump from, if any.
    pub input_path: Option<PathBuf>,
    /// Where to write periodic textdumps of the database, if any.
    pub output_path: Option<PathBuf>,
    /// What encoding to use for reading textdumps (ISO-8859-1 or UTF-8).
    pub input_encoding: EncodingMode,
    /// What encoding to use for writing textdumps (ISO-8859-1 or UTF-8).
    pub output_encoding: EncodingMode,
    /// Interval between database checkpoints.
    /// If None, no checkpoints will be made.
    pub checkpoint_interval: Option<Duration>,
    /// Version override string to put into the textdump.
    /// If None, the moor version + a serialization of the features config is used + the encoding.
    /// If set, this string will be used instead.
    /// This is useful for producing textdumps that are compatible with other servers, but be
    /// careful to not lie about the features (and encoding) you support.
    pub version_override: Option<String>,
}

impl Default for TextdumpConfig {
    fn default() -> Self {
        Self {
            input_path: None,
            output_path: None,
            input_encoding: EncodingMode::UTF8,
            output_encoding: EncodingMode::UTF8,
            checkpoint_interval: Some(Duration::from_secs(60)),
            version_override: None,
        }
    }
}

impl TextdumpConfig {
    pub fn version_string(&self, moor_version: &str, features_config: &FeaturesConfig) -> String {
        self.version_override.clone().unwrap_or_else(|| {
            // Set of features enabled:
            //   flyweight_type=yes/no, lexical_scopes=yes/no, map_type=yes/no, etc.
            let features_string = format!(
                "flyweight_type={}, lexical_scopes={}, map_type={}",
                features_config.flyweight_type,
                features_config.lexical_scopes,
                features_config.map_type
            );

            format!(
                "Moor {} (features: {:?}, encoding: {:?})",
                moor_version, features_string, self.output_encoding
            )
        })
    }
}
