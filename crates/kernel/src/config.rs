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

use moor_compiler::CompileOptions;
use moor_db::DatabaseConfig;
use moor_textdump::{EncodingMode, TextdumpVersion};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database: Option<DatabaseConfig>,
    pub features: Arc<FeaturesConfig>,
    pub import_export: ImportExportConfig,
    pub runtime: RuntimeConfig,
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
    /// Whether to create objects using uuobjids (UUID-based object IDs) instead of objids (integer-based object IDs).
    /// This provides better uniqueness guarantees and avoids integer overflow issues.
    pub use_uuobjids: bool,
    /// Whether to enable persistent event logging. When disabled, events are not persisted to disk
    /// and history features are unavailable.
    pub enable_eventlog: bool,
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
            use_uuobjids: false,
            enable_eventlog: true,
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
}

/// Configuration for runtime/scheduler behavior
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    /// Interval between automatic garbage collection cycles.
    /// If None, automatic GC uses database settings or default.
    #[serde(deserialize_with = "parse_duration")]
    pub gc_interval: Option<Duration>,
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
    #[serde(deserialize_with = "parse_duration")]
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
            checkpoint_interval: None,
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
                features_config.compile_options(),
                self.output_encoding,
            );
            tv.to_version_string()
        })
    }
}

// Use humantime to parse durations from strings
fn parse_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => humantime::parse_duration(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}
