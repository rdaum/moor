# MOO Tasks

A _task_ is an execution of a MOO program. There are five kinds of tasks in ToastStunt:

- Every time a player types a command, a task is created to execute that command; we call these _command tasks_.
- Whenever a player connects or disconnects from the MOO, the server starts a task to do whatever processing is necessary, such as printing out `Munchkin has connected` to all of the players in the same room; these are called _server tasks_.
- The `fork` statement in the programming language creates a task whose execution is delayed for at least some given number of seconds; these are _forked tasks_. Sub-second forking is possible (eg. 0.1)
- The `suspend()` function suspends the execution of the current task. A snapshot is taken of whole state of the execution, and the execution will be resumed later. These are called _suspended tasks_. Sub-second suspending is possible.
- The `read()` function also suspends the execution of the current task, in this case waiting for the player to type a line of input. When the line is received, the task resumes with the `read()` function returning the input line as result. These are called _reading tasks_.

The last three kinds of tasks above are collectively known as _queued tasks_ or _background tasks_, since they may not run immediately.

To prevent a maliciously- or incorrectly-written MOO program from running forever and monopolizing the server, limits are placed on the running time of every task. One limit is that no task is allowed to run longer than a certain number of seconds; command and server tasks get five seconds each while other tasks get only three seconds. This limit is, in practice, rarely reached. The reason is that there is also a limit on the number of operations a task may execute.

The server counts down _ticks_ as any task executes. Roughly speaking, it counts one tick for every expression evaluation (other than variables and literals), one for every `if`, `fork` or `return` statement, and one for every iteration of a loop. If the count gets all the way down to zero, the task is immediately and unceremoniously aborted. By default, command and server tasks begin with a store of 60,000 ticks; this is enough for almost all normal uses. Forked, suspended, and reading tasks are allotted 30,000 ticks each.

These limits on seconds and ticks may be changed from within the database, as can the behavior of the server after it aborts a task for running out; see the chapter on server assumptions about the database for details.

Because queued tasks may exist for long periods of time before they begin execution, there are functions to list the ones that you own and to kill them before they execute. These functions, among others, are discussed in the following section.

Some server functions, when given large or complicated amounts of data, may take a significant amount of time to complete their work. When this happens, the MOO can't process any other player input or background tasks and users will experience lag. To help diagnose the causes of lag, ToastStunt provides the `DEFAULT_LAG_THRESHOLD` option in options.h (which can be overridden in the database. See the Server Assumptions About the Database section). When a running task exceeds this number of seconds, the server will make a note in the server log and call the verb `#0:handle_lagging_task()` with the arguments: `{callers, execution time}`. Callers will be a `callers()`-style list of every verb call leading up to the lagging verb, and execution time will be the total time it took the verb to finish executing. This can help you gauge exactly what verb is causing the problem.

> Note: Depending on your system configuration, FG_SECONDS and BG_SECONDS may not necessarily correspond to actual seconds in real time. They often measure CPU time. This is why your verbs can lag for several seconds in real life and still not raise an 'out of seconds' error."
