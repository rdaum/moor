# OAuth2 Authentication

The web client can work with `moor-web-host` to authenticate players using OAuth2 providers such as Discord, GitHub, or Google. Which providers are available depends on the web-host configuration for your deployment. OAuth2 flows run through the web-host API and complete back in the web client.

## How It Works

1. **Login Screen**: The login screen shows OAuth2 provider buttons (Discord, GitHub, Google) when enabled on the web host.

2. **Authorization**: Clicking a provider button redirects the browser to that provider's authorization page.

3. **Callback**: After authorization, the provider redirects back to the web client with an authorization code.

4. **Token Exchange**: The web client sends the code to `moor-web-host`, which exchanges it for user info.

5. **Account Choice**: The player can either:
   - Create a new character linked to their OAuth2 identity
   - Link to an existing character (requires existing credentials)

6. **Session**: The resulting session uses PASETO authentication tokens, identical to username/password login.

## Supported Providers

| Provider | Icon | Configuration Required |
|----------|------|------------------------|
| Discord | Discord logo | OAuth2 app in Discord Developer Portal |
| GitHub | GitHub logo | OAuth app in GitHub Settings |
| Google | Google logo | OAuth2 credentials in Google Cloud Console |

Providers only appear on the login screen if configured in `moor-web-host`.

## Account Creation with OAuth2

When creating a new account via OAuth2, the wizard guides the player through:

1. **Credentials**: Choose a character name (username is pre-filled from OAuth2 profile)
2. **Privacy Policy**: Accept site privacy policy (if configured)
3. **Encryption Info**: Learn about event log encryption (if enabled)
4. **Encryption Password**: Choose whether to use OAuth2 login password or a separate encryption password

### Encryption Password Options

If event logging is enabled, players can choose:

- **Use login password**: Convenient but ties encryption to the OAuth2 provider
- **Separate password**: Independent encryption key, more secure but requires remembering two passwords

## Linking to Existing Account

Players with an existing username/password account can link their OAuth2 identity:

1. Click the OAuth2 provider button
2. After OAuth2 authorization, choose "Link to existing account"
3. Enter existing username and password
4. The OAuth2 identity is now linked—future logins can use either method

## Notes for Operators

### Web-Host Configuration

OAuth2 providers are configured in `moor-web-host` configuration, not the web client. Each provider requires:

- Client ID from the OAuth2 provider
- Client Secret (kept server-side only)
- Redirect URI matching your deployment URL

Example web-host configuration:

```yaml
oauth2:
  discord:
    client_id: "your-discord-client-id"
    client_secret: "your-discord-client-secret"
  github:
    client_id: "your-github-client-id"
    client_secret: "your-github-client-secret"
```

### Provider Setup

Each provider has its own developer console for creating OAuth2 applications:

| Provider | Developer Console | Redirect URI Path |
|----------|-------------------|-------------------|
| Discord | discord.com/developers | `/auth/discord/callback` |
| GitHub | github.com/settings/developers | `/auth/github/callback` |
| Google | console.cloud.google.com | `/auth/google/callback` |

### Security Considerations

- OAuth2 secrets are never exposed to the web client
- The web-host validates all OAuth2 callbacks server-side
- PASETO tokens are used for session management after OAuth2 authentication
- Event log encryption remains independent of OAuth2 provider

## Troubleshooting

### Provider Button Not Showing

- Verify the provider is configured in web-host
- Check web-host logs for configuration errors
- Ensure the web-host is reachable

### Callback Errors

- Verify redirect URI matches exactly (including trailing slashes)
- Check that client ID and secret are correct
- Review provider's OAuth2 error messages

### Linking Fails

- Ensure the existing account credentials are correct
- The account may already be linked to a different OAuth2 identity
- Check web-host logs for detailed error information

## Related Docs

- [Server Architecture](../the-system/server-architecture.md)
- [Event Logging](../the-system/event-logging.md)
- [Server Configuration](../the-system/server-configuration.md)
