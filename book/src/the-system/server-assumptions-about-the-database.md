# Server Assumptions About the Database

There are a small number of circumstances under which the mooR server directly and specifically accesses a particular verb or
property in the database. This section gives a complete list of such circumstances for the mooR server.

## Server Options Set in the Database

The mooR server can be controlled from within the database by creating the property
`#0.server_options` (also known as `$server_options`), assigning as its value a valid object number, and then defining
various properties on that object. The server checks for whether the property `$server_options`
exists and has an object number as its value. If so, then the server looks for the following properties on that
`$server_options` object and uses their values to control server operation:

| Property        | Description                                                               |
|-----------------|---------------------------------------------------------------------------|
| bg_seconds      | The number of seconds allotted to background tasks.                      |
| bg_ticks        | The number of ticks allotted to background tasks.                        |
| fg_seconds      | The number of seconds allotted to foreground tasks.                      |
| fg_ticks        | The number of ticks allotted to foreground tasks.                        |
| max_stack_depth | The maximum number of levels of nested verb calls.                       |

> **Note**: The mooR server does NOT implement the `protect_*` properties (e.g., `protect_location`) that were available in LambdaMOO and ToastStunt for restricting access to built-in functions and properties.

## Other System Properties

Some server options are read directly from system object `#0`, not from the `$server_options` object:

| Property        | Location        | Description                                                               |
|-----------------|-----------------|---------------------------------------------------------------------------|
| dump_interval   | `#0.dump_interval` | The interval in seconds for automatic database checkpoints.           |

## Command-Line Overrides

Some server options can be overridden by command-line arguments when starting the daemon:

| Database Property | Command-Line Override      | Description                                           |
|-------------------|----------------------------|-------------------------------------------------------|
| dump_interval     | --checkpoint-interval      | Automatic database checkpoint interval                |


## Server Messages Set in the Database

> **Note**: The mooR server does NOT currently implement customizable server messages. The message customization system described below was available in LambdaMOO and ToastStunt but is not yet implemented in mooR.

In LambdaMOO and ToastStunt, there were a number of circumstances under which the server itself generated customizable messages on network connections. Properties on `$server_options` could be used to customize these messages. The mooR server may implement this functionality in the future.

For reference, the LambdaMOO/ToastStunt customizable messages were:

| Property Name   | Default Message                     | Description                                                                  |
|-----------------|-------------------------------------|------------------------------------------------------------------------------|
| boot_msg        | "*** Disconnected ***"             | The function boot_player() was called on this connection.                   |
| connect_msg     | "*** Connected ***"                 | User logged in (existing user object).                                      |
| create_msg      | "*** Created ***"                   | User logged in (new user object created).                                   |
| recycle_msg     | "*** Recycled ***"                  | The logged-in user of this connection has been recycled or renumbered.      |
| redirect_from_msg | "*** Redirecting connection to new port ***" | User logged in on another connection.                      |
| redirect_to_msg | "*** Redirecting old connection to this port ***" | User was already logged in elsewhere.                   |
| server_full_msg | "*** Sorry, but the server cannot accept any more connections right now. ***" | Server cannot accept more connections. |
| timeout_msg     | "*** Timed-out waiting for login. ***" | Connection idle and un-logged-in for too long.                          |

## Checkpointing (or backing up) the Database

The mooR server maintains the entire MOO database in main memory and on disk in a binary format. Restarting the server will
always restore the system to the state it was in when the server was last run. However, this binary format is not
human-readable, and can change between different versions of the server. Thus, it is important to periodically
_checkpoint_ the database, which means writing a copy of the current state of the database to disk in a human-readable
format.

### Automatic Checkpointing

The mooR server supports automatic checkpointing at regular intervals. The interval can be configured in two ways (in order of precedence):

1. **Command-line**: Using `--checkpoint-interval-seconds` when starting the daemon
2. **Database**: Setting `#0.dump_interval` to the number of seconds between checkpoints

If neither is configured, automatic checkpointing is disabled.

### Manual Checkpointing  

Checkpoints can also be requested manually using the `dump_database()` built-in function.

### Checkpoint Formats

There are two formats in which the mooR server can write checkpoints:

**Objdef Format** (default): A directory-oriented format where each object is stored in its own file. The file contains the object number (or $sysobj-style name) and lists the object's attributes, properties, and verbs in a readable and editable format. This format is well-suited for use with version control systems such as git and can be effectively used with diff/merge tools.

**Textdump Format**: The "legacy" LambdaMOO-compatible format. Non-mooR "core" files or checkpoints are likely to have been written in this format. This format outputs a single large text file containing the entire database.

