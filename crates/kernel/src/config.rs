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

//! Config is created by the host daemon, and passed through the scheduler, whereupon it is
//! available to all components. Used to hold things typically configured by CLI flags, etc.

use crate::textdump::EncodingMode;
use moor_compiler::CompileOptions;
use moor_db::DatabaseConfig;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use strum::{Display, FromRepr};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database_config: DatabaseConfig,
    pub features_config: FeaturesConfig,
    pub import_export_config: ImportExportConfig,
}

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
    /// Whether to support a Map datatype ([ k -> v, .. ]) compatible with Stunt/ToastStunt
    pub map_type: bool,
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
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            persistent_tasks: true,
            rich_notify: true,
            lexical_scopes: true,
            map_type: true,
            bool_type: true,
            symbol_type: true,
            type_dispatch: true,
            flyweight_type: true,
            list_comprehensions: true,
            use_boolean_returns: false,
            use_symbols_in_builtins: false,
            custom_errors: false,
        }
    }
}

impl FeaturesConfig {
    pub fn compile_options(&self) -> CompileOptions {
        CompileOptions {
            lexical_scopes: self.lexical_scopes,
            map_type: self.map_type,
            flyweight_type: self.flyweight_type,
            list_comprehensions: self.list_comprehensions,
            bool_type: self.bool_type,
            symbol_type: self.symbol_type,
            custom_errors: self.custom_errors,
            call_unsupported_builtins: false,
        }
    }

    /// Returns true if the configuration is backwards compatible with LambdaMOO 1.8 features
    pub fn is_lambdamoo_compatible(&self) -> bool {
        !self.lexical_scopes
            && !self.map_type
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

    /// Returns true if the configuration is compatible with another configuration, for the pur
    /// poses of textdump loading
    /// Which means that if the other configuration has a feature enabled, this configuration
    /// must also have it enabled.
    /// The other way around is fine.
    pub fn is_textdump_compatible(&self, other: &FeaturesConfig) -> bool {
        // Note that tasks/rich_notify are not included in this check, as they do not affect
        // the database format.
        (!other.lexical_scopes || self.lexical_scopes)
            && (!other.map_type || self.map_type)
            && (!other.bool_type || self.bool_type)
            && (!other.use_boolean_returns || self.use_boolean_returns)
            && (!other.type_dispatch || self.type_dispatch)
            && (!other.flyweight_type || self.flyweight_type)
            && (!other.symbol_type || self.symbol_type)
            && (!other.list_comprehensions || self.list_comprehensions)
            && (!other.custom_errors || self.custom_errors)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Eq, PartialEq)]
pub enum ImportExportFormat {
    /// The legacy LambdaMOO textdump format.
    #[default]
    Textdump,
    /// The new-style directory based objectdef format.
    Objdef,
}

/// Configuration for the import/export of textdumps or objdefs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportExportConfig {
    /// Where to read the initial import from, if any.
    pub input_path: Option<PathBuf>,
    /// Directory to write periodic export/backups of the database, if any.
    pub output_path: Option<PathBuf>,
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
    /// Which format to use for import.
    pub import_format: ImportExportFormat,
    /// Which format to use for export.
    pub export_format: ImportExportFormat,
}

impl Default for ImportExportConfig {
    fn default() -> Self {
        Self {
            input_path: None,
            output_path: None,
            output_encoding: EncodingMode::UTF8,
            checkpoint_interval: Some(Duration::from_secs(60)),
            version_override: None,
            import_format: ImportExportFormat::Textdump,
            export_format: ImportExportFormat::Textdump,
        }
    }
}

impl ImportExportConfig {
    pub fn version_string(
        &self,
        moor_version: &Version,
        features_config: &FeaturesConfig,
    ) -> String {
        //    //      Moor 0.1.0, features: "flyweight_type=true lexical_scopes=true map_type=true", encoding: UTF8
        self.version_override.clone().unwrap_or_else(|| {
            let tv = TextdumpVersion::Moor(
                moor_version.clone(),
                features_config.clone(),
                self.output_encoding,
            );
            tv.to_version_string()
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TextdumpVersion {
    LambdaMOO(LambdaMOODBVersion),
    ToastStunt(ToastStuntDBVersion),
    Moor(Version, FeaturesConfig, EncodingMode),
}

/// Versions corresponding to ToastStunt's version.h
#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Display, Ord, PartialOrd, Copy, Clone, FromRepr)]
pub enum LambdaMOODBVersion {
    DbvPrehistory = 0, // Before format versions
    DbvExceptions = 1, // Addition of the `try', `except', `finally', and `endtry' keywords.
    DbvBreakCont = 2,  // Addition of the `break' and `continue' keywords.
    DbvFloat = 3, // Addition of `FLOAT' and `INT' variables and the `E_FLOAT' keyword, along with version numbers on each frame of a suspended task.
    DbvBfbugFixed = 4, // Bug in built-in function overrides fixed by making it use tail-calling. This DB_Version change exists solely to turn off special bug handling in read_bi_func_data().
}

#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Display, Ord, PartialOrd, Copy, Clone, FromRepr)]
pub enum ToastStuntDBVersion {
    ToastDbvNextGen = 5, // Introduced the next-generation database format which fixes the data locality problems in the v4 format.
    ToastDbvTaskLocal = 6, // Addition of task local value.
    ToastDbvMap = 7,     // Addition of `MAP' variables
    ToastDbvFileIo = 8,  // Includes addition of the 'E_FILE' keyword.
    ToastDbvExec = 9,    // Includes addition of the 'E_EXEC' keyword.
    ToastDbvInterrupt = 10, // Includes addition of the 'E_INTRPT' keyword.
    ToastDbvThis = 11,   // Varification of `this'.
    ToastDbvIter = 12,   // Addition of map iterator
    ToastDbvAnon = 13,   // Addition of anonymous objects
    ToastDbvWaif = 14,   // Addition of waifs
    ToastDbvLastMove = 15, // Addition of the 'last_move' built-in property
    ToastDbvThreaded = 16, // Store threading information
    ToastDbvBool = 17,   // Boolean type
}

impl TextdumpVersion {
    pub fn parse(s: &str) -> Option<TextdumpVersion> {
        if s.starts_with("** LambdaMOO Database, Format Version ") {
            let version = s
                .trim_start_matches("** LambdaMOO Database, Format Version ")
                .trim_end_matches(" **");
            let version = version.parse::<u16>().ok()?;
            // For now anything over 4 is assumed to be ToastStunt
            if version > 4 {
                return Some(TextdumpVersion::ToastStunt(ToastStuntDBVersion::from_repr(
                    version,
                )?));
            } else {
                return Some(TextdumpVersion::LambdaMOO(LambdaMOODBVersion::from_repr(
                    version,
                )?));
            }
        } else if s.starts_with("Moor ") {
            let parts = s.split(", ").collect::<Vec<_>>();
            let version = parts.iter().find(|s| s.starts_with("Moor "))?;
            let version = version.trim_start_matches("Moor ");
            // "Moor 0.1.0, features: "flyweight_type=true lexical_scopes=true map_type=true", encoding: UTF8"
            let semver = version.split(' ').next()?;
            let semver = semver::Version::parse(semver).ok()?;
            let features = parts.iter().find(|s| s.starts_with("features: "))?;
            let features = features
                .trim_start_matches("features: \"")
                .trim_end_matches("\"");
            let features = features.split(' ').collect::<Vec<_>>();
            let features = FeaturesConfig {
                flyweight_type: features.iter().any(|s| s == &"flyweight_type=true"),
                lexical_scopes: features.iter().any(|s| s == &"lexical_scopes=true"),
                map_type: features.iter().any(|s| s == &"map_type=true"),
                ..Default::default()
            };
            let encoding = parts.iter().find(|s| s.starts_with("encoding: "))?;
            let encoding = encoding.trim_start_matches("encoding: ");
            let encoding = EncodingMode::try_from(encoding).ok()?;
            return Some(TextdumpVersion::Moor(semver, features, encoding));
        }
        None
    }

    pub fn to_version_string(&self) -> String {
        match self {
            TextdumpVersion::LambdaMOO(v) => {
                format!("** LambdaMOO Database, Format Version {} **", v)
            }
            TextdumpVersion::ToastStunt(v) => {
                unimplemented!("ToastStunt dump format ({v}) not supported for output");
            }
            TextdumpVersion::Moor(v, features, encoding) => {
                let features = format!(
                    "flyweight_type={} lexical_scopes={} map_type={}",
                    features.flyweight_type, features.lexical_scopes, features.map_type
                );
                format!(
                    "Moor {}, features: \"{}\", encoding: {:?}",
                    v, features, encoding
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{LambdaMOODBVersion, TextdumpVersion};

    #[test]
    fn parse_textdump_version_lambda() {
        let version = super::TextdumpVersion::parse("** LambdaMOO Database, Format Version 4 **");
        assert_eq!(
            version,
            Some(super::TextdumpVersion::LambdaMOO(
                LambdaMOODBVersion::DbvBfbugFixed
            ))
        );
    }

    #[test]
    fn parse_textdump_version_moor() {
        let td = TextdumpVersion::Moor(
            semver::Version::parse("0.1.0").unwrap(),
            super::FeaturesConfig {
                flyweight_type: true,
                lexical_scopes: true,
                map_type: true,
                ..Default::default()
            },
            super::EncodingMode::UTF8,
        );
        let version = td.to_version_string();
        let parsed = TextdumpVersion::parse(&version);
        assert_eq!(parsed, Some(td));
    }
}
