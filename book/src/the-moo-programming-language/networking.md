# Networking

## Handling Network Connections

When the server first accepts a new, incoming network connection, it is given the low-level network address of computer
on the other end. It immediately attempts to convert this address into the human-readable host name that will be entered
in the server log and returned by the `connection_name()` function. This conversion can, for the TCP/IP networking
configurations, involve a certain amount of communication with remote name servers, which can take quite a long time
and/or fail entirely. While the server is doing this conversion, it is not doing anything else at all; in particular, it
it not responding to user commands or executing MOO tasks.

By default, the server will wait no more than 5 seconds for such a name lookup to succeed; after that, it behaves as if
the conversion had failed, using instead a printable representation of the low-level address. If the property
`name_lookup_timeout` exists on `$server_options` and has an integer as its value, that integer is used instead as the
timeout interval.

## Connection Objects and Player Objects

mooR follows the classic LambdaMOO connection model, which makes a clear distinction between _connection objects_ and
_player objects_:

- **Connection Objects**: Each network connection is represented by a unique object with a negative ID (e.g., #-123).
  These represent the physical network connection or "line" and persist for the entire duration of the connection,
  regardless of login status.
- **Player Objects**: These have positive IDs and represent logged-in users. A player object only exists when someone
  has successfully logged in.

This dual-object model is important to understand because:

1. **Connection objects don't "go away" after login** - Unlike LambdaMOO implementations, the negative connection object
   continues to exist and can be used with functions like `notify()` even after a player has logged in.

2. **Multiple connections per player** - Unlike traditional LambdaMOO, mooR supports multiple simultaneous connections
   for the same player object. Each connection maintains its own connection object.

3. **Both objects work with functions** - You can use `notify()`, `boot_player()`, and other functions with either the
   negative connection object or the positive player object.

You can use the `connections()` builtin function to discover the relationship between connection and player objects for
any session.

4. **The connection object is useful** - You can use it to send events that are relevant only for that specific physical
   connection.

## Content Types and Rich Client Support

mooR supports rich client capabilities through a content type system. Each connection advertises which content types it
can handle, allowing MOO code to send appropriately formatted content to different types of clients.

### Supported Content Types

- **`text/plain`**: Plain text (always supported by all connections)
- **`text/markdown`**: Markdown-formatted text (supported by telnet clients)
- **`text/html`**: HTML-formatted text (supported by web clients)
- **`text/djot`**: Djot-formatted text (supported by web clients)

### Content Type Negotiation

The content types are automatically negotiated when a connection is established:

- **Telnet connections** advertise: `["text/plain", "text/markdown"]`
- **Web connections** advertise: `["text/plain", "text/html", "text/djot"]`
- **Default/fallback** connections advertise: `["text/plain"]`

You can discover the content types supported by a connection using the `connections()` function:

```moo
// Check what content types a connection supports
foreach conn in (connections())
    {connection_obj, hostname, idle_time, content_types} = conn;
    if ("text/html" in content_types)
        notify(connection_obj, "<h1>Welcome!</h1>", false, false, "text/html");
    else
        notify(connection_obj, "# Welcome!", false, false, "text/markdown");
    endif
endfor
```

### Web Client Inline Links

When sending `text/html` or `text/djot` content, the web client converts anchors into interactive elements and recognizes
`moo://` link schemes:

| Link Pattern | Action |
|--------------|--------|
| `moo://cmd/<command>` | Run the URL-decoded command as if typed by the player |
| `moo://inspect/<ref>` | Show object info popover (calls `<ref>:inspection`) |

Standard `http(s)` links open in a new tab.

```moo
// Inline command link in djot
notify(player, "[look](moo://cmd/look)", false, false, "text/djot");
```

For more details on web client output features, see [Client Output and Presentations](../web-client/client-output-and-presentations.md).

### Accessibility Metadata (Web Client)

If your core emits narrative or prompt metadata, the web client recognizes:

- `tts_text`: Alternate narration text for screen readers (used instead of formatted output).
- `tts_prompt`: Alternate prompt text for rich input prompts.

For full accessibility guidelines, see [Web Client Accessibility](../web-client/accessibility.md).

### Advanced notify() Options

The `notify()` function supports additional optional arguments for performance control and content types:

**Full Syntax:** `notify(player, message [, no_flush [, no_newline [, content_type]]])`

- **3rd argument (`no_flush`)**: If true, don't immediately flush network buffers - useful for sending multiple messages
  quickly
- **4th argument (`no_newline`)**: If true, don't add a newline after the message - useful for building output on one
  line
- **5th argument (`content_type`)**: Specify content type for rich clients ("text/plain", "text/html", "
  text/markdown", "text/djot", etc.)

```moo
// Basic usage
notify(player, "Hello world!");

// Performance optimization - send multiple parts without flushing
notify(player, "Loading", true, true);        // no flush, no newline
notify(player, ".", true, true);              // no flush, no newline  
notify(player, ".", true, true);              // no flush, no newline
notify(player, ". done!", false, false);      // flush and add newline

// Rich content with different formats
notify(player, "Hello **world**!", false, false, "text/markdown");
notify(player, "<strong>Hello world!</strong>", false, false, "text/html");
notify(player, "Hello world!");               // defaults to text/plain
```

### The `connections()` Function

The `connections([player])` builtin function returns detailed connection information for the current player
or a specified player (with wizard permissions).

**Syntax**: `connections()` or `connections(player)`

**Returns**: A list of lists containing connection details. Each inner list contains:

- Index 0: The connection object (negative ID, e.g., #-42)
- Index 1: The hostname/connection name (string)
- Index 2: The idle time in seconds (float)
- Index 3: The acceptable content types for this connection (list of strings/symbols)

**Examples**:

```moo
// Get connection info for current session
connections()
=> {{#-42, "player.example.com", 15.3, {"text/plain", "text/markdown"}}}  // One telnet connection

// Get connection info for another player (requires wizard permissions)
connections(#456)
=> {{#-89, "other.example.com", 0.5, {"text/plain", "text/html", "text/djot"}}}  // Web connection

// Multiple connections for same player (telnet + web)
connections()
=> {{#-42, "desktop.example.com", 5.0, {"text/plain", "text/markdown"}}, 
    {#-43, "mobile.example.com", 300.5, {"text/plain", "text/html", "text/djot"}}}

// Check an unlogged connection (would need to be called from that context)
connections()  // Called from an unlogged connection
=> {{#-55, "unknown.host.com", 0.0, {"text/plain"}}}  // Default content type
```

**Permission Requirements**:

- `connections()` with no arguments: Available to all users for their own session
- `connections(player)` with a player argument: Requires wizard permissions OR the player must be the caller's own
  object

**Important Note About Connection Order**:

When `connections()` is called with no arguments, the **first connection** in the returned list is always the connection
object that initiated the current task. This is useful when a player has multiple active connections and you need to
identify which specific connection is executing the current code. When `connections(player)` is called with a player
argument, there is no guarantee about the ordering of connections.

### The `connection()` Function

The `connection()` builtin function returns the specific connection object that initiated the current task. This is a
convenience function that is equivalent to getting the first element from `connections()`, but more explicit about intent.

**Syntax**: `connection()`

**Returns**: The connection object (negative ID) that initiated the current task

**Raises**:
- `E_INVARG`: If no connection is found for the current task (e.g., in background tasks)

**Examples**:

```moo
// Get the connection object for the current task
my_connection = connection();
=> #-42

// Use it to send output to only this specific connection
notify(connection(), "This message goes only to the connection that ran this command");

// Useful when a player has multiple connections
// and you want to affect only the one they're currently using
set_connection_option(connection(), "hold-input", 1);

// Compare with getting all connections
all_conns = connections();
current_conn = connection();
=> current_conn == all_conns[1][1]  // Always true when connections() returns results

// Useful in verbs that need to know which connection triggered them
if (connection() == some_specific_connection_obj)
    notify(player, "You're using the expected connection");
else
    notify(player, "This command should be run from a different connection");
endif
```

**Use Cases**:

- **Per-connection state**: Maintain different state for each connection a player has open
- **Connection-specific commands**: Implement commands that should only affect the connection they're run from
- **Debugging**: Identify which connection is causing issues when a player has multiple sessions
- **Security**: Verify that sensitive operations are coming from expected connections

**Background Tasks**:

Note that tasks started via `fork()` or other background mechanisms may not have an associated connection object,
in which case `connection()` will raise `E_INVARG`. Always handle this case if your code might run in background tasks:

```moo
try
    conn = connection();
    // Do something with conn
except e (E_INVARG)
    // No connection available (background task)
endtry
```

## Associating Network Connections with Players

When a network connection is first made to the MOO, it is identified by a unique, negative object number. Such a
connection is said to be _un-logged-in_ and is not yet associated with any MOO player object.

Each line of input on an un-logged-in connection is first parsed into words in the usual way (see the chapter on command
parsing for details) and then these words are passed as the arguments in a call to the verb `$do_login_command()`. For
example, the input line

```
connect Munchkin frebblebit
```

would result in the following call being made:

```
$do_login_command("connect", "Munchkin", "frebblebit")
```

In that call, the variable `player` will have as its value the negative object number associated with the appropriate
network connection. The functions `notify()` and `boot_player()` can be used with such object numbers to send output to
and disconnect un-logged-in connections. Also, the variable `argstr` will have as its value the unparsed command line as
received on the network connection.

If `$do_login_command()` returns a valid player object and the connection is still open, then the connection is
considered to have _logged into_ that player. The server then makes one of the following verbs calls, depending on the
player object that was returned:

```
$user_created(player)
$user_connected(player)
$user_reconnected(player)
```

The first of these is used if the returned object number is greater than the value returned by the `max_object()`
function before `$do_login_command()` was invoked, that is, it is called if the returned object appears to have been
freshly created. If this is not the case, then one of the other two verb calls is used. The `$user_connected()` call is
used if there was no existing active connection for the returned player object. Otherwise, the `$user_reconnected()`
call is used instead.

> Fine point: If a user reconnects and the user's old and new connections are on two different listening points being
> handled by different objects (see the description of the `listen()` function for more details), then
`user_client_disconnected` is called for the old connection and `user_connected` for the new one.

> Note: If any code suspends in do_login_command() or a verb called by do_login_command() (read(), suspend(), or any
> threaded function), you can no longer connect an object by returning it. This is a weird ancient MOO holdover. The
> best
> way to log a player in after suspending is to use the `switch_player()` function to switch their unlogged in negative
> object to their player object.

If an in-bound network connection does not successfully log in within a certain period of time, the server will
automatically shut down the connection, thereby freeing up the resources associated with maintaining it. Let L be the
object handling the listening point on which the connection was received (or `#0` if the connection came in on the
initial listening point). To discover the timeout period, the server checks on `L.server_options` or, if it doesn't
exist, on `$server_options` for a `connect_timeout` property. If one is found and its value is a positive integer, then
that's the number of seconds the server will use for the timeout period. If the `connect_timeout` property exists but
its value isn't a positive integer, then there is no timeout at all. If the property doesn't exist, then the default
timeout is 300 seconds.

When any network connection (even an un-logged-in or outbound one) is terminated, by either the server or the client,
then one of the following two verb calls is made:

```
$user_disconnected(player)
$user_client_disconnected(player)
```

The first is used if the disconnection is due to actions taken by the server (e.g., a use of the `boot_player()`
function or the un-logged-in timeout described above) and the second if the disconnection was initiated by the client
side.

It is not an error if any of these five verbs do not exist; the corresponding call is simply skipped.

> Note: Unlike traditional LambdaMOO, mooR supports multiple simultaneous connections for the same player object. Each
> connection maintains its own connection object (with a negative ID), while all connections associated with the same
> player share the same player object (positive ID). This allows users to connect from multiple devices or applications
> simultaneously. The `connections()` function can be used to discover the active connections for a player.

When the network connection is first established, the null command is automatically entered by the server, resulting in
an initial call to `$do_login_command()` with no arguments. This signal can be used by the verb to print out a welcome
message, for example.

> Warning: If there is no `$do_login_command()` verb defined, then lines of input from un-logged-in connections are
> simply discarded. Thus, it is _necessary_ for any database to include a suitable definition for this verb.

> Note that a database with a missing or broken $do_login_command may still be accessed (and perhaps repaired) by
> running the server with the -e command line option. See section Emergency Wizard Mode.

It is possible to compile the server with an option defining an `out-of-band prefix` for commands. This is a string that
the server will check for at the beginning of every line of input from players, regardless of whether or not those
players are logged in and regardless of whether or not reading tasks are waiting for input from those players. If a
given line of input begins with the defined out-of-band prefix (leading spaces, if any, are _not_ stripped before
testing), then it is not treated as a normal command or as input to any reading task. Instead, the line is parsed into a
list of words in the usual way and those words are given as the arguments in a call to `$do_out_of_band_command()`. For
example, if the out-of-band prefix were defined to be `#$#`, then the line of input

```
#$# client-type fancy
```

would result in the following call being made in a new server task:

```
$do_out_of_band_command("#$#", "client-type", "fancy")
```

During the call to `$do_out_of_band_command()`, the variable `player` is set to the object number representing the
player associated with the connection from which the input line came. Of course, if that connection has not yet logged
in, the object number will be negative. Also, the variable `argstr` will have as its value the unparsed input line as
received on the network connection.

Out-of-band commands are intended for use by fancy client programs that may generate asynchronous _events_ of which the
server must be notified. Since the client cannot, in general, know the state of the player's connection (logged-in or
not, reading task or not), out-of-band commands provide the only reliable client-to-server communications channel.

## Connection Options

Connection options control various behaviors associated with individual network connections. These options allow fine-grained
control over input handling, echo behavior, binary data transmission, and input flushing.

> **Note**: The connection options documented here apply specifically to **telnet connections** handled by the `telnet-host`.
> Other connection types (such as WebSocket connections or custom host implementations) may support different sets of
> connection options or handle these options differently. Always check the documentation for the specific host type you're
> working with.

### `set_connection_option(conn, option, value)`

Controls optional behaviors for a specific connection.

**Syntax**: `set_connection_option(conn, option, value)`

**Parameters**:
- `conn`: The connection object (negative ID) to configure
- `option`: A string naming the option to set
- `value`: The value to set for the option (type depends on the option)

**Returns**: None

**Raises**:
- `E_INVARG`: If `conn` does not specify a current connection
- `E_PERM`: If the programmer is neither `conn` nor a wizard

**Supported Options**:

#### `"hold-input"`

If `value` is true, input received on this connection will never be treated as a command. Instead, it will remain in the
queue until retrieved by a call to `read()`. This is useful for implementing custom input handlers or mini-parsers within
your MOO code.

```moo
// Enable hold-input mode to read raw input
set_connection_option(player, "hold-input", 1);
line = read();  // Will return the next line of input
set_connection_option(player, "hold-input", 0);  // Restore normal command processing
```

#### `"client-echo"`

Sends the Telnet Protocol `WONT ECHO` or `WILL ECHO` command, depending on whether `value` is true or false, respectively.
For clients that support the Telnet Protocol, this should toggle whether or not the client echoes locally the characters
typed by the user. Note that the server itself never echoes input characters under any circumstances.

- `value = true` (1): Server sends `WONT ECHO` - client should echo characters locally (normal mode)
- `value = false` (0): Server sends `WILL ECHO` - client should not echo characters (password mode)

```moo
// Disable client echo for password entry
set_connection_option(player, "client-echo", 0);
password = read();
set_connection_option(player, "client-echo", 1);  // Re-enable echo
```

> Note: This option is only available under TCP/IP networking configurations (telnet connections).

#### `"binary"`

If `value` is true, the connection switches to binary mode where arbitrary bytes can be transmitted. Input from a connection
in binary mode is not broken into lines at all; it is delivered to either the `read()` function or the built-in command
parser as binary values (where `typeof(x) == BINARY`), in whatever size chunks come back from the operating system.

When sending output to a connection in binary mode using `notify()`, you can send either:
- **Binary values** (where `typeof(x) == BINARY`) - sent as raw bytes to the connection
- **Strings** - must be valid UTF-8, sent as UTF-8 bytes to the connection

```moo
// Enable binary mode
set_connection_option(player, "binary", 1);

// Send binary data (arbitrary bytes including non-UTF8)
notify(player, b"AAECA");  // Binary literal

// Can also send regular UTF-8 strings
notify(player, "Hello, world!");

// Reading returns binary values (not line-delimited)
binary_input = read();  // Returns binary value like b"SGVsbG8="
typeof(binary_input);   // Returns BINARY
```

> Note: In classic LambdaMOO, the second argument to `notify()` must be a binary string in binary mode, or `E_INVARG` is
> raised. mooR is more flexible and accepts both binary values and UTF-8 strings.
>
> mooR uses a distinct binary type represented as `b"base64data"` where the content is base64-encoded.
> Regular strings are always valid UTF-8. See the [Binary Type](../the-database/moo-value-types.md#binary-type) section
> for details on working with binary data.

#### `"flush-command"`

If `value` is a non-empty string, it becomes the new flush command for this connection, by which the player can flush all
queued input that has not yet been processed by the server. If `value` is not a non-empty string, then the connection is
set to have no flush command at all.

The default value of this option can be set via the property `$server_options.default_flush_command`.

```moo
// Set a custom flush command
set_connection_option(player, "flush-command", ".clear");
// Player can now type ".clear" to flush queued input

// Disable flush command entirely
set_connection_option(player, "flush-command", "");
```

#### `"disable-oob"`

If `value` is true, out-of-band command processing is disabled for this connection. Lines beginning with the out-of-band
prefix will be treated as normal input instead of triggering calls to `$do_out_of_band_command()`.

```moo
// Disable out-of-band processing
set_connection_option(player, "disable-oob", 1);
```

### `connection_option(conn, option)`

Retrieves the current value of a specific connection option.

**Syntax**: `connection_option(conn, option)`

**Parameters**:
- `conn`: The connection object (negative ID) to query
- `option`: A string naming the option to retrieve

**Returns**: The current value of the specified option

**Raises**:
- `E_INVARG`: If `conn` does not specify a current connection
- `E_PERM`: If the programmer is neither `conn` nor a wizard

```moo
// Check if hold-input is enabled
if (connection_option(player, "hold-input"))
    notify(player, "Input is being held for read() calls");
endif
```

### `connection_options(conn)`

Returns a list of all connection options and their current values.

**Syntax**: `connection_options(conn)`

**Parameters**:
- `conn`: The connection object (negative ID) to query

**Returns**: A list of `{name, value}` pairs describing the current settings of all allowed options

**Raises**:
- `E_INVARG`: If `conn` does not specify a current connection
- `E_PERM`: If the programmer is neither `conn` nor a wizard

```moo
// Display all connection options
for opt in (connection_options(player))
    {name, value} = opt;
    notify(player, tostr(name, " = ", toliteral(value)));
endfor
// Example output:
// "hold-input" = 0
// "client-echo" = 1
// "binary" = 0
// "flush-command" = ".flush"
// "disable-oob" = 0
```

> **Important**: In mooR, connection options work only with connection objects (negative IDs), not player objects. Since
> mooR supports multiple simultaneous connections per player, each connection maintains its own independent set of options.
> Use the `connections()` function to get the connection objects associated with a player.

## Player Input Handlers

**$do_out_of_band_command**

On any connection for which the connection-option disable-oob has not been set, any unflushed incoming lines that begin
with the out-of-band prefix will be treated as out-of-band commands, meaning that if the verb $do_out_of_band_command()
exists and is executable, it will be called for each such line. For more on this, see Out-of-band Processing.

**$do_command**

As we previously described in The Built-in Command Parser, on any logged-in connection that

- is not the subject of a read() call,
- does not have a .program command in progress, and
- has not had its connection option hold-input set,

any incoming line that

- has not been flushed
- is in-band (i.e., has not been consumed by out-of-band processing) and
- is not itself .program or one of the other four intrinsic commands

will result in a call to $do_command() provided that verb exists and is executable. If this verb suspends or returns a
true value, then processing of that line ends at this point, otherwise, whether the verb returned false or did not exist
in the first place, the remainder of the builtin parsing process is invoked.

## Outbound Network Connections via `curl_worker`

Classic LambdaMOO provided a built-in function `open_network_connection()` for making outbound network connections. mooR
does things
differently. Instead of a built-in function, mooR uses separate companion processes called _workers_ to handle
things like outbound network connections. In particular, mooR supports outbound HTTP connections via the `curl_worker`
worker. This worker is a separate process that can be used to make outbound HTTP requests, and it is designed to be
used in conjunction with the `worker_request()` function to perform tasks that require network access, such as fetching
data from external APIs or sending notifications.

The rationale for this design is that it allows the MOO server to remain responsive and not block while waiting for
network.

But even more so, for security reasons, since the worker can be run with different permissions than the MOO server
itself and even
live in a different container or virtual machine or even in a different physical computer or cluster of computers.

`worker_request()` is used to send a request to the `curl` worker to perform an HTTP request like so:

```moo
worker_request("curl", {"GET", "https://example.com/api/data", {"Accept", "application/json"}})
```

In this example, the `curl_worker` is being asked to perform a GET request to the specified URL, with an optional header
indicating that the response should be in JSON format. The `worker_request()` function will then suspend the current
task
until the worker completes the request and then wake it to return the result.

### The `workers()` Function

The `workers()` builtin function provides information about all available workers and their current state. This is
useful for monitoring worker health, debugging worker-related issues, and understanding system capacity.

**Syntax**: `workers()`

**Returns**: A list of lists containing worker information. Each inner list contains:

- Index 1: The worker type (string or symbol, e.g., "curl")
- Index 2: The number of workers of this type currently active (integer)
- Index 3: The total number of requests currently queued across all workers of this type (integer)
- Index 4: The average response time in milliseconds (float, currently always 0.0)
- Index 5: The time in seconds since the last ping from workers of this type (float)

**Permission Requirements**: Wizard-only function. Raises `E_PERM` if called by a non-wizard.

**Examples**:

```moo
// Check all available workers
workers()
=> {{"curl", 2, 0, 0.0, 1.5}, {"email", 1, 3, 0.0, 2.1}}
// Two curl workers, no queued requests, last ping 1.5 seconds ago
// One email worker, 3 queued requests, last ping 2.1 seconds ago

// Monitor worker health in a loop
for worker_info in (workers())
    {worker_type, count, queue_size, avg_time, last_ping} = worker_info;
    if (last_ping > 30.0)
        server_log(tostr("WARNING: ", worker_type, " workers haven't pinged in ", 
                         last_ping, " seconds"), 1);
    endif
    if (queue_size > 10)
        server_log(tostr("WARNING: ", worker_type, " workers have ", 
                         queue_size, " queued requests"), 1);
    endif
endfor

// Check if a specific worker type is available before making requests
curl_workers = {};
for info in (workers())
    if (info[1] == "curl")
        curl_workers = {@curl_workers, info};
    endif
endfor
if (length(curl_workers) == 0)
    return E_INVARG;  // No curl workers available
endif
result = worker_request("curl", {"GET", "https://api.example.com/status"});
```

The `workers()` function is particularly useful for:

- **System monitoring**: Check if workers are healthy and responsive
- **Load balancing**: Understand queue sizes before sending requests
- **Debugging**: Diagnose worker-related issues when requests fail
- **Capacity planning**: Monitor worker utilization over time

**Note**: The average response time field is currently not implemented and always returns 0.0. This may be implemented
in future versions to provide more detailed performance metrics.

## Webhooks

For handling incoming HTTP requests and serving dynamic content from MOO code, see the [Webhooks](../the-moo-programming-language/webhooks.md) documentation. Webhooks allow the MOO to function as a web application server, serving HTML pages, JSON APIs, and other HTTP content directly through the `#0:invoke_http_handler` verb.
