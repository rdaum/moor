# OAuth2 Flows in `moor-web-host`

This document describes the two OAuth2 flow bindings supported by `moor-web-host`:

1. `cookie-bound` (browser nonce cookie)
2. `proof-bound` (PKCE verifier/challenge, no nonce cookie dependency)

Both bindings use the same provider callback endpoint:

- `GET /auth/oauth2/{provider}/callback`

## Why two bindings

- `cookie-bound` is straightforward for same-browser web login.
- `proof-bound` is suitable for desktop/mobile/native clients (and browser clients that prefer not
  to rely on cookie continuity).

The distinction is the redemption binding mechanism, not client type.

## Shared behavior

- OAuth2 provider state (`state`) is stored server-side and consumed once.
- One-time handoff codes are stored server-side and consumed once.
- TTLs are enforced:
  - CSRF/state entries: 10 minutes
  - Handoff/identity codes: 2 minutes

## Cookie-bound flow

### Endpoints

1. Start:
   - `GET /auth/oauth2/{provider}/authorize`
   - Returns `{ auth_url, state }`
   - Sets `moor_oauth_nonce` cookie
2. Provider callback:
   - `GET /auth/oauth2/{provider}/callback?code=...&state=...`
   - Validates `state` + nonce cookie binding
   - Redirects:
     - existing user: `/#auth_code={code}`
     - new user: `/#oauth2_code={code}&oauth2_display={json}`
3. Exchange:
   - `POST /auth/oauth2/exchange` with `{ code }`
   - Requires nonce cookie
   - Returns:
     - `auth_session` payload, or
     - `identity` payload
4. Account choice (new user):
   - `POST /auth/oauth2/account` with `{ mode, oauth2_code, ... }`
   - Requires nonce cookie

### Notes

- Existing web behavior is preserved.
- Browser nonce cookie remains required for cookie-bound exchange/account redemption.

## Proof-bound flow

### Configuration

`oauth2.allowed_app_redirect_uri_prefixes` must include approved redirect URI prefixes.
`/auth/oauth2/{provider}/app/start` rejects redirect URIs that do not match this allowlist.

### Endpoints

1. Start:
   - `POST /auth/oauth2/{provider}/app/start`
   - Body:
     - `redirect_uri`
     - `intent` (optional)
     - `code_challenge`
     - `code_challenge_method` (`S256`)
   - Returns:
     - `{ auth_url }`
2. Provider callback:
   - `GET /auth/oauth2/{provider}/callback?code=...&state=...`
   - Detects proof-bound pending state
   - Redirects to provided `redirect_uri` with query param:
     - `handoff_code=...`
3. Exchange:
   - `POST /auth/oauth2/app/exchange`
   - Body:
     - `handoff_code`
     - `code_verifier`
   - PKCE verification:
     - `BASE64URL(SHA256(code_verifier)) == code_challenge`
   - Returns:
     - `auth_session` payload (existing user), or
     - `identity` payload with `identity_code` (new user path)
4. Account choice (new user):
   - `POST /auth/oauth2/app/account`
   - Body:
     - `mode` (`oauth2_create` or `oauth2_connect`)
     - `identity_code`
     - `code_verifier`
     - create/link fields
   - Redeems identity with proof binding (no nonce cookie required)

### Notes

- Proof-bound flow does not require `moor_oauth_nonce` for exchange/account.
- PKCE method currently supported: `S256`.

## Choosing a binding

- Use `cookie-bound` when a browser client expects cookie-backed session continuity.
- Use `proof-bound` when clients need deep-link handoff and explicit proof-based redemption
  (desktop/mobile/native/CLI, or browser if desired).

## Minimal proof-bound client sequence

1. Generate PKCE verifier/challenge (`S256`).
2. Call `POST /auth/oauth2/{provider}/app/start`.
3. Open returned `auth_url` in external browser.
4. Receive app redirect with `handoff_code`.
5. Call `POST /auth/oauth2/app/exchange` with `handoff_code + code_verifier`.
6. If `type=auth_session`, login complete.
7. If `type=identity`, call `POST /auth/oauth2/app/account` with `identity_code + code_verifier` and
   account-choice fields.

## Security checklist

- Keep redirect URI allowlist strict.
- Keep handoff and identity codes single-use and short-lived.
- Never log `code_verifier`, `handoff_code`, `identity_code`, `auth_token`, or `client_token`.
- Use HTTPS for all deployed callback and exchange endpoints.
