# Event Logging

mooR includes an optional event logging system with end-to-end encryption. This feature is disabled by default and must
be explicitly enabled by server administrators who want to provide persistent message history.

When enabled, unlike traditional MOO servers where messages and events disappear once sent, mooR persistently stores
every narrative event that occurs in your virtual world, enabling rich features like message history, persistent UI
elements, and detailed audit trails—all while keeping your communications private through encryption.

When you're building a modern MOO experience, players expect to be able to scroll back through their conversation
history, just like they would in Discord or Slack. The event logging system makes this possible by capturing and
preserving the narrative flow of your world, with encryption protecting player privacy.

**Note**: Event logging is designed for modern clients that connect through mooR's web-host (like the official web
client). It is not currently available for traditional telnet or TCP-based MUD clients, which receive live events only
and do not have access to encrypted history. See the Configuration section below for how to enable this optional
feature.

## Understanding What Gets Preserved

The event logging system acts like a digital scribe, carefully recording different types of events as they occur in your
MOO:

**Player Communications**: Every time your MOO code calls `notify()` to send a message to a player, that message is
captured and stored. This includes everything from simple room descriptions to complex interactive dialogues, system
announcements, and private communications between players.

**Error Information**: When things go wrong in your MOO code and errors occur, the system preserves the traceback
information. This helps with debugging, as administrators can review exactly what happened and when, even after the
fact.

**User Interface Elements**: Modern MOO experiences often involve rich user interfaces with persistent elements like
status displays, interactive widgets, or informational panels. The event logging system tracks when these "
presentations" are shown to players and maintains their current state.

The system automatically springs into action whenever your MOO code generates any of these events. There's no special
coding required on your part—simply use `notify()` as you normally would, and the logging happens transparently in the
background.

## Privacy and Encryption

**Encryption is mandatory.** All events are encrypted before storage using modern age encryption (X25519 +
ChaCha20-Poly1305). Events cannot be logged without encryption—there is no plaintext storage option.

### How Encryption Works

Each player sets their own encryption password (separate from their MOO login password) when they first connect through
the web client. This password is used to:

1. **Derive encryption keys** (using Argon2, a secure password hashing algorithm)
2. **Encrypt all events** before they're written to disk
3. **Decrypt events** when viewing history

Importantly, the server never stores your password or private keys. Your browser derives the encryption keys from your
password and stores them in browser localStorage. When you request your message history, your browser decrypts the
events locally—the server never sees your private keys.

### What This Protects

The encryption architecture protects against realistic privacy threats:

- ✅ Administrators browsing database files directly
- ✅ Stolen database backups or improperly disposed drives
- ✅ Offline filesystem snooping
- ✅ Data breaches where raw database files are stolen

### What Happens If You Forget Your Password

If you forget your encryption password, you'll lose access to your existing message history. This is by design—the
system cannot decrypt your history without the password. You can reset your encryption password to start fresh, but old
events will remain encrypted with the old password and become unreadable.

**Important**: Write down your encryption password in a safe place when you first set it up.

### Cross-Device Access

Because key derivation is deterministic (same password always produces the same keys), you can access your encrypted
history from multiple devices. When you log in from a new device, enter the same encryption password you set up
originally, and you'll be able to see your full history.

Your browser caches the derived keys in its local storage so you don't have to enter your password every time you visit.

## How the System Works Behind the Scenes

The encryption/decryption flow works like this:

1. **Event generation**: Your MOO code calls `notify()` to send a message
2. **Encryption**: The daemon encrypts the event using your public key
3. **Storage**: The encrypted event is written to the database on disk
4. **Retrieval**: When you view history, the web client requests encrypted events from the server
5. **Client-side decryption**: Your browser decrypts events locally using your private key (never sent to server)
6. **Display**: Decrypted events are shown in your browser

To keep things fast, the system maintains a memory cache of recent events for quick access, while older events are read
from disk when needed. Events are batched together before being written to disk, which helps maintain good performance
even on busy servers.

All disk operations happen in a background thread, so your MOO code can generate events as quickly as needed without
being slowed down by database writes.

## The Web Client Experience

The official mooR web client uses the event logging system to provide a modern chat-like experience similar to Discord
or Slack. When you connect through the web interface, you automatically get scrollable message history, and new messages
appear in real time while preserving the ability to scroll back through older conversations.

### First-Time Setup

When you first log in through the web client:

1. After entering your MOO credentials, you'll be prompted to set an encryption password
2. The client displays a warning that this password cannot be recovered
3. Once you enter and confirm your password, the client derives your encryption keys client-side
4. The client generates an age keypair and extracts the public key
5. Your public key is sent to the server, and your private key is saved in your browser's localStorage
6. Your password itself is immediately discarded—never stored anywhere

### Subsequent Logins

When you log in again from the same browser:

- Your derived keys are loaded automatically from localStorage
- No password prompt needed
- You immediately have access to your full encrypted history

When you log in from a new device:

- The client detects you have encryption enabled
- You're prompted to enter your encryption password
- The client re-derives your keys from the password (same password = same keys)
- Keys are saved to localStorage on the new device
- You can now access your history from this device too

### How the Web Client Works

Behind the scenes, the web client communicates with the server through a REST API to retrieve historical events. The
primary endpoint is:

```
GET /api/history?since_seconds=3600&limit=50
```

This request asks for events from the last hour (3600 seconds), limited to 50 events. The system returns encrypted
events which are decrypted client-side in your browser. The system supports several different ways to query
history—you can ask for events from a specific time period using `since_seconds`, or request events relative to a
specific event using `since_event` or `until_event` with an event's unique identifier.

The response comes back as a FlatBuffer structure containing an array of encrypted events, with each event including
its unique identifier, timestamp, author information, and encrypted content that your browser decrypts locally.

Note that these API endpoints require proper authentication using PASETO tokens, which makes manual testing with tools
like curl quite complex. The web client handles all the authentication and token management automatically.

### How Infinite Scroll Works

The mooR web client implements infinite scroll for message history, similar to what users expect from social media
platforms. When a player first connects, the web client loads the most recent batch of events to populate the initial
view. As the player scrolls backward through history, the client requests older events using the `until_event` parameter
with the oldest currently loaded event's identifier.

New events that arrive via WebSocket connections are seamlessly integrated into the existing history view, maintaining
proper chronological order and avoiding duplicates.

## Configuring Event Logging

Event logging is disabled by default and must be explicitly enabled by server administrators. To enable event logging:

### Using Command Line

```bash
./moor-daemon --enable-eventlog true
```

### Using Configuration File

In your YAML configuration file:

```yaml
features_config:
  enable_eventlog: true
```

### Specifying Storage Location

You can also customize where event data is stored (relative to `data-dir` if not absolute):

```bash
./moor-daemon --enable-eventlog true --events-db /path/to/your/events.db
```

By default, event data is stored in `./moor-data/events.db`, separate from your main MOO database.

When event logging is disabled, no events are stored to disk and the history API endpoints will return empty results.
However, live events sent over WebSocket connections continue to work normally for real-time functionality.

## Understanding Storage and Performance

It's worth understanding how storage works so you can make informed decisions about your deployment.

The system uses LZ4 compression to keep storage requirements reasonable. The database will grow continuously as events
are added—there's currently no automatic cleanup mechanism, so events persist indefinitely.

For memory usage, the system keeps recent events cached in memory to provide fast access. The default configuration
keeps about a week's worth of events in memory, up to a maximum of 10,000 events. On a reasonably busy server, this
translates to maybe 10-50MB of memory usage for the cache.

The background persistence system batches writes together for efficiency. Under normal circumstances, events are written
to disk in groups of 100, which balances data safety and performance.

### Encryption Performance

Age encryption (ChaCha20-Poly1305) is fast and has minimal impact on event logging performance. Encryption
happens in the daemon when events are written, and decryption happens in the web host when history is requested. The
Argon2 key derivation is intentionally slow (to resist password brute-forcing) but only happens once per device in the
browser—subsequent access uses the cached derived keys.

## Administrative Considerations

### What Administrators Can and Cannot See

With the encryption architecture:

- **Administrators cannot** read player messages by accessing database files directly
- **Administrators cannot** decrypt events without the player's password
- **Events are protected** in backups, on stolen drives, and against filesystem snooping
- **Administrators can still** see metadata (timestamps, player IDs, event counts) but not content

The encrypted events remain in the database even if a player resets their password. Old events encrypted with the old
key are unreadable, but they remain on disk (consuming storage) unless manually deleted.

### Server Security

Since decryption happens client-side in the browser, the web host never sees private keys or plaintext events. However,
a compromised web host could still:

- Serve malicious JavaScript to steal passwords or keys from the browser
- Intercept live events as they arrive in real-time (before encryption)

Proper server security (keeping the web host process secure, using HTTPS, monitoring for intrusions) remains essential.
The encryption protects against offline attacks on the database and prevents the server from reading logged history, but
cannot protect against a fully compromised server serving malicious code.

### Backup and Disaster Recovery

When backing up your mooR server:

- **Do backup** the events database file (contains encrypted events)
- **Do backup** the main MOO database (contains public keys)
- **Do not store** player passwords—you don't have them anyway
- **Understand** that encrypted events cannot be bulk-decrypted by administrators

Players are responsible for remembering their own encryption passwords. If a player loses their password and needs to
access old history, there is no recovery mechanism—they must reset their encryption password and start fresh.

## Troubleshooting Common Issues

**Players Report They Can't See History**: This usually means:

- They haven't set up encryption yet (first-time users need to set a password)
- They're on a new device and need to enter their password
- They entered the wrong password (client should show an error)
- Their browser localStorage was cleared (need to re-enter password)

**Players Forgot Their Encryption Password**:

- They'll need to reset their encryption password in the web client settings
- This starts fresh encryption with a new password
- Old history remains in the database but becomes unreadable
- Warn players during setup to save their password in a safe place

**Events Not Being Logged**:

- Check that the player has set up encryption (events can't be logged without it)
- Verify the events database file exists and has proper permissions
- Check server logs for encryption-related errors
- Ensure the background persistence thread is running

**Performance Issues with Large Histories**:

- The event logging system is generally efficient, but very large message histories can impact performance
- Monitor server memory usage to ensure the cache isn't consuming excessive RAM
- Very old servers with millions of stored events might experience slower API responses for historical queries
- Consider implementing periodic cleanup of very old events (requires manual process)

**Database File Growing Too Large**:

- Events accumulate indefinitely by default
- Consider periodic cleanup of events older than a certain age
- Encrypted events from reset passwords remain on disk—can be manually deleted with careful database operations
- Monitor disk space usage on your server

If you're experiencing issues, enabling debug logging with the `--debug` flag will provide much more detailed
information about what the event logging system is doing. Look for log entries mentioning event IDs, cache operations,
encryption operations, and persistence thread activity.

The system is designed to gracefully handle various failure scenarios. Even if there are temporary issues with disk
writing, the in-memory cache ensures that recent events remain available. When problems are resolved, the system
automatically resumes normal persistence operations.

## API Reference for Developers

If you're building custom clients or tools, the event logging system exposes these key endpoints:

- **GET /api/event-log/pubkey** - Check if a player has encryption set up
- **PUT /api/event-log/pubkey** - Set up encryption for a player (body contains age public key)
- **GET /api/history** - Fetch encrypted history (returns encrypted FlatBuffer blobs for client-side decryption)
    - Query params: `since_seconds`, `since_event`, `until_event`, `limit`
- **POST /api/event-log/change-password** - Re-encrypt history with new password (not yet implemented)

All endpoints require PASETO authentication tokens. The client generates age keypairs from passwords and decrypts
history locally in the browser. Private keys never leave the client.
