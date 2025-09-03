# Event Logging

mooR includes a comprehensive event logging system. Unlike traditional MOO servers where messages and events disappear
once sent, mooR can persistently store every narrative event that occurs in your virtual world, enabling rich features
like message history, persistent UI elements, and detailed audit trails.

When you're building a modern MOO experience, players expect to be able to scroll back through their conversation
history, just like they would in Discord or Slack. The event logging system makes this possible by capturing and
preserving the narrative flow of your world.

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
status displays, interactive widgets, or informational panels. The event logging system tracks when these
"presentations" are shown to players and maintains their current state.

The system automatically springs into action whenever your MOO code generates any of these events. There's no special
coding required on your part - simply use `notify()` as you normally would, and the logging happens transparently in the
background.

## How the System Works Behind the Scenes

The event logging system uses an embedded database called `fjall` to store events persistently on disk. Each event gets
a
unique identifier based on UUID version 7, which has the clever property of naturally sorting in chronological order.

To keep things fast for active players, the system maintains a memory cache. Recent events are kept in
memory for quick access, while older events are still available by reading from disk when needed. When new events come
in rapidly, they're batched together before being written to disk, which helps maintain good performance even on busy
servers.

The system runs a background thread that handles all the disk writing operations, so the main MOO execution isn't slowed
down by database operations. This means your MOO code can generate events as quickly as it needs to without worrying
about storage performance.

## The Web Client Experience

The official mooR web client uses the event logging system to provide a modern chat-like experience similar to Discord
or Slack. When you connect through the web interface, you automatically get scrollable message history, and new messages
appear in real time while preserving the ability to scroll back through older conversations.

### How the Web Client Works

Behind the scenes, the web client communicates with the server through a REST API to retrieve historical events. The
primary endpoint is:

```
GET /api/history?since_seconds=3600&limit=50
```

This request asks for events from the last hour (3600 seconds), limited to 50 events. The system supports several
different ways to query history - you can ask for events from a specific time period using `since_seconds`, or request
events relative to a specific event using `since_event` or `until_event` with an event's unique identifier.

The response comes back as a JSON structure containing an array of events, with each event including its unique
identifier, timestamp, author information, and the actual event content.

Note that these API endpoints require proper authentication using PASETO tokens, which makes manual testing with tools
like curl quite complex. The web client handles all the authentication and token management automatically.

### Managing UI Presentations

The web client also uses additional endpoints for managing persistent UI elements. The `GET /api/presentations` endpoint
retrieves all currently active presentations for the authenticated player - things like status widgets, notification
panels, or interactive elements that should persist across browser sessions.

When a player dismisses a presentation (like closing a notification), the web client makes a
`DELETE /api/presentations/{presentation_id}` request to remove it from the player's active set.

### How Infinite Scroll Works

The mooR web client implements infinite scroll for message history, similar to what users expect from social media
platforms. When a player first connects, the web client loads the most recent batch of events to populate the initial
view. As the player scrolls backward through history, the client requests older events using the `until_event` parameter
with the oldest currently loaded event's identifier.

New events that arrive via WebSocket connections are seamlessly integrated into the existing history view, maintaining
proper chronological order and avoiding duplicates.

## Configuring Event Logging

The event logging system is enabled by default, but server administrators have full control over whether to use it:

```yaml
features_config:
  enable_eventlog: true  # The default setting
```

You can also control this from the command line when starting the server:

```bash
./moor-daemon --enable-eventlog true
```

When event logging is disabled, the system switches to a "no-op" mode where events are discarded rather than stored.
This means no disk space is used for logging, and history API endpoints will return empty results. However, live events
sent over WebSocket connections continue to work normally, so real-time functionality isn't affected.

Event data is stored in a dedicated database file, separate from your main MOO database. By default, this is located at
`./moor-data/events.db`, but you can specify a different location:

```bash
./moor-daemon --events-db /path/to/your/events.db
```

## Understanding Storage and Performance

It's worth understanding how storage works so you can make informed decisions about your deployment.

Each narrative event typically consumes around 1KB of storage on average, though this varies depending on the content.
The system uses LZ4 compression to keep storage requirements reasonable. The database will grow continuously as events
are added - there's currently no automatic cleanup mechanism, so events persist indefinitely.

For memory usage, the system keeps recent events cached in memory to provide fast access. The default configuration
keeps about a week's worth of events in memory, up to a maximum of 10,000 events. On a reasonably busy server, this
translates to maybe 10-50MB of memory usage for the cache.

The background persistence system batches writes together for efficiency. Under normal circumstances, events are written
to disk in groups of 100, which balances data safety and performance.

## Privacy and Administrative Considerations

It's important to understand the privacy implications of persistent event logging. When enabled, the system stores all
player communications in plaintext on the server's disk. This includes private messages, room conversations, and any
other text that gets sent to players through the `notify()` function.

As a server administrator, you have access to this complete historical record. This can be valuable for debugging
issues, understanding player behavior, or investigating problems. However, it also means you're essentially keeping logs
of all player communications.

Depending on your jurisdiction and use case, you may need to inform players that their communications are being logged.
Some deployments may choose to disable event logging entirely for privacy reasons, while others might implement policies
around log retention and access.

If you do keep event logging enabled, make sure the event database files have appropriate file system permissions to
prevent unauthorized access. The data is stored in plaintext without encryption, so file-level security is your primary
protection mechanism.

## Troubleshooting Common Issues

**Players Report Missing Message History**: First, verify that event logging is actually enabled in your server
configuration. Check your server startup logs for a message like "Event log persistence thread started". If logging is
disabled, you'll see "Event logging is disabled - using no-op implementation" instead.

**Poor Performance with Large Histories**: The event logging system is generally efficient, but very large message
histories can impact performance. Monitor your server's memory usage to ensure the cache isn't consuming excessive RAM.
Very old servers with millions of stored events might experience slower API responses for historical queries.

**Events Missing After Server Restart**: This usually indicates a problem with the event database file. Check that the
path specified for your events database is correct and that the server process has write access to that location. Also
verify that the background persistence thread is successfully writing events by looking for related log messages during
normal operation.

If you're experiencing issues, enabling debug logging with the `--debug` flag will provide much more detailed
information about what the event logging system is doing. Look for log entries mentioning event IDs, cache operations,
and persistence thread activity.

The system is designed to gracefully handle various failure scenarios. Even if there are temporary issues with disk
writing, the in-memory cache ensures that recent events remain available. When problems are resolved, the system
automatically resumes normal persistence operations.