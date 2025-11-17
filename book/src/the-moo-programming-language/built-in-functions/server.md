## Player Management Functions

### `notify`

**Description:** Sends a notification message to a player connection. This function supports both basic text output and
rich content types for enhanced clients.

**Syntax:** `int notify(obj player, any message [, bool no_flush [, bool no_newline [, str content_type]]])`

**Arguments:**

- `player`: The player or connection object to notify
- `message`: The message content to send (can be any type
  in [rich mode](../../the-system/server-configuration.md#language-features-configuration), must be string otherwise)
- `no_flush`: (Optional) If true, don't immediately flush network buffers (performance optimization)
- `no_newline`: (Optional) If true, don't add a newline character after the message
- `content_type`: (Optional) Content type for rich clients ("text/plain", "text/html", "text/djot", etc.)

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
- [Rich mode](../../the-system/server-configuration.md#language-features-configuration) allows any value type for
  `message` - the client determines how to render it
- `no_flush` controls network buffer flushing for performance optimization
- `no_newline` controls whether a newline is automatically appended to messages
- Content types help rich clients (web, mobile) render content appropriately
- Telnet clients receive formatted plain text regardless of content type

### `present`

**Description:** Emits a presentation event to the client. The client should interpret this as a request to present
the content provided as a pop-up, panel, or other client-specific UI element (depending on 'target').
If only the first two arguments are provided, the client should "unpresent" the presentation with that ID.

**Syntax:** `none present(obj player, str id [, str content_type, str target, str content [, list attributes]])`

**Arguments:**

- `player`: The player or connection object to send the presentation to
- `id`: A unique identifier for this presentation (used to update or remove it later)
- `content_type`: (Optional) Content type for the presentation (e.g., "text/html", "text/markdown", "text/djot")
- `target`: (Optional) Target UI element type. Supported values:
    - `"window"`: Floating window
    - `"navigation"`: Navigation panel (left dock on desktop, top on mobile)
    - `"inventory"`: Inventory panel (right dock on desktop, bottom on mobile)
    - `"status"`: Status panel (right dock on desktop, top on mobile)
    - `"tools"`: Tools panel (right dock on desktop, bottom on mobile)
    - `"communication"`: Communication panel (left dock on desktop, top on mobile)
    - `"help"`: Help panel (right dock by default)
    - `"verb-editor"`: Verb editor window
- `content`: (Optional) The actual content to display
- `attributes`: (Optional) Additional attributes as a list of {key, value} pairs or a map

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
```

**Notes:**

- Only available when the `rich_notify` server configuration option is enabled
- Telnet clients will ignore presentation events
- Web and mobile clients can render presentations as appropriate UI elements
- The `id` parameter allows updating or removing presentations later
- With only 2 arguments (player, id), removes/unpresents the specified presentation

### `connected_players`

**Description:** Returns a list of all players currently connected to the server.  
**Arguments:** None

### `is_player`

**Description:** Determines if a given object is a player or player-like entity.  
**Arguments:**

- : The object to check `object`

### `boot_player`

**Description:** Forcibly disconnects a player from the server.  
**Arguments:**

- : The player to disconnect `player`
- `reason`: Optional message explaining the reason for disconnection

### `player_event_log_stats`

**Description:** Returns summary information about a player's encrypted event history.

**Syntax:** `list player_event_log_stats(obj player [, num|float|none since [, num|float|none until]])`

**Arguments:**

- `player`: Player object to inspect (must be owned by the caller or the caller must be a wizard)
- `since`: (Optional) Earliest timestamp (seconds since UNIX epoch) to include; pass `none` to ignore
- `until`: (Optional) Latest timestamp (seconds since UNIX epoch) to include; pass `none` to ignore

**Returns:** A list `[total_events, earliest, latest]` where `total_events` is the count of matching entries,
`earliest` and `latest` are timestamps (seconds since epoch) or `none` if no events exist.

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

**Description:** Deletes part or all of a player's encrypted history, optionally removing their stored public key.

**Syntax:** `list purge_player_event_log(obj player [, num|float|none before [, any drop_pubkey]])`

**Arguments:**

- `player`: Player object to purge (must be owned by the caller or the caller must be a wizard)
- `before`: (Optional) Remove events at or before this timestamp (seconds since UNIX epoch). Pass `none` to remove all
  history.
- `drop_pubkey`: (Optional) Truthy value indicates the player's stored public key should also be removed.

**Returns:** A list `[deleted_events, pubkey_deleted]` where `deleted_events` is the number of entries removed and
`pubkey_deleted` is `1` if the stored encryption public key was removed, `0` otherwise.

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

- Removing the public key causes the client to generate a fresh keypair on next login, making older encrypted entries
  unreadable.
- Passing `none` for `before` deletes all stored history for the player.

## Permission and Caller Management

### `caller_perms`

**Description:** Returns the object representing the permissions of the code that directly called the current verb.

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

- Returns the identity of the code that called this verb, not the current task's effective permissions
- The current task's permissions are initially determined by the owner of the verb being executed
- Use `set_task_perms()` to temporarily change the task's permissions for subsequent operations
- Commonly used for permission checks in verbs that need to validate or restrict access based on who called them
- Different from `this` (the object the verb is defined on)

### `set_task_perms`

**Description:** Changes the effective permissions of the current task for subsequent operations. The task's initial
permissions are determined by the owner of the verb, but this function allows temporary elevation or downgrade of
permissions.

**Syntax:** `none set_task_perms(obj perms)`

**Arguments:**

- `perms`: The object whose permissions should be used for subsequent operations in this task

**Returns:** None

**Permission Requirements:**

- If the caller is a wizard, they can set task permissions to any object
- If the caller is not a wizard, they can only set task permissions to themselves (`perms` must equal `caller_perms()`)

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
    // Note: cannot call set_task_perms() again from here
endverb
```

**Errors:**

- `E_PERM`: Raised if a non-wizard tries to set permissions to an object other than themselves
- `E_TYPE`: Raised if the argument is not an object
- `E_ARGS`: Raised if the wrong number of arguments is provided

**Notes:**

- This affects permission checks in subsequent operations called within this task
- The change persists until the task completes or `set_task_perms()` is called again
- Only affects the active stack frame's effective permissions; does not change what `caller_perms()` returns, and does
  not alter what happens in subsequent verb calls
- `caller_perms()` always returns the identity of the original caller, regardless of `set_task_perms()` calls
- Commonly used in wizard-owned verbs to safely downgrade permissions or temporarily elevate them for privileged
  operations

### `callers`

**Description:** Returns a list of all callers in the current call stack.  
**Arguments:**

- `level`: Optional parameter to specify how many levels of the call stack to return

## Task and Connection Management

### `task_id`

**Description:** Returns the unique identifier for the current task.  
**Arguments:** None

### `idle_seconds`

**Description:** Returns the number of seconds a player or entity has been idle.  
**Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connected_seconds`

**Description:** Returns the total duration a player has been connected in seconds.  
**Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connection_name`

**Description:** Returns the name of the current connection.  
**Arguments:**

- : Optional player to check (defaults to current player if omitted) `player`

### `connections`

**Description:** Returns the connection objects associated with the current or another player. Connection objects all
have negative IDs (e.g., #-123) and represent the physical connection or line to the server.
**Arguments:**

- `player`: Optional player object to query. If omitted, returns information for the current session. If provided,
  requires wizard permissions or must be the caller's own object.

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

### `queued_tasks`

**Description:** Returns a list of tasks currently in the queue waiting to be executed.  
**Arguments:** None

### `active_tasks`

**Description:** Returns a list of tasks that are currently running.  
**Arguments:** None

### `queue_info`

**Description:** Provides detailed information about the task queue.  
**Arguments:**

- : Optional ID to get information about a specific queued task `task_id`

### `kill_task`

**Description:** Terminates a specific task by its ID.  
**Arguments:**

- : The ID of the task to terminate `task_id`

### `ticks_left`

**Description:** Returns the number of execution ticks remaining for the current task.  
**Arguments:** None

### `seconds_left`

**Description:** Returns the number of seconds remaining before the current task times out.  
**Arguments:** None

## Time Functions

### `time`

**Description:** Returns the current server time, likely as a Unix timestamp.  
**Arguments:** None

### `ftime`

**Description:** Formats a timestamp into a human-readable string.  
**Arguments:**

- : The timestamp to format `time`
- : Optional format string for controlling the output format `format`

### `ctime`

**Description:** Converts a timestamp to a standard calendar time representation.  
**Arguments:**

- : The timestamp to convert `time`

## Server Control and Information

### `shutdown`

**Description:** Initiates a server shutdown process.  
**Arguments:**

- `delay`: Optional delay in seconds before shutdown
- `reason`: Optional message explaining the reason for shutdown

### `server_version`

**Description:** Returns the version information for the server.  
**Arguments:** None

### `suspend`

**Description:** Temporarily suspends the execution of the current task.  
**Arguments:**

- : Optional number of seconds to suspend execution `seconds`

### `resume`

**Description:** Resumes execution of a previously suspended task.  
**Arguments:**

- : The ID of the task to resume `task_id`

### `server_log`

**Description:** Writes a message to the server log.  
**Arguments:**

- : The message to log `message`
- `level`: Optional log level (e.g., "info", "warning", "error")

### `memory_usage`

**Description:** Returns information about the server's memory usage.  
**Arguments:**

- `detailed`: Optional boolean flag for requesting detailed information

### `db_disk_size`

**Description:** Returns the size of the database on disk.  
**Arguments:** None

### `load_server_options`

**Description:** Loads or reloads the server configuration options.  
**Arguments:**

- `filename`: Optional path to a configuration file

## Database Operations

### `commit`

**Description:** Commits pending changes to the database.  
**Arguments:** None

### `rollback`

**Description:** Rolls back pending changes to the database.  
**Arguments:** None

### `read`

**Description:** Reads data from a specified source, likely a file or database entry.  
**Arguments:**

- `source`: The source to read from (file path, database key, etc.)
- `options`: Optional parameters controlling the read operation

### `dump_database`

**Description:** Creates a dump of the database, typically for backup purposes.  
**Arguments:**

- `filename`: Optional output filename for the dump
- `options`: Optional flags controlling the dump format

## Event Handling

### `listen`

**Description:** Registers to listen for specific events.  
**Arguments:**

- `event_type`: The type of event to listen for
- `callback`: The function to call when the event occurs

### `listeners`

**Description:** Returns a list of all current event listeners.  
**Arguments:**

- `event_type`: Optional parameter to filter listeners by event type

### `unlisten`

**Description:** Removes a previously registered event listener.  
**Arguments:**

- `event_type`: The type of event to stop listening for
- `callback`: Optional specific callback to remove (if omitted, removes all listeners for the event)

## Code Execution

### `eval`

**Description:** Compiles and evaluates a MOO expression or statement dynamically at runtime.
**Arguments:**

- `program`: String containing MOO code to evaluate
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

- On success: list containing `{result, ...}` where `result` is the value produced by evaluating the code
- On compilation failure with `verbosity` 0-2: list where first element is error object, followed by formatted error
  strings
- On compilation failure with `verbosity` 3: list where first element is error object, second is map containing
  structured error data (use `format_compile_error()` to format)

**Note:** Requires programmer bit. The evaluated code runs with the permissions of the current task.

**Examples:**

```moo
// Basic evaluation
eval("2 + 2");
=> {4}

// Evaluation with compilation error (default summary)
eval("2 +");
=> {E_INVARG, "Parse error at line 1, column 4: ..."}

// Get detailed error with context
eval("return 1 + ;", 2);
=> {E_INVARG, "Parse error at line 1, column 12:", "  return 1 + ;", "             ⚠", ...}

// Get structured error data for custom handling
result = eval("return 1 + ;", 3);
if (typeof(result[1]) == MAP)
  // result[1] is the error map, result[2] is structured data
  formatted = format_compile_error(result[2], 0);
endif
```

### `call_function`

**Description:** Calls a specified function with provided arguments.  
**Arguments:**

- `function`: The function to call
- : The arguments to pass to the function `args`

### `function_info`

**Description:** Returns information about a specified function.
**Arguments:**

- `function`: The function to get information about

### `function_help`

**Description:** Returns documentation for a specified builtin function.
**Arguments:**

- `function_name` (str): The name of the builtin function

**Returns:** A list of strings containing the documentation extracted from the builtin's implementation.

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

Documentation is extracted at compile time from the doc-comments of each builtin's Rust implementation, ensuring it
always matches the running program.

### `wait_task`

**Description:** Waits for a specified task to complete.  
**Arguments:**

- : The ID of the task to wait for `task_id`
- `timeout`: Optional timeout in seconds

## Performance Monitoring

### `bf_counters`

**Description:** Returns performance counters related to built-in functions.  
**Arguments:**

- : Optional parameter to control the return format `format`
- `reset`: Optional boolean to reset counters after reading

### `db_counters`

**Description:** Returns performance counters related to database operations.  
**Arguments:**

- : Optional parameter to control the return format `format`
- `reset`: Optional boolean to reset counters after reading

### `vm_counters`

**Description:** Returns performance counters related to the virtual machine.  
**Arguments:**

- : Optional parameter to control the return format `format`
- `reset`: Optional boolean to reset counters after reading

### `sched_counters`

**Description:** Returns performance counters related to the task scheduler.  
**Arguments:**

- : Optional parameter to control the return format `format`
- `reset`: Optional boolean to reset counters after reading

## Miscellaneous

### `raise`

**Description:** Raises an error or exception.  
**Arguments:**

- `error_type`: The type of error to raise
- : Optional error message `message`

### `force_input`

**Description:** Forces input to be processed as if it came from a specific source.  
**Arguments:**

- `source`: The source entity (typically a player)
- `input`: The input text to process
