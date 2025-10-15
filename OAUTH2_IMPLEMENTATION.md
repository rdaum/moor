# OAuth2 Implementation for mooR

This document describes the OAuth2 authentication implementation for mooR web-host and web-client.

## Overview

OAuth2 support allows users to authenticate using Google, GitHub, or Discord accounts instead of
traditional username/password authentication. The implementation keeps OAuth2 complexity in the host
layer, with minimal changes required in the MOO core.

## Architecture

### Backend (web-host)

**Files created/modified:**

- `crates/web-host/src/host/oauth2.rs` - OAuth2Manager and configuration
- `crates/web-host/src/host/oauth2_handlers.rs` - HTTP endpoint handlers
- `crates/web-host/src/main.rs` - OAuth2 router integration
- `crates/web-host/src/host/mod.rs` - Module exports

**Key components:**

1. **OAuth2Manager** - Manages provider configurations and OAuth2 flows
2. **OAuth2 HTTP endpoints:**
   - `GET /auth/oauth2/:provider/authorize` - Get authorization URL
   - `GET /auth/oauth2/:provider/callback` - Handle provider callback
   - `POST /auth/oauth2/account` - Complete account creation/linking

**Provider support:**

- Google OAuth2
- GitHub OAuth2
- Discord OAuth2

### Frontend (web-client)

**Files created/modified:**

- `web-client/src/lib/oauth2.ts` - OAuth2 API client functions
- `web-client/src/components/OAuth2Buttons.tsx` - OAuth2 provider buttons
- `web-client/src/components/Login.tsx` - Updated to show OAuth2 options
- `web-client/src/main.tsx` - OAuth2 callback handling

## OAuth2 Flow

### New User (Account Creation)

1. User clicks OAuth2 provider button (e.g., "Sign in with Google")
2. Frontend calls `/auth/oauth2/google/authorize`
3. Backend returns authorization URL
4. User is redirected to Google for authentication
5. Google redirects back to `/auth/oauth2/google/callback?code=...&state=...`
6. Backend exchanges code for access token
7. Backend fetches user info from Google
8. Backend checks if OAuth2 identity exists (calls MOO with `oauth2_check`)
9. If new user, backend redirects to frontend with user info
10. Frontend shows account creation options
11. User chooses username and submits
12. Frontend calls `/auth/oauth2/account` with mode=`oauth2_create`
13. Backend calls MOO with `do_login_command("oauth2_create", ...)`
14. MOO creates new player and links OAuth2 identity
15. Backend returns auth token
16. User is logged in

### Existing User (Login)

Steps 1-8 same as above, then:

9. If existing user, backend returns auth token directly
10. User is redirected to frontend with auth_token
11. User is logged in immediately

### Linking OAuth2 to Existing Account

1. User follows steps 1-9 above (new OAuth2 identity)
2. Frontend shows option to link to existing account
3. User enters existing username/password
4. Frontend calls `/auth/oauth2/account` with mode=`oauth2_connect`
5. Backend validates existing credentials and links OAuth2 identity
6. User is logged in

## Configuration

### Development Setup

#### 1. Get OAuth2 Credentials

**Google:**

1. Go to https://console.cloud.google.com/
2. Create project → Enable Google+ API
3. Create OAuth Client ID (Web application)
4. Add redirect URI: `http://localhost:8080/auth/oauth2/google/callback`
5. Copy Client ID and Client Secret

**GitHub:**

1. Go to https://github.com/settings/developers
2. New OAuth App
3. Callback URL: `http://localhost:8080/auth/oauth2/github/callback`
4. Copy Client ID and Client Secret

**Discord:**

1. Go to https://discord.com/developers/applications
2. New Application → OAuth2
3. Add redirect: `http://localhost:8080/auth/oauth2/discord/callback`
4. Copy Client ID and Client Secret

#### 2. Configure web-host

Create `crates/web-host/my-config.yaml`:

```yaml
listen_address: "0.0.0.0:8080"
rpc_address: "tcp://127.0.0.1:7899"
events_address: "tcp://127.0.0.1:7898"
public_key: "path/to/public_key.pem"
private_key: "path/to/private_key.pem"

oauth2:
  enabled: true
  base_url: "http://localhost:8080"
  providers:
    google:
      client_id: "YOUR_GOOGLE_CLIENT_ID.apps.googleusercontent.com"
      client_secret: "YOUR_GOOGLE_CLIENT_SECRET"
      auth_url: "https://accounts.google.com/o/oauth2/v2/auth"
      token_url: "https://oauth2.googleapis.com/token"
      user_info_url: "https://www.googleapis.com/oauth2/v3/userinfo"
      scopes:
        - "openid"
        - "email"
        - "profile"

    github:
      client_id: "YOUR_GITHUB_CLIENT_ID"
      client_secret: "YOUR_GITHUB_CLIENT_SECRET"
      auth_url: "https://github.com/login/oauth/authorize"
      token_url: "https://github.com/login/oauth/access_token"
      user_info_url: "https://api.github.com/user"
      scopes:
        - "read:user"
        - "user:email"

    discord:
      client_id: "YOUR_DISCORD_CLIENT_ID"
      client_secret: "YOUR_DISCORD_CLIENT_SECRET"
      auth_url: "https://discord.com/api/oauth2/authorize"
      token_url: "https://discord.com/api/oauth2/token"
      user_info_url: "https://discord.com/api/users/@me"
      scopes:
        - "identify"
        - "email"
```

#### 3. Run web-host

```bash
cargo run -p moor-web-host -- --config-file crates/web-host/my-config.yaml
```

#### 4. Run web-client (Vite dev server)

```bash
cd web-client
npm run dev
```

The Vite dev server runs on port 3000 and proxies `/auth` requests to port 8080.

### Production Setup

For production:

1. Use HTTPS (required by OAuth2 providers)
2. Update `base_url` to your domain: `https://yourdomain.com`
3. Update all OAuth2 provider redirect URIs to match
4. Store client secrets securely (environment variables, secrets manager)

Example production config:

```yaml
oauth2:
  enabled: true
  base_url: "https://yourgame.com"
  providers:
    google:
      client_id: "${GOOGLE_CLIENT_ID}"
      client_secret: "${GOOGLE_CLIENT_SECRET}"
      # ... rest same as development
```

## MOO Integration

The MOO core needs to implement OAuth2 support in `do_login_command`. The web-host sends these
command modes:

### `oauth2_check`

Check if OAuth2 identity exists:

```
do_login_command("oauth2_check", provider, external_id)
```

Returns: LoginResult with success=true if user exists

### `oauth2_create`

Create new player with OAuth2 identity:

```
do_login_command("oauth2_create", provider, external_id, email, name, username, player_name)
```

Returns: LoginResult with auth_token and player

### `oauth2_connect`

Link OAuth2 identity to existing account:

```
do_login_command("oauth2_connect", provider, external_id, email, name, username, "", existing_email, existing_password)
```

Returns: LoginResult with auth_token and player

## Security Considerations

1. **CSRF Protection**: OAuth2 state parameter validated (stored in sessionStorage)
2. **Client Secrets**: Never exposed to frontend, only used server-side
3. **HTTPS Required**: Production must use HTTPS for OAuth2
4. **localhost Exception**: Google/GitHub/Discord allow `http://localhost` for development

## Testing

### Manual Testing

1. Start moor-daemon with a database
2. Start web-host with OAuth2 config
3. Start web-client (Vite)
4. Visit `http://localhost:3000`
5. Click "Sign in with Google" (or GitHub/Discord)
6. Complete OAuth2 flow
7. Create account or link to existing account
8. Verify login successful

### Direct API Testing

Test authorization URL:

```bash
curl http://localhost:8080/auth/oauth2/google/authorize
```

## Future Enhancements

1. **Direct token login**: Update AuthContext to support setting auth token directly (for faster
   OAuth2 login)
2. **Account management UI**: Allow users to link/unlink OAuth2 providers from settings
3. **Profile sync**: Option to sync email/name from OAuth2 provider
4. **More providers**: Support for Microsoft, Apple, etc.
5. **Frontend OAuth2 account choice UI**: Modal dialog for create vs. link choice

## Troubleshooting

**"OAuth2 not enabled" error:**

- Check `oauth2.enabled: true` in config
- Restart web-host after config changes

**"Invalid redirect URI" error:**

- Ensure redirect URI in provider matches exactly: `{base_url}/auth/oauth2/{provider}/callback`
- Check for trailing slashes, http vs https

**"Failed to fetch user info" error:**

- Verify user_info_url is correct for provider
- Check that scopes include necessary permissions

**OAuth2 buttons not showing:**

- Check browser console for errors
- Verify OAuth2Buttons component imported in Login.tsx
- Check Vite dev server is proxying `/auth` correctly
