<!-- Generated from crates/web-host/openapi.yaml — do not edit by hand. -->
<!-- Regenerate with: python3 tools/generate-api-docs.py -->

# HTTP API Reference

HTTP/WebSocket API for the mooR virtual-world server.

**Content negotiation** — Most `/v1/*` endpoints support two response
formats selected via the `Accept` header:

| Accept value                    | Response format   |
|---------------------------------|-------------------|
| `application/x-flatbuffers`     | FlatBuffers binary (default) |
| `application/json`              | JSON              |

If neither is acceptable, the server returns `406 Not Acceptable`.

**Authentication** — Authenticated endpoints require an
`X-Moor-Auth-Token` header containing a PASETO v4.public token obtained
from `/auth/connect` or `/auth/create`.  Some endpoints also require
`X-Moor-Client-Token` and `X-Moor-Client-Id` headers for session
continuity.

**FlatBuffers note** — FlatBuffer request/response bodies are opaque
binary blobs whose schemas are defined in the `moor-schema` crate.
This spec documents their logical content in prose; the wire format is
governed by the `.fbs` files, not JSON Schema.

## Auth

Password-based authentication (login / validate / logout)

### `POST /auth/connect`

**Authenticate an existing player**

Runs the `connect` login command against the daemon.  On success the
response body is a FlatBuffer `LoginResult` and the three session
headers are set.


**Request body** (required)

- Content-Type: `application/x-www-form-urlencoded`

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `player` | string | Yes | Player name |
  | `password` | string | Yes | Player password |
  | `event_log_pubkey` | string | No | Optional age public key for event-log encryption |


**Responses**

- **200**: Login successful
  - Header `X-Moor-Auth-Token`: PASETO auth token for subsequent requests
  - Header `X-Moor-Client-Token`: Session token for this connection
  - Header `X-Moor-Client-Id`: Unique ID for this client session
  - Content-Type: `application/x-flatbuffers`
- **401**: Missing or invalid auth token
- **403**: Login rejected by daemon
- **500**: Internal server error
- **502**: Malformed response from daemon
- **503**: Daemon is unreachable

---

### `POST /auth/create`

**Create a new player account**

Runs the `create` login command against the daemon.  Identical to
`/auth/connect` except the daemon verb is `:do_login_command("create", …)`.


**Request body** (required)

- Content-Type: `application/x-www-form-urlencoded`

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `player` | string | Yes | Player name |
  | `password` | string | Yes | Player password |
  | `event_log_pubkey` | string | No | Optional age public key for event-log encryption |


**Responses**

- **200**: Account created and logged in
  - Header `X-Moor-Auth-Token`
  - Header `X-Moor-Client-Token`
  - Header `X-Moor-Client-Id`
  - Content-Type: `application/x-flatbuffers`
- **401**: Missing or invalid auth token
- **403**: Account creation rejected by daemon
- **500**: Internal server error
- **502**: Malformed response from daemon
- **503**: Daemon is unreachable

---

### `GET /auth/validate`

**Validate an auth token**

Round-trips to the daemon to verify the token is still valid.
An ephemeral attach/detach is performed; the connection is cleaned
up automatically.


Requires: `X-Moor-Auth-Token`


**Responses**

- **200**: Token is valid (empty body)
- **401**: Missing or invalid auth token
- **503**: Daemon is unreachable

---

### `POST /auth/logout`

**Explicitly log out a player session**

Sends a detach message to the daemon with `disconnected=true`,
triggering `user_disconnected`.  Requires both the auth token and
client credentials.


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `X-Moor-Client-Token` | header | string | Yes | Client session token from login response |
| `X-Moor-Client-Id` | header | string (uuid) | Yes | Client session UUID from login response |


**Responses**

- **200**: Logout successful (empty body)
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

## OAuth2

OAuth2 federated authentication.  These routes are only registered
when OAuth2 is enabled in the server configuration; otherwise they
return 404.

### `GET /v1/oauth2/config`

**Get OAuth2 configuration**

Returns whether OAuth2 is enabled and the list of configured
providers.  Route is only present when OAuth2 is enabled.


**Responses**

- **200**: OAuth2 configuration
  - Content-Type: `application/json`
- **404**: OAuth2 not enabled (route absent)

---

### `GET /auth/oauth2/{provider}/authorize`

**Start an OAuth2 authorization flow**

Generates a provider authorization URL and a CSRF state token.
Sets a `moor_oauth_nonce` cookie for browser-binding.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `provider` | path | string | Yes | OAuth2 provider identifier |


**Responses**

- **200**: Authorization URL generated
  - Header `Set-Cookie`: `moor_oauth_nonce` HttpOnly cookie
  - Content-Type: `application/json`
- **400**: Unknown provider
  - Content-Type: `application/json`
- **404**: OAuth2 not enabled

---

### `GET /auth/oauth2/{provider}/callback`

**OAuth2 provider callback**

Called by the OAuth2 provider after user consent.  Validates the
CSRF state, exchanges the authorization code for user info, and
redirects the browser:

- **Existing user** → `/#auth_code={code}`
- **New user** → `/#oauth2_code={code}&oauth2_display={json}`

The code is a server-side one-time token redeemable via
`/auth/oauth2/exchange`.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `provider` | path | string | Yes | OAuth2 provider identifier |
| `code` | query | string | Yes | Authorization code from the OAuth2 provider |
| `state` | query | string | Yes | CSRF state token |


**Responses**

- **302**: Redirect to client with one-time code in fragment
- **404**: OAuth2 not enabled (route absent)

---

### `POST /auth/oauth2/exchange`

**Exchange a one-time code for tokens or identity**

Redeems a server-side one-time code produced by the callback.
Returns either an `auth_session` (existing user) or an `identity`
(new user needing account choice).  Requires the `moor_oauth_nonce`
cookie.


**Request body** (required)

- Content-Type: `application/json`

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `code` | string | Yes | One-time server-side code from callback redirect |


**Responses**

- **200**: Code exchanged
  - Content-Type: `application/json`
- **401**: Invalid/expired code or missing nonce cookie
  - Content-Type: `application/json`
- **404**: OAuth2 not enabled (route absent)

---

### `POST /auth/oauth2/account`

**Create or link an account after OAuth2**

Submits the user's account-choice decision (create new player or
link to existing player).  The `oauth2_code` must resolve to a
verified `Identity` on the server side.  Requires `moor_oauth_nonce`
cookie.


**Request body** (required)

- Content-Type: `application/json`

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `mode` | `oauth2_create` \| `oauth2_connect` | Yes |  |
  | `oauth2_code` | string | Yes | One-time code from callback redirect |
  | `player_name` | string | No | Desired player name (for oauth2_create) |
  | `existing_email` | string | No | Email of existing account to link (for oauth2_connect) |
  | `existing_password` | string | No | Password of existing account (for oauth2_connect) |


**Responses**

- **200**: Account created/linked, session established
  - Content-Type: `application/json`
- **400**: Invalid mode or code type mismatch
  - Content-Type: `application/json`
- **401**: Authentication failed or invalid code
  - Content-Type: `application/json`
- **404**: OAuth2 not enabled
- **500**: Internal server error

---

## WebSocket

WebSocket connections for real-time interaction

### `GET /ws/attach/connect`

**WebSocket upgrade — attach with existing player**

Upgrades to a WebSocket connection for an already-authenticated
player (the `connect` login path).

Auth credentials can be passed as HTTP headers or via
`Sec-WebSocket-Protocol` subprotocols:

| Subprotocol prefix | Equivalent header       |
|--------------------|-------------------------|
| `paseto.{token}`   | `X-Moor-Auth-Token`    |
| `client_id.{uuid}` | `X-Moor-Client-Id`     |
| `client_token.{t}` | `X-Moor-Client-Token`  |
| `initial_attach.{bool}` | (session hint)    |

The server selects `moor` as the WebSocket subprotocol.

**Note:** Full WebSocket message framing is FlatBuffer-based and
is outside the scope of this HTTP spec.


Requires: `X-Moor-Auth-Token`


**Responses**

- **101**: WebSocket upgrade successful
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

### `GET /ws/attach/create`

**WebSocket upgrade — attach with new player**

Same as `/ws/attach/connect` but used when the player was created
via `/auth/create`.  See that endpoint for auth details.


Requires: `X-Moor-Auth-Token`


**Responses**

- **101**: WebSocket upgrade successful
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

## Eval

Server-side MOO expression evaluation

### `POST /v1/eval`

**Evaluate a MOO expression**

Evaluates a MOO expression server-side on behalf of the
authenticated player.  Uses an ephemeral daemon connection that is
automatically cleaned up.


Requires: `X-Moor-Auth-Token`


**Request body** (required)

- Content-Type: `text/plain`

  Example:
  ```
  player:name()
  ```


**Responses**

- **200**: Evaluation result
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error
- **503**: Daemon is unreachable

---

## Verbs

Verb listing, retrieval, programming, and invocation

### `GET /v1/verbs/{object}`

**List verbs on an object**


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `inherited` | query | boolean | No | Include inherited verbs (default: `false`) |


**Responses**

- **200**: Verb listing
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/verbs/{object}/{name}`

**Retrieve a single verb**


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `name` | path | string | Yes | Verb name |


**Responses**

- **200**: Verb details
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `POST /v1/verbs/{object}/{name}`

**Set a verb's program code**

Uploads MOO source code for a verb.  Uses an ephemeral daemon
connection (attach + detach).


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `name` | path | string | Yes | Verb name |


**Request body** (required)

- Content-Type: `text/plain`

  Example:
  ```
  return "Hello, " + argstr + "!";
  ```


**Responses**

- **200**: Programming result
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error
- **503**: Daemon is unreachable

---

### `POST /v1/verbs/{object}/{name}/invoke`

**Invoke a verb and return the result**

Calls a verb on the given object with FlatBuffer-encoded arguments.
Returns a `VerbCallResponse` containing either a success result
(with output narrative events) or an error.  Times out after 60 s.


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `name` | path | string | Yes | Verb name |


**Request body** (required)

- Content-Type: `application/x-flatbuffers`


**Responses**

- **200**: Verb call result
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE or bad request body
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error
- **503**: Daemon is unreachable
- **504**: Verb execution timed out (60 s)

---

## Properties

Property listing, retrieval, and update

### `GET /v1/properties/{object}`

**List properties on an object**


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `inherited` | query | boolean | No | Include inherited properties (default: `false`) |


**Responses**

- **200**: Property listing
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/properties/{object}/{name}`

**Retrieve a single property value**


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `name` | path | string | Yes | Property name |


**Responses**

- **200**: Property value
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `POST /v1/properties/{object}/{name}`

**Update a property value**

Sets a property to a new value specified as a MOO literal in the
request body (e.g. `"hello"`, `42`, `{1, 2, 3}`).


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |
| `name` | path | string | Yes | Property name |


**Request body** (required)

- Content-Type: `text/plain`

  Example:
  ```
  "new value"
  ```


**Responses**

- **200**: Update result
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE or unparseable literal
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

## Objects

Object listing and resolution

### `GET /v1/objects`

**List all objects visible to the player**


Requires: `X-Moor-Auth-Token`


**Responses**

- **200**: Object listing
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/objects/{object}`

**Resolve an object reference**

Resolves a CURIE (e.g. `moor:1`, `moor:system`) and returns object
details.


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `object` | path | string | Yes | Object reference in CURIE form (e.g. `moor:1` or `moor:system`) |


**Responses**

- **200**: Object details
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid object CURIE
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

## EventLog

Encrypted event-log history and presentation management

### `GET /v1/history`

**Retrieve narrative history**

Returns narrative events for the authenticated player.  Exactly one
of `since_seconds`, `since_event`, or `until_event` must be
provided.


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `since_seconds` | query | integer (uint64) | No | Return events from this many seconds ago |
| `since_event` | query | string (uuid) | No | Return events after this event ID |
| `until_event` | query | string (uuid) | No | Return events up to this event ID |
| `limit` | query | integer | No | Maximum number of events to return |


**Responses**

- **200**: History events
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid UUID or missing query parameter
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/presentations`

**List active presentations**


Requires: `X-Moor-Auth-Token`


**Responses**

- **200**: Current presentations
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `DELETE /v1/presentations/{presentation_id}`

**Dismiss a presentation**


Requires: `X-Moor-Auth-Token`


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `presentation_id` | path | string | Yes |  |


**Responses**

- **200**: Presentation dismissed
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

### `GET /v1/event-log/pubkey`

**Get event-log encryption public key**


Requires: `X-Moor-Auth-Token`


**Responses**

- **200**: Current public key
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

### `PUT /v1/event-log/pubkey`

**Set event-log encryption public key**


Requires: `X-Moor-Auth-Token`


**Request body** (required)

- Content-Type: `application/json`

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `public_key` | string | Yes | age public key |


**Responses**

- **200**: Key set
  - Content-Type: `application/json`
- **400**: Missing public_key field
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

### `DELETE /v1/event-log/history`

**Delete event-log history for the player**


Requires: `X-Moor-Auth-Token`


**Responses**

- **200**: History deleted
  - Content-Type: `application/json`
- **401**: Missing or invalid auth token
- **500**: Internal server error

---

## System

Health, version, features, and system properties

### `GET /v1/system_property/{path}`

**Read a system property by dotted path**

Retrieves a property from `$system` by path (e.g. `server_version`).
Auth token is optional — unauthenticated requests are allowed.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Dotted property path (catch-all) |


**Responses**

- **200**: Property value
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **400**: Invalid property path
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/features`

**Get server feature flags**

Returns the set of features the server supports.  Response is
cached.


**Responses**

- **200**: Feature flags
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /v1/invoke_welcome_message`

**Get the server welcome message**

Invokes `:do_login_command` on the system object to produce the
welcome/MOTD text shown before login.


**Responses**

- **200**: Welcome message
  - Content-Type: `application/x-flatbuffers`
  - Content-Type: `application/json`
- **406**: Accept header does not include `application/x-flatbuffers` or `application/json`
- **500**: Internal server error

---

### `GET /health`

**Health check**

Returns 200 if the web host is running and has received a daemon
ping recently; 503 otherwise.


**Responses**

- **200**: Healthy (empty body)
- **503**: No recent daemon ping

---

### `GET /version`

**Get server version**


**Responses**

- **200**: Version info
  - Content-Type: `application/json`

---

### `GET /openapi.yaml`

**OpenAPI specification**

Returns this spec (embedded at compile time).


**Responses**

- **200**: OpenAPI 3.1.0 YAML document
  - Content-Type: `text/yaml`

---

## Webhooks

External webhook receiver (forwarded to MOO)

### `GET /webhooks/{path}`

**Receive an external webhook (GET)**

Forwards the request to the MOO `:handle_webhook` verb on
`$system`.  The MOO verb determines the response status code,
body, and content type.

**No authentication required** — the request runs as the system
user.  Body limit is 2 MB.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Catch-all webhook path |


**Responses**

- **200**: Webhook handled (status, body, and content type determined by MOO verb)
  - Content-Type: `*/*`
- **404**: No webhook handler found
- **408**: Webhook handler timed out (30 s)
- **503**: Daemon is unreachable

---

### `POST /webhooks/{path}`

**Receive an external webhook (POST)**

Forwards the request to the MOO `:handle_webhook` verb on
`$system`.  The MOO verb determines the response status code,
body, and content type.

**No authentication required** — the request runs as the system
user.  Body limit is 2 MB.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Catch-all webhook path |


**Request body**

- Content-Type: `*/*`


**Responses**

- **200**: Webhook handled (status, body, and content type determined by MOO verb)
  - Content-Type: `*/*`
- **404**: No webhook handler found
- **408**: Webhook handler timed out (30 s)
- **503**: Daemon is unreachable

---

### `PUT /webhooks/{path}`

**Receive an external webhook (PUT)**

Forwards the request to the MOO `:handle_webhook` verb on
`$system`.  The MOO verb determines the response status code,
body, and content type.

**No authentication required** — the request runs as the system
user.  Body limit is 2 MB.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Catch-all webhook path |


**Request body**

- Content-Type: `*/*`


**Responses**

- **200**: Webhook handled (status, body, and content type determined by MOO verb)
  - Content-Type: `*/*`
- **404**: No webhook handler found
- **408**: Webhook handler timed out (30 s)
- **503**: Daemon is unreachable

---

### `DELETE /webhooks/{path}`

**Receive an external webhook (DELETE)**

Forwards the request to the MOO `:handle_webhook` verb on
`$system`.  The MOO verb determines the response status code,
body, and content type.

**No authentication required** — the request runs as the system
user.  Body limit is 2 MB.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Catch-all webhook path |


**Responses**

- **200**: Webhook handled (status, body, and content type determined by MOO verb)
  - Content-Type: `*/*`
- **404**: No webhook handler found
- **408**: Webhook handler timed out (30 s)
- **503**: Daemon is unreachable

---

### `PATCH /webhooks/{path}`

**Receive an external webhook (PATCH)**

Forwards the request to the MOO `:handle_webhook` verb on
`$system`.  The MOO verb determines the response status code,
body, and content type.

**No authentication required** — the request runs as the system
user.  Body limit is 2 MB.


**Parameters**

| Name | In | Type | Required | Description |
|------|----|------|----------|-------------|
| `path` | path | string | Yes | Catch-all webhook path |


**Request body**

- Content-Type: `*/*`


**Responses**

- **200**: Webhook handled (status, body, and content type determined by MOO verb)
  - Content-Type: `*/*`
- **404**: No webhook handler found
- **408**: Webhook handler timed out (30 s)
- **503**: Daemon is unreachable

---
