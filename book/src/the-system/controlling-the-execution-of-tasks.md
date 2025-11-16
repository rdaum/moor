# Controlling the Execution of Tasks

As described earlier, in the section describing MOO tasks, the server places limits on the number of seconds for which
any task may run continuously and the number of “ticks,” or low-level operations, any task may execute in one unbroken
period. By default, foreground tasks may use 60,000 ticks and five seconds, and background tasks may use 30,000 ticks
and three seconds. These defaults can be overridden from within the database by defining any or all of the following
properties on $server_options and giving them integer values:

| Property   | Description                                         |
|------------|-----------------------------------------------------|
| bg_seconds | The number of seconds allotted to background tasks. |
| bg_ticks   | The number of ticks allotted to background tasks.   |
| fg_seconds | The number of seconds allotted to foreground tasks. |
| fg_ticks   | The number of ticks allotted to foreground tasks.   |

The server ignores the values of `fg_ticks` and `bg_ticks` if they are less than 100 and similarly ignores `fg_seconds`
and `bg_seconds` if their values are less than 1. This may help prevent utter disaster should you accidentally give them
uselessly-small values.

Recall that command tasks and server tasks are deemed _foreground_ tasks, while forked, suspended, and reading tasks are
defined as _background_ tasks. The settings of these variables take effect only at the beginning of execution or upon
resumption of execution after suspending or reading.

The server also places a limit on the number of levels of nested verb calls, raising `E_MAXREC` from a verb-call
expression if the limit is exceeded. The limit is 50 levels by default, but this can be increased from within the
database by defining the `max_stack_depth` property on `$server_options` and giving it an integer value greater than 50.
The maximum stack depth for any task is set at the time that task is created and cannot be changed thereafter. This
implies that suspended tasks, even after being saved in and restored from the DB, are not affected by later changes to $
server_options.max_stack_depth.

Finally, the server can place a limit on the number of forked or suspended tasks any player can have queued at a given
time. Each time a `fork` statement or a call to `suspend()` is executed in some verb, the server checks for a property
named `queued_task_limit` on the programmer. If that property exists and its value is a non-negative integer, then that
integer is the limit. Otherwise, if `$server_options.queued_task_limit` exists and its value is a non-negative integer,
then that's the limit. Otherwise, there is no limit. If the programmer already has a number of queued tasks that is
greater than or equal to the limit, `E_QUOTA` is raised instead of either forking or suspending. Reading tasks are
affected by the queued-task limit.

## Preventing Tick Limit Exhaustion

For long-running tasks that perform many operations (such as iterating over large datasets), there's a risk of
exceeding the tick limit and having the task aborted. The `suspend_if_needed([threshold])` function provides a
way to manage this:

```moo
for item in (large_list)
    // Do some work with item
    process_item(item);

    // Check if we're running low on ticks and commit if needed
    // Default threshold is 4000 ticks
    if (suspend_if_needed())
        // We committed and resumed - continue with fresh tick budget
    endif
endfor
```

The function checks the remaining tick count against a threshold (4000 ticks by default). If fewer ticks remain than
the threshold, it commits the current transaction and immediately resumes in a new transaction with a fresh tick
budget, returning `true`. If plenty of ticks remain, it simply returns `false` without suspending.

You can specify a custom threshold:

```moo
// Only commit when fewer than 1000 ticks remain
suspend_if_needed(1000);

// More aggressive - commit when fewer than 100 ticks remain
suspend_if_needed(100);
```

### Important: Transaction Boundaries and Data Consistency

**Critical Warning**: When `suspend_if_needed()` returns `true`, it means your current transaction has been committed
and you're now running in a completely new transaction. This creates potential consistency issues:

```moo
// DANGEROUS - assumptions can become invalid across transaction boundaries
obj = #123;
initial_value = obj.counter;

for i in [1..10000]
    obj.counter = obj.counter + 1;

    if (suspend_if_needed())
        // WARNING: Another task may have modified obj.counter!
        // Your assumption about its value may now be wrong
    endif
endfor
```

**Data races can occur** because other tasks may modify the same objects between your transaction commits:

- Properties you read earlier may have changed
- Objects may have been moved, recycled, or modified
- Lists or collections you're iterating over may have been altered
- Any cached values in local variables may be stale

**Best practices when using `suspend_if_needed()`:**

1. **Re-validate assumptions after each commit:**
   ```moo
   for item in (large_list)
       if (suspend_if_needed())
           // Re-read any critical data after transaction boundary
           item = // re-fetch from database
       endif
       // work with item
   endfor
   ```

2. **Use for append-only or independent operations:**
   ```moo
   // SAFE - each iteration is independent
   for player in (connected_players())
       notify(player, "Maintenance message");
       suspend_if_needed(); // Safe - notifications are independent
   endfor
   ```

3. **Avoid when maintaining complex invariants:**
   ```moo
   // AVOID - complex state that must remain consistent
   balance_accounts();  // Don't use suspend_if_needed() during complex updates
   ```

4. **Consider transaction retry risks:**
   If conflicts occur, your entire task may be retried from the beginning, potentially re-executing work done before
   the suspend. Design your logic to be idempotent when possible.

In summary: `suspend_if_needed()` is excellent for preventing tick exhaustion in long-running tasks, but you must
carefully consider transaction boundaries and the possibility that other tasks may modify data between commits.
