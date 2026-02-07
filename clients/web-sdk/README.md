# @moor/web-sdk

Shared TypeScript SDK for mooR web-facing clients.

This package is intended to hold protocol-level logic shared by Meadow and other clients, while
UI/application-specific code can remain in each client repository.

It provides TypesScript bindings to call the moor-web-host API.

## Scope

- Auth header helpers for mooR web-host
- HTTP endpoint wrappers
- WebSocket attach/reattach protocol helpers
- FlatBuffer decoding/encoding helpers

## Publishing

This package is published to the Codeberg npm registry under the `@moor` scope, following the same
release pattern used by `@moor/schema`.

## License

`@moor/web-sdk` is licensed under `LGPL-3.0-or-later`. See `clients/web-sdk/LICENSE`. You can build
on top of it, but must also comply with the LGPL-3.0-or-later license if you modify the source code
to the library itself.

(The remainder of mooR is GPL 3.0)
