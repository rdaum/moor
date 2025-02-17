# Signals to the Server

The server is able to intercept [signals](https://en.wikipedia.org/wiki/Signal_(IPC)) from the operating system and perform certain actions, a list of which can be found below. Two signals, USR1 and USR2, can be processed from within the MOO database. When SIGUSR1 or SIGUSR2 is received, the server will call `#0:handle_signal()` with the name of the signal as the only argument. If this verb returns a true value, it is assumed that the database handled it and no further action is taken. If the verb returns a negative value, the server will proceed to execute the default action for that signal. The following is a list of signals and their default actions:

| Signal | Action                                               |
| ------ | ---------------------------------------------------- |
| HUP    | Panic the server.                                    |
| ILL    | Panic the server.                                    |
| QUIT   | Panic the server.                                    |
| SEGV   | Panic the server.                                    |
| BUS    | Panic the server.                                    |
| INT    | Cleanly shut down the server.                        |
| TERM   | Cleanly shut down the server.                        |
| USR1   | Reopen the log file.                                 |
| USR2   | Schedule a checkpoint to happen as soon as possible. |

For example, imagine you're a system administrator logged into the machine running the MOO. You want to shut down the MOO server, but you'd like to give players the opportunity to say goodbye to one another rather than immediately shutting the server down. You can do so by intercepting a signal in the database and invoking the @shutdown command.

```
@prog #0:handle_signal
set_task_perms(caller_perms());
{signal} = args;
if (signal == "SIGUSR2" && !$code_utils:task_valid($wiz_utils.shutdown_task))
  force_input(#2, "@shutdown in 1 Shutdown signal received.");
  force_input(#2, "yes");
  return 1;
endif
.
```

Now you can signal the MOO with the kill command: `kill -USR2 <MOO process ID`
