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

//! CURVE25519 key generation and storage for hosts/workers

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::info;
use uuid::Uuid;

/// A CURVE25519 keypair in Z85-encoded format
#[derive(Debug, Clone)]
pub struct CurveKeyPair {
    /// Z85-encoded secret key (40 characters)
    pub secret: String,
    /// Z85-encoded public key (40 characters)
    pub public: String,
}

/// Host/worker identity and enrollment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostIdentity {
    /// Unique identifier for this service instance
    pub service_uuid: String,
    /// Type of service (web-host, telnet-host, curl-worker)
    pub service_type: String,
    /// Hostname for logging/debugging
    pub hostname: String,
    /// Daemon's CURVE public key (Z85-encoded, 40 characters)
    pub daemon_curve_public_key: String,
    /// When this identity was created
    pub enrolled_at: String,
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

/// Load or generate host/worker CURVE keypair
///
/// Looks for keys at ~/.moor/{service_type}-curve.{key,pub}
/// If not found, generates new keys and saves them
pub fn load_or_generate_keypair(data_dir: &Path, service_type: &str) -> Result<CurveKeyPair> {
    let secret_path = data_dir.join(format!("{}-curve.key", service_type));
    let public_path = data_dir.join(format!("{}-curve.pub", service_type));

    if secret_path.exists() && public_path.exists() {
        info!(
            "Loading existing {} CURVE keys from {:?}",
            service_type, data_dir
        );
        load_keypair(&secret_path, &public_path)
    } else {
        info!("Generating new {} CURVE keys", service_type);
        let keypair = generate_keypair()?;
        save_keypair(&keypair, &secret_path, &public_path, service_type)?;
        info!(
            "Saved {} CURVE keys to {:?} and {:?}",
            service_type, secret_path, public_path
        );
        Ok(keypair)
    }
}

/// Load host identity from file
///
/// Returns None if identity file doesn't exist (host not yet enrolled)
pub fn load_identity(data_dir: &Path, service_type: &str) -> Result<Option<HostIdentity>> {
    let identity_path = data_dir.join(format!("{}-identity.json", service_type));

    if !identity_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&identity_path)
        .with_context(|| format!("Failed to read identity from {:?}", identity_path))?;

    let identity: HostIdentity = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse identity from {:?}", identity_path))?;

    Ok(Some(identity))
}

/// Save host identity to file
pub fn save_identity(
    data_dir: &Path,
    service_type: &str,
    service_uuid: Uuid,
    hostname: &str,
    daemon_public_key: &str,
) -> Result<()> {
    // Ensure data directory exists
    fs::create_dir_all(data_dir)
        .with_context(|| format!("Failed to create directory {:?}", data_dir))?;

    let identity_path = data_dir.join(format!("{}-identity.json", service_type));

    let identity = HostIdentity {
        service_uuid: service_uuid.to_string(),
        service_type: service_type.to_string(),
        hostname: hostname.to_string(),
        daemon_curve_public_key: daemon_public_key.to_string(),
        enrolled_at: chrono::Utc::now().to_rfc3339(),
    };

    let content =
        serde_json::to_string_pretty(&identity).context("Failed to serialize host identity")?;

    fs::write(&identity_path, content)
        .with_context(|| format!("Failed to write identity to {:?}", identity_path))?;

    info!(
        "Saved {} identity (UUID: {}) to {:?}",
        service_type, service_uuid, identity_path
    );

    Ok(())
}

/// Load a keypair from separate secret and public key files
fn load_keypair(secret_path: &Path, public_path: &Path) -> Result<CurveKeyPair> {
    let secret_content = fs::read_to_string(secret_path)
        .with_context(|| format!("Failed to read secret key from {:?}", secret_path))?;

    let public_content = fs::read_to_string(public_path)
        .with_context(|| format!("Failed to read public key from {:?}", public_path))?;

    // Parse files - they have format "secret=<key>" or "public=<key>"
    let secret = parse_key_file(&secret_content, "secret")?;
    let public = parse_key_file(&public_content, "public")?;

    Ok(CurveKeyPair { secret, public })
}

/// Save a keypair to separate secret and public key files
fn save_keypair(
    keypair: &CurveKeyPair,
    secret_path: &Path,
    public_path: &Path,
    service_type: &str,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = secret_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    let secret_content = format!(
        "# mooR {} CURVE Secret Key\n\
         # Generated: {}\n\
         secret={}\n",
        service_type,
        chrono::Utc::now().to_rfc3339(),
        keypair.secret
    );

    let public_content = format!(
        "# mooR {} CURVE Public Key\n\
         # Generated: {}\n\
         public={}\n",
        service_type,
        chrono::Utc::now().to_rfc3339(),
        keypair.public
    );

    fs::write(secret_path, secret_content)
        .with_context(|| format!("Failed to write secret key to {:?}", secret_path))?;

    fs::write(public_path, public_content)
        .with_context(|| format!("Failed to write public key to {:?}", public_path))?;

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

    #[test]
    fn test_generate_keypair() {
        let keypair = generate_keypair().unwrap();

        assert_eq!(keypair.secret.len(), 40);
        assert_eq!(keypair.public.len(), 40);
        assert_ne!(keypair.secret, keypair.public);
    }

    #[test]
    fn test_identity_serialization() {
        let identity = HostIdentity {
            service_uuid: Uuid::new_v4().to_string(),
            service_type: "web-host".to_string(),
            hostname: "test-host".to_string(),
            daemon_curve_public_key: "a".repeat(40),
            enrolled_at: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&identity).unwrap();
        let parsed: HostIdentity = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.service_uuid, identity.service_uuid);
        assert_eq!(parsed.service_type, identity.service_type);
        assert_eq!(parsed.hostname, identity.hostname);
        assert_eq!(
            parsed.daemon_curve_public_key,
            identity.daemon_curve_public_key
        );
    }
}
