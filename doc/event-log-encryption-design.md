# Event Log Encryption Design

## Why This Exists

For moor to be a viable Discord/Slack alternative, it needs persistent event logging with the same
features users expect:

- See events from the past
- See events that happened when you weren't at the console
- See events that happened while you were on another device/client

**The problem**: Without encryption, this creates a privacy nightmare where admins have unfettered
access to all user communications in plaintext. Discord and Slack likely have admin keys that
decrypt all content. We can do better.

**Design goal**: Provide meaningful protection against realistic threats (lazy sysadmin browsing
database files, stolen backups, improper disposal) without pretending to solve unsolvable problems
(compromised server, in-memory attacks).

**Key principle**: Having _some_ encryption is vastly better than having _zero_ encryption. Perfect
is the enemy of good enough.

## Overview

This document describes the end-to-end encryption architecture for MOO narrative event logs. The
system provides:

- **Encryption at rest** using age (modern file encryption with X25519/ChaCha20-Poly1305)
- **Per-user encryption** with user-chosen passwords (separate from MOO login)
- **Deterministic key derivation** enabling cross-device access (same password = same keys)
- **Client-side decryption** enabling E2E protection
- **Mandatory for history** - events cannot be logged to history without encryption setup

**What this offers**: End-to-end encryption of historical events - neither web-host nor daemon can
read plaintext events. Protects against filesystem access, stolen backups, and compromised server
components.

### Client Support

**Web client**: Full E2E encryption support via age.js in browser. Password-derived keys stored in
localStorage. This is the reference implementation.

**Telnet client**: Does not support retrieving encrypted history or providing encryption keys.
Telnet clients only see live, real-time events as they occur - no scrollback, no history recall.
However, events generated during telnet sessions _are_ still logged to the event log (encrypted) if
the player has encryption configured. The telnet client simply cannot retrieve that encrypted
history later.

**Future clients**: Any client that wants history support must implement:

1. Age encryption (for decryption)
2. FlatBuffer parsing (for protocol)
3. Key derivation (Argon2 + bech32 encoding)

The system is designed to be client-agnostic - the daemon/web-host handle encrypted blobs, clients
handle decryption. A native desktop client or mobile app could implement the same flow.

## Threat Model

### What We Protect Against

- ✅ Sysadmin reading fjall database files directly
- ✅ Stolen database backups
- ✅ Offline filesystem snooping
- ✅ Disk disposal without wiping

### What We Accept As Limitations

- ❌ Live events in transit (in-memory snooping, HTTPS MitM) - no solution for real-time events as
  they arrive
- ❌ Compromised web-client sources serving malicious JavaScript to steal passwords/keys

Note: Logged history is fully protected - only live, actively-arriving events are potentially
visible. Past encrypted events stored in the database cannot be accessed without the user's
password.

### Security Model

**Client-side encryption for logged history**. Decryption happens entirely client-side in the
browser. The web-host never sees plaintext events from history - it only handles encrypted
FlatBuffer blobs. Private keys never leave the client.

**Important distinction**: This provides end-to-end encryption for the web-host component (browser
to web-host), but not for the system as a whole. The daemon performs encryption before storage,
which means:

- A compromised daemon binary could be modified to dump plaintext events before encryption
- A compromised daemon could tee off unencrypted event streams before they're encrypted
- True E2E (like Signal) would mean the server never sees plaintext at all, which isn't feasible for
  a MOO where events must be generated server-side in a shared environment

**What this actually provides**: Strong protection against offline attacks (stolen backups, database
snooping, improper disposal) and lazy administrative access. The daemon must be trusted, but
web-host and database storage do not expose plaintext.

---

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│  Browser (JavaScript)                       │
│  - User enters event log password           │
│  - Derives age identity (client-side)       │
│  - Stores identity in localStorage          │
│  - Requests encrypted FlatBuffer blobs      │
│  - Decrypts blobs client-side with age.js   │
│  - Parses decrypted FlatBuffers             │
└─────────────────────────────────────────────┘
                    ↓ HTTPS (TLS encrypted)
┌─────────────────────────────────────────────┐
│  Web-host (Rust)                            │
│  - Stores player public keys                │
│  - Passes encrypted blobs to browser        │
│  - Never handles private keys               │
│  - Never sees plaintext events              │
└─────────────────────────────────────────────┘
                    ↓ RPC (ZMQ)
┌─────────────────────────────────────────────┐
│  Daemon (Rust)                              │
│  - Stores player public keys                │
│  - EventLog encrypts events with pubkeys    │
│  - Returns encrypted blobs (never decrypts) │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│  Fjall Database (encrypted at rest)         │
│  - All event blobs are age-encrypted        │
│  - Sysadmin sees only encrypted data        │
└─────────────────────────────────────────────┘
```

---

## Component Details

### 1. Browser (JavaScript Client)

All cryptographic operations happen client-side in JavaScript. The browser derives age identities
from passwords, stores them in localStorage, and decrypts encrypted event blobs received from the
server using age.js.

**IMPORTANT**: Encryption is MANDATORY. Events cannot be logged without encryption. There is no
plaintext storage option. Private keys never leave the client.

#### Key Derivation (JavaScript)

```javascript
// Using argon2-browser library
async function deriveKeyBytes(password, playerOid) {
    const saltString = `moor-event-log-v1-${playerOid}`;

    const result = await argon2.hash({
        pass: password,
        salt: saltString,
        type: 2, // Argon2id
        time: 3,
        mem: 65536, // 64 MiB
        parallelism: 4,
        hashLen: 32,
    });

    // Convert to base64 for transmission
    const bytes = new Uint8Array(result.hash);
    return btoa(String.fromCharCode(...bytes));
}
```

**Important**: Same password + same player OID = same 32 bytes (deterministic). The client generates
age keypairs from these bytes deterministically, enabling cross-device recovery.

#### First-time Setup Flow

1. User logs in with MOO credentials (existing auth)
2. After login, client prompts: **"Set encryption password for your event log history"**
   - Display warning: "This password cannot be recovered. Write it down."
   - Password is separate from MOO login password
3. User enters encryption password
4. **Client validates password is different from MOO password** (prevent password reuse - security
   best practice)
5. **Client derives 32 bytes from password** (deterministic Argon2)
6. **Client generates age keypair from derived bytes** (client-side)
7. **Client extracts public key** from keypair
8. Client sends **only public key** to web-host via PUT /api/event-log/pubkey
9. Web-host stores public key in daemon (never sees private key)
10. **Client saves age identity (private key)** to localStorage for future use
11. Password is discarded (never stored)

**Implementation note**: Password validation against MOO password requires the client to have access
to the MOO password at setup time. If MOO password isn't available client-side, this validation can
be skipped with a warning to the user.

#### Subsequent Logins (Same Browser)

1. User logs in with MOO credentials
2. Client checks localStorage for saved age identity (private key)
3. If found: Use saved identity automatically (no password needed)
4. If not found: Prompt for password, derive identity, save (see "New Browser" below)

**LocalStorage Key**: `moor_event_log_identity_${playerOid}` **Value**: Age identity string
(AGE-SECRET-KEY-1...) derived from password

#### Login from New Browser/Device

1. User logs in with MOO credentials
2. Client checks if user has encryption enabled (GET /api/event-log/pubkey returns public key)
3. If enabled, prompt: **"Enter your event log encryption password"**
4. User enters password
5. **Client derives 32 bytes from password** (deterministic - same password = same bytes)
6. **Client generates age identity from derived bytes**
7. **Client extracts public key from generated identity**
8. **Client validates by comparing generated public key with retrieved public key** (no need to
   fetch/decrypt history)
9. If match: **Save age identity** (not password) to localStorage for future sessions
10. If mismatch: Show error "Incorrect password" and prompt again

#### Forgotten Password Flow (Reset)

1. User logs in but doesn't remember event log password
2. User clicks **"I forgot my password"** link
3. Client shows warning: **"Resetting will generate a new encryption key. Old history will remain
   encrypted with your old password. If you remember your old password later, you can re-enter it to
   access old events, but new events will only be readable with your new password. Continue?"**
4. If user confirms:
   - Client prompts for new password
   - Client derives new 32 bytes from new password
   - Client generates new age keypair from new derived bytes
   - Client extracts new public key
   - Calls PUT /api/event-log/pubkey with new public key
   - Server stores new public key
   - Old events remain in database (encrypted with old key)
   - New events use new key going forward
   - User has access to old history ONLY if they re-enter old password later
5. Save new age identity to localStorage

**Implementation note**: Currently the server does not reject events encrypted with old keys, so
users who remember their old password can still access pre-reset history. If you want to prevent
this (force new-key-only access), the server would need to track key versions and reject old
encrypted events.

#### Change Password Flow (Re-encrypt)

**Status**: Design complete, not yet implemented. Currently users must use the reset flow.

1. User clicks **"Change encryption password"** in settings
2. Client prompts for **current password**
3. Client prompts for **new password**
4. **Client validates new password is different from MOO password** (prevent password reuse)
5. Client derives **old identity** from current password (from localStorage or re-derived)
6. Client derives **new bytes** from new password and generates new age keypair
7. Client fetches all encrypted history from server
8. Client decrypts all events with old identity
9. Client re-encrypts all events with new public key
10. Client sends re-encrypted events + new public key back to server
11. Server performs validation:
    - **Check total size of re-encrypted data is within ~33% of original** (age overhead is
      consistent)
    - **Check event count matches** (no additions/deletions)
    - **Check timestamps are unchanged** (no data injection)
    - Reject if validation fails
12. Server stores new public key and re-encrypted events
13. On success:
    - Client saves **new age identity** to localStorage
    - Client discards old identity
    - User can now access all history with new password

**Security considerations**:

- Without validation, this allows arbitrary encrypted data upload (no way to verify what's inside)
- Size checks prevent abuse (new data should be roughly same size as old)
- Event count and timestamp checks prevent data injection
- Rate limiting on this endpoint is recommended (expensive operation)

**Note**: This operation preserves all history by re-encrypting it client-side. The server never
sees either private key.

#### History Retrieval

Client fetches encrypted events from GET /api/history endpoint and decrypts them client-side using
the age identity derived from localStorage.

#### Summary of Login Flows

| Scenario                 | Has Encryption Setup? | Has Key in localStorage? | What Happens                                              |
| ------------------------ | --------------------- | ------------------------ | --------------------------------------------------------- |
| First-time user          | No                    | N/A                      | Prompt for password → derive key → save to localStorage   |
| Same browser, returning  | Yes                   | Yes                      | Auto-use saved key (no password prompt)                   |
| New browser/device       | Yes                   | No                       | Prompt for password → derive key → validate → save        |
| Forgot password          | Yes                   | No or wrong              | Reset: new password → new key → old history unreadable    |
| Change password          | Yes                   | Yes                      | Re-encrypt all history with new password (preserves data) |
| User clears browser data | Yes                   | No                       | Prompt for password to re-derive key (or reset)           |

#### Presentation State

Presentations are ephemeral. Client reconstructs active presentations on connect:

```javascript
// Fetch recent history (last hour or since last login)
const events = await fetchHistory({ since_seconds: 3600 });

// Scan for Present/Unpresent events
const presentations = {};
for (const event of events) {
    if (event.type === "present") {
        presentations[event.presentation.id] = event.presentation;
    } else if (event.type === "unpresent") {
        delete presentations[event.id];
    }
}
```

---

### 2. Web-host (Rust)

The web-host acts as a pass-through for encrypted data and stores public keys. It never handles
private keys or performs decryption.

**Note**: All cryptographic operations (key derivation, keypair generation, encryption, decryption)
happen client-side in the browser. The server only stores public keys and encrypted blobs.

#### API Endpoints

- **GET /api/event-log/pubkey** - Check if user has encryption set up (returns public_key if
  present)
- **PUT /api/event-log/pubkey** - Set up encryption (body: public_key as age1... string)
- **GET /api/history** - Fetch encrypted history (returns encrypted FlatBuffer blobs)
- **POST /api/event-log/change-password** - Re-encrypt all history with new password (not yet
  implemented)

---

### 3. Daemon (Rust)

The daemon stores encrypted events and manages public keys. It **never** decrypts events.

**CRITICAL**: Events can ONLY be logged if the player has a public key. There is no plaintext
fallback.

**Design rationale**: This explicit opt-in approach keeps encryption separate from authentication
and makes security properties clear. Users must actively set up encryption to enable event logging.

**Alternative approach considered but rejected**: Auto-generate encryption keys from username + MOO
password for all users. This would give telnet users encrypted history automatically and lower the
barrier to entry. However, it was rejected because:

- Ties event log encryption to MOO password, creating entanglement between auth and history
- MOO password changes become complicated (trigger encryption reset? force re-encryption?)
- Weaker security model (MOO password compromise = history compromise)
- Adds significant complexity to password change flows
- Blurs the security boundary between authentication and encryption
- Telnet clients still can't retrieve history (no decryption capability anyway)

**Current approach** forces users to make an explicit decision about encryption, take it seriously
(write down the password), and accept the consequences (lost password = lost history). This honesty
about limitations is better than creating false expectations or complex recovery mechanisms.

#### Public Key Storage

Public keys are stored in the event_log's pubkey_partition (fjall keyspace). No MOO database
involvement.

#### EventLog Implementation

The actual implementation:

- `EventLog` has a `pubkey_partition` (fjall keyspace) for storing public keys
- `append()` encrypts events using the player's public key before storage
- Events are stored in `events_partition` with encrypted FlatBuffer blobs
- All query methods return encrypted blobs only
- Presentations are also encrypted and stored separately in `presentations_partition`

See source code in `crates/daemon/src/event_log/` for implementation details.

#### RPC Message Handling

The daemon handles RPC messages for encryption operations:

- `GetEventLogPubkey` - Returns the player's public key if set
- `SetEventLogPubkey` - Stores a new public key for the player
- `ReencryptEventLog` - Re-encrypts all history with a new key (not yet implemented)

See source code in `crates/daemon/src/rpc/message_handler.rs` for implementation details.

---

### 4. FlatBuffer Schema Changes

See `crates/schema/schema/moor_rpc.fbs` and `crates/schema/schema/moor_event_log.fbs` for the actual
schema definitions.

Key changes:

- `LoggedNarrativeEvent` now contains `encrypted_blob` field instead of inline event data
- Added `GetEventLogPubkey` and `SetEventLogPubkey` RPC messages
- Added `StoredPresentation` for encrypted presentation storage

---

## Implementation Status

**COMPLETE**: All core functionality has been implemented.

**NOT YET IMPLEMENTED**:

- Password change with history re-encryption (users must use reset flow)
- Presentation persistence (presentations are ephemeral, reconstructed client-side from history)

---

## Edge Cases & Error Handling

### User Has No Public Key Set

- Events silently skip logging (no warning spam)
- History requests return empty
- User must set up encryption to enable event logging

### User Enters Wrong Password

- Client derives wrong bytes from incorrect password
- Client sends wrong bytes to web-host
- Web-host generates wrong age identity
- Decryption fails for all events
- History appears empty or throws error

### User Forgets Password

User must use reset flow (PUT /api/event-log/pubkey with new derived bytes). Old history becomes
unreadable.

### Password Change (Not Implemented)

See design above. Would require web-host to re-encrypt all history with new key.

---

## Performance Considerations

- Age encryption is fast (ChaCha20-Poly1305) - minimal impact on event logging
- Web-host decrypts on-demand during history requests
- Argon2 key derivation is intentionally slow (anti-brute-force) but happens client-side once per
  device
- Encrypted blobs are larger than plaintext (~33% from age overhead), but acceptable trade-off

---

## Security Analysis

### Attack Vectors

#### Filesystem Access (Mitigated)

Attacker reads fjall database files → sees only encrypted blobs → cannot decrypt without password.

#### MOO Database Access (Mitigated)

Attacker reads MOO properties → finds public keys → cannot decrypt (need private key from password).

#### Memory Dump While User Active (Accepted Limitation)

Attacker dumps web-host process memory → may find derived private key → limited window (key
discarded after request).

#### Password Brute Force (Mitigated)

Attacker tries to brute force password offline → Argon2 makes this very expensive → would need
stolen encrypted events + lots of compute.

#### Compromised Web-host (Partially Mitigated)

With client-side encryption (implemented as of commit 83df0656f):

**Protected against:**

- Web-host cannot read event history (events are encrypted blobs)
- Database admins cannot read events (stored encrypted)
- Backup theft doesn't expose event content

**Remaining risks if web-host is compromised:**

- Serve malicious JavaScript to steal passwords/keys from browser
- Exfiltrate auth tokens to impersonate users
- Forward requests to compromised daemon

**Note**: Web-host has end-to-end encryption (browser to web-host), but the system as a whole does
not. See "Compromised Daemon" below.

#### Compromised Daemon (Accepted Risk)

**If daemon binary is modified or compromised:**

- Events are generated in plaintext by MOO code before encryption
- Modified daemon could dump events before encrypting them
- Modified daemon could tee off unencrypted event streams to external storage
- Could disable encryption entirely while claiming it's enabled

**Why this is accepted**: True end-to-end encryption (where server never sees plaintext) isn't
feasible for a MOO where events must be generated server-side in a shared environment. The daemon
must be trusted.

**Mitigation**: Verify daemon binary integrity, run in isolated environment, monitor for
unauthorized modifications.

**Note on PASETO tokens and SSL/TLS**: These provide authentication and tamper-detection between
components. SSL/TLS protects data in transit. The client-side encryption ensures stored events
remain confidential even if the web-host or database is compromised, but cannot protect against a
compromised daemon.

### Why This Design Is Sufficient

For a community MOO server:

1. **Typical threat**: Lazy sysadmin browsing database files
2. **Real risk**: Backup tapes/drives stolen or improperly disposed
3. **Not defending against**: Nation-state attackers, sophisticated malware

This design provides **meaningful protection** against realistic threats without pretending to solve
unsolvable problems (compromised server, in-memory attacks).

---

## Alternative Designs Considered

### Client-Side Encryption (Rejected)

**Why rejected**: Too complex for minimal benefit. Requires age.js library, key management in
browser, doesn't protect against compromised server anyway. Requires flatbuffer knowledge and
decoding in browser.

### Per-Server Master Key (Rejected)

**Why rejected**: Single point of failure. If master key is compromised, all events for all users
are readable. Per-user keys provide defense in depth.

### Store Private Keys Encrypted with MOO Password (Rejected)

**Why rejected**: Ties event log to MOO password. Password changes would lose history. Separate
passwords provide better isolation.

---

## Migration Strategy

**N/A** - This is an unlaunched project. Existing event logs (if any) are not migrated. Old events
are deleted. Fresh start with encryption enabled from day one.

---

## Q & A

1. **Can encryption be disabled?**
   - **No**. Encryption is mandatory. Events cannot be logged without encryption. There is no
     plaintext storage.

2. **Can users have multiple decryption keys?**
   - **No**. One password, one derived key. Forgotten password requires reset (abandons old
     history).

3. **Are presentations encrypted?**
   - **Yes**. Presentations are encrypted as part of events and stored separately in encrypted form.

---
