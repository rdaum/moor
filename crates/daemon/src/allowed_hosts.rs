// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Registry of allowed host public keys for CURVE authentication

use eyre::{Context, Result};
use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};
use uuid::Uuid;

/// Registry of authorized host public keys
/// Maps service UUID -> Z85-encoded CURVE public key
#[derive(Clone)]
pub struct AllowedHostsRegistry {
    /// Directory containing allowed host public key files
    hosts_dir: PathBuf,
    /// In-memory cache of UUID -> public_key mappings
    cache: Arc<RwLock<HashMap<Uuid, String>>>,
}

impl AllowedHostsRegistry {
    /// Create a new registry at an explicit allowed-hosts directory
    ///
    /// Creates the allowed-hosts directory if it doesn't exist, initializes cache, and loads entries.
    pub fn from_dir(hosts_dir: &Path) -> Result<Self> {
        fs::create_dir_all(hosts_dir).with_context(|| {
            format!("Failed to create allowed-hosts directory at {hosts_dir:?}")
        })?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(hosts_dir)
                .with_context(|| format!("Failed to stat allowed-hosts dir {hosts_dir:?}"))?
                .permissions();
            perms.set_mode(0o700); // owner rwx only
            fs::set_permissions(hosts_dir, perms)
                .with_context(|| format!("Failed to set permissions on {hosts_dir:?}"))?;
            let mode = fs::metadata(hosts_dir)?.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                warn!(
                    ?hosts_dir,
                    mode = format_args!("{:o}", mode),
                    "allowed-hosts directory permissions are too permissive; expected 700"
                );
            }
        }

        let mut registry = Self {
            hosts_dir: hosts_dir.to_path_buf(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load all existing hosts
        registry.reload()?;

        Ok(registry)
    }

    /// Reload all host public keys from disk
    pub fn reload(&mut self) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.clear();

        let entries = fs::read_dir(&self.hosts_dir).with_context(|| {
            format!(
                "Failed to read allowed-hosts directory {hosts_dir:?}",
                hosts_dir = self.hosts_dir
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip non-files
            if !path.is_file() {
                continue;
            }

            // Parse UUID from filename
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                warn!(?path, "Skipping invalid filename in allowed-hosts");
                continue;
            };

            let Ok(uuid) = Uuid::parse_str(filename) else {
                warn!(?path, "Skipping non-UUID filename in allowed-hosts");
                continue;
            };

            // Load the public key
            match load_host_public_key(&path) {
                Ok(public_key) => {
                    cache.insert(uuid, public_key);
                }
                Err(e) => {
                    warn!(?path, error = ?e, "Failed to load host public key, skipping");
                }
            }
        }

        info!("Loaded {} authorized hosts", cache.len());
        Ok(())
    }

    /// Check if a host with the given public key is authorized
    ///
    /// Returns the UUID if found
    pub fn is_authorized(&self, public_key: &str) -> Option<Uuid> {
        let cache = self.cache.read().unwrap();
        cache
            .iter()
            .find(|(_, pk)| pk.as_str() == public_key)
            .map(|(uuid, _)| *uuid)
    }

    /// Add a new authorized host
    ///
    /// Writes the public key to disk and updates the cache
    pub fn add_host(
        &self,
        uuid: Uuid,
        public_key: &str,
        service_type: &str,
        hostname: &str,
    ) -> Result<()> {
        let file_path = self.hosts_dir.join(uuid.to_string());

        // Check if already exists
        if file_path.exists() {
            warn!(?uuid, "Host already enrolled, overwriting");
        }

        // Write to disk
        let content = format!(
            "# Service: {} ({})\n\
             # Enrolled: {}\n\
             public={}\n",
            service_type,
            hostname,
            chrono::Utc::now().to_rfc3339(),
            public_key
        );

        fs::write(&file_path, content)
            .with_context(|| format!("Failed to write host public key to {file_path:?}"))?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&file_path)
                .with_context(|| format!("Failed to stat host key file {file_path:?}"))?
                .permissions();
            perms.set_mode(0o600); // owner rw only
            fs::set_permissions(&file_path, perms)
                .with_context(|| format!("Failed to set permissions on {file_path:?}"))?;
            let mode = fs::metadata(&file_path)?.permissions().mode() & 0o777;
            if mode & 0o177 != 0 {
                warn!(
                    ?file_path,
                    mode = format_args!("{:o}", mode),
                    "host key file permissions are too permissive; expected 600"
                );
            }
        }

        // Update cache
        let mut cache = self.cache.write().unwrap();
        cache.insert(uuid, public_key.to_string());

        info!(?uuid, service_type, hostname, "Added authorized host");

        Ok(())
    }

    /// Remove an authorized host
    #[allow(dead_code)]
    pub fn remove_host(&self, uuid: Uuid) -> Result<bool> {
        let file_path = self.hosts_dir.join(uuid.to_string());

        // Remove from disk
        if file_path.exists() {
            fs::remove_file(&file_path)
                .with_context(|| format!("Failed to remove host file {file_path:?}"))?;
        }

        // Remove from cache
        let mut cache = self.cache.write().unwrap();
        let existed = cache.remove(&uuid).is_some();

        if existed {
            info!(?uuid, "Removed authorized host");
        }

        Ok(existed)
    }

    /// Get the number of authorized hosts
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.cache.read().unwrap().len()
    }
}

/// Load a host public key from a file
///
/// File format:
/// ```
/// # Comment lines
/// public=<Z85-encoded-key>
/// ```
fn load_host_public_key(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read host public key from {path:?}"))?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "public"
        {
            let value = value.trim();
            if value.len() == 40 {
                // Z85-encoded CURVE keys are always 40 chars
                return Ok(value.to_string());
            } else {
                eyre::bail!(
                    "Invalid public key length: expected 40, got {}",
                    value.len()
                );
            }
        }
    }

    eyre::bail!("No public key found in file")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_registry_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let registry =
            AllowedHostsRegistry::from_dir(&temp_dir.path().join("allowed-hosts")).unwrap();

        assert!(temp_dir.path().join("allowed-hosts").exists());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_add_and_check_host() {
        let temp_dir = TempDir::new().unwrap();
        let registry =
            AllowedHostsRegistry::from_dir(&temp_dir.path().join("allowed-hosts")).unwrap();

        let uuid = Uuid::new_v4();
        let public_key = "a".repeat(40);

        registry
            .add_host(uuid, &public_key, "web-host", "test-host")
            .unwrap();

        assert_eq!(registry.count(), 1);
        assert_eq!(registry.is_authorized(&public_key), Some(uuid));
        assert_eq!(registry.is_authorized(&"b".repeat(40)), None);
    }

    #[test]
    fn test_remove_host() {
        let temp_dir = TempDir::new().unwrap();
        let registry =
            AllowedHostsRegistry::from_dir(&temp_dir.path().join("allowed-hosts")).unwrap();

        let uuid = Uuid::new_v4();
        let public_key = "a".repeat(40);

        registry
            .add_host(uuid, &public_key, "web-host", "test-host")
            .unwrap();
        assert_eq!(registry.count(), 1);

        let removed = registry.remove_host(uuid).unwrap();
        assert!(removed);
        assert_eq!(registry.count(), 0);
        assert_eq!(registry.is_authorized(&public_key), None);
    }

    #[test]
    fn test_reload_persists_across_instances() {
        let temp_dir = TempDir::new().unwrap();

        let uuid = Uuid::new_v4();
        let public_key = "a".repeat(40);

        // First registry instance
        {
            let registry =
                AllowedHostsRegistry::from_dir(&temp_dir.path().join("allowed-hosts")).unwrap();
            registry
                .add_host(uuid, &public_key, "web-host", "test-host")
                .unwrap();
        }

        // Second registry instance - should load from disk
        {
            let registry =
                AllowedHostsRegistry::from_dir(&temp_dir.path().join("allowed-hosts")).unwrap();
            assert_eq!(registry.count(), 1);
            assert_eq!(registry.is_authorized(&public_key), Some(uuid));
        }
    }
}
