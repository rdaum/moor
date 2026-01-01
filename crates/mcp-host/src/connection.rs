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

//! Connection manager for multiple mooR connections
//!
//! Manages lazy creation of programmer and wizard connections, allowing
//! operations to use appropriate privilege levels.

use crate::moor_client::{MoorClient, MoorClientConfig};
use eyre::{Result, eyre};
use tracing::{debug, error, info, warn};

/// Credentials for connecting to the mooR daemon
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Configuration for the connection manager
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub client_config: MoorClientConfig,
    pub programmer_credentials: Option<Credentials>,
    pub wizard_credentials: Option<Credentials>,
}

/// Manages lazy creation of programmer and wizard connections
pub struct ConnectionManager {
    config: ConnectionConfig,
    programmer_client: Option<MoorClient>,
    wizard_client: Option<MoorClient>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            programmer_client: None,
            wizard_client: None,
        }
    }

    /// Get the programmer (default) client, creating it lazily if needed
    ///
    /// If the connection exists but has become stale, automatically reconnects.
    pub async fn programmer(&mut self) -> Result<&mut MoorClient> {
        self.ensure_healthy_connection(false).await?;
        Ok(self.programmer_client.as_mut().unwrap())
    }

    /// Get the wizard client, creating it lazily if needed
    ///
    /// If the connection exists but has become stale, automatically reconnects.
    pub async fn wizard(&mut self) -> Result<&mut MoorClient> {
        self.ensure_healthy_connection(true).await?;
        Ok(self.wizard_client.as_mut().unwrap())
    }

    /// Get the appropriate client based on whether wizard privileges are requested
    ///
    /// Falls back to programmer connection if wizard is not configured.
    /// Verifies connection health and auto-reconnects if needed.
    pub async fn get(&mut self, wizard: bool) -> Result<&mut MoorClient> {
        if wizard {
            // Check if wizard credentials are configured
            if self.config.wizard_credentials.is_some() {
                return self.wizard().await;
            }
            // Fall back to programmer with a warning
            warn!("Wizard connection requested but not configured, using programmer connection");
        }
        self.programmer().await
    }

    /// Ensure we have a healthy connection, creating or reconnecting as needed
    async fn ensure_healthy_connection(&mut self, wizard: bool) -> Result<()> {
        let role = if wizard { "wizard" } else { "programmer" };

        // Check if client exists
        let client_exists = if wizard {
            self.wizard_client.is_some()
        } else {
            self.programmer_client.is_some()
        };

        // If no client exists, create one
        if !client_exists {
            debug!("No {} client exists, creating new connection", role);
            let client = self.create_and_connect(wizard).await?;
            if wizard {
                self.wizard_client = Some(client);
            } else {
                self.programmer_client = Some(client);
            }
            return Ok(());
        }

        // Client exists, verify it's healthy
        debug!("Checking {} connection health...", role);
        let health_result = {
            let client = if wizard {
                self.wizard_client.as_ref().unwrap()
            } else {
                self.programmer_client.as_ref().unwrap()
            };
            client.check_connection().await
        };

        match health_result {
            Ok(()) => {
                debug!("{} connection is healthy", role);
                Ok(())
            }
            Err(e) => {
                warn!(
                    "{} connection health check failed: {}. Attempting reconnect...",
                    role, e
                );

                // Try to reconnect
                let reconnect_result = {
                    let client = if wizard {
                        self.wizard_client.as_mut().unwrap()
                    } else {
                        self.programmer_client.as_mut().unwrap()
                    };
                    client.reconnect_with_backoff(3).await
                };

                match reconnect_result {
                    Ok(()) => {
                        info!("{} connection restored", role);
                        Ok(())
                    }
                    Err(reconnect_err) => {
                        error!(
                            "Failed to restore {} connection: {}. Recreating client...",
                            role, reconnect_err
                        );

                        // Last resort: recreate the client entirely
                        if wizard {
                            self.wizard_client = None;
                        } else {
                            self.programmer_client = None;
                        }
                        let new_client = self.create_and_connect(wizard).await?;
                        if wizard {
                            self.wizard_client = Some(new_client);
                        } else {
                            self.programmer_client = Some(new_client);
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    /// Check if any client is authenticated
    #[allow(dead_code)]
    pub fn is_authenticated(&self) -> bool {
        self.programmer_client
            .as_ref()
            .is_some_and(|c| c.is_authenticated())
            || self
                .wizard_client
                .as_ref()
                .is_some_and(|c| c.is_authenticated())
    }

    /// Check if programmer client is authenticated
    #[allow(dead_code)]
    pub fn programmer_is_authenticated(&self) -> bool {
        self.programmer_client
            .as_ref()
            .is_some_and(|c| c.is_authenticated())
    }

    /// Check if wizard client is authenticated
    #[allow(dead_code)]
    pub fn wizard_is_authenticated(&self) -> bool {
        self.wizard_client
            .as_ref()
            .is_some_and(|c| c.is_authenticated())
    }

    /// Get programmer player object if authenticated
    #[allow(dead_code)]
    pub fn programmer_player(&self) -> Option<&moor_var::Obj> {
        self.programmer_client.as_ref().and_then(|c| c.player())
    }

    /// Get wizard player object if authenticated
    #[allow(dead_code)]
    pub fn wizard_player(&self) -> Option<&moor_var::Obj> {
        self.wizard_client.as_ref().and_then(|c| c.player())
    }

    /// Check if wizard credentials are configured
    pub fn has_wizard_credentials(&self) -> bool {
        self.config.wizard_credentials.is_some()
    }

    /// Check if programmer credentials are configured
    pub fn has_programmer_credentials(&self) -> bool {
        self.config.programmer_credentials.is_some()
    }

    /// Attempt to reconnect the specified connection
    pub async fn reconnect(&mut self, wizard: bool) -> Result<()> {
        if wizard {
            if let Some(client) = &mut self.wizard_client {
                client.reconnect_with_backoff(3).await
            } else {
                Err(eyre!("Wizard connection not established"))
            }
        } else if let Some(client) = &mut self.programmer_client {
            client.reconnect_with_backoff(3).await
        } else {
            Err(eyre!("Programmer connection not established"))
        }
    }

    /// Reconnect all established connections
    ///
    /// Clears stale connection state and re-establishes all connections.
    /// Returns info about which connections were reconnected.
    pub async fn reconnect_all(&mut self) -> Result<String> {
        let mut results = Vec::new();

        if let Some(desc) = self.try_reconnect_one(false).await {
            results.push(desc);
        }
        if let Some(desc) = self.try_reconnect_one(true).await {
            results.push(desc);
        }

        if results.is_empty() {
            return Ok("No connections to reconnect".to_string());
        }
        Ok(format!("Reconnected: {}", results.join(", ")))
    }

    /// Attempt to reconnect a single client, returning a description on success
    async fn try_reconnect_one(&mut self, wizard: bool) -> Option<String> {
        let role = if wizard { "wizard" } else { "programmer" };

        // Check if client exists and try reconnecting (limited borrow scope)
        let reconnect_result = {
            let client = if wizard {
                self.wizard_client.as_mut()?
            } else {
                self.programmer_client.as_mut()?
            };
            client.reconnect_with_backoff(3).await
        };

        if reconnect_result.is_ok() {
            let client = if wizard {
                self.wizard_client.as_ref().unwrap()
            } else {
                self.programmer_client.as_ref().unwrap()
            };
            let player = client
                .player()
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Some(format!("{} ({})", role, player));
        }

        // Reconnect failed, try recreating from scratch
        warn!("Failed to reconnect {} client, recreating...", role);

        if wizard {
            self.wizard_client = None;
        } else {
            self.programmer_client = None;
        }

        let Ok(new_client) = self.create_and_connect(wizard).await else {
            error!("Failed to recreate {} client", role);
            return None;
        };

        let player = new_client
            .player()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let result = format!("{} ({}) [recreated]", role, player);

        if wizard {
            self.wizard_client = Some(new_client);
        } else {
            self.programmer_client = Some(new_client);
        }

        Some(result)
    }

    /// Gracefully disconnect all active connections
    pub async fn disconnect_all(&mut self) {
        if let Some(client) = &mut self.programmer_client
            && let Err(e) = client.disconnect().await
        {
            warn!("Error disconnecting programmer client: {}", e);
        }
        if let Some(client) = &mut self.wizard_client
            && let Err(e) = client.disconnect().await
        {
            warn!("Error disconnecting wizard client: {}", e);
        }
        self.programmer_client = None;
        self.wizard_client = None;
        info!("All connections disconnected");
    }

    /// Create and connect a client
    async fn create_and_connect(&self, wizard: bool) -> Result<MoorClient> {
        let credentials = if wizard {
            self.config.wizard_credentials.as_ref()
        } else {
            self.config.programmer_credentials.as_ref()
        };

        let role = if wizard { "wizard" } else { "programmer" };

        let credentials = credentials.ok_or_else(|| eyre!("No {} credentials configured", role))?;

        info!("Creating {} connection...", role);
        let mut client = MoorClient::new(self.config.client_config.clone())?;

        info!("Connecting {} client to mooR daemon...", role);
        client.connect().await?;

        info!("Authenticating as {} ({})...", credentials.username, role);
        client
            .login(&credentials.username, &credentials.password)
            .await?;

        info!(
            "Successfully connected as {} ({})",
            credentials.username, role
        );
        Ok(client)
    }
}
