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

//! Configuration for mdmoot

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration (mdmoot.toml)
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Project settings
    pub project: ProjectConfig,
    /// Implementation handlers
    #[serde(default)]
    pub implementations: HashMap<String, ImplementationConfig>,
    /// Web server settings
    #[serde(default)]
    pub server: ServerConfig,
}

/// Project-level configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// Root directory for spec files
    pub root: PathBuf,
    /// Default implementation to run tests against
    #[serde(default = "default_impl")]
    pub default_impl: String,
}

fn default_impl() -> String {
    "moor".to_string()
}

/// Configuration for a single implementation
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "handler")]
pub enum ImplementationConfig {
    /// In-process moor execution
    #[serde(rename = "in-process")]
    InProcess,
    /// Telnet connection to external server
    #[serde(rename = "telnet")]
    Telnet { host: String, port: u16 },
}

/// Web server configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8080
}

impl Config {
    /// Load config from file
    pub fn load(path: &std::path::Path) -> eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Find config file by walking up from current directory
    pub fn find_and_load() -> eyre::Result<(PathBuf, Self)> {
        let mut dir = std::env::current_dir()?;
        loop {
            let config_path = dir.join("mdmoot.toml");
            if config_path.exists() {
                let config = Self::load(&config_path)?;
                return Ok((config_path, config));
            }
            if !dir.pop() {
                eyre::bail!("No mdmoot.toml found in current directory or parents");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml = r#"
[project]
root = "specs/"
default_impl = "moor"

[implementations.moor]
handler = "in-process"

[implementations.lambdamoo]
handler = "telnet"
host = "localhost"
port = 7777

[server]
port = 8080
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.project.root, PathBuf::from("specs/"));
        assert_eq!(config.project.default_impl, "moor");
        assert!(matches!(
            config.implementations.get("moor"),
            Some(ImplementationConfig::InProcess)
        ));
    }
}
