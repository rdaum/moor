# @moor/web-sdk

Shared TypeScript SDK for mooR web-facing clients.

This package is intended to hold protocol-level logic shared by Meadow and other clients, while
UI/application-specific code remains in each client.

It provides TypesScript bindings to call the moor-web-host API.

## Scope

- Auth header helpers for mooR web-host
- HTTP endpoint wrappers
- WebSocket attach/reattach protocol helpers
- FlatBuffer decoding/encoding helpers

## Narrative decoding

`parseNarrativeValue` returns the shared `ParsedNarrativePayload` discriminated union for
notifications, presentations, tracebacks, unpresent events, and data events. WebSocket and history
parsing use this same decoder. Captured invocation output adapts the decoded payload to its
`eventType` envelope. Decoder callbacks receive the generated FlatBuffer `Var` type; decoded
notification content must be a string or an array of strings.

Notifications accept `text/plain`, `text/djot`, `text/html`, and `text/x-uri`, including their
underscore wire spellings. An omitted content type defaults to plain text. Explicit unsupported or
empty types return `null`, as do missing required payload fields and invalid notification content.
Presentations accept the same types except `text/x-uri`. `text/traceback` is an application
rendering type for traceback events, not a notification content type.

Optional metadata is checked before it enters typed fields. Opaque MOO values remain `unknown` until
a consumer narrows them. Malformed binary buffers can throw during FlatBuffer access or Var
decoding; callers must catch those failures at the transport boundary. Meadow discards invalid
live/history payloads and retains history pagination metadata independently of accepted events.

## Structured actions

Use `encodeAction(payload)` to retain an action's context when sending it through a connected
WebSocket or `SseSessionTransport`. The result is a binary frame. The core must provide a
`do_action(payload)` handler that validates the action as the authenticated player.

Action payloads are untrusted JSON objects, limited to 16 KiB. They do not grant permissions. A core
can use captured identities to reject links whose target has changed since the client displayed
them. Send typed commands as ordinary strings.

## 2.0 Development

This package is a private npm workspace during the 2.0 development cycle. Install dependencies and
run its build from the mooR repository root so npm resolves `@moor/schema` from the same checkout.

External package distribution will be reconsidered when the 2.0 API is ready for independent
clients. The monorepo does not publish development snapshots to an npm registry.

## License

`@moor/web-sdk` is licensed under `LGPL-3.0-or-later`. See `clients/web-sdk/LICENSE`. You can build
on top of it, but must also comply with the LGPL-3.0-or-later license if you modify the source code
to the library itself.

(The remainder of mooR is GPL 3.0)
