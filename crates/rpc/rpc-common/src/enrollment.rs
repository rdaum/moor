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

//! Enrollment message types and builders for host/worker registration

use moor_schema::rpc;

// Re-export FlatBuffer enrollment types
pub use rpc::{EnrollmentRequest, EnrollmentRequestRef, EnrollmentResponse, EnrollmentResponseRef};

// ============================================================================
// Enrollment message builders
// ============================================================================

/// Create an EnrollmentRequest
pub fn mk_enrollment_request(
    enrollment_token: String,
    curve_public_key: String,
    service_type: String,
    hostname: String,
) -> EnrollmentRequest {
    EnrollmentRequest {
        enrollment_token,
        curve_public_key,
        service_type,
        hostname,
    }
}

/// Create a successful EnrollmentResponse
pub fn mk_enrollment_response_success(
    service_uuid: String,
    daemon_curve_public_key: String,
) -> EnrollmentResponse {
    EnrollmentResponse {
        success: true,
        service_uuid: Some(service_uuid),
        daemon_curve_public_key: Some(daemon_curve_public_key),
        error: None,
    }
}

/// Create a failed EnrollmentResponse
pub fn mk_enrollment_response_failure(error: String) -> EnrollmentResponse {
    EnrollmentResponse {
        success: false,
        service_uuid: None,
        daemon_curve_public_key: None,
        error: Some(error),
    }
}
