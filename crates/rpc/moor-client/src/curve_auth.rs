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

//! CURVE authentication setup for mooR clients.
//!
//! Provides a simple interface for setting up CURVE encryption when connecting
//! to a mooR daemon over TCP. Handles enrollment, key management, and graceful
//! fallback when CURVE is not available.

use std::path::Path;
use tracing::{info, warn};

/// CURVE key tuple: (client_secret, client_public, server_public)
pub type CurveKeys = (String, String, String);

/// Setup CURVE authentication for connecting to a mooR daemon.
///
/// This function handles the full CURVE setup workflow:
/// 1. Checks if CURVE is needed (TCP endpoints only)
/// 2. Enrolls with the daemon if not already enrolled
/// 3. Loads or generates client CURVE keys
/// 4. Returns the keys needed for encrypted connections
///
/// # Arguments
///
/// * `rpc_address` - The RPC endpoint address (e.g., "tcp://127.0.0.1:7899")
/// * `enrollment_address` - The enrollment server address (e.g., "tcp://localhost:7900")
/// * `enrollment_token_file` - Optional path to enrollment token file
/// * `service_type` - Service identifier (e.g., "moor-lsp", "mcp-host")
/// * `data_dir` - Directory for storing identity and CURVE keys
///
/// # Returns
///
/// * `Some((secret, public, server_public))` - CURVE keys for encrypted connection
/// * `None` - CURVE not needed (IPC endpoint) or setup failed gracefully
///
/// # Example
///
/// ```ignore
/// let curve_keys = setup_curve_auth(
///     "tcp://127.0.0.1:7899",
///     "tcp://localhost:7900",
///     Some(Path::new("/path/to/token")),
///     "my-service",
///     Path::new("/path/to/data"),
/// );
///
/// let config = MoorClientConfig {
///     rpc_address: "tcp://127.0.0.1:7899".to_string(),
///     events_address: "tcp://127.0.0.1:7898".to_string(),
///     curve_keys,
/// };
/// ```
/// Synchronous version - use when blocking is acceptable (e.g., at startup).
pub fn setup_curve_auth(
    rpc_address: &str,
    enrollment_address: &str,
    enrollment_token_file: Option<&Path>,
    service_type: &str,
    data_dir: &Path,
) -> Option<CurveKeys> {
    // Only need CURVE auth for TCP endpoints
    if !rpc_address.starts_with("tcp://") {
        return None;
    }

    match rpc_async_client::enrollment_client::setup_curve_auth(
        rpc_address,
        enrollment_address,
        enrollment_token_file,
        service_type,
        data_dir,
    ) {
        Ok(keys) => {
            if keys.is_some() {
                info!("CURVE authentication enabled");
            }
            keys
        }
        Err(e) => {
            warn!(
                "CURVE auth setup failed: {}. Attempting connection without CURVE.",
                e
            );
            None
        }
    }
}

/// Async-compatible version - wraps the blocking call in spawn_blocking.
/// Use this when called from async code where blocking would be problematic.
pub async fn setup_curve_auth_async(
    rpc_address: String,
    enrollment_address: String,
    enrollment_token_file: Option<std::path::PathBuf>,
    service_type: String,
    data_dir: std::path::PathBuf,
) -> Option<CurveKeys> {
    tokio::task::spawn_blocking(move || {
        setup_curve_auth(
            &rpc_address,
            &enrollment_address,
            enrollment_token_file.as_deref(),
            &service_type,
            &data_dir,
        )
    })
    .await
    .ok()
    .flatten()
}
