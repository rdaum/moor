# The First Tasks Run By the Server

Whenever the server is booted, there are a few tasks it runs right at the beginning, before accepting connections or getting the value of $server_options.dump_interval to schedule the first checkpoint (see below for more information on checkpoint scheduling).

First, the server calls $do_start_script() and passes in script content via the args built-in variable. The script content is specified on the command line when the server is started. The server can call this verb multiple times, once each for the -c and -f command line arguments.

Next, the server calls $user_disconnected() once for each user who was connected at the time the database file was written; this allows for any cleaning up that`s usually done when users disconnect (e.g., moving their player objects back to some`home` location, etc.).

Next, it checks for the existence of the verb $server_started(). If there is such a verb, then the server runs a task invoking that verb with no arguments and with player equal to #-1. This is useful for carefully scheduling checkpoints and for re-initializing any state that is not properly represented in the database file (e.g., re-opening certain outbound network connections, clearing out certain tables, etc.).
