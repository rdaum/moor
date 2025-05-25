# Transactions in the MOO Database

In classic LambdaMOO and ToastStunt every single command takes turns modifying the database, and if they take too long
or if there's a lot of them running, you get what is coloquially known as a "lag spike". This is because the server
has to take turns to process each command, and if one command takes too long, it blocks the others from running.

mooR introduces a transactional model for managing changes to the database, which allows for more efficient and
concurrent
modifications. Each task runs in its own transaction, which is automatically committed when the task completes
successfully.

This adds a bit of overhead to the server such that mooR will be a bit slower than LambdaMOO for a single user or
when there's a small number of users, but it allows for much better performance when there are many users or when tasks
are running concurrently.

## Serializable isolation level

mooR offers a consistent (serializable) isolation level, which means that transactions behave as if they were executed
one after the other, even if they are actually running concurrently. This means that changes made by one task are not
visible to other tasks until the transaction is committed. This allows for progress to be made without worrying about
other tasks interfering with the current task's changes, and -- unlike LambdaMOO -- multiple programs run truly in
parallel,
taking advantage of multi-core CPUs, without waiting on each other.

If conflicts occur at the time of committing a transaction, the whole task is re-executed from the beginning, allowing
the task to retry its work. The server will do this automatically, so the programmer does not need to worry about it.

## When do transactions start and end?

Each command or task runs in its own transaction, which is automatically committed when the task completes successfully.

This means that when a user types a command like `pet the kitty`, the server starts a new transaction right after the
command is parsed, and it will commit the transaction when the command finishes executing. Any verbs that the command
performs on the database will be part of that transaction, and they will not be visible to other tasks until the
transaction
completes, which is usually when the command finishes executing and the output is sent to the user.

## What happens when transactions conflict?

If two tasks try to modify the same object at the same time, a conflict occurs. In this case, the server will only
notice the conflict when the task tries to commit the transaction. If a conflict occurs, the server will re-execute the
entire task from the beginning, allowing the task to retry its work.

## What about output and user interaction?

When a task is running, it can output text to the user using the `notify()` function. This output is buffered and will
not be sent to the user until the transaction is committed. This means that if a task is restarted due to a conflict,
the output will be sent to the user only once, after the task has completed successfully.