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

//! OAuth2 authentication support for web-host

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl, basic::BasicClient,
};
use rpc_common::{AuthToken, ClientToken};
use serde_derive::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use uuid::Uuid;

/// Configuration for a single OAuth2 provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2ProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub scopes: Vec<String>,
}

/// OAuth2 configuration section from YAML
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuth2Config {
    pub enabled: bool,
    pub base_url: String,
    #[serde(default)]
    pub cookie_secure: Option<bool>,
    #[serde(default)]
    pub allowed_app_redirect_uri_prefixes: Vec<String>,
    pub providers: HashMap<String, OAuth2ProviderConfig>,
}

/// External user information retrieved from OAuth2 provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub external_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
}

const CSRF_TOKEN_TTL: Duration = Duration::from_secs(600); // 10 minutes
const PENDING_CODE_TTL: Duration = Duration::from_secs(120); // 2 minutes

#[derive(Clone)]
pub enum FlowBinding {
    Cookie {
        browser_nonce: String,
    },
    Proof {
        redirect_uri: String,
        code_challenge: String,
        code_challenge_method: String,
        intent: Option<String>,
    },
}

#[derive(Clone)]
struct PendingCsrfEntry {
    binding: FlowBinding,
    created: Instant,
}

/// What a pending OAuth2 code resolves to when redeemed.
#[derive(Clone)]
pub enum PendingOAuth2Code {
    /// Existing user — contains a ready-to-use auth session.
    AuthSession {
        auth_token: AuthToken,
        player_curie: String,
        player_flags: u16,
        client_token: ClientToken,
        client_id: Uuid,
    },
    /// New user — contains the verified identity from the provider.
    Identity(ExternalUserInfo),
}

/// Entry in the pending-codes map.
#[derive(Clone)]
struct PendingEntry {
    payload: PendingOAuth2Code,
    binding: FlowBinding,
    created: Instant,
}

/// Server-side store for OAuth2 pending state: CSRF tokens and one-time codes.
///
/// CSRF tokens are keyed as `"provider:token_value"` to bind them to the provider
/// that initiated the flow, preventing cross-provider mix-up attacks.
///
/// One-time codes are used for both the existing-user auth-session handoff and the
/// new-user identity handoff, so that neither auth tokens nor identity proof tokens
/// ever appear in redirect URL query parameters.
pub struct PendingOAuth2Store {
    /// CSRF tokens: "provider:token_value" -> browser-bound entry
    csrf_tokens: RwLock<HashMap<String, PendingCsrfEntry>>,
    /// One-time codes: code -> pending entry (auth session or identity)
    pending_codes: RwLock<HashMap<String, PendingEntry>>,
}

impl PendingOAuth2Store {
    pub fn new() -> Self {
        Self {
            csrf_tokens: RwLock::new(HashMap::new()),
            pending_codes: RwLock::new(HashMap::new()),
        }
    }

    /// Store a CSRF token bound to a provider.
    pub fn store_csrf_token(&self, provider: &str, token: &str, binding: FlowBinding) {
        let key = format!("{}:{}", provider, token);
        if let Ok(mut tokens) = self.csrf_tokens.write() {
            tokens.insert(
                key,
                PendingCsrfEntry {
                    binding,
                    created: Instant::now(),
                },
            );
        }
    }

    /// Validate and consume a CSRF token. Returns the binding details if valid.
    pub fn consume_csrf_token(
        &self,
        provider: &str,
        token: &str,
        browser_nonce: Option<&str>,
    ) -> Option<FlowBinding> {
        let key = format!("{}:{}", provider, token);
        let Ok(mut tokens) = self.csrf_tokens.write() else {
            return None;
        };
        let Some(entry) = tokens.remove(&key) else {
            return None;
        };
        if entry.created.elapsed() >= CSRF_TOKEN_TTL {
            return None;
        }
        match &entry.binding {
            FlowBinding::Cookie {
                browser_nonce: expected_nonce,
            } => {
                let provided_nonce = browser_nonce?;
                if expected_nonce == provided_nonce {
                    Some(entry.binding)
                } else {
                    None
                }
            }
            FlowBinding::Proof { .. } => Some(entry.binding),
        }
    }

    /// Store a pending entry (auth session or identity) and return the one-time code.
    pub fn store_pending_code(
        &self,
        payload: PendingOAuth2Code,
        binding: FlowBinding,
    ) -> Option<String> {
        let code = Uuid::new_v4().to_string();
        let entry = PendingEntry {
            payload,
            binding,
            created: Instant::now(),
        };
        let Ok(mut codes) = self.pending_codes.write() else {
            return None;
        };
        codes.insert(code.clone(), entry);
        Some(code)
    }

    /// Redeem a one-time code, consuming it. Returns the payload if valid and not expired.
    pub fn redeem_pending_code_cookie(
        &self,
        code: &str,
        browser_nonce: &str,
    ) -> Option<PendingOAuth2Code> {
        let Ok(mut codes) = self.pending_codes.write() else {
            return None;
        };
        let Some(entry) = codes.remove(code) else {
            return None;
        };
        if entry.created.elapsed() >= PENDING_CODE_TTL {
            return None;
        }
        match entry.binding {
            FlowBinding::Cookie {
                browser_nonce: expected_nonce,
            } if expected_nonce == browser_nonce => Some(entry.payload),
            _ => None,
        }
    }

    pub fn redeem_pending_code_proof(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Option<PendingOAuth2Code> {
        self.redeem_pending_code_proof_with_binding(code, code_verifier)
            .map(|(payload, _)| payload)
    }

    pub fn redeem_pending_code_proof_with_binding(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Option<(PendingOAuth2Code, FlowBinding)> {
        let Ok(mut codes) = self.pending_codes.write() else {
            return None;
        };
        let Some(entry) = codes.remove(code) else {
            return None;
        };
        if entry.created.elapsed() >= PENDING_CODE_TTL {
            return None;
        }
        match entry.binding {
            FlowBinding::Proof {
                code_challenge,
                code_challenge_method,
                redirect_uri,
                intent,
            } => verify_pkce(code_verifier, &code_challenge, &code_challenge_method).then_some((
                entry.payload,
                FlowBinding::Proof {
                    redirect_uri,
                    code_challenge,
                    code_challenge_method,
                    intent,
                },
            )),
            FlowBinding::Cookie { .. } => None,
        }
    }

    /// Reap expired entries from all stores.
    pub fn reap_expired(&self) {
        if let Ok(mut csrf) = self.csrf_tokens.write() {
            let before = csrf.len();
            csrf.retain(|_, entry| entry.created.elapsed() < CSRF_TOKEN_TTL);
            let reaped = before - csrf.len();
            if reaped > 0 {
                debug!("Reaped {} expired CSRF tokens", reaped);
            }
        }

        if let Ok(mut codes) = self.pending_codes.write() {
            let before = codes.len();
            codes.retain(|_, entry| entry.created.elapsed() < PENDING_CODE_TTL);
            let reaped = before - codes.len();
            if reaped > 0 {
                debug!("Reaped {} expired pending codes", reaped);
            }
        }
    }
}

// Type alias for a fully-configured OAuth2 client with auth and token URLs set
type ConfiguredClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    oauth2::StandardTokenIntrospectionResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
    oauth2::EndpointSet,    // HasAuthUrl
    oauth2::EndpointNotSet, // HasDeviceAuthUrl
    oauth2::EndpointNotSet, // HasIntrospectionUrl
    oauth2::EndpointNotSet, // HasRevocationUrl
    oauth2::EndpointSet,    // HasTokenUrl
>;

/// OAuth2 manager handles provider configurations and authentication flows
pub struct OAuth2Manager {
    config: OAuth2Config,
    clients: HashMap<String, ConfiguredClient>,
    http_client: reqwest::Client,
}

impl OAuth2Manager {
    /// Create a new OAuth2Manager from configuration
    pub fn new(config: OAuth2Config) -> Result<Self, eyre::Error> {
        if !config.enabled {
            return Ok(Self {
                config,
                clients: HashMap::new(),
                http_client: reqwest::Client::new(),
            });
        }

        let mut clients = HashMap::new();
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        // Initialize OAuth2 clients for each configured provider
        for (provider_name, provider_config) in &config.providers {
            debug!("Initializing OAuth2 provider: {}", provider_name);

            let client_id = ClientId::new(provider_config.client_id.clone());
            let client_secret = ClientSecret::new(provider_config.client_secret.clone());

            let auth_url = AuthUrl::new(provider_config.auth_url.clone())
                .map_err(|e| eyre::eyre!("Invalid auth URL for {}: {}", provider_name, e))?;

            let token_url = TokenUrl::new(provider_config.token_url.clone())
                .map_err(|e| eyre::eyre!("Invalid token URL for {}: {}", provider_name, e))?;

            let redirect_url =
                format!("{}/auth/oauth2/{}/callback", config.base_url, provider_name);
            let redirect_url = RedirectUrl::new(redirect_url)
                .map_err(|e| eyre::eyre!("Invalid redirect URL for {}: {}", provider_name, e))?;

            let client = BasicClient::new(client_id)
                .set_client_secret(client_secret)
                .set_auth_uri(auth_url)
                .set_token_uri(token_url)
                .set_redirect_uri(redirect_url);

            clients.insert(provider_name.clone(), client);
        }

        info!(
            "OAuth2 manager initialized with {} providers",
            clients.len()
        );

        Ok(Self {
            config,
            clients,
            http_client,
        })
    }

    /// Check if OAuth2 is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get list of available provider names
    pub fn available_providers(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Whether OAuth2 nonce cookies should include the `Secure` attribute.
    ///
    /// If `cookie_secure` is configured explicitly, use that value.
    /// Otherwise infer from `base_url` and enable only for `https://`.
    pub fn oauth_cookie_secure(&self) -> bool {
        self.config
            .cookie_secure
            .unwrap_or_else(|| self.config.base_url.starts_with("https://"))
    }

    pub fn app_redirect_allowed(&self, redirect_uri: &str) -> bool {
        !self.config.allowed_app_redirect_uri_prefixes.is_empty()
            && self
                .config
                .allowed_app_redirect_uri_prefixes
                .iter()
                .any(|prefix| redirect_uri.starts_with(prefix))
    }

    /// Generate an authorization URL for a provider
    pub fn get_authorization_url(
        &self,
        provider: &str,
    ) -> Result<(String, CsrfToken), eyre::Error> {
        let client = self
            .clients
            .get(provider)
            .ok_or_else(|| eyre::eyre!("Unknown OAuth2 provider: {}", provider))?;

        let provider_config = self
            .config
            .providers
            .get(provider)
            .ok_or_else(|| eyre::eyre!("Provider config not found: {}", provider))?;

        // Build authorization URL with scopes
        let mut auth_request = client.authorize_url(CsrfToken::new_random);

        for scope in &provider_config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        debug!(
            "Generated authorization URL for provider {}: {}",
            provider, auth_url
        );

        Ok((auth_url.to_string(), csrf_token))
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(&self, provider: &str, code: String) -> Result<String, eyre::Error> {
        let client = self
            .clients
            .get(provider)
            .ok_or_else(|| eyre::eyre!("Unknown OAuth2 provider: {}", provider))?;

        debug!("Exchanging authorization code for access token");

        let token_result = client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(&self.http_client)
            .await
            .map_err(|e| eyre::eyre!("Token exchange failed: {}", e))?;

        let access_token = token_result.access_token().secret().clone();

        debug!("Successfully exchanged code for access token");

        Ok(access_token)
    }

    /// Fetch user information from provider using access token
    pub async fn get_user_info(
        &self,
        provider: &str,
        access_token: &str,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        let provider_config = self
            .config
            .providers
            .get(provider)
            .ok_or_else(|| eyre::eyre!("Provider config not found: {}", provider))?;

        debug!(
            "Fetching user info from {} at {}",
            provider, provider_config.user_info_url
        );

        let response = self
            .http_client
            .get(&provider_config.user_info_url)
            .bearer_auth(access_token)
            .header("User-Agent", "moor-web-host/0.9.0")
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| eyre::eyre!("Failed to fetch user info: {}", e))?;

        if !response.status().is_success() {
            return Err(eyre::eyre!(
                "Failed to fetch user info: HTTP {}",
                response.status()
            ));
        }

        let user_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| eyre::eyre!("Failed to parse user info: {}", e))?;

        debug!("Received user info from provider");

        // Parse provider-specific user info into our standard format
        self.parse_user_info(provider, user_data)
    }

    /// Complete the full OAuth2 flow: exchange code and fetch user info
    pub async fn complete_oauth2_flow(
        &self,
        provider: &str,
        code: String,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        let access_token = self.exchange_code(provider, code).await?;
        self.get_user_info(provider, &access_token).await
    }

    /// Parse provider-specific user info JSON into our standard format
    fn parse_user_info(
        &self,
        provider: &str,
        data: serde_json::Value,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        match provider {
            "google" => self.parse_google_user_info(data),
            "github" => self.parse_github_user_info(data),
            "discord" => self.parse_discord_user_info(data),
            _ => Err(eyre::eyre!("Unknown provider: {}", provider)),
        }
    }

    fn parse_google_user_info(
        &self,
        data: serde_json::Value,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        let external_id = data["sub"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("Missing 'sub' field in Google user info"))?
            .to_string();

        Ok(ExternalUserInfo {
            provider: "google".to_string(),
            external_id,
            email: data["email"].as_str().map(String::from),
            name: data["name"].as_str().map(String::from),
            username: data["email"]
                .as_str()
                .map(|e| e.split('@').next().unwrap_or(e).to_string()),
        })
    }

    fn parse_github_user_info(
        &self,
        data: serde_json::Value,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        let external_id = data["id"]
            .as_i64()
            .ok_or_else(|| eyre::eyre!("Missing 'id' field in GitHub user info"))?
            .to_string();

        Ok(ExternalUserInfo {
            provider: "github".to_string(),
            external_id,
            email: data["email"].as_str().map(String::from),
            name: data["name"].as_str().map(String::from),
            username: data["login"].as_str().map(String::from),
        })
    }

    fn parse_discord_user_info(
        &self,
        data: serde_json::Value,
    ) -> Result<ExternalUserInfo, eyre::Error> {
        let external_id = data["id"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("Missing 'id' field in Discord user info"))?
            .to_string();

        Ok(ExternalUserInfo {
            provider: "discord".to_string(),
            external_id,
            email: data["email"].as_str().map(String::from),
            name: data["global_name"]
                .as_str()
                .or_else(|| data["username"].as_str())
                .map(String::from),
            username: data["username"].as_str().map(String::from),
        })
    }
}

fn verify_pkce(code_verifier: &str, expected_challenge: &str, method: &str) -> bool {
    if method != "S256" {
        return false;
    }

    let digest = Sha256::digest(code_verifier.as_bytes());
    let calculated = URL_SAFE_NO_PAD.encode(digest);
    calculated == expected_challenge
}
