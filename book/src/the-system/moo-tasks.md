# MOO Tasks

A _task_ is an execution of a MOO program. There are several kinds of tasks in mooR:

- Every time a player types a command, a task is created to execute that command; we call these _command tasks_.
- External processes can directly call verbs in the database via RPC, which creates a task to execute that verb;
  these are also _verb tasks_.
- Whenever a player connects or disconnects from the MOO, the server starts a task to do whatever processing is
  necessary, such as printing out `Munchkin has connected` to all of the players in the same room; these are called
  _server tasks_.
- The `fork` statement in the programming language creates a task whose execution is delayed for at least some given
  number of seconds; these are _forked tasks_. Sub-second forking is possible (eg. 0.1)
- The `suspend()` function suspends the execution of the current task. A snapshot is taken of whole state of the
  execution, and the execution will be resumed later. These are called _suspended tasks_. Sub-second suspending is
  possible.
- The `read()` function also suspends the execution of the current task, in this case waiting for the player to type a
  line of input. When the line is received, the task resumes with the `read()` function returning the input line as
  result. These are called _reading tasks_.
- `worker_request()` creates a _worker task_, which is a task that waits for a worker to perform some action. The
  worker task is queued until a worker (an external helper process) completes the action and returns the result.

The last three kinds of tasks above are collectively known as _queued tasks_ or _background tasks_, since they may not
run immediately.

To prevent a maliciously- or incorrectly-written MOO program from running forever and monopolizing the server, limits
are placed on the running time of every task. One limit is that no task is allowed to run longer than a certain number
of seconds; command and server tasks get five seconds each while other tasks get only three seconds. This limit is, in
practice, rarely reached. The reason is that there is also a limit on the number of operations a task may execute.

The server counts down _ticks_ as any task executes. Roughly speaking, it counts one tick for every expression
evaluation (other than variables and literals), one for every `if`, `fork` or `return` statement, and one for every
iteration of a loop. If the count gets all the way down to zero, the task is immediately and unceremoniously aborted. By
default, command and server tasks begin with a store of 60,000 ticks; this is enough for almost all normal uses. Forked,
suspended, and reading tasks are allotted 30,000 ticks each.

These limits on seconds and ticks may be changed from within the database, as can the behavior of the server after it
aborts a task for running out; see the chapter on server assumptions about the database for details.

Because queued tasks may exist for long periods of time before they begin execution, there are functions to list the
ones that you own and to kill them before they execute. These functions, among others, are discussed in the following
section.

### Active versus queued tasks

Unlike LambdaMOO, mooR can run multiple tasks in parallel, taking advantage of multi-core CPUs. In LambdaMOO only one
task is running at a time, and the server switches between tasks to give the illusion of parallelism. In mooR,
there are two kinds of tasks: _active tasks_ and _queued tasks_.

Queued tasks are tasks that are in some kind of waiting or suspended state. They are not currently running, but they
may run in the future. Examples of queued tasks include:

- Forked tasks that have not yet been executed because their delay has not yet expired.
- Suspended tasks that are waiting to be resumed.
- Reading tasks that are waiting for input from the player.
- Worker tasks that are waiting for a worker to perform some action.

The `queued_tasks()` function returns a list of all queued tasks that you own, and the `kill_task()` function can be
used to kill a queued task before it runs. Because queued tasks are not currently running, information on them is more
detailed, including the verb and line number where the task is suspended at, etc. Queued tasks can be aborted / killed.

Active tasks, on the other hand, are tasks that are currently running. They are executing MOO code and because of this
it is not efficient to gather detailed information about them. The `active_tasks()` function returns a list of all
active tasks that you own, and the `kill_task()` function can be used to kill an active task. However, killing an active
task is more of a "best effort" operation, since the task may be in the middle of executing some code.

### Transactional management

Unlike LambdaMOO, mooR supports a transactional model for managing changes to the database. Each task runs in its own
transaction, which is automatically committed when the task completes successfully. If a task is suspended, it is
committed and a new database transaction is started when the task resumes.

From the point of view of the MOO programmer, transactions should be considered an implementation detail of the server,
but they do provide some useful features:

* Each transaction is isolated from other transactions, meaning that changes made by one task are not visible to
  other tasks until the transaction is committed. mooR offers a consistent (serializable) isolation level, which means
  that transactions behave as if they were executed one after the other, even if they are actually running concurrently.
  This means that progress can be made without worrying about other tasks interfering with the current task's changes,
  and -- unlike LambdaMOO -- multiple programs run truly in parallel, taking advantage of multi-core CPUs, without
  waiting on each other.
* If conflicts occur at the time of committing a transaction, the whole task is re-executed from the beginning,
  allowing the task to retry its work. The server will do this automatically, so the programmer does not need to
  worry about it.

This does however, have some implications for how tasks are written:

* Long running tasks that perform a lot of work should be split into smaller tasks that can be
  committed more frequently. This is because the server will re-execute the entire task if a conflict occurs, which
  can lead to wasted work if the task is too long.
* Tasks may not have predictable runtimes, since they may be re-executed multiple times if conflicts
  occur. This means that tasks should not rely on a specific amount of time to complete, and should be written to
  handle being re-executed multiple times.

For explicit transaction management, the following functions are available at the wizard level:

- The (wizard-only) `commit()` function is used to commit the mutations made by a task to the database, and it
  suspends the task until the commit is complete. This is functionally equivalent suspending for 0 seconds, but is
  more explicit about the intent to commit the changes, and is optimized for this purpose.
- The (wizard-only) `rollback()` function is used to abort the current task, reverting any changes made to the database
  since the last commit.
