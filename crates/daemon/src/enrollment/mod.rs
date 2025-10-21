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
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Enrollment request from a host/worker
#[derive(Debug, Deserialize)]
pub struct EnrollmentRequest {
    /// Enrollment token (one-time shared secret)
    pub enrollment_token: String,
    /// Host's CURVE public key (Z85-encoded, 40 characters)
    pub curve_public_key: String,
    /// Service type (e.g., "web-host", "telnet-host", "curl-worker")
    pub service_type: String,
    /// Hostname for logging/debugging
    pub hostname: String,
}

/// Enrollment response to host/worker
#[derive(Debug, Serialize)]
pub struct EnrollmentResponse {
    /// Whether enrollment succeeded
    pub success: bool,
    /// UUID assigned to this service instance (if successful)
    pub service_uuid: Option<String>,
    /// Daemon's CURVE public key (Z85-encoded, if successful)
    pub daemon_curve_public_key: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

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
            .with_context(|| format!("Failed to bind enrollment socket to {}", endpoint))?;

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

            // Send response
            let response_bytes =
                serde_json::to_vec(&response).context("Failed to serialize enrollment response")?;

            if let Err(e) = socket.send(&response_bytes, 0) {
                error!(error = ?e, "Failed to send enrollment response");
            }
        }

        Ok(())
    }

    /// Handle a single enrollment request
    fn handle_request(&self, request_bytes: &[u8]) -> EnrollmentResponse {
        // Parse request
        let request: EnrollmentRequest = match serde_json::from_slice(request_bytes) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = ?e, "Invalid enrollment request");
                return EnrollmentResponse {
                    success: false,
                    error: Some(format!("Invalid request format: {}", e)),
                    service_uuid: None,
                    daemon_curve_public_key: None,
                };
            }
        };

        // Validate public key format
        if request.curve_public_key.len() != 40 {
            return EnrollmentResponse {
                success: false,
                error: Some(format!(
                    "Invalid CURVE public key length: expected 40, got {}",
                    request.curve_public_key.len()
                )),
                service_uuid: None,
                daemon_curve_public_key: None,
            };
        }

        // Validate enrollment token
        if let Err(e) = self.validate_enrollment_token(&request.enrollment_token) {
            warn!(
                service_type = %request.service_type,
                hostname = %request.hostname,
                error = %e,
                "Enrollment failed: invalid token"
            );
            return EnrollmentResponse {
                success: false,
                error: Some(e),
                service_uuid: None,
                daemon_curve_public_key: None,
            };
        }

        info!(
            service_type = %request.service_type,
            hostname = %request.hostname,
            "Enrolling new host"
        );

        // Generate service UUID
        let service_uuid = Uuid::new_v4();

        // Add to allowed hosts
        if let Err(e) = self.allowed_hosts.add_host(
            service_uuid,
            &request.curve_public_key,
            &request.service_type,
            &request.hostname,
        ) {
            error!(error = ?e, "Failed to save host public key");
            return EnrollmentResponse {
                success: false,
                error: Some(format!("Failed to save host public key: {}", e)),
                service_uuid: None,
                daemon_curve_public_key: None,
            };
        }

        EnrollmentResponse {
            success: true,
            service_uuid: Some(service_uuid.to_string()),
            daemon_curve_public_key: Some(self.daemon_curve_public_key.clone()),
            error: None,
        }
    }

    /// Validate enrollment token
    fn validate_enrollment_token(&self, provided_token: &str) -> Result<(), String> {
        let current_token = self
            .load_current_enrollment_token()
            .map_err(|e| format!("No enrollment token available: {}", e))?;

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
            .with_context(|| format!("Failed to read enrollment token from {:?}", token_path))?
            .trim()
            .to_string();
        info!("Using enrollment token from {:?}", token_path);
        Ok(token)
    } else {
        let token = Uuid::new_v4().to_string();
        fs::write(token_path, &token)
            .with_context(|| format!("Failed to write enrollment token to {:?}", token_path))?;

        // Restrict permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(token_path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(token_path, perms)?;
        }

        info!("Generated new enrollment token: {}", token);
        info!("Hosts must set MOOR_ENROLLMENT_TOKEN={}", token);
        Ok(token)
    }
}
