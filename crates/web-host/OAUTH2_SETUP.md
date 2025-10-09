# OAuth2 Setup Guide

This guide walks you through setting up OAuth2 authentication for mooR development.

## Quick Start

### 1. Get OAuth2 Credentials

Choose a provider (we recommend starting with GitHub for simplicity):

#### GitHub (Easiest)

1. Visit https://github.com/settings/developers
2. Click **"New OAuth App"**
3. Fill in:
   - Application name: `mooR Development`
   - Homepage URL: `http://localhost:8080`
   - Authorization callback URL: `http://localhost:8080/auth/oauth2/github/callback`
4. Click **"Register application"**
5. Copy the **Client ID**
6. Click **"Generate a new client secret"** and copy it

#### Google

1. Visit https://console.cloud.google.com/
2. Create a new project or select existing
3. Enable the **Google+ API**
4. Go to **Credentials** → **Create Credentials** → **OAuth 2.0 Client ID**
5. Configure OAuth consent screen (External, add scopes: openid, email, profile)
6. Application type: **Web application**
7. Add Authorized redirect URI: `http://localhost:8080/auth/oauth2/google/callback`
8. Copy **Client ID** and **Client Secret**

#### Discord

1. Visit https://discord.com/developers/applications
2. Click **"New Application"**
3. Go to **OAuth2** section
4. Copy **Client ID**
5. Click **"Reset Secret"** and copy the new secret
6. Add redirect: `http://localhost:8080/auth/oauth2/discord/callback`

### 2. Configure mooR

Copy the example config and add your credentials:

```bash
# From the project root
cp crates/web-host/web-host-oauth2-example.yaml my-oauth2.yaml
# Edit my-oauth2.yaml and add your client_id and client_secret
```

**Note**: Two example configs are provided:

- `web-host-oauth2-minimal.yaml` - Minimal config with just GitHub
- `web-host-oauth2-example.yaml` - Full example with all providers (Google, GitHub, Discord)

**For GitHub example:**

```yaml
oauth2:
  enabled: true
  base_url: "http://localhost:8080"
  providers:
    github:
      client_id: "Iv1.abc123def456"  # Your GitHub Client ID
      client_secret: "abc123...xyz"   # Your GitHub Client Secret
      # ... rest stays the same
```

### 3. Run with OAuth2

Use the npm script:

```bash
npm run full:dev:oauth2
```

Or manually:

```bash
# Terminal 1: Daemon
npm run daemon:dev

# Terminal 2: Web-host with OAuth2
MOOR_OAUTH2_CONFIG=my-oauth2.yaml npm run web-host:dev:oauth2

# Terminal 3: Web client
npm run dev
```

### 4. Test OAuth2 Login

1. Visit http://localhost:3000
2. You should see "Sign in with GitHub" (or Google/Discord) button
3. Click it and complete OAuth2 flow
4. You'll be redirected back to create a new account

## Configuration Options

### Environment Variables

- `MOOR_OAUTH2_CONFIG` - Path to OAuth2 config file (default: `web-host-oauth2.yaml`)
- `MOOR_CORE` - Path to MOO core to import (used by daemon)

### Config File Structure

```yaml
# Standard web-host settings
listen_address: "0.0.0.0:8080"
debug: true
rpc_address: "tcp://127.0.0.1:7899"
events_address: "tcp://127.0.0.1:7898"
public_key: "moor-data/moor_host.pem.pub"
private_key: "moor-data/moor_host.pem"

# OAuth2 settings
oauth2:
  enabled: true
  base_url: "http://localhost:8080"  # Must match your deployment
  providers:
    # Add providers here
```

### CLI Override

You can override config file settings with CLI args:

```bash
cargo run -p moor-web-host -- \
  --config-file my-oauth2.yaml \
  --listen-address 0.0.0.0:9090  # Override listen address
```

## Troubleshooting

### "OAuth2 not enabled" error

- Check that `oauth2.enabled: true` in config
- Restart web-host after config changes

### "Invalid redirect URI" error

- Ensure redirect URI in provider settings exactly matches:
  `{base_url}/auth/oauth2/{provider}/callback`
- No trailing slashes
- Check http vs https

### OAuth2 buttons not showing in UI

- Check browser console for errors
- Verify web-client built successfully
- Try hard refresh (Ctrl+Shift+R)

### "Failed to fetch user info" error

- Verify `user_info_url` is correct for provider
- Check that scopes include necessary permissions (email, profile, etc.)

## Production Deployment

For production:

1. **Use HTTPS** (required by most OAuth2 providers)
2. **Update base_url** to your domain:
   ```yaml
   oauth2:
     base_url: "https://yourgame.com"
   ```

3. **Update redirect URIs** in all OAuth2 providers to match

4. **Secure secrets** using environment variables:
   ```yaml
   oauth2:
     providers:
       google:
         client_id: "${GOOGLE_CLIENT_ID}"
         client_secret: "${GOOGLE_CLIENT_SECRET}"
   ```

5. **Set environment variables** in your deployment:
   ```bash
   export GOOGLE_CLIENT_ID="your-client-id"
   export GOOGLE_CLIENT_SECRET="your-secret"
   ```

## Security Notes

- Never commit `client_secret` values to git
- Use `.gitignore` for `*-oauth2.yaml` config files
- Rotate secrets periodically
- Use different credentials for dev/staging/production

## Further Reading

- [OAuth 2.0 RFC](https://datatracker.ietf.org/doc/html/rfc6749)
- [Google OAuth2 Docs](https://developers.google.com/identity/protocols/oauth2)
- [GitHub OAuth Apps Docs](https://docs.github.com/en/developers/apps/building-oauth-apps)
- [Discord OAuth2 Docs](https://discord.com/developers/docs/topics/oauth2)
