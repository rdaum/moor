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

//! Client for enrolling hosts/workers with the daemon

use eyre::{Context, Result, eyre};
use planus::ReadAsRoot;
use rpc_common::{EnrollmentResponseRef, mk_enrollment_request};
use std::path::Path;
use tracing::info;
use uuid::Uuid;

/// Enroll this host/worker with the daemon
///
/// Returns the daemon's public key and the assigned service UUID
pub fn enroll_with_daemon(
    enrollment_endpoint: &str,
    enrollment_token: &str,
    service_type: &str,
    data_dir: &Path,
) -> Result<(String, Uuid)> {
    // Generate or load our CURVE keypair
    let keypair = crate::curve_keys::load_or_generate_keypair(data_dir, service_type)?;

    // Get hostname
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        service_type = %service_type,
        hostname = %hostname,
        endpoint = %enrollment_endpoint,
        "Enrolling with daemon"
    );

    // Create ZMQ context and REQ socket
    let ctx = zmq::Context::new();
    let socket = ctx
        .socket(zmq::REQ)
        .context("Failed to create enrollment socket")?;

    // Connect to enrollment endpoint
    socket.connect(enrollment_endpoint).with_context(|| {
        format!("Failed to connect to enrollment endpoint: {enrollment_endpoint}")
    })?;

    // Build enrollment request
    let request = mk_enrollment_request(
        enrollment_token.to_string(),
        keypair.public.clone(),
        service_type.to_string(),
        hostname.clone(),
    );

    // Serialize FlatBuffer request
    let mut builder = planus::Builder::new();
    let request_bytes = builder.finish(&request, None).to_vec();

    socket
        .send(&request_bytes, 0)
        .context("Failed to send enrollment request")?;

    // Receive response
    let response_bytes = socket
        .recv_bytes(0)
        .context("Failed to receive enrollment response")?;

    // Deserialize FlatBuffer response
    let response = EnrollmentResponseRef::read_as_root(&response_bytes)
        .context("Failed to deserialize enrollment response")?;

    // Check if enrollment succeeded
    let success = response
        .success()
        .context("Missing success field in enrollment response")?;

    if !success {
        let error_msg = response.error().ok().flatten().unwrap_or("Unknown error");
        return Err(eyre!("Enrollment failed: {}", error_msg));
    }

    // Extract daemon public key and service UUID
    let daemon_public_key = response
        .daemon_curve_public_key()
        .ok()
        .flatten()
        .ok_or_else(|| eyre!("No daemon public key in response"))?
        .to_string();

    let service_uuid_str = response
        .service_uuid()
        .ok()
        .flatten()
        .ok_or_else(|| eyre!("No service UUID in response"))?;

    let service_uuid =
        Uuid::parse_str(service_uuid_str).context("Invalid service UUID from daemon")?;

    info!(
        service_uuid = %service_uuid,
        "Successfully enrolled with daemon"
    );

    // Save identity to disk
    crate::curve_keys::save_identity(
        data_dir,
        service_type,
        service_uuid,
        &hostname,
        &daemon_public_key,
    )?;

    Ok((daemon_public_key, service_uuid))
}

/// Try to read enrollment token from a file path
fn read_enrollment_token_from_file(token_path: &Path) -> Option<String> {
    std::fs::read_to_string(token_path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Resolve the default XDG enrollment token path
///
/// Returns $XDG_CONFIG_HOME/moor/enrollment-token or ~/.config/moor/enrollment-token
fn xdg_default_token_path() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(dir).join("moor/enrollment-token")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config/moor/enrollment-token")
    } else {
        // Last-resort fallback for non-standard environments
        std::path::PathBuf::from("enrollment-token")
    }
}

/// Check if already enrolled and return identity, or enroll if needed
///
/// This is the main entry point for host/worker startup.
/// Retries enrollment with exponential backoff if daemon is not yet ready.
pub fn ensure_enrolled(
    enrollment_endpoint: &str,
    enrollment_token: Option<&str>,
    enrollment_token_file: Option<&Path>,
    service_type: &str,
    data_dir: &Path,
) -> Result<(String, Uuid)> {
    // Check if we already have an identity
    if let Some(identity) = crate::curve_keys::load_identity(data_dir, service_type)? {
        info!(
            service_uuid = %identity.service_uuid,
            service_type = %identity.service_type,
            "Using existing enrollment"
        );
        let uuid =
            Uuid::parse_str(&identity.service_uuid).context("Invalid UUID in stored identity")?;
        return Ok((identity.daemon_curve_public_key, uuid));
    }

    // Not enrolled yet - need enrollment token
    // Priority:
    //   1) explicit token arg
    //   2) token file arg
    //   3) XDG default token file ($XDG_CONFIG_HOME/moor/enrollment-token or ~/.config/moor/enrollment-token)
    //   4) MOOR_ENROLLMENT_TOKEN env var
    let token = enrollment_token
        .map(|s| s.to_string())
        .or_else(|| enrollment_token_file.and_then(read_enrollment_token_from_file))
        .or_else(|| read_enrollment_token_from_file(&xdg_default_token_path()))
        .or_else(|| std::env::var("MOOR_ENROLLMENT_TOKEN").ok())
        .ok_or_else(|| {
            eyre!(
                "Not enrolled and no enrollment token provided. Either:\n\
             1. Set MOOR_ENROLLMENT_TOKEN environment variable, or\n\
             2. Use --enrollment-token-file to specify token file path, or\n\
             3. Place token in ${{XDG_CONFIG_HOME:-$HOME/.config}}/moor/enrollment-token"
            )
        })?;

    // Perform enrollment with retry logic (daemon might not be ready yet)
    let mut retry_delay_ms = 100;
    let max_retry_delay_ms = 5000;
    let max_retries = 30; // ~30 seconds total with exponential backoff

    for attempt in 1..=max_retries {
        match enroll_with_daemon(enrollment_endpoint, &token, service_type, data_dir) {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_retries {
                    return Err(e.wrap_err("Failed to enroll after maximum retries"));
                }

                info!(
                    attempt = attempt,
                    retry_delay_ms = retry_delay_ms,
                    error = %e,
                    "Enrollment failed, retrying..."
                );

                std::thread::sleep(std::time::Duration::from_millis(retry_delay_ms));
                retry_delay_ms = (retry_delay_ms * 2).min(max_retry_delay_ms);
            }
        }
    }

    unreachable!()
}

/// Setup CURVE encryption by enrolling with the daemon and loading keys
///
/// This is a high-level helper that encapsulates the common pattern used by
/// hosts and workers for setting up CURVE authentication.
///
/// Returns:
/// - None if the RPC address uses IPC (no encryption needed)
/// - Some((client_secret, client_public, server_public)) if using TCP
///
/// All returned keys are Z85-encoded strings.
pub fn setup_curve_auth(
    rpc_address: &str,
    enrollment_endpoint: &str,
    enrollment_token_file: Option<&Path>,
    service_type: &str,
    data_dir: &Path,
) -> Result<Option<(String, String, String)>> {
    // Check if we need CURVE encryption (only for TCP endpoints, not IPC)
    let use_curve = rpc_address.starts_with("tcp://");

    if !use_curve {
        info!("IPC endpoint detected - CURVE encryption disabled");
        return Ok(None);
    }

    info!("TCP endpoint detected - enrolling with daemon and loading CURVE keys");

    // Get enrollment token from environment variable
    let enrollment_token = std::env::var("MOOR_ENROLLMENT_TOKEN").ok();

    // Enroll with daemon
    let (daemon_public_key, _service_uuid) = ensure_enrolled(
        enrollment_endpoint,
        enrollment_token.as_deref(),
        enrollment_token_file,
        service_type,
        data_dir,
    )?;

    // Load or generate CURVE keypair
    let keypair = crate::curve_keys::load_or_generate_keypair(data_dir, service_type)?;

    Ok(Some((keypair.secret, keypair.public, daemon_public_key)))
}
