## Player Management Functions

### `notify`

**Description:** Sends a notification message to a player connection. This function supports both
basic text output and rich content types for enhanced clients.

**Syntax:**
`int notify(obj player, any message [, bool no_flush [, bool no_newline [, str content_type]]])`

**Arguments:**

- `player`: The player or connection object to notify
- `message`: The message content to send (can be any type in
  [rich mode](../../the-system/server-configuration.md#language-features-configuration), must be
  string otherwise)
- `no_flush`: (Optional) If true, don't immediately flush network buffers (performance optimization)
- `no_newline`: (Optional) If true, don't add a newline character after the message
- `content_type`: (Optional) Content type for rich clients ("text/plain", "text/html", "text/djot",
  etc.)

**Returns:** Integer (typically 1 on success)

**Permission Requirements:**

- Must be the target player, own the target player, or be a wizard

**Examples:**

```moo
// Basic notification
notify(player, "Hello, world!");

// Buffer control for performance
notify(player, "Multiple ", true, true);  // No flush, no newline
notify(player, "messages ", true, true);  // No flush, no newline
notify(player, "together", false, false);   // Flush and add newline

// Rich content notification (requires rich_notify server option)
notify(player, "# Welcome\nThis is *markdown*!", false, false, "text/markdown");
notify(player, "<h1>Welcome</h1><p>HTML content</p>", false, false, "text/html");
notify(player, "= Welcome\nThis is {em}djot{/em}!", false, false, "text/djot");
```

**Notes:**

- In non-rich mode, only strings are allowed for `message` and `content_type` is ignored
- [Rich mode](../../the-system/server-configuration.md#language-features-configuration) allows any
  value type for `message` - the client determines how to render it
- `no_flush` controls network buffer flushing for performance optimization
- `no_newline` controls whether a newline is automatically appended to messages
- Content types help rich clients (web, mobile) render content appropriately
- Telnet clients receive formatted plain text regardless of content type

**See Also:**
[Web Client Output and Presentations](../../web-client/client-output-and-presentations.md)

### `event_log`

**Description:** Logs an event to a player's persistent event log without broadcasting to their
connections. This allows MOO code to render content in different formats for different connections
while still logging a canonical format to the event log for later retrieval.

**Syntax:** `int event_log(obj player, any message [, symbol content_type [, map metadata]])`

**Arguments:**

- `player`: The player object (must be a player, not a connection object)
- `message`: The message content to log (can be any type in rich mode, must be string otherwise)
- `content_type`: (Optional) Symbol indicating content type (e.g., `'text_plain`, `'text_djot`,
  `'text_html`)
- `metadata`: (Optional) Map or alist of additional metadata to include with the log entry

**Returns:** Integer (1 on success)

**Permission Requirements:**

- Must be the target player, own the target player, or be a wizard

**Examples:**

```moo
// Log a message without sending to connections
event_log(player, "Player performed action X");

// Log with content type
event_log(player, "= Heading\nSome {em}djot{/em} content", 'text_djot);

// Log with metadata
event_log(player, "Important event", 'text_plain, ["category" -> "system"]);
```

**Notes:**

- Unlike `notify()`, this function never sends output to the player's connections
- Events logged with this function are stored in the player's encrypted event log
- Use this when you need to iterate over connections with `notify()` (rendering per-connection
  formats) while still maintaining a canonical event log entry
- Web client metadata keys include `presentation_hint` (styling hint), `group_id` (group related
  lines), `tts_text` (screen-reader-friendly narration), and `thumbnail` (`[content_type, binary]`
  image data)
- Only works with player objects, not connection objects (since event logs are per-player)
- The event log requires the player to have a pubkey set for encryption

**See Also:** [notify](#notify), [player_event_log_stats](#player_event_log_stats),
[purge_player_event_log](#purge_player_event_log)

### `present`

**Description:** Emits a presentation event to the client. The client should interpret this as a
request to present the content provided as a pop-up, panel, or other client-specific UI element
(depending on 'target'). If only the first two arguments are provided, the client should "unpresent"
the presentation with that ID.

**Syntax:**
`none present(obj player, str id [, str content_type, str target, str content [, list attributes]])`

**Arguments:**

- `player`: The player or connection object to send the presentation to
- `id`: A unique identifier for this presentation (used to update or remove it later)
- `content_type`: (Optional) Content type for the presentation (e.g., "text/html", "text/markdown",
  "text/djot")
- `target`: (Optional) Target UI element type. Supported values:
  - `"window"`: Floating window
  - `"navigation"`: Navigation panel (left dock on desktop, top on mobile)
  - `"inventory"`: Inventory panel (right dock on desktop, bottom on mobile)
  - `"status"`: Status panel (right dock on desktop, top on mobile)
  - `"tools"`: Tools panel (right dock on desktop, bottom on mobile)
  - `"communication"`: Communication panel (left dock on desktop, top on mobile)
  - `"help"`: Help panel (right dock by default)
  - `"verb-editor"`: Verb editor window (requires `object`/`verb` attributes)
  - `"property-editor"`: Property editor window (requires `object`/`property` attributes)
  - `"property-value-editor"`: Property value editor window (requires `object`/`property`
    attributes)
  - `"object-browser"`: Object browser panel/window
  - `"text-editor"`: Text editor window (requires `object`/`verb` attributes)
  - `"profile-setup"`: Profile setup dialog
- `content`: (Optional) The actual content to display
- `attributes`: (Optional) Additional attributes as a list of {key, value} pairs or a map. Common
  keys include `title`/`name` for the window title and `fields` for `profile-setup`
  (comma-separated: `pronouns`, `description`, `picture`).

**Returns:** None

**Permission Requirements:**

- Must be the target player, own the target player, or be a wizard
- Only available when rich_notify server option is enabled

**Examples:**

```moo
// Remove/unpresent a presentation
present(player, "my-window");

// Create a floating window with HTML content
present(player, "welcome-msg", "text/html", "window",
        "<h1>Welcome!</h1><p>Thanks for joining our world.</p>");

// Status panel with server info
present(player, "server-info", "text/html", "status",
        "<div>Online: 42 players<br>Uptime: 5 days</div>",
        {{"title", "Server Status"}});

// Help content
present(player, "command-help", "text/markdown", "help",
        "# Commands\n\n**Basic:**\n- `look` - examine surroundings\n- `say <message>` - speak to others");

// Navigation menu
present(player, "nav-menu", "text/html", "navigation",
        "<ul><li>Lobby</li><li>Garden</li><li>Library</li></ul>");

// Inventory display
present(player, "my-items", "text/html", "inventory",
        "<ul><li>a rusty key</li><li>leather bag</li></ul>");

// Tools panel with djot content
present(player, "builder-tools", "text/djot", "tools",
        "= Builder Tools\n\n{*@dig*} - create rooms\n{*@create*} - make objects");

// Communication panel
present(player, "chat-window", "text/html", "communication",
        "<div class='chat'>Welcome to the chat!</div>");

// Launch verb editor (requires object and verb attributes)
present(player, "edit-look", "text/plain", "verb-editor", "",
        {{"object", "#123"}, {"verb", "look"}, {"title", "Edit look verb"}});

// Profile setup panel (fields controls which sections appear)
present(player, "profile-setup", "text/plain", "profile-setup", "",
        {{"title", "Set up your profile"}, {"fields", "pronouns,description,picture"}});
```

**Notes:**

- Only available when the `rich_notify` server configuration option is enabled
- Telnet clients will ignore presentation events
- Web and mobile clients can render presentations as appropriate UI elements
- The `id` parameter allows updating or removing presentations later
- With only 2 arguments (player, id), removes/unpresents the specified presentation

**See Also:** [Web Client Presentations](../../web-client/presentations.md),
[Authoring Tools](../../web-client/authoring-tools.md)

### `connected_players`

**Description:** Returns a list of all players currently connected to the server. **Arguments:**
None

### `is_player`

**Description:** Determines if a given object is a player or player-like entity. **Arguments:**

- : The object to check `object`

### `boot_player`

**Description:** Forcibly disconnects a player from the server. **Arguments:**

- : The player to disconnect `player`
- `reason`: Optional message explaining the reason for disconnection

### `player_event_log_stats`

**Description:** Returns summary information about a player's encrypted event history.

**Syntax:**
`list player_event_log_stats(obj player [, num|float|none since [, num|float|none until]])`

**Arguments:**

- `player`: Player object to inspect (must be owned by the caller or the caller must be a wizard)
- `since`: (Optional) Earliest timestamp (seconds since UNIX epoch) to include; pass `none` to
  ignore
- `until`: (Optional) Latest timestamp (seconds since UNIX epoch) to include; pass `none` to ignore

**Returns:** A list `[total_events, earliest, latest]` where `total_events` is the count of matching
entries, `earliest` and `latest` are timestamps (seconds since epoch) or `none` if no events exist.

**Permission Requirements:**

- Caller must own the target player or be a wizard.

**Examples:**

```moo
// Summary of entire history
player_event_log_stats(player);

// Only consider events from the last day
player_event_log_stats(player, time() - 86400);
```

**Notes:**

- Timestamps may be provided as integers or floating point numbers.
- Returns `none` for `earliest`/`latest` when no matching entries exist.

### `purge_player_event_log`

**Description:** Deletes part or all of a player's encrypted history, optionally removing their
stored public key.

**Syntax:** `list purge_player_event_log(obj player [, num|float|none before [, any drop_pubkey]])`

**Arguments:**

- `player`: Player object to purge (must be owned by the caller or the caller must be a wizard)
- `before`: (Optional) Remove events at or before this timestamp (seconds since UNIX epoch). Pass
  `none` to remove all history.
- `drop_pubkey`: (Optional) Truthy value indicates the player's stored public key should also be
  removed.

**Returns:** A list `[deleted_events, pubkey_deleted]` where `deleted_events` is the number of
entries removed and `pubkey_deleted` is `1` if the stored encryption public key was removed, `0`
otherwise.

**Permission Requirements:**

- Caller must own the target player or be a wizard.

**Examples:**

```moo
// Remove all history and force the player to regenerate encryption keys
purge_player_event_log(player, none, 1);

// Trim history older than a week but keep the existing key
purge_player_event_log(player, time() - 7 * 86400, 0);
```

**Notes:**

- Removing the public key causes the client to generate a fresh keypair on next login, making older
  encrypted entries unreadable.
- Passing `none` for `before` deletes all stored history for the player.

## Permission and Caller Management

### `caller_perms`

**Description:** Returns the object representing the permissions of the code that directly called
the current verb.

**Syntax:** `obj caller_perms()`

**Arguments:** None

**Returns:** An object representing the caller's permissions

**Permission Requirements:** None - available to all users

**Examples:**

```moo
// Check if the caller has wizard permissions
if (caller_perms() in {#1, #2})  // Assuming #1 and #2 are wizards
    // Do wizard-only operation
endif

// Log who is calling this verb
server_log("Verb called by: " + tostr(caller_perms()));
```

**Notes:**

- Returns the identity of the code that called this verb, not the current task's effective
  permissions
- The current task's permissions are initially determined by the owner of the verb being executed
- Use `set_task_perms()` to temporarily change the task's permissions for subsequent operations
- Commonly used for permission checks in verbs that need to validate or restrict access based on who
  called them
- Different from `this` (the object the verb is defined on)

### `task_perms`

**Description:** Returns the current task permissions principal and any active capability grants.

**Syntax:** `list task_perms()`

**Arguments:** None

**Returns:** A list whose first element is the current permissions principal. Remaining elements, if
any, are active capability grant specifications.

**Examples:**

```moo
{principal, @grants} = task_perms();
```

**Errors:**

- `E_ARGS`: Raised if any arguments are provided

**Notes:**

- Capability grant entries use the same MOO-facing shapes as `set_task_perms()` where possible
- Verb grants are returned with the current verb names string, matching `verb_info()`
- Deleted verb grants are omitted because there is no current verb name to report

### `set_task_perms`

**Description:** Changes the effective permissions of the current task for subsequent operations.
The task's initial permissions are determined by the owner of the verb, but this function allows
temporary elevation or downgrade of permissions. mooR also supports a wizard-only two-argument form
that attaches narrow capability grants to the new task permissions.

**Syntax:**

```moo
none set_task_perms(obj perms)
none set_task_perms(obj perms, list grants)
```

**Arguments:**

- `perms`: The object whose permissions should be used for subsequent operations in this task
- `grants`: Optional list of capability grant specifications. This argument is a mooR extension and
  is wizard-only.

**Returns:** None

**Permission Requirements:**

- If the current task is running with wizard permissions, it can set task permissions to any object
- If the current task is not running with wizard permissions, it can only set task permissions to
  the same permissions object it already has
- The two-argument form is always wizard-only

**Examples:**

```moo
// Downgrade permissions to caller after initial privileged check
verb "@create" (any any any) owner: #2 flags: "rd"
    // Initial permissions are from the verb owner (#2, a wizard)
    // Do something privileged first (e.g., validate arguments)

    // Now downgrade to the caller's permissions for safety
    set_task_perms(caller_perms());
    // Subsequent operations will use the caller's permissions
endverb

// Check if caller is a wizard before allowing privileged operation
verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
        // Only wizards can perform this initialization
        this.privileged_data = 42;
    endif
endverb

// Downgrade to a specific restricted object for sandbox execution
verb dangerous_utility () owner: #2 flags: "rd"
    set_task_perms($hacker);  // Downgrade to object with minimal permissions
    // Execute user-provided code or external operations safely...
    // Note: this task cannot regain wizard permissions from here
endverb

// Delegate only the ability to read one property while running as the player
verb read_secret_for_player (this none this) owner: #2 flags: "rxd"
    set_task_perms(player, {{"property_read", this, "secret"}});
    return this.secret;
endverb
```

**Errors:**

- `E_PERM`: Raised if a non-wizard task tries to set permissions to a different object, or if any
  non-wizard calls the two-argument form
- `E_TYPE`: Raised if `perms` is not an object, if `grants` is not a list, or if a grant entry has
  the wrong type
- `E_INVARG`: Raised if a grant entry has the wrong shape or names an unsupported grant type
- `E_ARGS`: Raised if the wrong number of arguments is provided
- Verb lookup errors, such as `E_VERBNF`, may be raised if a verb capability grant cannot be
  resolved

**Notes:**

- This affects permission checks in subsequent operations called within this task
- The change persists until the task completes or `set_task_perms()` is called again
- Only affects the active stack frame's effective permissions; does not change what `caller_perms()`
  returns, and does not alter what happens in subsequent verb calls
- `caller_perms()` always returns the identity of the original caller, regardless of
  `set_task_perms()` calls
- Commonly used in wizard-owned verbs to safely downgrade permissions or temporarily elevate them
  for privileged operations
- Capability grants are additive and operation-specific. They are documented in
  [Task Permissions and Capability Grants](../task-permissions-and-capability-grants.md).

### `callers`

**Description:** Returns a list of all callers in the current call stack. **Arguments:**

- `level`: Optional parameter to specify how many levels of the call stack to return

### `task_stack`

**Description:** Not supported in moor. Tasks execute in parallel threads, so the stack of a running
task is a moving target. Capturing it would require pausing tasks (which changes scheduling and can
block unrelated work) or adding heavy instrumentation to record stack changes, both of which carry
significant performance costs. In classic MOO servers, tasks are cooperatively scheduled and
therefore naturally paused while inspecting state; moor does not have that property. **Arguments:**
None

## Task and Connection Management

### `task_id`

**Description:** Returns the unique identifier for the current task. **Arguments:** None

### `idle_seconds`

**Description:** Returns the number of seconds a player or entity has been idle. **Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connected_seconds`

**Description:** Returns the total duration a player has been connected in seconds. **Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connection_name`

**Description:** Returns the name of the current connection. **Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connections`

**Description:** Returns the connection objects associated with the current or another player.
Connection objects all have negative IDs (e.g., #-123) and represent the physical connection or line
to the server. **Arguments:**

- `player`: Optional player object to query. If omitted, returns information for the current
  session. If provided, requires wizard permissions or must be the caller's own object.

**Returns:** A list of lists, where each inner list contains connection details:

- Index 0: The connection object (negative ID)
- Index 1: The hostname/connection name (string)
- Index 2: The idle time in seconds (float)
- Index 3: The acceptable content types for this connection (list of strings/symbols)

**Permission Requirements:**

- No arguments: Available to all users for their own session
- With player argument: Requires wizard permissions OR the player must be the caller's own object

**Examples:**

```moo
// Get connection info for current session (telnet connection)
connections()
=> {{#-42, "player.example.com", 15.3, {"text/plain", "text/markdown"}}}

// Get connection info for another player (requires wizard permissions)
connections(#123)
=> {{#-89, "other.example.com", 0.5, {"text/plain", "text/html", "text/djot"}},
    {#-90, "mobile.example.com", 120.0, {"text/plain", "text/markdown"}}}

// Multiple connections for same player (web + telnet)
connections()
=> {{#-42, "desktop.example.com", 5.0, {"text/plain", "text/html", "text/djot"}},
    {#-43, "mobile.example.com", 300.5, {"text/plain", "text/markdown"}}}
```

**Notes:**

- Connection objects use negative IDs (e.g., #-123) and represent the physical connection/line
- Player objects use positive IDs and represent the logged-in user
- Unlike LambdaMOO, mooR supports multiple connections per player
- Both connection and player objects can be used with `notify()` and other functions
- The function now returns enriched connection information including hostname and idle time
- **Content types** indicate what formats each connection can handle:
  - Telnet connections: `["text/plain", "text/markdown"]`
  - Web connections: `["text/plain", "text/html", "text/djot"]`
  - Default connections: `["text/plain"]`

### `switch_player`

**Description:** Associates a network connection with a different player object.

**Syntax:**

```moo
none switch_player(obj target)
none switch_player(obj source, obj target [, bool silent [, bool preserve_history]])
```

**Arguments:**

- `source`: The player or connection to switch. If omitted, the function uses the current task's
  connection.
- `target`: The new player object for the connection.
- `silent`: A client hint for a quiet identity transition. The default is `false`.
- `preserve_history`: If true, retains the current history owner. The default is `false`.

The server includes `silent` in the `PlayerSwitched` client event. Each host or client defines how
it uses this hint. The hint does not suppress output from MOO verbs.

`preserve_history` controls the owner of subsequent persistent history. It does not change the
active player. See
[Event Logging](../../the-system/event-logging.md#player-switches-and-history-ownership).

| `silent` | `preserve_history` | Client hint       | History owner          |
| -------- | ------------------ | ----------------- | ---------------------- |
| `false`  | `false`            | Normal transition | Target player          |
| `true`   | `false`            | Quiet transition  | Target player          |
| `false`  | `true`             | Normal transition | Existing history owner |
| `true`   | `true`             | Quiet transition  | Existing history owner |

**Source selection:**

- If `source` is omitted, the function switches the connection that initiated the current task.
- If `source` is the current task's player, the function also uses that task's connection.
- If `source` is a connection object, the function switches that exact connection.
- If another player has one connection, the function switches that connection.
- If another player has multiple connections, the function returns `E_INVARG`.

If code must select the connection that initiated the current task, use `connection()`.

**Returns:** `none`

**Permission Requirements:**

- Wizard-only

**Errors:**

- `E_ARGS`: The function received an unsupported number of arguments.
- `E_TYPE`: An object argument or Boolean argument has the wrong type.
- `E_PERM`: The caller does not have wizard permissions.
- `E_INVARG`: The target is not a valid player, or the source does not select one connection.

**Examples:**

```moo
// Switch this task's connection and select the target player's history.
switch_player(#74);

// Switch this task's connection and retain the current history owner.
switch_player(connection(), #74, true, true);

// Switch one specific connection owned by another player.
switch_player(#-91, #74, false, false);
```

### `worker_request`

**Description:** Sends a request to an external worker and suspends the task until the response
arrives.

**Syntax:** `any worker_request(symbol worker_type, list args [, map options])`

**Arguments:**

- `worker_type`: The worker to target (e.g., `'curl`)
- `args`: Worker-specific request arguments
- `options`: (Optional) Map or alist of options. Currently supported:
  - `timeout_seconds`: Float timeout before the request fails

**Returns:** Worker-specific response value

**Permission Requirements:**

- Wizard-only

For the filesystem worker's request operations and server-side sandbox configuration, see
[File Worker](../../the-system/file-worker.md).

### `queued_tasks`

**Description:** Returns a list of tasks currently in the queue waiting to be executed.
**Arguments:** None

### `valid_task`

**Description:** Returns true if `task_id` refers to a currently active or queued task.
**Arguments:**

- : Task ID to check `task_id`

### `active_tasks`

**Description:** Returns a list of tasks that are currently running. **Arguments:** None

### `task_telemetry`

**Description:** Returns scheduler and operating-system telemetry for active tasks. This function is
available only to wizards or callers with an explicit `builtin_call` grant. See
[Active task telemetry](../../the-system/moo-tasks.md#active-task-telemetry) for operational
examples and interpretation guidance.

**Arguments:**

- `task_id`: Optional active task ID. With an ID, returns one telemetry map. Without an ID, returns
  a list of telemetry maps for all active tasks.

Every telemetry map contains the task ID, player, phase, dispatch wait time, and, for a running
task, the worker index and running time. On Linux, maps also contain the PID and TID plus available
per-run CPU, scheduling, page-fault, context-switch, CPU-placement, and wait-channel information.
Linux counters are deltas for the current uninterrupted execution of the task, not lifetime counters
for the reused worker thread.

Common fields:

| Key                | Meaning                                                    |
| ------------------ | ---------------------------------------------------------- |
| `task_id`          | MOO task ID                                                |
| `player`           | Player associated with the task                            |
| `phase`            | `dispatching` while awaiting a worker, otherwise `running` |
| `dispatch_wait_ns` | Time spent awaiting a worker; fixed once execution begins  |
| `worker_index`     | Task-pool worker index; present once execution begins      |
| `running_ns`       | Wall-clock execution time for the current run              |

Linux fields are included when the corresponding operating-system counter is available:

| Key                            | Meaning                                       |
| ------------------------------ | --------------------------------------------- |
| `pid`                          | Server process ID                             |
| `tid`                          | Linux thread ID                               |
| `state`                        | Current Linux scheduling state                |
| `cpu_ns`                       | CPU time consumed by the current run          |
| `cpu_percent`                  | Average single-core utilization               |
| `user_cpu_ns`                  | User-mode CPU time                            |
| `system_cpu_ns`                | Kernel-mode CPU time                          |
| `minor_faults`, `major_faults` | Page-fault counts                             |
| `voluntary_context_switches`   | Voluntary context switches                    |
| `involuntary_context_switches` | Involuntary context switches                  |
| `last_cpu`                     | Most recently observed logical CPU            |
| `wchan`                        | Kernel wait channel, or `0` while not waiting |

**Errors:**

- `E_INVARG` if a requested task is not active
- `E_PERM` if the caller is not a wizard and lacks an explicit grant
- `E_TYPE` if `task_id` is not an integer

### `queue_info`

**Description:** Returns information about queued background tasks. **Arguments:**

- `player`: Optional player object

**Returns:**

- With no arguments: a list of players with queued tasks
- With `player`: the number of queued tasks for that player

**Errors:**

- `E_ARGS` if more than one argument is provided
- `E_TYPE` if `player` is provided and is not an object

### `kill_task`

**Description:** Terminates a specific task by its ID. **Arguments:**

- : The ID of the task to terminate `task_id`

### `ticks_left`

**Description:** Returns the number of execution ticks remaining for the current task.
**Arguments:** None

### `seconds_left`

**Description:** Returns the number of seconds remaining before the current task times out.
**Arguments:** None

## Time Functions

### `time`

**Description:** Returns the current server time, likely as a Unix timestamp. **Arguments:** None

### `ftime`

**Description:** Formats a timestamp into a human-readable string. **Arguments:**

- : The timestamp to format `time`
- : Optional format string for controlling the output format `format`

### `ctime`

**Description:** Converts a timestamp to a standard calendar time representation. **Arguments:**

- : The timestamp to convert `time`

## Server Control and Information

### `shutdown`

**Description:** Initiates a server shutdown process. **Arguments:**

- `delay`: Optional delay in seconds before shutdown
- `reason`: Optional message explaining the reason for shutdown

### `server_version`

**Description:** Returns the version information for the server. **Arguments:** None

### `suspend`

**Description:** Temporarily suspends the execution of the current task. **Arguments:**

- : Optional number of seconds to suspend execution `seconds`

### `suspend_if_needed`

**Description:** Conditionally commits and suspends the current task if it is close to running out
of ticks. Returns true if the task was suspended and immediately resumed in a new transaction,
otherwise false. **Arguments:**

- `threshold`: Optional tick threshold (default 4000). If remaining ticks are below this value, the
  task commits and suspends.

### `resume`

**Description:** Resumes execution of a previously suspended task. **Arguments:**

- : The ID of the task to resume `task_id`

### `server_log`

**Description:** Writes a message to the server log. **Arguments:**

- : The message to log `message`
- `level`: Optional log level (e.g., "info", "warning", "error")

### `log_cache_stats`

**Description:** Logs verb/property/ancestry cache statistics to the server log. **Arguments:** None

**Permission Requirements:**

- Wizard-only

### `verb_cache_stats`

**Description:** Returns verb cache statistics. **Arguments:** None

**Returns:** List `{hits, negative_hits, misses, flushes, histogram}`

**Permission Requirements:**

- Wizard-only

### `property_cache_stats`

**Description:** Returns property cache statistics. **Arguments:** None

**Returns:** List `{hits, negative_hits, misses, flushes, histogram}`

**Permission Requirements:**

- Wizard-only

### `ancestry_cache_stats`

**Description:** Returns ancestry cache statistics. **Arguments:** None

**Returns:** List `{hits, negative_hits, misses, flushes, histogram}`

**Permission Requirements:**

- Wizard-only

**Notes:**

- `histogram` is a simplified list containing `{0, entries}`
- `flushes` is the closest analog to the LambdaMOO cache generation counter

### `memory_usage`

**Description:** Returns information about the server's memory usage. **Arguments:**

- `detailed`: Optional boolean flag for requesting detailed information

### `db_disk_size`

**Description:** Returns the size of the database on disk. **Arguments:** None

### `db_compact`

**Syntax:** `list db_compact([list relations])`

**Description:** Major-compacts selected database relations. The task suspends while a dedicated
maintenance thread performs the work.

**Arguments:**

- `relations`: Optional list of relation names. If omitted, all relations are compacted.

**Returns:** One map for each selected relation. Each map contains `relation`, `bytes_before`,
`bytes_after`, `bytes_reclaimed`, and `error`. `error` is `none` when that relation succeeded.

**Permission Requirements:** Wizard-only

Checkpoint export and storage compaction cannot run at the same time. If another maintenance
operation is active, each returned map describes that admission error.

### `load_server_options`

**Description:** Reloads options from `$server_options` and applicable properties on `#0`.
**Arguments:** None

## Database Operations

### `commit`

**Description:** Commits pending changes to the database. **Arguments:** None

### `rollback`

**Description:** Rolls back pending changes to the database. **Arguments:** None

### `read`

**Description:** Suspends the current task until the player enters a line of input. Supports
optional metadata for rich input prompts in web clients.

**Syntax:** `str read([obj player [, map metadata]])`

**Arguments:**

- `player`: (Optional) The player to read input from. Must be the current player.
- `metadata`: (Optional) Map of UI hints for rich clients. Supported keys:
  - `input_type`: Type of input control (`"text"`, `"text_area"`, `"number"`, `"choice"`,
    `"yes_no"`, `"yes_no_alternative"`, `"yes_no_alternative_all"`, `"confirmation"`, `"image"`,
    `"file"`)
  - `prompt`: Prompt text to display (supports Djot formatting)
  - `tts_prompt`: Accessible alternative prompt for screen readers
  - `placeholder`: Placeholder text for text inputs
  - `default`: Default value
  - `choices`: List of options for `choice` input type
  - `min` / `max`: Constraints for `number` input type
  - `rows`: Number of rows for `text_area`
  - `accept_content_types`: Allowed MIME types for `image`/`file` inputs
  - `max_file_size`: Maximum file size in bytes for uploads
  - `alternative_label`: Label for alternative input in `yes_no_alternative*` types

**Returns:** The string entered by the player

**Examples:**

```moo
// Simple read
input = read();

// Read with rich prompt
input = read(player, [
    "input_type" -> "choice",
    "prompt" -> "Choose a direction:",
    "choices" -> {"North", "South", "East", "West"}
]);
```

**See Also:**
[Web Client Rich Input Prompts](../../web-client/client-output-and-presentations.md#rich-input-prompts)

### `dump_database`

**Description:** Creates a dump of the database, typically for backup purposes. **Arguments:**

- `filename`: Optional output filename for the dump
- `options`: Optional flags controlling the dump format

### `gc_collect`

**Description:** Forces garbage collection of anonymous objects. **Arguments:** None

**Permission Requirements:**

- Wizard-only

**Notes:**

- Only affects anonymous objects; numbered and UUID objects are not reclaimed
- Collection is scheduled asynchronously

## Event Handling

### `listen`

**Description:** Registers to listen for specific events. **Arguments:**

- `event_type`: The type of event to listen for
- `callback`: The function to call when the event occurs

### `listeners`

**Description:** Returns a list of all current event listeners. **Arguments:**

- `event_type`: Optional parameter to filter listeners by event type

### `unlisten`

**Description:** Removes a previously registered event listener. **Arguments:**

- `event_type`: The type of event to stop listening for
- `callback`: Optional specific callback to remove (if omitted, removes all listeners for the event)

## Code Execution

### `eval`

**Description:** Compiles and evaluates a MOO expression or statement dynamically at runtime.
**Arguments:**

- `program`: String containing MOO code to evaluate
- `initial_env`: (Optional) Map of variable bindings to pre-populate in the eval frame. Keys are
  variable names (strings or symbols), values can be any MOO type. Only variables that are assigned
  to in the program will be populated from this map.
- `verbosity`: (Optional) Controls error output detail level (default: 0)
  - `0` - Summary: Brief error message only (default for eval)
  - `1` - Context: Message with error location (graphical display when output_mode > 0)
  - `2` - Detailed: Message, location, and diagnostic hints
  - `3` - Structured: Returns error data as a map for programmatic handling
- `output_mode`: (Optional) Controls error formatting style (default: 0)
  - `0` - Plain text without special characters
  - `1` - Graphics characters for visual clarity
  - `2` - Graphics with ANSI color codes

**Returns:**

- On success: list containing `{result, ...}` where `result` is the value produced by evaluating the
  code
- On compilation failure with `verbosity` 0-2: list where first element is error object, followed by
  formatted error strings
- On compilation failure with `verbosity` 3: list where first element is error object, second is map
  containing structured error data (use `format_compile_error()` to format)

**Note:** Requires programmer bit. The evaluated code runs with the permissions of the current task.

**Examples:**

```moo
// Basic evaluation
eval("2 + 2");
=> {4}

// Evaluation with pre-populated variables
eval("return x + y;", ["x" -> 10, "y" -> 32]);
=> {1, 42}

// Variables can be any MOO type
eval("return items[2];", ["items" -> {1, 2, 3}]);
=> {1, 2}

// Evaluation with compilation error (default summary)
eval("2 +");
=> {E_INVARG, "Parse error at line 1, column 4: ..."}

// Get detailed error with context (using empty map for initial_env)
eval("return 1 + ;", [], 2);
=> {E_INVARG, "Parse error at line 1, column 12:", "  return 1 + ;", "             ⚠", ...}

// Get structured error data for custom handling
result = eval("return 1 + ;", [], 3);
if (typeof(result[1]) == TYPE_MAP)
  // result[1] is the error map, result[2] is structured data
  formatted = format_compile_error(result[2], 0);
endif
```

### `call_function`

**Description:** Calls a specified function with provided arguments. **Arguments:**

- `function`: The function to call
- : The arguments to pass to the function `args`

### `function_info`

**Description:** Returns information about a specified function. **Arguments:**

- `function`: The function to get information about

### `function_help`

**Description:** Returns documentation for a specified builtin function. **Arguments:**

- `function_name` (str): The name of the builtin function

**Returns:** A list of strings containing the documentation extracted from the builtin's
implementation.

**Errors:**

- `E_INVARG`: Raised if the builtin doesn't exist or has no documentation
- `E_TYPE`: Raised if the argument is not a string
- `E_ARGS`: Raised if the wrong number of arguments is provided

**Example:**

```moo
help = function_help("abs");
=> {"MOO: `num abs(num x)`", "Returns the absolute value of x."}

help = function_help("min");
=> {"MOO: `num min(num x, ...)`", "Returns the minimum value among the arguments."}
```

**Notes:**

Documentation is extracted at compile time from the doc-comments of each builtin's Rust
implementation, ensuring it always matches the running program.

### `wait_task`

**Description:** Waits for a specified task to complete. **Arguments:**

- : The ID of the task to wait for `task_id`
- `timeout`: Optional timeout in seconds

### `task_send`

**Description:** Sends a message to another task's message queue. The message is buffered and only
delivered when the sending task's current transaction commits (on `suspend`, `commit`, `read`, task
completion, etc.). If the transaction is aborted or retried due to a conflict, buffered messages are
discarded.

**Syntax:** `task_send(int task_id, any value)`

**Arguments:**

- `task_id`: The ID of the target task (must be an active or queued task)
- `value`: Any MOO value to send as the message

**Returns:** `0` on success

**Permission Requirements:**

- Caller must be the owner of the target task, or a wizard (same permission model as `kill_task` /
  `resume`)

**Errors:**

- `E_INVARG`: Target task does not exist
- `E_PERM`: Caller is not the owner of the target task and is not a wizard
- `E_QUOTA`: Target task's mailbox is full (exceeds `$server_options.max_task_mailbox`,
  default 1000)

### `task_recv`

**Description:** Commits the current transaction and returns all queued messages as a list, draining
the queue. Like `commit()` and `read()`, this function always commits the current transaction and
resumes in a new one, respecting snapshot isolation.

**Syntax:** `list task_recv([num wait_time])`

**Arguments:**

- `wait_time`: Optional maximum number of seconds to wait for messages. Supports fractional seconds
  (e.g. `0.5`).

**Returns:** A list of all messages that were in the queue (possibly empty).

**Behavior:**

- **No arguments:** Commits the current transaction and immediately returns all queued messages. If
  no messages are queued, returns an empty list `{}`. This is the fast path, similar to `commit()`.
- **With `wait_time`:** Commits the current transaction. If messages are already queued, returns
  them immediately. If the queue is empty, suspends the task for up to `wait_time` seconds. The task
  wakes when a message arrives or the timeout expires, and returns all accumulated messages (empty
  list if none arrived).

## Scheduled Tasks

The `schedule_*` builtins arrange for a verb to be called later — once or on a cadence — without a
task suspended in between. The scheduler holds only a deadline and a `(object, verb, args)` triple,
and starts a fresh background task for each firing. See
[Scheduled Tasks](../../the-system/scheduled-tasks.md) for the model, the options and the recipes;
this section is the argument-level reference.

Creation and cancellation are buffered until the calling task commits, exactly like `task_send`: a
schedule created in a task that rolls back never exists.

### `schedule_at`

**Description:** Runs `target:verb(@args)` once, as a new background task, at Unix time `when`. A
`when` at or before now fires on the next scheduler tick.

**Syntax:** `int schedule_at(obj target, str|sym verb, num when [, list args] [, map options])`

**Arguments:**

- `target`: The object whose verb to call
- `verb`: The verb name
- `when`: Unix epoch seconds, int or float (`ftime()` for sub-second precision)
- `args`: Arguments to pass; default `{}`
- `options`: See below

**Returns:** A schedule id (its own id space; never a task id)

**Options** (string or symbol keys; any other key is `E_INVARG`):

| Key            | Type                                | Default                       | Meaning                                                                                          |
| -------------- | ----------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------ |
| `adaptive`     | bool                                | `1` for `at`, `0` for `every` | Positive numeric return re-arms in that many seconds; `0` retires; negative retires with a fault |
| `catchup`      | `"skip"`, `"once"`, `"all"`         | `"skip"`                      | Missed-deadline policy after a restart or a late firing                                          |
| `overlap`      | `"skip"`, `"queue"`, `"concurrent"` | `"skip"`                      | Policy when the previous firing is still running                                                 |
| `jitter`       | num seconds                         | `0`                           | Randomise each deadline by up to ± this much                                                     |
| `max_faults`   | int                                 | `50`                          | Consecutive faults before retirement; `0` = unlimited                                            |
| `pass_elapsed` | bool                                | `1`                           | Append real seconds since the previous firing (float) to the args                                |
| `state`        | any                                 | none                          | Opaque value (≤ 4 KB serialised) appended to args before `elapsed`                               |
| `persist`      | bool                                | `1`                           | Survive a server restart                                                                         |
| `player`       | obj                                 | `target`                      | The value of `player` inside the fired verb; the caller must control it                          |

**Permission Requirements:** The caller's current task permissions become the schedule's owner. The
verb must be found on `target` or an ancestor and be executable under those permissions at creation
time, as for an ordinary `target:verb()` call: the `x` flag, not the `r` flag, decides. The fired
verb's `player` (`target`, or the `player` option) must be an object the caller controls: wizards
may name any object; anyone else must be it or own it. Inside the fired verb, `caller` and
`caller_perms()` are the schedule's owner.

**Errors:**

- `E_ARGS`: Wrong argument count
- `E_TYPE`: Wrong argument type, or a wrongly typed option value
- `E_INVARG`: `when` negative, not finite, or later than `18446744073` (the latest deadline a
  schedule can hold, in July 2554); `target` invalid or `verb` not callable on it; unknown option
  key; unknown `catchup`/`overlap` value; `jitter` too large to be a duration; `state` too large
- `E_PERM`: The caller does not control the fired verb's `player`

### `schedule_every`

**Description:** Runs `target:verb(@args)` every `interval` seconds, each firing a new background
task. Deadlines are computed from the previous deadline, so the cadence does not drift with the
verb's running time. An interval shorter than the scheduler tick is clamped to one tick and reported
as `interval_clamped` in `schedule_info`.

**Syntax:**
`int schedule_every(obj target, str|sym verb, num interval [, list args] [, map options])`

**Arguments:** As `schedule_at`, with `interval` in seconds (int or float, `> 0`) in place of
`when`. Same options; `adaptive` defaults to off.

**Returns:** A schedule id

**Errors:** As `schedule_at`; `E_INVARG` for `interval <= 0`, or for an interval whose first
deadline (now + `interval`) would be later than `18446744073` Unix seconds.

### `schedule_stop`

**Description:** Cancels a schedule. Takes effect when the calling task commits. **Never raises for
a stale id**: a schedule that already fired, retired or never existed is an ordinary race and
returns `false`. Stopping a retired schedule you own also discards its retained diagnostics, so
`schedule_info()` on it then raises `E_INVARG`.

**Syntax:** `bool schedule_stop(int schedule_id)`

**Returns:** `true` if a live schedule (or one this task created and has not yet committed) was
cancelled; `false` otherwise

**Permission Requirements:** Owner of the schedule, or a wizard

**Errors:**

- `E_PERM`: The schedule is live and the caller is neither its owner nor a wizard

### `schedule_valid`

**Description:** Whether `schedule_id` refers to a schedule that will fire again, including one the
calling task has created but not yet committed.

**Syntax:** `bool schedule_valid(int schedule_id)`

### `schedule_info`

**Description:** Everything the scheduler knows about a schedule. A retired schedule remains
inspectable for 60 seconds after it retires, or until `schedule_stop()` is called on it; after that
its id is unknown and `schedule_info()` raises `E_INVARG`.

**Syntax:** `map schedule_info(int schedule_id)`

**Returns:** A string-keyed map: `id`, `target`, `verb`, `args`, `owner`, `authority`, `kind`
(`"at"` or `"every"`), `interval` (float seconds, `0.0` for one-shots), `created_at`, `next_run`,
`last_run` (float Unix seconds, `0.0` for never), `run_count`, `fault_count`, `consecutive_faults`,
`last_fault`, `missed_count`, `overlap_count`, `last_duration_ns`, `mean_duration_ns`,
`p99_duration_ns`, `running_task` (`0` if idle; the oldest firing's task under `"concurrent"`),
`interval_clamped`, `retired`, `retire_reason` (`""` while live; otherwise `"one_shot_done"`,
`"returned_zero"`, `"negative_return"`, `"max_faults"`, `"invalid_target"` or
`"deadline_out_of_range"`), and the options `adaptive`, `catchup`, `overlap`, `jitter`,
`max_faults`, `pass_elapsed`, `persist`, `player`, `state`.

**Permission Requirements:** Owner or wizard

**Errors:**

- `E_INVARG`: No such schedule
- `E_PERM`: Not the owner and not a wizard

### `schedules`

**Description:** Live schedule ids visible to the caller.

**Syntax:** `list schedules([obj owner])`

**Returns:** With no argument, every live schedule for a wizard, or the caller's own otherwise. With
`owner`, that owner's live schedules (wizard, or `owner` is the caller).

**Errors:**

- `E_PERM`: A non-wizard asked for someone else's schedules

### `schedules_for`

**Description:** Live schedule ids whose target is `target`, regardless of who created them. This is
what a core's recycle and "unregister everything on this object" paths use, so it needs no special
permission.

**Syntax:** `list schedules_for(obj target)`

## Performance Monitoring

### `bf_counters`

**Description:** Returns performance counters related to built-in function execution.

**Syntax:** `map bf_counters()`

**Arguments:** None

**Returns:** A map of `name -> [count, nanos]`

**Permission Requirements:**

- Wizard only

**Notes:**

- `count` is the exact invocation count for the builtin
- `nanos` is cumulative duration in nanoseconds
- Duration totals may be sampled depending on runtime timing configuration
- See [Performance and Concurrency](../../the-system/performance-and-concurrency.md) for guidance on
  interpreting the counters

### `db_counters`

**Description:** Returns performance counters related to database operations.

**Syntax:** `map db_counters()`

**Arguments:** None

**Returns:** A map of `name -> [count, nanos]`

**Permission Requirements:**

- Wizard only

**Notes:**

- For operation counters, `count` is the exact invocation count
- For timers, `nanos` is cumulative duration in nanoseconds
- This includes counter families related to world-state operations, commit phases, provider lock
  waits, and writer backpressure
- See [Performance and Concurrency](../../the-system/performance-and-concurrency.md) for guidance on
  interpreting the counters

Live persistence status and PostgreSQL totals use `{value, 0}` entries in the same map. These values
can change during a transaction. See
[Persistence diagnostics](../../the-system/server-configuration.md#persistence-diagnostics) for
names, units, and sampling rules.

### `sched_counters`

**Description:** Returns performance counters related to the task scheduler.

**Syntax:** `map sched_counters()`

**Arguments:** None

**Returns:** A map of `name -> [count, nanos]`

**Permission Requirements:**

- Wizard only

**Notes:**

- `count` is the exact invocation count for the operation
- `nanos` is cumulative duration in nanoseconds
- This includes scheduler request latency, task lifecycle timing, wakeup and handoff latency, and GC
  phase counters
- See [Performance and Concurrency](../../the-system/performance-and-concurrency.md) for guidance on
  interpreting the counters

## Miscellaneous

### `raise`

**Description:** Raises an error or exception. **Arguments:**

- `error_type`: The type of error to raise
- : Optional error message `message`

### `force_input`

**Description:** Forces input to be processed as if it came from a specific source. **Arguments:**

- `source`: The source entity (typically a player)
- `input`: The input text to process
