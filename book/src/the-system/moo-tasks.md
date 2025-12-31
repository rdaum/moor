# MOO Tasks

A _task_ is an execution of a MOO program. **Every running task operates within its own database transaction** (see [Transactions](../the-database/transactions.md) for details about how database changes work). This means that while a task is actively running, all its database changes are held in a private transaction that other tasks cannot see until the task completes or suspends.

There are several kinds of tasks in mooR:

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
  possible. **When a task suspends, its transaction is committed** (meaning all database changes become permanent and visible to other tasks), **and when it resumes, it starts with a completely new transaction**.
- The `read()` function also suspends the execution of the current task, in this case waiting for the player to type a
  line of input. When the line is received, the task resumes with the `read()` function returning the input line as
  result. These are called _reading tasks_. Like `suspend()`, the transaction commits when waiting for input and a new transaction begins when input is received.
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

### Tasks and transactions: How they work together

**Every active task runs within its own database transaction.** This is a fundamental concept that affects how your MOO programs behave:

- **While running:** Your task's database changes (setting properties, moving objects, etc.) are private to your task. Other tasks cannot see these changes until your transaction finishes.
- **When completing:** If your task finishes successfully, its transaction automatically commits—all changes become permanent and visible to everyone.
- **When suspending:** If your task calls `suspend()`, `read()`, or `fork`, the current transaction commits immediately. When the task resumes later, it starts with a brand new transaction.

This transactional system is what allows mooR to run multiple tasks truly in parallel without them interfering with each other. For complete details about how transactions work, see the [Transactions](../the-database/transactions.md) section.

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

### Task introspection and safe patterns

mooR does not expose `task_stack()` for active tasks. Tasks run in parallel threads, so a running task's stack is a
moving target. Capturing it would require pausing tasks or heavy instrumentation, both of which impose significant
performance costs and scheduling side effects. Queued tasks can be inspected more deeply because they are not running.

If you are used to LambdaMOO/Toast patterns that rely on `task_stack()`, consider these approaches instead:

- Prefer the metadata available from `active_tasks()` and `queued_tasks()` (task id, start info, verb location) to
  identify what to kill.
- Treat `kill_task()` as best-effort for active tasks; it takes effect when the task next suspends or exits its main
  loop. For workflows that need to wait for termination, combine `kill_task()` with `wait_task()` in a loop.
- Instead of actively recycling objects, consider using anonymous objects. Once all references are gone, they become
  unreachable and can be reclaimed after any in-flight tasks drop their references.
- For expected "object missing" failures, handle them in `$handle_uncaught_exception` to avoid cascading tracebacks and
  to add contextual handling.
- For tasks that must be cancellable quickly, add voluntary checks in the code paths you control (a property flag or a
  helper that returns "abort now") and return early when set.

### Advanced transaction management

Unlike LambdaMOO, mooR supports a transactional model for managing changes to the database, as described above. The automatic transaction management should handle most use cases, but mooR also supports explicit transaction control for advanced scenarios.

From the point of view of most MOO programmers, transactions should be considered an implementation detail of the server that "just works." However, understanding transactions provides some useful insights:

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

For explicit transaction management, the following functions are available:

- The (wizard-only) `commit([value])` function commits the mutations made by a task to the database, then suspends
  and resumes the task in a new transaction. If provided, `value` is returned when the task resumes; otherwise,
  `commit()` returns `0`. This is similar to suspending for 0 seconds, but is more explicit about the intent to
  commit changes, and is optimized for this purpose. The optional return value is useful when you need to commit
  changes and then return a result that depends on those committed changes being visible.
- The (wizard-only) `rollback()` function is used to abort the current task, reverting any changes made to the database
  since the last commit.
- The `suspend_if_needed([threshold])` function checks if the remaining tick count is below the specified threshold
  (defaulting to 4000 ticks). If so, it commits the current transaction and immediately resumes in a new transaction,
  returning `true`. If the tick budget is still sufficient, it returns `false` without suspending. This is useful
  for long-running tasks that need to periodically commit their work to avoid hitting the tick limit, while
  minimizing unnecessary commits when plenty of ticks remain.
