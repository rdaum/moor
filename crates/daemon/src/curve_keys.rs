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

//! CURVE25519 key generation and storage for daemon

use eyre::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::info;

/// A CURVE25519 keypair in Z85-encoded format
#[derive(Debug, Clone)]
pub struct CurveKeyPair {
    /// Z85-encoded secret key (40 characters)
    pub secret: String,
    /// Z85-encoded public key (40 characters)
    pub public: String,
}

/// Generate a new CURVE25519 keypair
pub fn generate_keypair() -> Result<CurveKeyPair> {
    let keypair = zmq::CurveKeyPair::new().context("Failed to generate CURVE keypair")?;

    let secret =
        zmq::z85_encode(&keypair.secret_key).context("Failed to encode secret key to Z85")?;

    let public =
        zmq::z85_encode(&keypair.public_key).context("Failed to encode public key to Z85")?;

    Ok(CurveKeyPair { secret, public })
}

/// Load or generate daemon's CURVE keypair
///
/// Looks for keys at ~/.moor/daemon-curve.{key,pub}
/// If not found, generates new keys and saves them
pub fn load_or_generate_daemon_keypair(data_dir: &Path) -> Result<CurveKeyPair> {
    let secret_path = data_dir.join("daemon-curve.key");
    let public_path = data_dir.join("daemon-curve.pub");

    if secret_path.exists() && public_path.exists() {
        info!("Loading existing daemon CURVE keys from {:?}", data_dir);
        load_keypair(&secret_path, &public_path)
    } else {
        info!("Generating new daemon CURVE keys");
        let keypair = generate_keypair()?;
        save_keypair(&keypair, &secret_path, &public_path)?;
        info!(
            "Saved daemon CURVE keys to {:?} and {:?}",
            secret_path, public_path
        );
        Ok(keypair)
    }
}

/// Load a keypair from separate secret and public key files
fn load_keypair(secret_path: &Path, public_path: &Path) -> Result<CurveKeyPair> {
    let secret_content = fs::read_to_string(secret_path)
        .with_context(|| format!("Failed to read secret key from {secret_path:?}"))?;

    let public_content = fs::read_to_string(public_path)
        .with_context(|| format!("Failed to read public key from {public_path:?}"))?;

    // Parse files - they have format "secret=<key>" or "public=<key>"
    let secret = parse_key_file(&secret_content, "secret")?;
    let public = parse_key_file(&public_content, "public")?;

    Ok(CurveKeyPair { secret, public })
}

/// Save a keypair to separate secret and public key files
fn save_keypair(keypair: &CurveKeyPair, secret_path: &Path, public_path: &Path) -> Result<()> {
    let secret_content = format!(
        "# mooR Daemon CURVE Secret Key\n\
         # Generated: {}\n\
         secret={}\n",
        chrono::Utc::now().to_rfc3339(),
        keypair.secret
    );

    let public_content = format!(
        "# mooR Daemon CURVE Public Key\n\
         # Generated: {}\n\
         public={}\n",
        chrono::Utc::now().to_rfc3339(),
        keypair.public
    );

    fs::write(secret_path, secret_content)
        .with_context(|| format!("Failed to write secret key to {secret_path:?}"))?;

    fs::write(public_path, public_content)
        .with_context(|| format!("Failed to write public key to {public_path:?}"))?;

    // Restrict permissions on secret key (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(secret_path)?.permissions();
        perms.set_mode(0o600); // Read/write for owner only
        fs::set_permissions(secret_path, perms)?;
    }

    Ok(())
}

/// Parse a key file with format "key=value"
fn parse_key_file(content: &str, expected_prefix: &str) -> Result<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once('=')
            && key.trim() == expected_prefix
        {
            let value = value.trim();
            if value.len() == 40 {
                // Z85-encoded CURVE keys are always 40 chars
                return Ok(value.to_string());
            } else {
                eyre::bail!(
                    "Invalid {} key length: expected 40, got {}",
                    expected_prefix,
                    value.len()
                );
            }
        }
    }

    eyre::bail!("No {} key found in file", expected_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_keypair() {
        let keypair = generate_keypair().unwrap();

        assert_eq!(keypair.secret.len(), 40);
        assert_eq!(keypair.public.len(), 40);
        assert_ne!(keypair.secret, keypair.public);
    }

    #[test]
    fn test_save_and_load_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let secret_path = temp_dir.path().join("test.key");
        let public_path = temp_dir.path().join("test.pub");

        let original = generate_keypair().unwrap();
        save_keypair(&original, &secret_path, &public_path).unwrap();

        let loaded = load_keypair(&secret_path, &public_path).unwrap();

        assert_eq!(loaded.secret, original.secret);
        assert_eq!(loaded.public, original.public);
    }

    #[test]
    fn test_load_or_generate_creates_keys() {
        let temp_dir = TempDir::new().unwrap();

        let keypair = load_or_generate_daemon_keypair(temp_dir.path()).unwrap();

        assert_eq!(keypair.secret.len(), 40);
        assert_eq!(keypair.public.len(), 40);

        // Should have created the files
        assert!(temp_dir.path().join("daemon-curve.key").exists());
        assert!(temp_dir.path().join("daemon-curve.pub").exists());
    }

    #[test]
    fn test_load_or_generate_reuses_existing() {
        let temp_dir = TempDir::new().unwrap();

        let first = load_or_generate_daemon_keypair(temp_dir.path()).unwrap();
        let second = load_or_generate_daemon_keypair(temp_dir.path()).unwrap();

        // Should be identical (loaded from file, not regenerated)
        assert_eq!(first.secret, second.secret);
        assert_eq!(first.public, second.public);
    }
}
