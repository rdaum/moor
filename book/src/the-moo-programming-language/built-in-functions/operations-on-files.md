# Operations on Files

There are several administrator-only builtins for manipulating files from inside the MOO. Security is enforced by making these builtins executable with wizard permissions only as well as only allowing access to a directory under the current directory (the one the server is running in). The new builtins are structured similarly to the stdio library for C. This allows MOO-code to perform stream-oriented I/O to files.

Granting MOO code direct access to files opens a hole in the otherwise fairly good wall that the ToastStunt server puts up between the OS and the database. The security is fairly well mitigated by restricting where files can be opened and allowing the builtins to be called by wizard permissions only. It is still possible execute various forms denial of service attacks, but the MOO server allows this form of attack as well.

> Warning: Depending on what Core you are using (ToastCore, LambdaMOO, etc) you may have a utility that acts as a wrapper around the FileIO code. This is the preferred method for dealing with files and directly using the built-ins is discouraged. On ToastCore you may have a $file WAIF you can utilize for this purpose.

> Warning: The FileIO code looks for a 'files' directory in the same directory as the MOO executable. This directory must exist for your code to work.

> Note: More detailed information regarding the FileIO code can be found in the docs/FileioDocs.txt folder of the ToastStunt repo.

The FileIO system has been updated in ToastCore and includes a number of enhancements over earlier LambdaMOO and Stunt versions.

- Faster reading
- Open as many files as you want, configurable with FILE_IO_MAX_FILES or $server_options.file_io_max_files

**FileIO Error Handling**

Errors are always handled by raising some kind of exception. The following exceptions are defined:

`E_FILE`

This is raised when a stdio call returned an error value. CODE is set to E_FILE, MSG is set to the return of strerror() (which may vary from system to system), and VALUE depends on which function raised the error. When a function fails because the stdio function returned EOF, VALUE is set to "EOF".

`E_INVARG`

This is raised for a number of reasons. The common reasons are an invalid FHANDLE being passed to a function and an invalid pathname specification. In each of these cases MSG will be set to the cause and VALUE will be the offending value.

`E_PERM`

This is raised when any of these functions are called with non- wizardly permissions.

**General Functions**

### Function: `file_version`

file_version -- Returns the package shortname/version number of this package e.g.

str `file_version`()

`file_version() => "FIO/1.7"`

**Opening and closing of files and related functions**

File streams are associated with FHANDLES. FHANDLES are similar to the FILE\* using stdio. You get an FHANDLE from file_open. You should not depend on the actual type of FHANDLEs (currently TYPE_INT). FHANDLEs are not persistent across server restarts. That is, files open when the server is shut down are closed when it comes back up and no information about open files is saved in the DB.

### Function: `file_open`

file_open -- Open a file

FHANDLE `file_open`(STR pathname, STR mode)

Raises: E_INVARG if mode is not a valid mode, E_QUOTA if too many files are open.

This opens a file specified by pathname and returns an FHANDLE for it. It ensures pathname is legal. Mode is a string of characters indicating what mode the file is opened in. The mode string is four characters.

The first character must be (r)ead, (w)rite, or (a)ppend. The second must be '+' or '-'. This modifies the previous argument.

- r- opens the file for reading and fails if the file does not exist.
- r+ opens the file for reading and writing and fails if the file does not exist.
- w- opens the file for writing, truncating if it exists and creating if not.
- w+ opens the file for reading and writing, truncating if it exists and creating if not.
- a- opens a file for writing, creates it if it does not exist and positions the stream at the end of the file.
- a+ opens the file for reading and writing, creates it if does not exist and positions the stream at the end of the file.

The third character is either (t)ext or (b)inary. In text mode, data is written as-is from the MOO and data read in by the MOO is stripped of unprintable characters. In binary mode, data is written filtered through the binary-string->raw-bytes conversion and data is read filtered through the raw-bytes->binary-string conversion. For example, in text mode writing " 1B" means three bytes are written: ' ' Similarly, in text mode reading " 1B" means the characters ' ' '1' 'B' were present in the file. In binary mode reading " 1B" means an ASCII ESC was in the file. In text mode, reading an ESC from a file results in the ESC getting stripped.

It is not recommended that files containing unprintable ASCII data be read in text mode, for obvious reasons.

The final character is either 'n' or 'f'. If this character is 'f', whenever data is written to the file, the MOO will force it to finish writing to the physical disk before returning. If it is 'n' then this won't happen.

This is implemented using fopen().

### Function: `file_close`

file_close -- Close a file

void `file_close`(FHANDLE fh)

Closes the file associated with fh.

This is implemented using fclose().

### Function: `file_name`

file_name -- Returns the pathname originally associated with fh by file_open(). This is not necessarily the file's current name if it was renamed or unlinked after the fh was opened.

STR `file_name`(FHANDLE fh)

### Function: `file_openmode`

file_open_mode -- Returns the mode the file associated with fh was opened in.

str `file_openmode`(FHANDLE fh)

### Function: `file_handles`

file_handles -- Return a list of open files

LIST `file_handles` ()

**Input and Output Operations**

### Function: `file_readline`

file_readline -- Reads the next line in the file and returns it (without the newline).

str `file_readline`(FHANDLE fh)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

### Function: `file_readlines`

file_readlines -- Rewinds the file and then reads the specified lines from the file, returning them as a list of strings. After this operation, the stream is positioned right after the last line read.

list `file_readlines`(FHANDLE fh, INT start, INT end)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

### Function: `file_writeline`

file_writeline -- Writes the specified line to the file (adding a newline).

void `file_writeline`(FHANDLE fh, STR line)

Not recommended for use on files in binary mode.

This is implemented using fputs()

### Function: `file_read`

file_read -- Reads up to the specified number of bytes from the file and returns them.

str `file_read`(FHANDLE fh, INT bytes)

Not recommended for use on files in text mode.

This is implemented using fread().

### Function: `file_write`

file_write -- Writes the specified data to the file. Returns number of bytes written.

int `file_write`(FHANDLE fh, STR data)

Not recommended for use on files in text mode.

This is implemented using fwrite().

### Function: `file_count_lines`

file_count_lines -- count the lines in a file

INT `file_count_lines` (FHANDLER fh)

### Function: `file_grep`

file_grep -- search for a string in a file

LIST `file_grep`(FHANDLER fh, STR search [,?match_all = 0])

Assume we have a file `test.txt` with the contents:

```
asdf asdf 11
11
112
```

And we have an open file handler from running:

```
;file_open("test.txt", "r-tn")
```

If we were to execute a file grep:

```
;file_grep(1, "11")
```

We would get the first result:

```
{{"asdf asdf 11", 1}}
```

The resulting LIST is of the form {{STR match, INT line-number}}

If you pass in the optional third argument

```
;file_grep(1, "11", 1)
```

we will receive all the matching results:

```
{{"asdf asdf 11", 1}, {"11", 2}, {"112", 3}}
```

**Getting and setting stream position**

### Function: `file_tell`

file_tell -- Returns position in file.

INT `file_tell`(FHANDLE fh)

This is implemented using ftell().

### Function: `file_seek`

file_seek -- Seeks to a particular location in a file.

void `file_seek`(FHANDLE fh, INT loc, STR whence)

whence is one of the strings:

- "SEEK_SET" - seek to location relative to beginning
- "SEEK_CUR" - seek to location relative to current
- "SEEK_END" - seek to location relative to end

This is implemented using fseek().

### Function: `file_eof`

file_eof -- Returns true if and only if fh's stream is positioned at EOF.

int `file_eof`(FHANDLE fh)

This is implemented using feof().

**Housekeeping operations**

### Function: `file_size`

### Function: `file_last_access`

### Function: `file_last_modify`

### Function: `file_last_change`

### Function: `file_size`

int `file_size`(STR pathname)

int `file_last_access`(STR pathname)

int `file_last_modify`(STR pathname)

int `file_last_change`(STR pathname)

int `file_size`(FHANDLE filehandle)

int `file_last_access`(FHANDLE filehandle)

int `file_last_modify`(FHANDLE filehandle)

int `file_last_change`(FHANDLE filehandle)

Returns the size, last access time, last modify time, or last change time of the specified file. All of these functions also take FHANDLE arguments and then operate on the open file.

### Function: `file_mode`

int `file_mode`(STR filename)

int `file_mode`(FHANDLE fh)

Returns octal mode for a file (e.g. "644").

This is implemented using stat().

### Function: `file_stat`

void `file_stat`(STR pathname)

void `file_stat`(FHANDLE fh)

Returns the result of stat() (or fstat()) on the given file.

Specifically a list as follows:

`{file size in bytes, file type, file access mode, owner, group, last access, last modify, and last change}`

owner and group are always the empty string.

It is recommended that the specific information functions file_size, file_type, file_mode, file_last_access, file_last_modify, and file_last_change be used instead. In most cases only one of these elements is desired and in those cases there's no reason to make and free a list.

### Function: `file_rename`

file_rename - Attempts to rename the oldpath to newpath.

void `file_rename`(STR oldpath, STR newpath)

This is implemented using rename().

### Function: `file_remove`

file_remove -- Attempts to remove the given file.

void `file_remove`(STR pathname)

This is implemented using remove().

### Function: `file_mkdir`

file_mkdir -- Attempts to create the given directory.

void `file_mkdir`(STR pathname)

This is implemented using mkdir().

### Function: `file_rmdir`

file_rmdir -- Attempts to remove the given directory.

void `file_rmdir`(STR pathname)

This is implemented using rmdir().

### Function: `file_list`

file_list -- Attempts to list the contents of the given directory.

LIST `file_list`(STR pathname, [ANY detailed])

Returns a list of files in the directory. If the detailed argument is provided and true, then the list contains detailed entries, otherwise it contains a simple list of names.

detailed entry:

`{STR filename, STR file type, STR file mode, INT file size}`

normal entry:

STR filename

This is implemented using scandir().

### Function: `file_type`

file_type -- Returns the type of the given pathname, one of "reg", "dir", "dev", "fifo", or "socket".

STR `file_type`(STR pathname)

This is implemented using stat().

### Function: `file_chmod`

file_chmod -- Attempts to set mode of a file using mode as an octal string of exactly three characters.

void `file_chmod`(STR filename, STR mode)

This is implemented using chmod().

## Operations on SQLite

SQLite allows you to store information in locally hosted SQLite databases.

### Function: `sqlite_open`

sqlite_open -- The function `sqlite_open` will attempt to open the database at path for use with SQLite.

int `sqlite_open`(STR path to database, [INT options])

The second argument is a bitmask of options. Options are:

SQLITE_PARSE_OBJECTS [4]: Determines whether strings beginning with a pound symbol (#) are interpreted as MOO object numbers or not. The default is true, which means that any queries that would return a string (such as "#123") will be returned as objects.

SQLITE_PARSE_TYPES [2]: If unset, no parsing of rows takes place and only strings are returned.

SQLITE_SANITIZE_STRINGS [8]: If set, newlines (\n) are converted into tabs (\t) to avoid corrupting the MOO database. Default is unset.

> Note: If the MOO doesn't support bitmasking, you can still specify options. You'll just have to manipulate the int yourself. e.g. if you want to parse objects and types, arg[2] would be a 6. If you only want to parse types, arg[2] would be 2.

If successful, the function will return the numeric handle for the open database.

If unsuccessful, the function will return a helpful error message.

If the database is already open, a traceback will be thrown that contains the already open database handle.

### Function: `sqlite_close`

sqlite_close -- This function will close an open database.

int `sqlite_close`(INT database handle)

If successful, return 1;

If unsuccessful, returns E_INVARG.

### Function: `sqlite_execute`

sqlite_execute -- This function will attempt to create and execute the prepared statement query given in query on the database referred to by handle with the values values.

list | str `sqlite_execute`(INT database handle, STR SQL prepared statement query, LIST values)

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

`sqlite_execute` uses prepared statements, so it's the preferred function to use for security and performance reasons.

Example:

```
sqlite_execute(0, "INSERT INTO users VALUES (?, ?, ?);", {#7, "lisdude", "Albori Sninvel"})
```

ToastStunt supports the REGEXP pattern matching operator:

```
sqlite_execute(4, "SELECT rowid FROM notes WHERE body REGEXP ?;", {"albori (sninvel)?"})
```

> Note: This is a threaded function.

### Function: `sqlite_query`

sqlite_query -- This function will attempt to execute the query given in query on the database referred to by handle.

list | str `sqlite_query`(INT database handle, STR database query[, INT show columns])

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

If show columns is true, the return list will include the name of the column before its results.

> Warning: sqlite_query does NOT use prepared statements and should NOT be used on queries that contain user input.

> Note: This is a threaded function.

### Function: `sqlite_limit`

sqlite_limit -- This function allows you to specify various construct limitations on a per-database basis.

int `sqlite_limit`(INT database handle, STR category INT new value)

If new value is a negative number, the limit is unchanged. Each limit category has a hardcoded upper bound. Attempts to increase a limit above its hard upper bound are silently truncated to the hard upper bound.

Regardless of whether or not the limit was changed, the sqlite_limit() function returns the prior value of the limit. Hence, to find the current value of a limit without changing it, simply invoke this interface with the third parameter set to -1.

As of this writing, the following limits exist:

| Limit                     | Description                                                                                                                                                                                                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| LIMIT_LENGTH              | The maximum size of any string or BLOB or table row, in bytes.                                                                                                                                                                                                           |
| LIMIT_SQL_LENGTH          | The maximum length of an SQL statement, in bytes.                                                                                                                                                                                                                        |
| LIMIT_COLUMN              | The maximum number of columns in a table definition or in the result set of a SELECT or the maximum number of columns in an index or in an ORDER BY or GROUP BY clause.                                                                                                  |
| LIMIT_EXPR_DEPTH          | The maximum depth of the parse tree on any expression.                                                                                                                                                                                                                   |
| LIMIT_COMPOUND_SELECT     | The maximum number of terms in a compound SELECT statement.                                                                                                                                                                                                              |
| LIMIT_VDBE_OP             | The maximum number of instructions in a virtual machine program used to implement an SQL statement. If sqlite3_prepare_v2() or the equivalent tries to allocate space for more than this many opcodes in a single prepared statement, an SQLITE_NOMEM error is returned. |
| LIMIT_FUNCTION_ARG        | The maximum number of arguments on a function.                                                                                                                                                                                                                           |
| LIMIT_ATTACHED            | The maximum number of attached databases.                                                                                                                                                                                                                                |
| LIMIT_LIKE_PATTERN_LENGTH | The maximum length of the pattern argument to the LIKE or GLOB operators.                                                                                                                                                                                                |
| LIMIT_VARIABLE_NUMBER     | The maximum index number of any parameter in an SQL statement.                                                                                                                                                                                                           |
| LIMIT_TRIGGER_DEPTH       | The maximum depth of recursion for triggers.                                                                                                                                                                                                                             |
| LIMIT_WORKER_THREADS      | The maximum number of auxiliary worker threads that a single prepared statement may start.                                                                                                                                                                               |

For an up-to-date list of limits, see the [SQLite documentation](https://www.sqlite.org/c3ref/c_limit_attached.html).

### Function: `sqlite_last_insert_row_id`

sqlite_last_insert_row_id -- This function identifies the row ID of the last insert command executed on the database.

int `sqlite_last_insert_row_id`(INT database handle)

### Function: `sqlite_interrupt`

sqlite_interrupt -- This function causes any pending database operation to abort at its earliest opportunity.

none `sqlite_interrupt`(INT database handle)

If the operation is nearly finished when sqlite_interrupt is called, it might not have an opportunity to be interrupted and could continue to completion.

This can be useful when you execute a long-running query and want to abort it.

> NOTE: As of this writing (server version 2.7.0) the @kill command WILL NOT abort operations taking place in a helper thread. If you want to interrupt an SQLite query, you must use sqlite_interrupt and NOT the @kill command.

### Function: `sqlite_info`

sqlite_info -- This function returns a map of information about the database at handle

map `sqlite_info`(INT database handle)

The information returned is:

- Database Path
- Type parsing enabled?
- Object parsing enabled?
- String sanitation enabled?

### Function: `sqlite_handles`

sqlite_handles -- Returns a list of open SQLite database handles.

list `sqlite_handles()`

## Operations on The Server Environment

### Function: `exec`

exec -- Asynchronously executes the specified external executable, optionally sending input.

list `exec` (list command[, str input])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

When the process exits, it returns a list of the form:

`{code, output, error}`

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                   ⇒   {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                  ⇒   {0, "foo", ""}
exec({"echo", "one", "two"})                          ⇒   {0, "one two~0A", ""}
```

You are able to set environmental variables with `exec`, imagine you had a `vars.sh` (in your executables directory):

```
#!/bin/bash
echo "pizza = ${pizza}"
```

And then you did:

```
exec({"vars.sh"}, "", {"pizza=tasty"}) => {0, "pizza = tasty~0A", ""}
exec({"vars.sh"}) => {0, "pizza = ~0A", ""}
```

The second time pizza doesn't exist. The darkest timeline.

### Function: `getenv`

getenv -- Returns the value of the named environment variable.

str `getenv` (str name)

If no such environment variable exists, 0 is returned. If the programmer is not a wizard, then E_PERM is raised.

```
getenv("HOME")                                          ⇒   "/home/foobar"
getenv("XYZZY")
```

## Operations on Network Connections

### Function: `connected_players`

connected_players -- returns a list of the object numbers of those player objects with currently-active connections

list `connected_players` ([include-all])

If include-all is provided and true, then the list includes the object numbers associated with _all_ current connections, including ones that are outbound and/or not yet logged-in.

### Function: `connected_seconds`

connected_seconds -- return the number of seconds that the currently-active connection to player has existed

int `connected_seconds` (obj player) ##### Function: `idle_seconds`

idle_seconds -- return the number of seconds that the currently-active connection to player has been idle

int `idle_seconds` (obj player)

If player is not the object number of a player object with a currently-active connection, then `E_INVARG` is raised.

### Function: `notify`

notify -- enqueues string for output (on a line by itself) on the connection conn

none `notify` (obj conn, str string [, INT no-flush [, INT suppress-newline])

If the programmer is not conn or a wizard, then `E_PERM` is raised. If conn is not a currently-active connection, then this function does nothing. Output is normally written to connections only between tasks, not during execution.

The server will not queue an arbitrary amount of output for a connection; the `MAX_QUEUED_OUTPUT` compilation option (in `options.h`) controls the limit (`MAX_QUEUED_OUTPUT` can be overridden in-database by adding the property `$server_options.max_queued_output` and calling `load_server_options()`). When an attempt is made to enqueue output that would take the server over its limit, it first tries to write as much output as possible to the connection without having to wait for the other end. If that doesn't result in the new output being able to fit in the queue, the server starts throwing away the oldest lines in the queue until the new output will fit. The server remembers how many lines of output it has 'flushed' in this way and, when next it can succeed in writing anything to the connection, it first writes a line like `>> Network buffer overflow: X lines of output to you have been lost <<` where X is the number of flushed lines.

If no-flush is provided and true, then `notify()` never flushes any output from the queue; instead it immediately returns false. `Notify()` otherwise always returns true.

If suppress-newline is provided and true, then `notify()` does not add a newline add the end of the string.

### Function: `buffered_output_length`

buffered_output_length -- returns the number of bytes currently buffered for output to the connection conn

int `buffered_output_length` ([obj conn])

If conn is not provided, returns the maximum number of bytes that will be buffered up for output on any connection.

### Function: `read`

read -- reads and returns a line of input from the connection conn (or, if not provided, from the player that typed the command that initiated the current task)

str `read` ([obj conn [, non-blocking]])

If non-blocking is false or not provided, this function suspends the current task, resuming it when there is input available to be read. If non-blocking is provided and true, this function never suspends the calling task; if there is no input currently available for input, `read()` simply returns 0 immediately.

If player is provided, then the programmer must either be a wizard or the owner of `player`; if `player` is not provided, then `read()` may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, `E_PERM` is raised.

If the given `player` is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then `read()` raises `E_INVARG`.

The restriction on the use of `read()` without any arguments preserves the following simple invariant: if input is being read from a player, it is for the task started by the last command that player typed. This invariant adds responsibility to the programmer, however. If your program calls another verb before doing a `read()`, then either that verb must not suspend or else you must arrange that no commands will be read from the connection in the meantime. The most straightforward way to do this is to call

```
set_connection_option(player, "hold-input", 1)
```

before any task suspension could happen, then make all of your calls to `read()` and other code that might suspend, and finally call

```
set_connection_option(player, "hold-input", 0)
```

to allow commands once again to be read and interpreted normally.

### Function: `force_input`

force_input -- inserts the string line as an input task in the queue for the connection conn, just as if it had arrived as input over the network

none `force_input` (obj conn, str line [, at-front])

If at_front is provided and true, then the new line of input is put at the front of conn's queue, so that it will be the very next line of input processed even if there is already some other input in that queue. Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

### Function: `flush_input`

flush_input -- performs the same actions as if the connection conn's defined flush command had been received on that connection

none `flush_input` (obj conn [show-messages])

I.E., removes all pending lines of input from conn's queue and, if show-messages is provided and true, prints a message to conn listing the flushed lines, if any. See the chapter on server assumptions about the database for more information about a connection's defined flush command.

### Function: `output_delimiters`

output_delimiters -- returns a list of two strings, the current _output prefix_ and _output suffix_ for player.

list `output_delimiters` (obj player)

If player does not have an active network connection, then `E_INVARG` is raised. If either string is currently undefined, the value `""` is used instead. See the discussion of the `PREFIX` and `SUFFIX` commands in the next chapter for more information about the output prefix and suffix.

### Function: `boot_player`

boot_player -- marks for disconnection any currently-active connection to the given player

none `boot_player` (obj player)

The connection will not actually be closed until the currently-running task returns or suspends, but all MOO functions (such as `notify()`, `connected_players()`, and the like) immediately behave as if the connection no longer exists. If the programmer is not either a wizard or the same as player, then `E_PERM` is raised. If there is no currently-active connection to player, then this function does nothing.

If there was a currently-active connection, then the following verb call is made when the connection is actually closed:

```
$user_disconnected(player)
```

It is not an error if this verb does not exist; the call is simply skipped.

### Function: `connection_info`

connection_info -- Returns a MAP of network connection information for `connection`. At the time of writing, the following information is returned:

list `connection_info` (OBJ `connection`)

| Key                 | Value                                                                                                                                                                                          |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| destination_address | The hostname of the connection. For incoming connections, this is the hostname of the connected user. For outbound connections, this is the hostname of the outbound connection's destination. |
| destination_ip      | The unresolved numeric IP address of the connection.                                                                                                                                           |
| destination_port    | For incoming connections, this is the local port used to make the connection. For outbound connections, this is the port the connection was made to.                                           |
| source_address      | This is the hostname of the interface an incoming connection was made on. For outbound connections, this value is meaningless.                                                                 |
| source_ip           | The unresolved numeric IP address of the interface a connection was made on. For outbound connections, this value is meaningless.                                                              |
| source_port         | The local port a connection connected to. For outbound connections, this value is meaningless.                                                                                                 |
| protocol            | Describes the protocol used to make the connection. At the time of writing, this could be IPv4 or IPv6.                                                                                        |
| outbound            | Indicates whether a connection is outbound or not                                                                                                                                              |

### Function: `connection_name`

connection_name -- returns a network-specific string identifying the connection being used by the given player

str `connection_name` (obj player, [INT method])

When provided just a player object this function only returns obj's hostname (e.g. `1-2-3-6.someplace.com`). An optional argument allows you to specify 1 if you want a numeric IP address, or 2 if you want to return the legacy connection_name string.

> Warning: If you are using a LambdaMOO core, this is a semi-breaking change. You'll want to update any code on your server that runs `connection_name` to pass in the argument for returning the legacy connection_name string if you want things to work exactly the same.

If the programmer is not a wizard and not player, then `E_PERM` is raised. If player is not currently connected, then `E_INVARG` is raised.

Legacy Connection String Information:

For the TCP/IP networking configurations, for in-bound connections, the string has the form:

```
"port lport from host, port port"
```

where lport is the decimal TCP listening port on which the connection arrived, host is either the name or decimal TCP address of the host from which the player is connected, and port is the decimal TCP port of the connection on that host.

For outbound TCP/IP connections, the string has the form

```
"port lport to host, port port"
```

where lport is the decimal local TCP port number from which the connection originated, host is either the name or decimal TCP address of the host to which the connection was opened, and port is the decimal TCP port of the connection on that host.

For the System V 'local' networking configuration, the string is the UNIX login name of the connecting user or, if no such name can be found, something of the form:

```
"User #number"
```

where number is a UNIX numeric user ID.

For the other networking configurations, the string is the same for all connections and, thus, useless.

### Function: `connection_name_lookup`

connection_name_lookup - This function performs a DNS name lookup on connection's IP address.

str `connection_name_lookup` (OBJ connection [, INT record_result])

If a hostname can't be resolved, the function simply returns the numeric IP address. Otherwise, it will return the resolved hostname.

If record_result is true, the resolved hostname will be saved with the connection and will overwrite it's existing 'connection_name()'. This means that you can call 'connection_name_lookup()' a single time when a connection is created and then continue to use 'connection_name()' as you always have in the past.

This function is primarily intended for use when the 'NO_NAME_LOOKUP' server option is set. Barring temporarily failures in your nameserver, very little will be gained by calling this when the server is performing DNS name lookups for you.

> Note: This function runs in a separate thread. While this is good for performance (long lookups won't lock your MOO like traditional pre-2.6.0 name lookups), it also means it will require slightly more work to create an entirely in-database DNS lookup solution. Because it explicitly suspends, you won't be able to use it in 'do_login_command()' without also using the 'switch_player()' function. For an example of how this can work, see '#0:do_login_command()' in ToastCore.

### Function: `switch_player`

switch_player -- Silently switches the player associated with this connection from object1 to object2.

`switch_player`(OBJ object1, OBJ object2 [, INT silent])

object1 must be connected and object2 must be a player. This can be used in do_login_command() verbs that read or suspend (which prevents the normal player selection mechanism from working.

If silent is true, no connection messages will be printed.

> Note: This calls the listening object's user_disconnected and user_connected verbs when appropriate.

### Function: `set_connection_option`

set_connection_option -- controls a number of optional behaviors associated the connection conn

none `set_connection_option` (obj conn, str option, value)

Raises E_INVARG if conn does not specify a current connection and E_PERM if the programmer is neither conn nor a wizard. Unless otherwise specified below, options can only be set (value is true) or unset (otherwise). The following values for option are currently supported:

The following values for option are currently supported:

`"binary"`
When set, the connection is in binary mode, in which case both input from and output to conn can contain arbitrary bytes. Input from a connection in binary mode is not broken into lines at all; it is delivered to either the read() function or normal command parsing as binary strings, in whatever size chunks come back from the operating system. (See fine point on binary strings, for a description of the binary string representation.) For output to a connection in binary mode, the second argument to `notify()` must be a binary string; if it is malformed, E_INVARG is raised.

> Fine point: If the connection mode is changed at any time when there is pending input on the connection, said input will be delivered as per the previous mode (i.e., when switching out of binary mode, there may be pending “lines” containing tilde-escapes for embedded linebreaks, tabs, tildes and other characters; when switching into binary mode, there may be pending lines containing raw tabs and from which nonprintable characters have been silently dropped as per normal mode. Only during the initial invocation of $do_login_command() on an incoming connection or immediately after the call to open_network_connection() that creates an outgoing connection is there guaranteed not to be pending input. At other times you will probably want to flush any pending input immediately after changing the connection mode.

`"hold-input"`

When set, no input received on conn will be treated as a command; instead, all input remains in the queue until retrieved by calls to read() or until this connection option is unset, at which point command processing resumes. Processing of out-of-band input lines is unaffected by this option.

`"disable-oob"`

When set, disables all out of band processing (see section Out-of-Band Processing). All subsequent input lines until the next command that unsets this option will be made available for reading tasks or normal command parsing exactly as if the out-of-band prefix and the out-of-band quoting prefix had not been defined for this server.

`"client-echo"`
The setting of this option is of no significance to the server. However calling set_connection_option() for this option sends the Telnet Protocol `WONT ECHO` or `WILL ECHO` according as value is true or false, respectively. For clients that support the Telnet Protocol, this should toggle whether or not the client echoes locally the characters typed by the user. Note that the server itself never echoes input characters under any circumstances. (This option is only available under the TCP/IP networking configurations.)

`"flush-command"`
This option is string-valued. If the string is non-empty, then it is the flush command for this connection, by which the player can flush all queued input that has not yet been processed by the server. If the string is empty, then conn has no flush command at all. set_connection_option also allows specifying a non-string value which is equivalent to specifying the empty string. The default value of this option can be set via the property `$server_options.default_flush_command`; see Flushing Unprocessed Input for details.

`"intrinsic-commands"`

This option value is a list of strings, each being the name of one of the available server intrinsic commands (see section Command Lines That Receive Special Treatment). Commands not on the list are disabled, i.e., treated as normal MOO commands to be handled by $do_command and/or the built-in command parser

set_connection_option also allows specifying an integer value which, if zero, is equivalent to specifying the empty list, and otherwise is taken to be the list of all available intrinsic commands (the default setting).

Thus, one way to make the verbname `PREFIX` available as an ordinary command is as follows:

```
set_connection_option(
  player, "intrinsic-commands",
  setremove(connection_options(player, "intrinsic-commands"),
            "PREFIX"));
```

Note that connection_options() with no second argument will return a list while passing in the second argument will return the value of the key requested.

```
save = connection_options(player,"intrinsic-commands");
set_connection_options(player, "intrinsic-commands, 1);
full_list = connection_options(player,"intrinsic-commands");
set_connection_options(player,"intrinsic-commands", save);
return full_list;
```

is a way of getting the full list of intrinsic commands available in the server while leaving the current connection unaffected.

### Function: `connection_options`

connection_options -- returns a list of `{name, value}` pairs describing the current settings of all of the allowed options for the connection conn or the value if `name` is provided

ANY `connection_options` (obj conn [, STR name])

Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

Calling connection options without a name will return a LIST. Passing in name will return only the value for the option `name` requested.

### Function: `open_network_connection`

open_network_connection -- establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there

obj `open_network_connection` (STR host, INT port [, MAP options])

Establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there. The new connection, as usual, will not be logged in initially and will have a negative object number associated with it for use with `read()',`notify()', and `boot_player()'. This object number is the value returned by this function.

If the programmer is not a wizard or if the `OUTBOUND_NETWORK' compilation option was not used in building the server, then`E_PERM' is raised.

`host` refers to a string naming a host (possibly a numeric IP address) and `port` is an integer referring to a TCP port number. If a connection cannot be made because the host does not exist, the port does not exist, the host is not reachable or refused the connection, `E_INVARG' is raised.  If the connection cannot be made for other reasons, including resource limitations, then`E_QUOTA' is raised.

Optionally, you can specify a map with any or all of the following options:

listener: An object whose listening verbs will be called at appropriate points. (See HELP LISTEN() for more details.)

tls: If true, establish a secure TLS connection.

ipv6: If true, utilize the IPv6 protocol rather than the IPv4 protocol.

The outbound connection process involves certain steps that can take quite a long time, during which the server is not doing anything else, including responding to user commands and executing MOO tasks. See the chapter on server assumptions about the database for details about how the server limits the amount of time it will wait for these steps to successfully complete.

It is worth mentioning one tricky point concerning the use of this function. Since the server treats the new connection pretty much like any normal player connection, it will naturally try to parse any input from that connection as commands in the usual way. To prevent this treatment, you should use `set_connection_option()' to set the`hold-input' option true on the connection.

Example:

```
open_network_connection("2607:5300:60:4be0::", 1234, ["ipv6" -> 1, "listener" -> #6, "tls" -> 1])
```

Open a new connection to the IPv6 address 2607:5300:60:4be0:: on port 1234 using TLS. Relevant verbs will be called on #6.

### Function: `curl`

str `curl`(STR url [, INT include_headers, [ INT timeout])

The curl builtin will download a webpage and return it as a string. If include_headers is true, the HTTP headers will be included in the return string.

It's worth noting that the data you get back will be binary encoded. In particular, you will find that line breaks appear as ~0A. You can easily convert a page into a list by passing the return string into the decode_binary() function.

CURL_TIMEOUT is defined in options.h to specify the maximum amount of time a CURL request can take before failing. For special circumstances, you can specify a longer or shorter timeout using the third argument of curl().

### Function: `read_http`

map `read_http` (request-or-response [, OBJ conn])

Reads lines from the connection conn (or, if not provided, from the player that typed the command that initiated the current task) and attempts to parse the lines as if they are an HTTP request or response. request-or-response must be either the string "request" or "response". It dictates the type of parsing that will be done.

Just like read(), if conn is provided, then the programmer must either be a wizard or the owner of conn; if conn is not provided, then read_http() may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, E_PERM is raised. Likewise, if conn is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then read_http() raises E_INVARG.

If parsing fails because the request or response is syntactically incorrect, read_http() will return a map with the single key "error" and a list of values describing the reason for the error. If parsing succeeds, read_http() will return a map with an appropriate subset of the following keys, with values parsed from the HTTP request or response: "method", "uri", "headers", "body", "status" and "upgrade".

> Fine point: read_http() assumes the input strings are binary strings. When called interactively, as in the example below, the programmer must insert the literal line terminators or parsing will fail.

The following example interactively reads an HTTP request from the players connection.

```
read_http("request", player)
GET /path HTTP/1.1~0D~0A
Host: example.com~0D~0A
~0D~0A
```

In this example, the string ~0D~0A ends the request. The call returns the following (the request has no body):

```
["headers" -> ["Host" -> "example.com"], "method" -> "GET", "uri" -> "/path"]
```

The following example interactively reads an HTTP response from the players connection.

```
read_http("response", player)
HTTP/1.1 200 Ok~0D~0A
Content-Length: 10~0D~0A
~0D~0A
1234567890
```

The call returns the following:

```
["body" -> "1234567890", "headers" -> ["Content-Length" -> "10"], "status" -> 200]
```

### Function: `listen`

listen -- create a new point at which the server will listen for network connections, just as it does normally

value `listen` (obj object, port [, MAP options])

Create a new point at which the server will listen for network connections, just as it does normally. `Object` is the object whose verbs `do_login_command',`do_command', `do_out_of_band_command',`user_connected', `user_created',`user_reconnected', `user_disconnected', and`user_client_disconnected' will be called at appropriate points as these verbs are called on #0 for normal connections. (See the chapter in the LambdaMOO Programmer's Manual on server assumptions about the database for the complete story on when these functions are called.) `Port` is a TCP port number on which to listen. The listen() function will return `port` unless `port` is zero, in which case the return value is a port number assigned by the operating system.

An optional third argument allows you to set various miscellaneous options for the listening point. These are:

print-messages: If true, the various database-configurable messages (also detailed in the chapter on server assumptions) will be printed on connections received at the new listening port.

ipv6: Use the IPv6 protocol rather than IPv4.

tls: Only accept valid secure TLS connections.

certificate: The full path to a TLS certificate. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the certificate differs from the one specified in options.h.

key: The full path to a TLS private key. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the key differs from the one specified in options.h.

listen() raises E_PERM if the programmer is not a wizard, E_INVARG if `object` is invalid or there is already a listening point described by `point`, and E_QUOTA if some network-configuration-specific error occurred.

Example:

```
listen(#0, 1234, ["ipv6" -> 1, "tls" -> 1, "certificate" -> "/etc/certs/something.pem", "key" -> "/etc/certs/privkey.pem", "print-messages" -> 1]
```

Listen for IPv6 connections on port 1234 and print messages as appropriate. These connections must be TLS and will use the private key and certificate found in /etc/certs.

### Function: `unlisten`

unlisten -- stop listening for connections on the point described by canon, which should be the second element of some element of the list returned by `listeners()`

none `unlisten` (canon)

Raises `E_PERM` if the programmer is not a wizard and `E_INVARG` if there does not exist a listener with that description.

### Function: `listeners`

listeners -- returns a list describing all existing listening points, including the default one set up automatically by the server when it was started (unless that one has since been destroyed by a call to `unlisten()`)

list `listeners` ()

Each element of the list has the following form:

```
{object, canon, print-messages}
```

where object is the first argument given in the call to `listen()` to create this listening point, print-messages is true if the third argument in that call was provided and true, and canon was the value returned by that call. (For the initial listening point, object is `#0`, canon is determined by the command-line arguments or a network-configuration-specific default, and print-messages is true.)

Please note that there is nothing special about the initial listening point created by the server when it starts; you can use `unlisten()` on it just as if it had been created by `listen()`. This can be useful; for example, under one of the TCP/IP configurations, you might start up your server on some obscure port, say 12345, connect to it by yourself for a while, and then open it up to normal users by evaluating the statements:

```
unlisten(12345); listen(#0, 7777, 1)
```

## Operations Involving Times and Dates

### Function: `time`

time -- returns the current time, represented as the number of seconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time

int `time` ()

### Function: `ftime`

ftime -- Returns the current time represented as the number of seconds and nanoseconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time.

float `ftime` ([INT monotonic])

If the `monotonic` argument is supplied and set to 1, the time returned will be monotonic. This means that will you will always get how much time has elapsed from an arbitrary, fixed point in the past that is unaffected by clock skew or other changes in the wall-clock. This is useful for benchmarking how long an operation takes, as it's unaffected by the actual system time.

The general rule of thumb is that you should use ftime() with no arguments for telling time and ftime() with the monotonic clock argument for measuring the passage of time.

### Function: `ctime`

ctime -- interprets time as a time, using the same representation as given in the description of `time()`, above, and converts it into a 28-character, human-readable string

str `ctime` ([int time])

The string will be in the following format:

```
Mon Aug 13 19:13:20 1990 PDT
```

If the current day of the month is less than 10, then an extra blank appears between the month and the day:

```
Mon Apr  1 14:10:43 1991 PST
```

If time is not provided, then the current time is used.

Note that `ctime()` interprets time for the local time zone of the computer on which the MOO server is running.

## MOO-Code Evaluation and Task Manipulation

### Function: `raise`

raise -- raises code as an error in the same way as other MOO expressions, statements, and functions do

none `raise` (code [, str message [, value]])

Message, which defaults to the value of `tostr(code)`, and value, which defaults to zero, are made available to any `try`-`except` statements that catch the error. If the error is not caught, then message will appear on the first line of the traceback printed to the user.

### Function: `call_function`

call_function -- calls the built-in function named func-name, passing the given arguments, and returns whatever that function returns

value `call_function` (str func-name, arg, ...)

Raises `E_INVARG` if func-name is not recognized as the name of a known built-in function. This allows you to compute the name of the function to call and, in particular, allows you to write a call to a built-in function that may or may not exist in the particular version of the server you're using.

### Function: `function_info`

function_info -- returns descriptions of the built-in functions available on the server

list `function_info` ([str name])

If name is provided, only the description of the function with that name is returned. If name is omitted, a list of descriptions is returned, one for each function available on the server. Raised `E_INVARG` if name is provided but no function with that name is available on the server.

Each function description is a list of the following form:

```
{name, min-args, max-args, types
```

where name is the name of the built-in function, min-args is the minimum number of arguments that must be provided to the function, max-args is the maximum number of arguments that can be provided to the function or `-1` if there is no maximum, and types is a list of max-args integers (or min-args if max-args is `-1`), each of which represents the type of argument required in the corresponding position. Each type number is as would be returned from the `typeof()` built-in function except that `-1` indicates that any type of value is acceptable and `-2` indicates that either integers or floating-point numbers may be given. For example, here are several entries from the list:

```
{"listdelete", 2, 2, {4, 0}}
{"suspend", 0, 1, {0}}
{"server_log", 1, 2, {2, -1}}
{"max", 1, -1, {-2}}
{"tostr", 0, -1, {}}
```

`listdelete()` takes exactly 2 arguments, of which the first must be a list (`LIST == 4`) and the second must be an integer (`INT == 0`). `suspend()` has one optional argument that, if provided, must be a number (integer or float). `server_log()` has one required argument that must be a string (`STR == 2`) and one optional argument that, if provided, may be of any type. `max()` requires at least one argument but can take any number above that, and the first argument must be either an integer or a floating-point number; the type(s) required for any other arguments can't be determined from this description. Finally, `tostr()` takes any number of arguments at all, but it can't be determined from this description which argument types would be acceptable in which positions.

### Function: `eval`

eval -- the MOO-code compiler processes string as if it were to be the program associated with some verb and, if no errors are found, that fictional verb is invoked

list `eval` (str string)

If the programmer is not, in fact, a programmer, then `E_PERM` is raised. The normal result of calling `eval()` is a two element list. The first element is true if there were no compilation errors and false otherwise. The second element is either the result returned from the fictional verb (if there were no compilation errors) or a list of the compiler's error messages (otherwise).

When the fictional verb is invoked, the various built-in variables have values as shown below:

player the same as in the calling verb
this #-1
caller the same as the initial value of this in the calling verb

args {}
argstr ""

verb ""
dobjstr ""
dobj #-1
prepstr ""
iobjstr ""
iobj #-1

The fictional verb runs with the permissions of the programmer and as if its `d` permissions bit were on.

```
eval("return 3 + 4;")   =>   {1, 7}
```

### Function: `set_task_perms`

set_task_perms -- changes the permissions with which the currently-executing verb is running to be those of who

one `set_task_perms` (obj who)

If the programmer is neither who nor a wizard, then `E_PERM` is raised.

> Note: This does not change the owner of the currently-running verb, only the permissions of this particular invocation. It is used in verbs owned by wizards to make themselves run with lesser (usually non-wizard) permissions.

### Function: `caller_perms`

caller_perms -- returns the permissions in use by the verb that called the currently-executing verb

obj `caller_perms` ()

If the currently-executing verb was not called by another verb (i.e., it is the first verb called in a command or server task), then `caller_perms()` returns `#-1`.

### Function: `set_task_local`

set_task_local -- Sets a value that gets associated with the current running task.

void set_task_local(ANY value)

This value persists across verb calls and gets reset when the task is killed, making it suitable for securely passing sensitive intermediate data between verbs. The value can then later be retrieved using the `task_local` function.

```
set_task_local("arbitrary data")
set_task_local({"list", "of", "arbitrary", "data"})
```

### Function: `task_local`

task_local -- Returns the value associated with the current task. The value is set with the `set_task_local` function.

mixed `task_local` ()

### Function: `threads`

threads -- When one or more MOO processes are suspended and working in a separate thread, this function will return a LIST of handlers to those threads. These handlers can then be passed to `thread_info' for more information.

list `threads`()

### Function: `set_thread_mode`

int `set_thread_mode`([INT mode])

With no arguments specified, set_thread_mode will return the current thread mode for the verb. A value of 1 indicates that threading is enabled for functions that support it. A value of 0 indicates that threading is disabled and all functions will execute in the main MOO thread, as functions have done in default LambdaMOO since version 1.

If you specify an argument, you can control the thread mode of the current verb. A mode of 1 will enable threading and a mode of 0 will disable it. You can invoke this function multiple times if you want to disable threading for a single function call and enable it for the rest.

When should you disable threading? In general, threading should be disabled in verbs where it would be undesirable to suspend(). Each threaded function will immediately suspend the verb while the thread carries out its work. This can have a negative effect when you want to use these functions in verbs that cannot or should not suspend, like $sysobj:do_command or $sysobj:do_login_command.

Note that the threading mode affects the current verb only and does NOT affect verbs called from within that verb.

### Function: `thread_info`

thread_info -- If a MOO task is running in another thread, its thread handler will give you information about that thread.

list `thread_info`(INT thread handler)

The information returned in a LIST will be:

English Name: This is the name the programmer of the builtin function has given to the task being executed.

Active: 1 or 0 depending upon whether or not the MOO task has been killed. Not all threads cleanup immediately after the MOO task dies.

### Function: `thread_pool`

void `thread_pool`(STR function, STR pool [, INT value])

This function allows you to control any thread pools that the server created at startup. It should be used with care, as it has the potential to create disasterous consequences if used incorrectly.

The function parameter is the function you wish to perform on the thread pool. The functions available are:

INIT: Control initialization of a thread pool.

The pool parameter controls which thread pool you wish to apply the designated function to. At the time of writing, the server creates the following thread pool:

MAIN: The main thread pool where threaded built-in function work takes place.

Finally, value is the value you want to pass to the function of pool. The following functions accept the following values:

INIT: The number of threads to spawn. NOTE: When executing this function, the existing pool will be destroyed and a new one created in its place.

Examples:

```
thread_pool("INIT", "MAIN", 1)     => Replace the existing main thread pool with a new pool consisting of a single thread.
```

### Function: `ticks_left`

ticks_left -- return the number of ticks left to the current task before it will be forcibly terminated

int `ticks_left` () ##### Function: `seconds_left`

seconds_left -- return the number of seconds left to the current task before it will be forcibly terminated

int `seconds_left` ()

These are useful, for example, in deciding when to call `suspend()` to continue a long-lived computation.

### Function: `task_id`

task_id -- returns the non-zero, non-negative integer identifier for the currently-executing task

int `task_id` ()

Such integers are randomly selected for each task and can therefore safely be used in circumstances where unpredictability is required.

### Function: `suspend`

suspend -- suspends the current task, and resumes it after at least seconds seconds

value `suspend` ([int|float seconds])

Sub-second suspend (IE: 0.1) is possible. If seconds is not provided, the task is suspended indefinitely; such a task can only be resumed by use of the `resume()` function.

When the task is resumed, it will have a full quota of ticks and seconds. This function is useful for programs that run for a long time or require a lot of ticks. If seconds is negative, then `E_INVARG` is raised. `Suspend()` returns zero unless it was resumed via `resume()`, in which case it returns the second argument given to that function.

In some sense, this function forks the 'rest' of the executing task. However, there is a major difference between the use of `suspend(seconds)` and the use of the `fork (seconds)`. The `fork` statement creates a new task (a _forked task_) while the currently-running task still goes on to completion, but a `suspend()` suspends the currently-running task (thus making it into a _suspended task_). This difference may be best explained by the following examples, in which one verb calls another:

```
.program   #0:caller_A
#0.prop = 1;
#0:callee_A();
#0.prop = 2;
.

.program   #0:callee_A
fork(5)
  #0.prop = 3;
endfork
.

.program   #0:caller_B
#0.prop = 1;
#0:callee_B();
#0.prop = 2;
.

.program   #0:callee_B
suspend(5);
#0.prop = 3;
.
```

Consider `#0:caller_A`, which calls `#0:callee_A`. Such a task would assign 1 to `#0.prop`, call `#0:callee_A`, fork a new task, return to `#0:caller_A`, and assign 2 to `#0.prop`, ending this task. Five seconds later, if the forked task had not been killed, then it would begin to run; it would assign 3 to `#0.prop` and then stop. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 3.

Now consider `#0:caller_B`, which calls `#0:callee_B` instead of `#0:callee_A`. This task would assign 1 to `#0.prop`, call `#0:callee_B`, and suspend. Five seconds later, if the suspended task had not been killed, then it would resume; it would assign 3 to `#0.prop`, return to `#0:caller_B`, and assign 2 to `#0.prop`, ending the task. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 2.

A suspended task, like a forked task, can be described by the `queued_tasks()` function and killed by the `kill_task()` function. Suspending a task does not change its task id. A task can be suspended again and again by successive calls to `suspend()`.

By default, there is no limit to the number of tasks any player may suspend, but such a limit can be imposed from within the database. See the chapter on server assumptions about the database for details.

### Function: `resume`

resume -- immediately ends the suspension of the suspended task with the given task-id; that task's call to `suspend()` will return value, which defaults to zero

none `resume` (int task-id [, value])

If value is of type `ERR`, it will be raised, rather than returned, in the suspended task. `Resume()` raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

### Function: `yin`

yin -- Suspend the current task if it's running out of ticks or seconds.

int `yin`([INT time, INT minimum ticks, INT minimum seconds] )

`yin` stands for yield if needed.

This is meant to provide similar functionality to the LambdaCore-based suspend_if_needed verb or manually specifying something like: ticks_left() < 2000 && suspend(0)

Time: How long to suspend the task. Default: 0

Minimum ticks: The minimum number of ticks the task has left before suspending.

Minimum seconds: The minimum number of seconds the task has left before suspending.

### Function: `queue_info`

queue_info -- if player is omitted, returns a list of object numbers naming all players that currently have active task queues inside the server

list `queue_info` ([obj player])
map `queue_info` ([obj player])

If player is provided, returns the number of background tasks currently queued for that user. It is guaranteed that `queue_info(X)` will return zero for any X not in the result of `queue_info()`.

If the caller is a wizard a map of debug information about task queues will be returned.

### Function: `queued_tasks`

queued_tasks -- returns information on each of the background tasks (i.e., forked, suspended or reading) owned by the programmer (or, if the programmer is a wizard, all queued tasks)

list `queued_tasks` ([INT show-runtime [, INT count-only])

The returned value is a list of lists, each of which encodes certain information about a particular queued task in the following format:

```
{task-id, start-time, x, y, programmer, verb-loc, verb-name, line, this, task-size}
```

where task-id is an integer identifier for this queued task, start-time is the time after which this task will begin execution (in time() format), x and y are obsolete values that are no longer interesting, programmer is the permissions with which this task will begin execution (and also the player who owns this task), verb-loc is the object on which the verb that forked this task was defined at the time, verb-name is that name of that verb, line is the number of the first line of the code in that verb that this task will execute, this is the value of the variable `this` in that verb, and task-size is the size of the task in bytes. For reading tasks, start-time is -1.

The x and y fields are now obsolete and are retained only for backward-compatibility reasons. They may be reused for new purposes in some future version of the server.

If `show-runtime` is true, all variables present in the task are presented in a map with the variable name as the key and its value as the value.

If `count-only` is true, then only the number of tasks is returned. This is significantly more performant than length(queued_tasks()).

> Warning: If you are upgrading to ToastStunt from a version of LambdaMOO prior to 1.8.1 you will need to dump your database, reboot into LambdaMOO emergency mode, and kill all your queued_tasks() before dumping the DB again. Otherwise, your DB will not boot into ToastStunt.

### Function: `kill_task`

kill_task -- removes the task with the given task-id from the queue of waiting tasks

none `kill_task` (int task-id)

If the programmer is not the owner of that task and not a wizard, then `E_PERM` is raised. If there is no task on the queue with the given task-id, then `E_INVARG` is raised.

### Function: `finished_tasks()`

finished_tasks -- returns a list of the last X tasks to finish executing, including their total execution time

list `finished_tasks`()

When enabled (via SAVE_FINISHED_TASKS in options.h), the server will keep track of the execution time of every task that passes through the interpreter. This data is then made available to the database in two ways.

The first is the finished_tasks() function. This function will return a list of maps of the last several finished tasks (configurable via $server_options.finished_tasks_limit) with the following information:

| Value      | Description                                                                                   |
| ---------- | --------------------------------------------------------------------------------------------- |
| foreground | 1 if the task was a foreground task, 0 if it was a background task                            |
| fullverb   | the full name of the verb, including aliases                                                  |
| object     | the object that defines the verb                                                              |
| player     | the player that initiated the task                                                            |
| programmer | the programmer who owns the verb                                                              |
| receiver   | typically the same as 'this' but could be the handler in the case of primitive values         |
| suspended  | whether the task was suspended or not                                                         |
| this       | the actual object the verb was called on                                                      |
| time       | the total time it took the verb to run), and verb (the name of the verb call or command typed |

The second is via the $handle_lagging_task verb. When the execution threshold defined in $server_options.task_lag_threshold is exceeded, the server will write an entry to the log file and call the $handle_lagging_task verb with the call stack of the task as well as the execution time.

> Note: This builtin must be enabled in options.h to be used.

### Function: `callers`

callers -- returns information on each of the verbs and built-in functions currently waiting to resume execution in the current task

list `callers` ([include-line-numbers])

When one verb or function calls another verb or function, execution of the caller is temporarily suspended, pending the called verb or function returning a value. At any given time, there could be several such pending verbs and functions: the one that called the currently executing verb, the verb or function that called that one, and so on. The result of `callers()` is a list, each element of which gives information about one pending verb or function in the following format:

```
{this, verb-name, programmer, verb-loc, player, line-number}
```

For verbs, this is the initial value of the variable `this` in that verb, verb-name is the name used to invoke that verb, programmer is the player with whose permissions that verb is running, verb-loc is the object on which that verb is defined, player is the initial value of the variable `player` in that verb, and line-number indicates which line of the verb's code is executing. The line-number element is included only if the include-line-numbers argument was provided and true.

For functions, this, programmer, and verb-loc are all `#-1`, verb-name is the name of the function, and line-number is an index used internally to determine the current state of the built-in function. The simplest correct test for a built-in function entry is

```
(VERB-LOC == #-1  &&  PROGRAMMER == #-1  &&  VERB-name != "")
```

The first element of the list returned by `callers()` gives information on the verb that called the currently-executing verb, the second element describes the verb that called that one, and so on. The last element of the list describes the first verb called in this task.

### Function: `task_stack`

task_stack -- returns information like that returned by the `callers()` function, but for the suspended task with the given task-id; the include-line-numbers argument has the same meaning as in `callers()`

list `task_stack` (int task-id [, INT include-line-numbers [, INT include-variables])

Raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

If include-line-numbers is passed and true, line numbers will be included.

If include-variables is passed and true, variables will be included with each frame of the provided task.

## Administrative Operations

### Function: `server_version`

server_version -- returns a string giving the version number of the running MOO server

str `server_version` ([int with-details])

If with-details is provided and true, returns a detailed list including version number as well as compilation options.

**Function `load_server_options`**
load_server_options -- This causes the server to consult the current values of properties on $server_options, updating the corresponding serveroption settings

none `load_server_options` ()

For more information see section Server Options Set in the Database.. If the programmer is not a wizard, then E_PERM is raised.

### Function: `server_log`

server_log -- The text in message is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages)

none server_log (str message [, int level])

If the programmer is not a wizard, then E_PERM is raised.

If level is provided and is an integer between 0 and 7 inclusive, then message is marked in the server log as one of eight predefined types, from simple log message to error message. Otherwise, if level is provided and true, then message is marked in the server log as an error.

### Function: `renumber`

renumber -- the object number of the object currently numbered object is changed to be the least nonnegative object number not currently in use and the new object number is returned

obj `renumber` (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised. If there are no unused nonnegative object numbers less than object, then object is returned and no changes take place.

The references to object in the parent/children and location/contents hierarchies are updated to use the new object number, and any verbs, properties and/or objects owned by object are also changed to be owned by the new object number. The latter operation can be quite time consuming if the database is large. No other changes to the database are performed; in particular, no object references in property values or verb code are updated.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

### Function: `reset_max_object`

reset_max_object -- the server's idea of the highest object number ever used is changed to be the highest object number of a currently-existing object, thus allowing reuse of any higher numbers that refer to now-recycled objects

none `reset_max_object` ()

If the programmer is not a wizard, then `E_PERM` is raised.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

### Function: `memory_usage`

memory_usage -- Return statistics concerning the server's consumption of system memory.

list `memory_usage` ()

The result is a list in the following format:

{total memory used, resident set size, shared pages, text, data + stack}

### Function: `usage`

usage -- Return statistics concerning the server the MOO is running on.

list `usage` ()

The result is a list in the following format:

```
{{load averages}, user time, system time, page reclaims, page faults, block input ops, block output ops, voluntary context switches, involuntary context switches, signals received}
```

### Function: `dump_database`

dump_database -- requests that the server checkpoint the database at its next opportunity

none `dump_database` ()

It is not normally necessary to call this function; the server automatically checkpoints the database at regular intervals; see the chapter on server assumptions about the database for details. If the programmer is not a wizard, then `E_PERM` is raised.

### Function: `panic`

panic -- Unceremoniously shut down the server, mimicking the behavior of a fatal error.

void panic([STR message])

The database will NOT be dumped to the file specified when starting the server. A new file will be created with the name of your database appended with .PANIC.

> Warning: Don't run this unless you really want to panic your server.

### Function: `db_disk_size`

db_disk_size -- returns the total size, in bytes, of the most recent full representation of the database as one or more disk files

int `db_disk_size` ()

Raises `E_QUOTA` if, for some reason, no such on-disk representation is currently available.

### Function: `exec`

exec -- Asynchronously executes the specified external executable, optionally sending input.

list `exec` (LIST command[, STR input][, LIST environment variables])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

Additionally, you can provide a list of environment variables to set in the shell.

When the process exits, it returns a list of the form:

```
{code, output, error}
```

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                      {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                     {0, "foo", ""}
exec({"echo", "one", "two"})                             {0, "one two~0A", ""}
```

### Function: `shutdown`

shutdown -- requests that the server shut itself down at its next opportunity

none `shutdown` ([str message])

Before doing so, a notice (incorporating message, if provided) is printed to all connected players. If the programmer is not a wizard, then `E_PERM` is raised.

### Function: `verb_cache_stats`

### Function: `log_cache_stats`

list verb_cache_stats ()

none log_cache_stats ()

The server caches verbname-to-program lookups to improve performance. These functions respectively return or write to the server log file the current cache statistics. For verb_cache_stats the return value will be a list of the form

```
{hits, negative_hits, misses, table_clears, histogram},
```

though this may change in future server releases. The cache is invalidated by any builtin function call that may have an effect on verb lookups (e.g., delete_verb()).
