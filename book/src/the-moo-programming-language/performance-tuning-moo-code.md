# Performance Tuning MOO Code

This chapter is about the performance of your MOO programs themselves.

The server can be well-configured and the machine can be healthy, and your world can still feel slow
because the verbs and helper code in the database are doing too much work, doing it in the wrong
shape, or creating unnecessary transaction conflicts.

This matters more in mooR than in classic single-threaded MOO servers. mooR can execute many tasks
at once, so well-structured code can scale much better. But that same concurrency means poorly
structured code can waste work through retries, amplify contention, and make a busy world behave as
though it were serialized anyway.

For server-side background on threading, scheduling, and database internals, see
[Performance and Concurrency](../the-system/performance-and-concurrency.md). For transactional
background, see [Transactions](../the-database/transactions.md). For task lifecycle details, see
[MOO Tasks](../the-system/moo-tasks.md).

This page is written for ordinary MOO programmers, not only server operators or wizards. You do not
need access to internal performance counters to use most of this advice. In many cases the first
sign of a problem is simply behavioral:

- one command feels much slower than similar commands
- the same subsystem gets worse as more players use it at once
- a verb works, but becomes erratic or inconsistent under load
- old LambdaMOO code that used to feel "careful" now feels slow in mooR

## Start With General Advice

Most performance work starts with the same discipline:

- make the code correct first
- measure before and after changing it
- optimize the shape of the algorithm before micro-optimizing expressions
- keep hot paths simple and predictable
- avoid work that will be thrown away on retry

In practice, that usually means:

- do less work per command
- touch fewer objects and fewer properties
- keep transactions short
- reduce repeated scans, string rebuilding, and list copying
- separate rare administrative work from common player-facing work

## Measure With `ftime()`

If you do not have access to server-side counters, `ftime()` is the simplest way to time a suspicious
piece of MOO code.

The basic pattern is:

```moo
start = ftime();
... do work ...
elapsed = ftime() - start;
player:tell("elapsed: ", elapsed, " seconds");
```

This is often enough to answer useful first questions:

- which branch or helper call is actually slow
- whether a rewrite made the command faster
- whether one loop or one builtin-heavy section dominates the cost

When using `ftime()`:

- measure the same operation several times before drawing conclusions
- time the specific part you are suspicious of, not only the whole verb
- compare single-user behavior with concurrent use if you suspect contention
- remove or disable noisy timing output after debugging

`ftime()` measures elapsed wall-clock time. That is usually what you want when tuning a verb, but
it does not tell you by itself why the code is slow. It helps you find the expensive section; then
you still need to inspect the shape of the code and the write pattern.

If you do have wizard or operator access, internal counters can help separate runtime pressure from
world-code cost:

- [`bf_counters()`](built-in-functions/server.md#bf_counters)
- [`db_counters()`](built-in-functions/server.md#db_counters)
- [`sched_counters()`](built-in-functions/server.md#sched_counters)

Those will not point to a specific verb, but they help separate "the system is contended" from "a
particular piece of MOO code is expensive." If you do not have access to them, you can still make a
lot of progress by looking at workload shape, shared objects, shared properties, and whether the
slowdown gets worse as concurrency rises.

## The First Question: Slow Server or Slow MOO Code?

When a command feels slow, ask which of these is happening:

- the task is delayed before it starts running
- the task runs promptly, but the verb does too much work
- the task keeps being retried because it conflicts with other work
- the task is waiting on the database or durable write path

As a rough guide:

- if the whole world feels delayed, the runtime may be under scheduler or storage pressure
- if one subsystem is slow but the rest of the world feels normal, inspect that subsystem's code
- if a feature gets worse mostly when several people use it at once, suspect contention and retries
- if a feature is slow even with one user, suspect unnecessary work inside the verb itself

If you do have access to internal counters:

- high scheduler delay points toward runtime scheduling pressure
- high builtin counters can mean your verbs lean heavily on expensive builtins
- high database commit or lock-wait counters can mean transaction pressure
- user-visible slowness with modest runtime counters often means the MOO code itself needs work

The rest of this page is about that last category.

## Write Less Per Command

The cheapest work is the work you do not perform.

Common sources of unnecessary cost in MOO code include:

- scanning large lists or maps on every command
- rebuilding the same derived strings repeatedly
- walking inheritance or object graphs many times in one verb
- repeated property lookups for values that do not change during the verb
- broad "refresh everything" patterns when only one item changed

Prefer code that narrows the scope of work early:

- return early when the request is invalid
- reject non-matching cases before constructing output
- look up the specific object you need instead of scanning whole collections
- cache stable intermediate values in local variables inside the verb

This is ordinary programming advice, but it matters more in a transactional system because wasted
work is not merely slow once. Under conflict, the same wasted work may be repeated.

## Keep Transactions Short

In mooR, each active task runs inside a transaction. If the transaction conflicts at commit time,
the task may run again from the beginning.

That means long verbs are expensive in two ways:

- they consume more CPU on the first run
- they waste more work when a retry happens

Prefer:

- shorter verbs with clearer phases
- explicit commit boundaries where the program model allows them
- splitting maintenance and bulk-processing work into smaller tasks
- avoiding large bursts of unrelated mutation in one transaction

If a long-running maintenance verb touches a lot of state, consider restructuring it so that it:

- processes a small batch
- commits or suspends
- resumes for the next batch

That reduces both retry cost and the amount of state held in flight at once.

## Do Not Carry Over LambdaMOO Tick Habits Uncritically

Classic LambdaMOO code often used `suspend(0)` or frequent `suspend_if_needed()` calls as a
defensive pattern. That made sense in a server where tasks were effectively taking turns on one main
execution path: yielding frequently could help avoid long stalls for other players.

That intuition does not carry over cleanly to mooR.

In mooR, it is often reasonable to set `$server_options.fg_ticks` and
`$server_options.bg_ticks` substantially higher than old LambdaMOO-style defaults or habits would
suggest.

Why:

- modern hardware is much faster than the machines these conventions were designed around
- tasks are not all waiting their turn behind one main execution path
- using suspension as a routine "yield" mechanism is usually unnecessary in a multithreaded runtime
- every suspend boundary is also a transaction boundary

That last point is the important one. In mooR, `suspend()`, `read()`, and similar task boundaries
commit the current transaction. Excessive suspension can therefore make a verb much slower, because
it turns one logical operation into many commits and resumptions.

Overusing suspension also changes semantics:

- other tasks can observe intermediate state between phases
- new races can appear between the pre-suspend and post-resume parts of the logic
- assumptions that were safe inside one transaction may no longer be safe across multiple
  transactions

This means a verb that was "careful" on LambdaMOO by suspending frequently can become both slower
and less correct on mooR.

Prefer:

- higher tick budgets for legitimate foreground and background work
- explicit chunking only when the work is truly large
- commit or suspend points that reflect real phase boundaries in the logic
- `suspend_if_needed()` as a tool for long-running maintenance work, not as routine decoration on
  ordinary verbs

If a verb is full of old `suspend(0)` calls whose purpose was merely "be polite to the server," that
is a strong candidate for cleanup when moving code to mooR.

## Contention and Retries Are Often the Real Problem

This is the most important thing to internalize when tuning mooR code.

Many performance problems are not caused by any one verb being individually expensive. They come
from many tasks all trying to mutate the same logical resource at about the same time.

Because mooR uses optimistic concurrency, those tasks do not wait behind one big lock. Instead,
they run, and conflicting work is rejected and retried. If the conflict pattern is bad, the system
can spend a lot of CPU redoing work that never commits.

From the point of view of an ordinary player or programmer, these retries are usually invisible as
events. The user does not normally see "your task conflicted and was retried." What they see is that
the command feels slow, inconsistent under load, or slower only when several people use the same
feature at once.

The crucial point is that contention is often effectively at the property level.

If many tasks keep writing the same property on the same object, or a small cluster of related
properties on the same hot object, they are competing for the same logical keys. That creates
retries even if the rest of the world is quiet.

This is one of the most important differences from LambdaMOO-era programming style. Code that uses a
single shared object as a convenient global registry, counter bucket, mailbox, or status board can
become a scalability bottleneck even when each individual verb looks simple.

If you remember only one idea from this page, make it this one: independent activity should avoid
mutating the same properties unless that sharing is truly necessary.

Wizard- and operator-level tooling can expose this more directly through logs and internal counters,
but ordinary programmers often have to infer it indirectly from the pattern of the slowdown.

## Conflict-Inducing Coding Patterns

The following patterns are especially worth avoiding in mooR.

### Hot Global Properties

Examples:

- incrementing one global counter property for every command
- updating one shared "last activity" property from many users
- appending to one global log property
- maintaining one shared queue property for all producers
- storing many independent items inside one shared map or list property

These patterns funnel unrelated work through the same keys.

Prefer:

- sharded counters
- per-room, per-player, or per-zone aggregation
- append-by-message or append-by-task patterns that merge later
- storing derived summaries separately from the hot write path

In mooR, it is often better to add another property than to keep extending one shared collection
property.

For example, storing many items inside one map property can be slower than storing them as separate
properties:

- property lookup is effectively constant-time in the common case
- map operations are logarithmic in the size of the map
- two tasks mutating different keys in the same map property still contend on the same property

The same warning applies to shared list properties:

- appending or rewriting a shared list can be more expensive than touching a narrower property
- unrelated updates packed into one list still collide because they mutate the same property value

If many actors are updating logically independent entries, prefer separate properties or separate
objects over one ever-growing shared map or list.

This often feels less elegant at first, especially if you are used to packing everything into one
map or list. In mooR, the flatter design is often the faster and safer one.

### Shared Mailboxes and Registries

A common design instinct is to have one object that "owns" a subsystem and stores all live state in
its properties. That can be clean conceptually, but it is often the wrong write pattern for mooR.

If every chat event, combat event, presence update, or job-state transition writes to properties on
the same coordinator object, that object becomes hot and the subsystem becomes conflict-prone.

Prefer designs where:

- ownership of mutable state is distributed
- unrelated actors write to their own objects or to partitioned shard objects
- aggregation is performed less often than raw event production
- large maps and lists are not used as the default container for all mutable subsystem state

If you are designing a new subsystem, it is often worth asking early: "Which objects will many tasks
write to at once?" That question is usually more important than "Which object feels like the right
owner conceptually?"

### Use Task Mailboxes For Coordination When They Fit

mooR adds per-task mailboxes through
[`task_send()`](built-in-functions/server.md#task_send) and
[`task_recv()`](built-in-functions/server.md#task_recv).

These are often a better coordination tool than:

- mutating one shared property from many tasks
- using one shared object as a queue
- making lots of cross-calling verbs just to signal state changes

The mailbox model is simple:

- `task_send(task_id, value)` queues a message for another task
- `task_recv([wait_time])` returns queued messages for the current task

This is useful when one task should own some piece of ongoing logic and other tasks only need to
send it events or requests.

Examples:

- a long-lived game tick loop receiving register, unregister, or interrupt events
- a controller task receiving work requests
- a long-lived subsystem task that should make the next decision without every caller mutating the
  same properties directly

This pattern can reduce contention because senders do not need to keep writing the same shared
properties just to get the owner task's attention.

One useful pattern is a game update loop that owns the current subscriber set and receives
register, unregister, or interrupt events through its mailbox. Other tasks send requests to that
long-lived loop task, and the loop applies the changes when it next receives its mailbox.

There is one important caveat: `task_recv()` is also a transaction boundary, just like `commit()` or
`read()`. So this is not free communication. It is best when that boundary is already a natural part
of the design, such as a loop that waits for the next tick or the next batch of work.

When `task_recv(wait_time)` actually waits, the task moves into a queued state. It is not sitting on
a worker thread burning CPU while it waits for the next message. That makes this pattern practical
for long-lived event loops and controller tasks that spend much of their time idle between events.

In other words:

- use task mailboxes when one task naturally owns the ongoing loop
- do not replace every ordinary property read or write with `task_send()` / `task_recv()`
- remember that mailbox-based coordination changes both performance and consistency behavior

The related task-management builtins are also worth knowing about:

- [`task_id()`](built-in-functions/server.md#task_id) to learn the current task id
- [`valid_task()`](built-in-functions/server.md#valid_task) to check whether a task still exists
- [`active_tasks()`](built-in-functions/server.md#active_tasks) and
  [`queued_tasks()`](built-in-functions/server.md#queued_tasks) to inspect your tasks
- [`kill_task()`](built-in-functions/server.md#kill_task) and
  [`resume()`](built-in-functions/server.md#resume) to control tasks
- [`wait_task()`](built-in-functions/server.md#wait_task) when a task needs to wait for another one

### Many Tasks Checking One Shared Thing

For example:

- many tasks read a shared property and then all attempt to update it
- many tasks test a shared "is available" flag and then flip it
- many tasks inspect a shared room or resource object and then mutate one hot property on it

Sometimes this cannot be avoided, but often it can be redesigned. Consider:

- splitting the resource into smaller independent pieces
- storing the changing state closer to the actor or item involved
- using a narrower claim or reservation object instead of one shared flag
- keeping closely related state together when that avoids several objects all coordinating with one
  another

### Rebuilding Shared Derived State on Every Update

Examples:

- rewriting a whole cached room summary every time one small detail changes
- rewriting a large index property after every insert
- updating multiple summary properties on a shared object for every local mutation

Prefer lazy derivation, coarser batch updates, or narrower caches. A derived structure that is cheap
to recompute on read may be better than one that is expensive and conflict-heavy to maintain on
every write.

## Shapes That Usually Scale Better

These are not laws, but they are good defaults.

### Partition Mutable State

Try to make unrelated actors update different objects or different areas of the world.

Examples:

- store per-player activity on the player object, not on one global activity object
- store room-local state on the room or nearby helper objects
- shard world-global counters or indexes by region, bucket, or time slice
- use separate properties for independent mutable fields instead of packing them into one hot map
- use separate helper objects when a shared list would otherwise become a write hotspot

### Separate Hot Writes From Cold Aggregation

If you need a world-global summary, do not necessarily maintain it synchronously on every command.

Often a better design is:

- write local facts in the hot path
- derive or aggregate the global summary later
- accept slightly stale summaries where the gameplay model allows it

This can make the difference between many tasks overlapping cleanly and many tasks colliding on one
shared object.

### Prefer Updates That Are Safe To Repeat

Tasks in mooR may be retried. That means code is easier to reason about when repeating the work does
not create a worse result.

For example, it is usually better to write code that says:

```moo
settings.enabled = 1;
```

than code that says:

```moo
settings.enabled = !settings.enabled;
```

if several tasks might reach the same code at once.

Likewise, code that says "make sure this state is true" is usually easier to retry safely than code
that says "flip this state" or "append one more copy."

This does not mean you should keep rewriting the same hot property unnecessarily. It means the logic
inside a retried task should not depend on a fragile sequence of one-time side effects.

### Move Background Work Off the Player Path

Maintenance, indexing, cleanup, exports, and broad recomputation usually should not happen inline in
the same command path that a player experiences directly.

If the work can be deferred, batch it, fork it, or spread it over time.

## Builtins Matter Too

Sometimes the MOO code is thin, but it drives expensive builtin usage.

Examples include:

- repeated regular-expression processing over large strings
- repeated list/set transformations on large collections
- repeated object/property introspection in loops
- XML/document processing on large payloads

If [`bf_counters()`](built-in-functions/server.md#bf_counters) shows a few builtins dominating time,
look at the surrounding verb structure:

- are you calling the builtin inside a loop when one precomputed result would do
- are you converting data between shapes repeatedly
- are you doing expensive parsing on every command instead of caching a compiled or normalized form

The slow part may be "MOO code plus builtin usage pattern," not just the builtin in isolation.

If you do not have access to builtin timing counters, you can still suspect this class of problem
when a verb does a lot of string processing, regular expressions, collection reshaping, or document
processing in loops.

## Handling Large Jobs

Do not read this as advice to sprinkle `suspend()` calls through ordinary verbs. Most normal
player-facing code should stay in one transaction.

This section is about genuinely large jobs: maintenance passes, rebuilding indexes, migrations,
cleanup work, exports, and other tasks that touch a lot of state.

For that kind of work, a single giant transaction can be a bad fit. It may run for a long time, hit
tick limits, or waste a great deal of work if it conflicts and retries.

Prefer:

- chunking work into batches
- committing between batches only at real phase boundaries
- using suspended or forked tasks for background maintenance
- storing explicit progress so resumed work can continue from a known point

The goal is not to "yield often." The goal is to keep very large jobs from becoming one huge,
fragile transaction.

## What To Look At When One Part Of The World Is Slow

If one subsystem is under suspicion, inspect:

- which objects it writes on the common path
- which properties on those objects are mutated most often
- whether the same helper object is being used by many unrelated tasks
- whether expensive derived state is being rewritten too eagerly
- whether a verb is doing repeated scans or repeated builtin work

Ask concrete questions:

- can this state be partitioned
- can this summary be deferred
- can this command touch fewer properties
- can this task commit sooner
- can this read-mostly structure be computed when needed instead of rewritten every time

Those questions usually lead to better results than trying to shave a few instructions off one loop.

## A Practical Tuning Order

When tuning MOO code, the following order is usually sensible:

1. identify the command or subsystem that feels slow
2. check runtime counters to rule out scheduler or storage path problems
3. inspect the shape of the MOO code: scans, loops, rebuilds, builtin-heavy paths
4. inspect the write pattern: which objects and which properties are hot
5. reduce conflict and retry pressure before micro-optimizing syntax
6. only then tune smaller expression-level costs

In mooR, reducing contention is often more valuable than making one transaction slightly faster in
isolation.
