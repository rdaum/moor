# MOO OAuth2 Implementation Guide

## Status

- ✅ **Backend (Rust)**: Complete - web-host OAuth2 fully working
- ✅ **Frontend (TypeScript)**: Complete - OAuth2 UI and callbacks working
- ⏳ **MOO Core**: Needs implementation in `do_login_command`

## What's Working Now

1. User clicks "Sign in with GitHub/Google/Discord"
2. OAuth2 flow completes, gets user info from provider
3. Web-host calls `do_login_command` with OAuth2 args
4. **MOO doesn't know what to do** - returns error
5. User sees blank page (redirect to wrong port, but data is there)

## Three New Login Modes to Implement

### 1. `oauth2_check` - Check if OAuth2 identity exists

**Called by**: OAuth2 callback handler **Args**: `["oauth2_check", provider, external_id]`
**Example**: `["oauth2_check", "github", "49694"]`

**Logic**:

- Search for player with property matching `(provider, external_id)`
- If found: Return success=true + auth_token for that player
- If not found: Return success=false (triggers account creation flow)

**Return**: LoginResult

### 2. `oauth2_create` - Create new player with OAuth2

**Called by**: Frontend after user chooses account name **Args**:
`["oauth2_create", provider, external_id, email, name, username, player_name]` **Example**:
`["oauth2_create", "github", "49694", "ryan.daum@gmail.com", "Ryan Daum", "rdaum", "Ryan"]`

**Logic**:

- Validate player_name available
- Create new player (like existing `create` verb)
- Store OAuth2 identity: `player.oauth2_identities = {{"github", "49694"}}`
- Generate auth_token
- Return success + auth_token + player

**Return**: LoginResult with auth_token and player

### 3. `oauth2_connect` - Link OAuth2 to existing account

**Called by**: Frontend when user wants to link OAuth2 to existing account **Args**:
`["oauth2_connect", provider, external_id, email, name, username, "", existing_email, existing_password]`

**Logic**:

- Verify existing_email/existing_password (like `connect` verb)
- Find that player
- Add OAuth2 identity: append `{provider, external_id}` to `player.oauth2_identities`
- Generate auth_token
- Return success + auth_token + player

**Return**: LoginResult with auth_token and player

## Data Storage

**Add to $player or $player_class**:

```moo
property oauth2_identities = {};
// Format: {{"github", "49694"}, {"google", "108234..."}, ...}
```

**Helper verb needed**:

```moo
verb find_by_oauth2(provider, external_id)
  // Search all players for matching oauth2_identities entry
  // Return player object or $failed_match
```

## Integration Points

**In `#0:do_login_command` or `$login:parse_command`**:

- Detect `args[1]` in `{"oauth2_check", "oauth2_create", "oauth2_connect"}`
- Route to appropriate handler

**Example skeleton**:

```moo
verb oauth2_check (any none any)
  {mode, provider, external_id} = args;
  if (player = this:find_by_oauth2(provider, external_id))
    // Generate auth_token for player
    // Return LoginResult success=true
  else
    return LoginResult success=false
  endif
endverb
```

## Config Changes Needed

**File**: `web-host-oauth2.yaml`

- Change `base_url: "http://localhost:3000"` (for Vite dev server)
- Update GitHub OAuth app redirect to `http://localhost:3000/auth/oauth2/github/callback`

## Testing

1. Start: `npm run full:dev:oauth2`
2. Visit: `http://localhost:3000`
3. Click "Sign in with GitHub"
4. Should redirect back with user info in URL params
5. MOO should recognize `oauth2_check` and return result

## Files Modified (Rust/TS - Already Done)

- `crates/web-host/src/host/oauth2.rs` - OAuth2Manager
- `crates/web-host/src/host/oauth2_handlers.rs` - HTTP endpoints
- `crates/web-host/src/main.rs` - Routes (use `{provider}` not `:provider`)
- `web-client/src/lib/oauth2.ts` - API client
- `web-client/src/components/OAuth2Buttons.tsx` - UI
- `web-client/src/main.tsx` - Callback handler

## Known Issues Fixed

- ✅ Axum route syntax: Use `{provider}` not `:provider`
- ✅ GitHub API 403: Added User-Agent header
- ✅ Port mismatch: Need base_url on port 3000 for Vite
- ✅ Type-state issue: Full OAuth2 client type definition with all 10 params

## Next Session TODO

1. Implement three OAuth2 verbs in MOO
2. Add `oauth2_identities` property to $player
3. Test complete flow end-to-end
