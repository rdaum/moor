## Player Management Functions

### `notify`

**Description:** Sends a notification message to a player or set of players.  
**Arguments:**

- : The player or list of players to notify `player`
- : The message text to send `message`

### `present`

**Description:** Checks if a specified object is present in the current context.  
**Arguments:**

- : The object to check for presence `object`

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

## Permission and Caller Management

### `caller_perms`

**Description:** Returns the permissions of the calling entity.  
**Arguments:** None

### `set_task_perms`

**Description:** Sets the permissions for the current task.  
**Arguments:**

- : The permission level or object to set for the current task `permissions`

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

**Description:** Evaluates code dynamically at runtime.  
**Arguments:**

- `code`: The code string to evaluate
- `environment`: Optional environment context for the evaluation

### `call_function`

**Description:** Calls a specified function with provided arguments.  
**Arguments:**

- `function`: The function to call
- : The arguments to pass to the function `args`

### `function_info`

**Description:** Returns information about a specified function.  
**Arguments:**

- `function`: The function to get information about

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
