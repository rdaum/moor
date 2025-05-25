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

> Note: Only one network connection can be controlling a given player object at a given time; should a second connection
> attempt to log in as that player, the first connection is unceremoniously closed (and `$user_reconnected()` called, as
> described above). This makes it easy to recover from various kinds of network problems that leave connections open but
> inaccessible.

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

`worker_request()` is used to send a request to the `curl_worker` to perform an HTTP request like so:

```moo
worker_request("curl_worker", { "GET", "https://example.com/api/data", { "Accept": "application/json" } })
```

In this example, the `curl_worker` is being asked to perform a GET request to the specified URL, with an optional header
indicating that the response should be in JSON format. The `worker_request()` function will then suspend the current
task
until the worker completes the request and then wake it to return the result.
