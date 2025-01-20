# Server Assumptions About the Database

There are a small number of circumstances under which the server directly and specifically accesses a particular verb or property in the database. This section gives a complete list of such circumstances.

## Server Options Set in the Database

Many optional behaviors of the server can be controlled from within the database by creating the property `#0.server_options` (also known as `$server_options`), assigning as its value a valid object number, and then defining various properties on that object. At a number of times, the server checks for whether the property `$server_options` exists and has an object number as its value. If so, then the server looks for a variety of other properties on that `$server_options` object and, if they exist, uses their values to control how the server operates.

The specific properties searched for are each described in the appropriate section below, but here is a brief list of all of the relevant properties for ease of reference:

| Property                         | Description                                                                                                                                                |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| bg_seconds                       | The number of seconds allotted to background tasks.                                                                                                        |
| bg_ticks                         | The number of ticks allotted to background tasks.                                                                                                          |
| connect_timeout                  | The maximum number of seconds to allow an un-logged-in in-bound connection to remain open.                                                                 |
| default_flush_command            | The initial setting of each new connection&apos;s flush command.                                                                                           |
| fg_seconds                       | The number of seconds allotted to foreground tasks.                                                                                                        |
| fg_ticks                         | The number of ticks allotted to foreground tasks.                                                                                                          |
| max_stack_depth                  | The maximum number of levels of nested verb calls. Only used if it is higher than default                                                                  |
| name_lookup_timeout              | The maximum number of seconds to wait for a network hostname/address lookup.                                                                               |
| outbound_connect_timeout         | The maximum number of seconds to wait for an outbound network connection to successfully open.                                                             |
| protect_`property`               | Restrict reading/writing of built-in `property` to wizards.                                                                                                |
| protect_`function`               | Restrict use of built-in `function` to wizards.                                                                                                            |
| queued_task_limit                | The maximum number of forked or suspended tasks any player can have queued at a given time.                                                                |
| support_numeric_verbname_strings | Enables use of an obsolete verb-naming mechanism.                                                                                                          |
| max_queued_output                | The maximum number of output characters the server is willing to buffer for any given network connection before discarding old output to make way for new. |
| dump_interval                    | an int in seconds for how often to checkpoint the database.                                                                                                |
| proxy_rewrite                    | control whether IPs from proxies get rewritten.                                                                                                            |
| file_io_max_files                | allow DB-changeable limits on how many files can be opened at once.                                                                                        |
| sqlite_max_handles               | allow DB-changeable limits on how many SQLite connections can be opened at once.                                                                           |
| task_lag_threshold               | override default task_lag_threshold for handling lagging tasks                                                                                             |
| finished_tasks_limit             | override default finished_tasks_limit (enables the finished_tasks function and define how many tasks get saved by default)                                 |
| no_name_lookup                   | override default no_name_lookup (disables automatic DNS name resolution on new connections)                                                                |
| max_list_concat                  | limit the size of user-constructed lists                                                                                                                   |
| max_string_concat                | limit the size of user-constructed strings                                                                                                                 |
| max_concat_catchable             | govern whether violating concat size limits causes out-of-seconds or E_QUOTA error                                                                         |

> Note: If you override a default value that was defined in options.h (such as no_name_lookup or finished_tasks_limit, or many others) you will need to call `load_server_options()` for your changes to take affect.

> Note: Verbs defined on #0 are not longer subject to the wiz-only permissions check on built-in functions generated by defining $server_options.protect_FOO with a true value. Thus, you can now write a `wrapper' for a built-in function without having to re-implement all of the server's built-in permissions checks for that function.

> Note: If a built-in function FOO has been made wiz-only (by defining $server_options.protect_FOO with a true value) and a call is made to that function from a non-wiz verb not defined on #0 (that is, if the server is about to raise E_PERM), the server first checks to see if the verb #0:bf_FOO exists. If so, it calls it instead of raising E_PERM and returns or raises whatever it returns or raises.

> Note: options.h #defines IGNORE_PROP_PROTECTED by default. If it is defined, the server ignores all attempts to protect built-in properties (such as $server_options.protect_location). Protecting properties is a significant performance hit, and most MOOs do not use this functionality.

## Server Messages Set in the Database

There are a number of circumstances under which the server itself generates messages on network connections. Most of these can be customized or even eliminated from within the database. In each such case, a property on `$server_options` is checked at the time the message would be printed. If the property does not exist, a default message is printed. If the property exists and its value is not a string or a list containing strings, then no message is printed at all. Otherwise, the string(s) are printed in place of the default message, one string per line. None of these messages are ever printed on an outbound network connection created by the function `open_network_connection()`.

The following list covers all of the customizable messages, showing for each the name of the relevant property on `$server_options`, the default message, and the circumstances under which the message is printed:

| Default Message                                                                                                                  | Description                                                                                                                                                      |
| -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| boot_msg = &quot;*** Disconnected ***&quot;                                                                                      | The function boot_player() was called on this connection.                                                                                                        |
| connect_msg = &quot;*** Connected ***&quot;                                                                                      | The user object that just logged in on this connection existed before $do_login_command() was called.                                                            |
| create_msg = &quot;*** Created ***&quot;                                                                                         | The user object that just logged in on this connection did not exist before $do_login_command() was called.                                                      |
| recycle_msg = &quot;*** Recycled ***&quot;                                                                                       | The logged-in user of this connection has been recycled or renumbered (via the renumber() function).                                                             |
| redirect_from_msg = &quot;*** Redirecting connection to new port ***&quot;                                                       | The logged-in user of this connection has just logged in on some other connection.                                                                               |
| redirect_to_msg = &quot;*** Redirecting old connection to this port ***&quot;                                                    | The user who just logged in on this connection was already logged in on some other connection.                                                                   |
| server_full_msg Default: *** Sorry, but the server cannot accept any more connections right now.<br> *** Please try again later. | This connection arrived when the server really couldn&apos;t accept any more connections, due to running out of a critical operating system resource.            |
| timeout_msg = &quot;*** Timed-out waiting for login. ***&quot;                                                                   | This in-bound network connection was idle and un-logged-in for at least CONNECT_TIMEOUT seconds (as defined in the file options.h when the server was compiled). |

> Fine point: If the network connection in question was received at a listening point (established by the `listen()` function) handled by an object obj other than `#0`, then system messages for that connection are looked for on `obj.server_options`; if that property does not exist, then `$server_options` is used instead.

## Checkpointing the Database

The server maintains the entire MOO database in main memory, not on disk. It is therefore necessary for it to dump the database to disk if it is to persist beyond the lifetime of any particular server execution. The server is careful to dump the database just before shutting down, of course, but it is also prudent for it to do so at regular intervals, just in case something untoward happens.

//TODO: is the date here still true in 64bit time?

To determine how often to make these _checkpoints_ of the database, the server consults the value of `$server_options.dump_interval`. If it exists and its value is an integer greater than or equal to 60, then it is taken as the number of seconds to wait between checkpoints; otherwise, the server makes a new checkpoint every 3600 seconds (one hour). If the value of `$server_options.dump_interval` implies that the next checkpoint should be scheduled at a time after 3:14:07 a.m. on Tuesday, January 19, 2038, then the server instead uses the default value of 3600 seconds in the future.

The decision about how long to wait between checkpoints is made again immediately after each one begins. Thus, changes to `$server_options.dump_interval` will take effect after the next checkpoint happens.

Whenever the server begins to make a checkpoint, it makes the following verb call:

```
$checkpoint_started()
```

When the checkpointing process is complete, the server makes the following verb call:

```
$checkpoint_finished(success)
```

where success is true if and only if the checkpoint was successfully written on the disk. Checkpointing can fail for a number of reasons, usually due to exhaustion of various operating system resources such as virtual memory or disk space. It is not an error if either of these verbs does not exist; the corresponding call is simply skipped.
