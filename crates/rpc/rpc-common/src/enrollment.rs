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

//! Entities used as JSON blobs sent to/from the host/worker <-> daemon for the enrollment process.

use serde::{Deserialize, Serialize};

/// Enrollment request from a host/worker
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
