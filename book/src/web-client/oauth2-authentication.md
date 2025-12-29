# OAuth2 Authentication

The web client can work with `moor-web-host` to authenticate players using OAuth2 providers such as Discord, GitHub, or
Google. Which providers are available depends on the web-host configuration for your deployment. OAuth2 flows run through
the web-host API and complete back in the web client.

## How It Works

- The login screen offers OAuth2 buttons when providers are enabled on the web host.
- The web client redirects the browser to the provider for authorization.
- The provider redirects back to the web client, which completes the flow with `moor-web-host`.
- The player can create a new character or link to an existing character after OAuth2 success.

The resulting session uses the same PASETO authentication tokens as username/password login, so the rest of the client
behavior is identical.

## Notes for Operators

- OAuth2 providers are configured on the web-host side, not in the web client.
- If a provider is not configured, its login button is not shown.
- OAuth2 account creation can prompt for the event-log encryption password if event logging is enabled.

Related docs:

- [Server Architecture](../the-system/server-architecture.md)
- [Event Logging](../the-system/event-logging.md)
