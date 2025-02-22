# Controlling the Execution of Tasks

As described earlier, in the section describing MOO tasks, the server places limits on the number of seconds for which any task may run continuously and the number of “ticks,” or low-level operations, any task may execute in one unbroken period. By default, foreground tasks may use 60,000 ticks and five seconds, and background tasks may use 30,000 ticks and three seconds. These defaults can be overridden from within the database by defining any or all of the following properties on $server_options and giving them integer values:

| Property   | Description                                         |
| ---------- | --------------------------------------------------- |
| bg_seconds | The number of seconds allotted to background tasks. |
| bg_ticks   | The number of ticks allotted to background tasks.   |
| fg_seconds | The number of seconds allotted to foreground tasks. |
| fg_ticks   | The number of ticks allotted to foreground tasks.   |

The server ignores the values of `fg_ticks` and `bg_ticks` if they are less than 100 and similarly ignores `fg_seconds` and `bg_seconds` if their values are less than 1. This may help prevent utter disaster should you accidentally give them uselessly-small values.

Recall that command tasks and server tasks are deemed _foreground_ tasks, while forked, suspended, and reading tasks are defined as _background_ tasks. The settings of these variables take effect only at the beginning of execution or upon resumption of execution after suspending or reading.

The server also places a limit on the number of levels of nested verb calls, raising `E_MAXREC` from a verb-call expression if the limit is exceeded. The limit is 50 levels by default, but this can be increased from within the database by defining the `max_stack_depth` property on `$server_options` and giving it an integer value greater than 50. The maximum stack depth for any task is set at the time that task is created and cannot be changed thereafter. This implies that suspended tasks, even after being saved in and restored from the DB, are not affected by later changes to $server_options.max_stack_depth.

Finally, the server can place a limit on the number of forked or suspended tasks any player can have queued at a given time. Each time a `fork` statement or a call to `suspend()` is executed in some verb, the server checks for a property named `queued_task_limit` on the programmer. If that property exists and its value is a non-negative integer, then that integer is the limit. Otherwise, if `$server_options.queued_task_limit` exists and its value is a non-negative integer, then that's the limit. Otherwise, there is no limit. If the programmer already has a number of queued tasks that is greater than or equal to the limit, `E_QUOTA` is raised instead of either forking or suspending. Reading tasks are affected by the queued-task limit.
