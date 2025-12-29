# Web Client

mooR ships with a browser-based web client alongside the daemon and host services. The web client is bundled in release
images and package distributions, so you do not need a separate build step to serve it in production.

## What It Is

The web client is a rich UI that connects to `moor-web-host` over HTTPS and WebSockets. It is designed to provide a modern
MOO experience with persistent history, rich content rendering, and UI panels that can be driven from in-world code.

The web client is optional. mooR still supports classical telnet/MUD clients, and the web client exists to augment and
demonstrate what is possible with richer protocols and UI surfaces.

## How It Is Served

In production deployments, the static web client assets are served by nginx, which also proxies API requests to
`moor-web-host`. In development, Vite serves the web client directly.

## Communication Model

The web client integrates tightly with the `moor-web-host` API. It uses REST endpoints for authentication and data access
and WebSockets for real-time narrative events. The client/server payloads are encoded with FlatBuffers for efficiency and
schema evolution.

If you are using the official release images or Debian packages, the web client assets are already included.

## Core Capabilities

The web client includes rich content rendering, interactive link handling, panel-based UI presentations, and integrated
builder tooling. It can also participate in encrypted event logging when enabled.

## Web Client Topics

- [Deployment](./deployment.md)
- [OAuth2 Authentication](./oauth2-authentication.md)
- [Authoring and Programming Tools](./authoring-tools.md)
- [Client Output and Presentations](./client-output-and-presentations.md)
- [Accessibility](./accessibility.md)
- [Presentations](./presentations.md)

## Related Documentation

- [Server Architecture](../the-system/server-architecture.md)
- [Event Logging](../the-system/event-logging.md)
- [Networking](../the-moo-programming-language/networking.md)
- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
