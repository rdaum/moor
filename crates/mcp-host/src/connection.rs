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

//! Connection manager for multiple mooR connections
//!
//! Manages lazy creation of programmer and wizard connections, allowing
//! operations to use appropriate privilege levels.

use crate::moor_client::{MoorClient, MoorClientConfig};
use eyre::{Result, eyre};
use tracing::{info, warn};

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
    pub async fn programmer(&mut self) -> Result<&mut MoorClient> {
        if self.programmer_client.is_none() {
            self.programmer_client = Some(self.create_and_connect(false).await?);
        }
        Ok(self.programmer_client.as_mut().unwrap())
    }

    /// Get the wizard client, creating it lazily if needed
    pub async fn wizard(&mut self) -> Result<&mut MoorClient> {
        if self.wizard_client.is_none() {
            self.wizard_client = Some(self.create_and_connect(true).await?);
        }
        Ok(self.wizard_client.as_mut().unwrap())
    }

    /// Get the appropriate client based on whether wizard privileges are requested
    ///
    /// Falls back to programmer connection if wizard is not configured.
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
