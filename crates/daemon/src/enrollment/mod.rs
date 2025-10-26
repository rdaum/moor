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

//! Host enrollment server - accepts unauthenticated connections for initial CURVE key registration
//!
//! This module provides a separate ZMQ endpoint where hosts can enroll themselves by providing:
//! - Enrollment token (shared secret, can be rotated)
//! - Their CURVE public key
//! - Service metadata (type, hostname)
//!
//! Upon successful token validation, the daemon stores the host's public key in the
//! allowed-hosts registry, enabling future CURVE-authenticated connections.

use crate::allowed_hosts::AllowedHostsRegistry;
use eyre::{Context, Result};
use planus::ReadAsRoot;
use rpc_common::{
    EnrollmentRequestRef, EnrollmentResponse, mk_enrollment_response_failure,
    mk_enrollment_response_success,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Enrollment server that listens for host registration requests
pub struct EnrollmentServer {
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
    daemon_curve_public_key: String,
    allowed_hosts: AllowedHostsRegistry,
    enrollment_token_path: PathBuf,
}

impl EnrollmentServer {
    /// Create a new enrollment server
    pub fn new(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        daemon_curve_public_key: String,
        allowed_hosts: AllowedHostsRegistry,
        enrollment_token_path: PathBuf,
    ) -> Self {
        Self {
            zmq_context,
            kill_switch,
            daemon_curve_public_key,
            allowed_hosts,
            enrollment_token_path,
        }
    }

    /// Start listening for enrollment requests on the given endpoint
    ///
    /// This is a blocking call that runs until the kill switch is activated.
    /// Typically run in a dedicated thread.
    pub fn listen(&self, endpoint: &str) -> Result<()> {
        let socket = self
            .zmq_context
            .socket(zmq::REP)
            .context("Failed to create enrollment socket")?;

        socket
            .bind(endpoint)
            .with_context(|| format!("Failed to bind enrollment socket to {endpoint}"))?;

        info!("Enrollment server listening on {}", endpoint);

        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("Enrollment server shutting down");
                break;
            }

            // Poll with timeout so we can check kill switch
            let poll_result = socket
                .poll(zmq::POLLIN, 1000)
                .context("Failed to poll enrollment socket")?;

            if poll_result == 0 {
                continue; // Timeout, check kill switch again
            }

            // Receive request
            let request_bytes = match socket.recv_bytes(0) {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!(error = ?e, "Failed to receive enrollment request");
                    continue;
                }
            };

            // Process request
            let response = self.handle_request(&request_bytes);

            // Send response - serialize FlatBuffer
            let mut builder = planus::Builder::new();
            let response_bytes = builder.finish(&response, None).to_vec();

            if let Err(e) = socket.send(&response_bytes, 0) {
                error!(error = ?e, "Failed to send enrollment response");
            }
        }

        Ok(())
    }

    /// Handle a single enrollment request
    fn handle_request(&self, request_bytes: &[u8]) -> EnrollmentResponse {
        // Parse FlatBuffer request
        let request = match EnrollmentRequestRef::read_as_root(request_bytes) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = ?e, "Invalid enrollment request");
                return mk_enrollment_response_failure(format!("Invalid request format: {e}"));
            }
        };

        let enrollment_token = match request.enrollment_token() {
            Ok(token) => token,
            Err(e) => {
                warn!(error = ?e, "Missing enrollment token");
                return mk_enrollment_response_failure("Missing enrollment token".to_string());
            }
        };

        let curve_public_key = match request.curve_public_key() {
            Ok(key) => key,
            Err(e) => {
                warn!(error = ?e, "Missing CURVE public key");
                return mk_enrollment_response_failure("Missing CURVE public key".to_string());
            }
        };

        let service_type = match request.service_type() {
            Ok(st) => st,
            Err(e) => {
                warn!(error = ?e, "Missing service type");
                return mk_enrollment_response_failure("Missing service type".to_string());
            }
        };

        let hostname = match request.hostname() {
            Ok(h) => h,
            Err(e) => {
                warn!(error = ?e, "Missing hostname");
                return mk_enrollment_response_failure("Missing hostname".to_string());
            }
        };

        // Validate public key format
        if curve_public_key.len() != 40 {
            return mk_enrollment_response_failure(format!(
                "Invalid CURVE public key length: expected 40, got {}",
                curve_public_key.len()
            ));
        }

        // Validate enrollment token
        if let Err(e) = self.validate_enrollment_token(enrollment_token) {
            warn!(
                service_type = %service_type,
                hostname = %hostname,
                error = %e,
                "Enrollment failed: invalid token"
            );
            return mk_enrollment_response_failure(e);
        }

        info!(
            service_type = %service_type,
            hostname = %hostname,
            "Enrolling new host"
        );

        // Generate service UUID
        let service_uuid = Uuid::new_v4();

        // Add to allowed hosts
        if let Err(e) =
            self.allowed_hosts
                .add_host(service_uuid, curve_public_key, service_type, hostname)
        {
            error!(error = ?e, "Failed to save host public key");
            return mk_enrollment_response_failure(format!("Failed to save host public key: {e}"));
        }

        mk_enrollment_response_success(
            service_uuid.to_string(),
            self.daemon_curve_public_key.clone(),
        )
    }

    /// Validate enrollment token
    fn validate_enrollment_token(&self, provided_token: &str) -> Result<(), String> {
        let current_token = self
            .load_current_enrollment_token()
            .map_err(|e| format!("No enrollment token available: {e}"))?;

        if provided_token != current_token {
            return Err("Invalid enrollment token".to_string());
        }

        Ok(())
    }

    /// Load current enrollment token from disk
    fn load_current_enrollment_token(&self) -> Result<String, std::io::Error> {
        let content = fs::read_to_string(&self.enrollment_token_path)?;
        Ok(content.trim().to_string())
    }
}

/// Generate or load enrollment token for daemon
///
/// If token file exists, loads it. Otherwise generates new UUID and saves it.
pub fn ensure_enrollment_token(token_path: &std::path::Path) -> Result<String> {
    if token_path.exists() {
        let token = fs::read_to_string(token_path)
            .with_context(|| format!("Failed to read enrollment token from {token_path:?}"))?
            .trim()
            .to_string();
        info!("Using enrollment token from {:?}", token_path);
        // Warn if permissions are too permissive (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(token_path)?.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                warn!(
                    ?token_path,
                    mode = format_args!("{:o}", mode),
                    "Enrollment token file permissions are too permissive; expected 600"
                );
            }
        }
        Ok(token)
    } else {
        let token = Uuid::new_v4().to_string();
        fs::write(token_path, &token)
            .with_context(|| format!("Failed to write enrollment token to {token_path:?}"))?;

        // Restrict permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(token_path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(token_path, perms)?;
            // Verify final permissions and warn if not as expected
            let mode = fs::metadata(token_path)?.permissions().mode() & 0o777;
            if mode != 0o600 {
                warn!(
                    ?token_path,
                    mode = format_args!("{:o}", mode),
                    "Enrollment token file permissions are {:o}, expected 600",
                    mode
                );
            }
        }

        info!("Generated new enrollment token at {:?}", token_path);
        info!(
            "Hosts must set MOOR_ENROLLMENT_TOKEN or provide --enrollment-token-file pointing to the token file"
        );
        Ok(token)
    }
}

/// Rotate the enrollment token by generating and saving a new shared secret.
pub fn rotate_enrollment_token(token_path: &Path) -> Result<String> {
    let new_token = Uuid::new_v4().to_string();

    let old_token = if token_path.exists() {
        Some(
            fs::read_to_string(token_path)
                .with_context(|| format!("Failed to read existing token from {token_path:?}"))?
                .trim()
                .to_string(),
        )
    } else {
        None
    };

    if let Some(parent) = token_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {parent:?}"))?;
    }

    fs::write(token_path, &new_token)
        .with_context(|| format!("Failed to write new enrollment token to {token_path:?}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(token_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(token_path, perms)?;
    }

    if let Some(old) = old_token {
        info!("Old enrollment token: {}", old);
    }
    info!("New enrollment token: {}", new_token);
    info!("Token saved to: {:?}", token_path);
    info!("");
    info!(
        "Hosts must set MOOR_ENROLLMENT_TOKEN={} or --enrollment-token-file={:?}",
        new_token, token_path
    );
    info!("");
    info!("Note: Hosts already enrolled with CURVE keys will continue to work.");
    info!("      Only new hosts need the new token to enroll.");

    Ok(new_token)
}
