# Controlling the Handling of Aborted Tasks

The server will abort the execution of tasks for either of two reasons:

1. an error was raised within the task but not caught

In each case, after aborting the task, the server attempts to call a particular _handler verb_ within the database to allow code there to handle this mishap in some appropriate way. If this verb call suspends or returns a true value, then it is considered to have handled the situation completely and no further processing will be done by the server. On the other hand, if the handler verb does not exist, or if the call either returns a false value without suspending or itself is aborted, the server takes matters into its own hands.

First, an error message and a MOO verb-call stack _traceback_ are printed to the player who typed the command that created the original aborted task, explaining why the task was aborted and where in the task the problem occurred. Then, if the call to the handler verb was itself aborted, a second error message and traceback are printed, describing that problem as well. Note that if the handler-verb call itself is aborted, no further 'nested' handler calls are made; this policy prevents what might otherwise be quite a vicious little cycle.

The specific handler verb, and the set of arguments it is passed, differs for the two causes of aborted tasks.

If an error is raised and not caught, then the verb-call

```
$handle_uncaught_error(code, msg, value, traceback, formatted)
```

is made, where code, msg, value, and traceback are the values that would have been passed to a handler in a `try`-`except` statement and formatted is a list of strings being the lines of error and traceback output that will be printed to the player if `$handle_uncaught_error` returns false without suspending.

If a task runs out of ticks or seconds, then the verb-call

```
$handle_task_timeout(resource, traceback, formatted)
```

is made, where `resource` is the appropriate one of the strings `"ticks"` or `"seconds"`, and `traceback` and `formatted` are as above.
