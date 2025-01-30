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

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database_config: DatabaseConfig,
    pub features_config: FeaturesConfig,
    pub textdump_config: TextdumpConfig,
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
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            persistent_tasks: true,
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

    /// Returns true if the configuration is backwards compatible with LambdaMOO 1.8 features
    pub fn is_lambdammoo_compatible(&self) -> bool {
        !self.lexical_scopes
            && !self.map_type
            && !self.type_dispatch
            && !self.flyweight_type
            && !self.rich_notify
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
            && (!other.type_dispatch || self.type_dispatch)
            && (!other.flyweight_type || self.flyweight_type)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextdumpConfig {
    /// Where to read the initial textdump from, if any.
    pub input_path: Option<PathBuf>,
    /// Directory to write periodic textdumps of the database, if any.
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
    /// If true, use the new-style directory based objectdef import format instead of traditional
    /// textdump.
    pub import_dirdump: bool,
    /// If true, use the new-style directory based objectdef dump format instead of traditional
    /// textdump.
    pub export_dirdump: bool,
}

impl Default for TextdumpConfig {
    fn default() -> Self {
        Self {
            input_path: None,
            output_path: None,
            output_encoding: EncodingMode::UTF8,
            checkpoint_interval: Some(Duration::from_secs(60)),
            version_override: None,
            import_dirdump: false,
            export_dirdump: false,
        }
    }
}

impl TextdumpConfig {
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
    LambdaMOO(u16),
    Moor(Version, FeaturesConfig, EncodingMode),
}

impl TextdumpVersion {
    pub fn parse(s: &str) -> Option<TextdumpVersion> {
        if s.starts_with("** LambdaMOO Database, Format Version ") {
            let version = s
                .trim_start_matches("** LambdaMOO Database, Format Version ")
                .trim_end_matches(" **");
            let version = version.parse::<u16>().ok()?;
            return Some(TextdumpVersion::LambdaMOO(version));
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
    use crate::config::TextdumpVersion;

    #[test]
    fn parse_textdump_version_lambda() {
        let version = super::TextdumpVersion::parse("** LambdaMOO Database, Format Version 4 **");
        assert_eq!(version, Some(super::TextdumpVersion::LambdaMOO(4)));
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
