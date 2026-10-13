# moor-web-host

TLDR: HTTP, WebSocket, and browser-facing host for mooR sessions and web APIs.

Downstream uses:

- Used by `moor-server` as an embedded host in single-process deployments.
- Can talk to a runtime through `moor-runtime-api` and `moor-zmq-client`.
- Keep web protocol, request routing, and browser session handling here; shared runtime contracts
  belong in `moor-runtime-api`.

## Structured client actions

A client can submit an action with context that the core must check before execution. For example,
an exit link can identify the room and connection that the player saw.

Send a binary WebSocket frame containing byte `0x04`, followed immediately by a UTF-8 JSON object.
For SSE sessions, POST that JSON object to `/v1/session/command` with
`Content-Type: application/json` and the session's authentication headers. The JSON body is limited
to 16 KiB.

The host calls the configured handler object's `do_action(payload)` method on the authenticated
connection. JSON objects become MOO maps with string keys. Booleans become `1` or `0`; null becomes
`#-1`. The core validates the payload and performs the operation with the caller's authority. The
host does not interpret exit identities, destinations, or other world-specific fields.

Plain text still follows the normal command path. Structured actions remain separate from replies to
`read()`. The SDK and WebSocket host queue actions while a prompt is waiting for input.
