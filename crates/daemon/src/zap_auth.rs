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

//! ZAP (ZeroMQ Authentication Protocol) handler for CURVE authentication
//!
//! Implements RFC 27 (https://rfc.zeromq.org/spec/27/) to validate client public keys
//! against the AllowedHostsRegistry.

use crate::allowed_hosts::AllowedHostsRegistry;
use eyre::{Context, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{debug, error, info, warn};

/// ZAP authentication handler
///
/// Binds to inproc://zeromq.zap.01 and validates CURVE client public keys
/// against the AllowedHostsRegistry.
pub struct ZapAuthHandler {
    zmq_context: zmq::Context,
    kill_switch: Arc<AtomicBool>,
    allowed_hosts: AllowedHostsRegistry,
}

impl ZapAuthHandler {
    /// Create a new ZAP authentication handler
    pub fn new(
        zmq_context: zmq::Context,
        kill_switch: Arc<AtomicBool>,
        allowed_hosts: AllowedHostsRegistry,
    ) -> Self {
        Self {
            zmq_context,
            kill_switch,
            allowed_hosts,
        }
    }

    /// Start the ZAP authentication handler
    ///
    /// This is a blocking call that runs until the kill switch is activated.
    /// Must be called before any CURVE-enabled sockets are created.
    pub fn run(&self) -> Result<()> {
        let socket = self
            .zmq_context
            .socket(zmq::REP)
            .context("Failed to create ZAP handler socket")?;

        // Bind to the ZAP endpoint
        socket
            .bind("inproc://zeromq.zap.01")
            .context("Failed to bind to ZAP endpoint")?;

        info!("ZAP authentication handler started");

        loop {
            if self.kill_switch.load(Ordering::Relaxed) {
                info!("ZAP authentication handler shutting down");
                break;
            }

            // Poll with timeout so we can check kill switch
            let poll_result = socket
                .poll(zmq::POLLIN, 1000)
                .context("Failed to poll ZAP socket")?;

            if poll_result == 0 {
                continue; // Timeout, check kill switch again
            }

            // Process ZAP request
            if let Err(e) = self.handle_request(&socket) {
                error!(error = ?e, "Error handling ZAP request");
            }
        }

        Ok(())
    }

    /// Handle a single ZAP authentication request
    ///
    /// ZAP request format (RFC 27):
    /// Frame 0: version (should be "1.0")
    /// Frame 1: request_id (opaque request ID)
    /// Frame 2: domain (ZAP domain for partitioning auth)
    /// Frame 3: address (client IP address)
    /// Frame 4: identity (ZMQ identity if set)
    /// Frame 5: mechanism ("CURVE", "PLAIN", or "NULL")
    /// Frame 6+: credentials (for CURVE: 32-byte public key)
    fn handle_request(&self, socket: &zmq::Socket) -> Result<()> {
        // Receive ZAP request (multi-part message)
        let version = socket
            .recv_string(0)
            .context("Failed to receive version")?
            .map_err(|_| eyre::eyre!("Version not valid UTF-8"))?;

        let request_id = socket
            .recv_string(0)
            .context("Failed to receive request_id")?
            .map_err(|_| eyre::eyre!("Request ID not valid UTF-8"))?;

        let domain = socket
            .recv_string(0)
            .context("Failed to receive domain")?
            .map_err(|_| eyre::eyre!("Domain not valid UTF-8"))?;

        let address = socket
            .recv_string(0)
            .context("Failed to receive address")?
            .map_err(|_| eyre::eyre!("Address not valid UTF-8"))?;

        let _identity = socket
            .recv_string(0)
            .context("Failed to receive identity")?
            .map_err(|_| eyre::eyre!("Identity not valid UTF-8"))?;

        let mechanism = socket
            .recv_string(0)
            .context("Failed to receive mechanism")?
            .map_err(|_| eyre::eyre!("Mechanism not valid UTF-8"))?;

        let credentials = socket
            .recv_bytes(0)
            .context("Failed to receive credentials")?;

        debug!(
            version = %version,
            request_id = %request_id,
            domain = %domain,
            address = %address,
            mechanism = %mechanism,
            "Received ZAP authentication request"
        );

        // Validate version
        if version != "1.0" {
            warn!(version = %version, "Unsupported ZAP version");
            return self.send_reply(
                socket,
                &version,
                &request_id,
                "400",
                "Unsupported version",
                "",
            );
        }

        // Only support CURVE mechanism
        if mechanism != "CURVE" {
            warn!(mechanism = %mechanism, "Unsupported mechanism");
            return self.send_reply(
                socket,
                &version,
                &request_id,
                "400",
                "Only CURVE is supported",
                "",
            );
        }

        // For CURVE, credentials is the client's 32-byte public key
        if credentials.len() != 32 {
            warn!(
                credentials_len = credentials.len(),
                "Invalid CURVE credentials length"
            );
            return self.send_reply(
                socket,
                &version,
                &request_id,
                "400",
                "Invalid credentials",
                "",
            );
        }

        // Convert raw bytes to Z85 encoding for lookup
        let client_public_key =
            zmq::z85_encode(&credentials).context("Failed to Z85-encode client public key")?;

        debug!(
            client_public_key = %client_public_key,
            "Checking if client is authorized"
        );

        // Check if this public key is authorized
        match self.allowed_hosts.is_authorized(&client_public_key) {
            Some(uuid) => {
                info!(
                    client_public_key = %client_public_key,
                    service_uuid = %uuid,
                    address = %address,
                    "Authorized CURVE connection"
                );
                self.send_reply(
                    socket,
                    &version,
                    &request_id,
                    "200",
                    "OK",
                    &uuid.to_string(),
                )
            }
            None => {
                warn!(
                    client_public_key = %client_public_key,
                    address = %address,
                    "Unauthorized CURVE connection attempt"
                );
                self.send_reply(socket, &version, &request_id, "400", "Unauthorized", "")
            }
        }
    }

    /// Send ZAP authentication reply
    ///
    /// ZAP reply format (RFC 27):
    /// Frame 0: version (echoed from request)
    /// Frame 1: request_id (echoed from request)
    /// Frame 2: status_code ("200" = OK, "400" = denied, "500" = error)
    /// Frame 3: status_text (human-readable description)
    /// Frame 4: user_id (application-defined user identifier)
    /// Frame 5: metadata (application-defined metadata)
    fn send_reply(
        &self,
        socket: &zmq::Socket,
        version: &str,
        request_id: &str,
        status_code: &str,
        status_text: &str,
        user_id: &str,
    ) -> Result<()> {
        socket
            .send(version, zmq::SNDMORE)
            .context("Failed to send version")?;
        socket
            .send(request_id, zmq::SNDMORE)
            .context("Failed to send request_id")?;
        socket
            .send(status_code, zmq::SNDMORE)
            .context("Failed to send status_code")?;
        socket
            .send(status_text, zmq::SNDMORE)
            .context("Failed to send status_text")?;
        socket
            .send(user_id, zmq::SNDMORE)
            .context("Failed to send user_id")?;
        socket
            .send("", 0) // Empty metadata frame
            .context("Failed to send metadata")?;

        Ok(())
    }
}
