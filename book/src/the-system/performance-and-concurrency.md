# Performance and Concurrency

This chapter is for server administrators and operators who want to understand how mooR actually
uses CPU time, threads, and the database under load.

mooR is not architected like classic LambdaMOO-style servers. LambdaMOO effectively serialized
almost all execution through one main path. mooR is intentionally multi-threaded: tasks can execute
concurrently, database transactions can overlap, and some durability work is handed off to
background threads. That gives mooR a very different performance profile, and it changes what
"bottleneck" means in practice.

This page explains the execution model, the database concurrency model, the main performance
counters, and the tuning knobs most likely to matter.

Not every performance problem is a runtime problem. Some worlds are slow because the MOO code itself
does too much work or creates avoidable transaction conflicts. For language-level guidance, see
[Performance Tuning MOO Code](../the-moo-programming-language/performance-tuning-moo-code.md).

This page assumes an operator or wizard audience. It discusses server-level counters, logs, thread
placement, and runtime configuration. Ordinary programmers without wizard access will usually want to
start with the language-side guidance above.

## Why Is My MOO Slow?

This is usually the question operators actually start with. The important step is to avoid treating
"slow" as one thing. In mooR, poor responsiveness can come from scheduler delay, worker saturation,
database conflict, or storage backpressure.

### Slow Command or Verb Response

Look first at:

- [scheduler counters](#scheduler-counters) such as submit, wakeup, and handoff latency
- [builtin counters](#builtin-counters) if commands spend time in a few heavy builtins
- [database counters](#database-counters) if the command performs a lot of object/property/verb mutation

Likely causes:

- the scheduler is delayed before the task reaches a worker
- the worker pool is saturated
- the command is builtin-heavy or VM-heavy
- the command is conflict-heavy and is retrying work

### Latency Spikes With Otherwise Low CPU Usage

Look first at:

- [scheduler counters](#scheduler-counters), especially wakeup and handoff counters
- [database counters](#database-counters), especially commit and provider lock-wait counters
- [database counters](#database-counters), especially batch-writer backpressure counters
- daemon log warnings around the storage path

Likely causes:

- intermittent commit serialization pressure
- slow durable flushes
- a barrier or checkpoint path waiting for background writes
- occasional contention on a hot object or mailbox

### Good CPU Availability but Writes Feel Slow

Look first at:

- [database counters](#database-counters), especially DB commit-phase counters
- [database counters](#database-counters), especially provider lock-wait counters
- [database counters](#database-counters), especially batch-writer backpressure counters
- conflict/retry behavior

Likely causes:

- the workload is logically contended even if CPUs are idle
- durable storage is slow enough that foreground commits start to feel it
- the writer path is coalescing under pressure rather than draining immediately

### Storage Is Slow

Slow storage has a distinct signature in mooR.

Look for both:

- warnings in the daemon log about slow flushes or batch-writer backpressure
- the [database counters](#database-counters), especially batch-writer backpressure counters, continuing to climb

In the current implementation, the writer path emits warnings when:

- Fjall flushes are slow
- the batch-writer queue is full and commits have to block
- barrier sends or backpressure blocks take too long

This is one of the clearer cases where logs and counters should be read together. A single warning
may just indicate a transient stall. Repeated warnings plus steadily rising batch-writer
backpressure counters are a strong sign that the storage layer is not keeping up with the write
rate.

When that happens, look at:

- disk latency and throughput on the host
- whether the deployment is on especially slow or burst-limited storage
- whether checkpoints, exports, or other maintenance activity are sharing the same device
- whether the workload is generating unusually large or bursty commits

### It Gets Worse As More Players Log In

Likely causes:

- more runnable tasks than the scheduler and worker pool can efficiently cycle through
- more contention on a small number of shared objects or properties
- mailbox or task-message hotspots
- write amplification from many tasks touching the same parts of the world

This is where mooR's concurrency model helps, but it does not remove all application-level
serialization. A world can still behave as if it is single-threaded if most activity pounds on the
same small set of logical resources.

## Execution Model

At a high level, the daemon has three important kinds of work:

- scheduler and control-plane work
- task execution work
- storage and durability work

The scheduler is responsible for orchestration:

- receiving requests from hosts and workers
- deciding which task should run next
- waking suspended tasks
- handling task lifecycle transitions such as suspend, retry, completion, and cancellation

Task bodies do not run on the scheduler thread. Instead, runnable tasks are dispatched onto a task
worker pool. Those workers execute verbs, builtins, and other VM activity in parallel. This is one
of mooR's core architectural differences from older single-threaded MOO servers.

In practice this means:

- independent tasks can make progress on different cores at the same time
- scheduler responsiveness still matters because all execution passes through it for orchestration
- affinity and core reservation can matter on heterogeneous CPUs

## Scheduler and Task Pool

The scheduler and the task pool have different jobs.

The scheduler:

- owns task queues and wakeup state
- processes messages from hosts, workers, and running tasks
- decides when to resume or dispatch tasks
- handles retries after transaction conflicts

The task pool:

- executes task bodies on worker threads
- runs verb dispatch and builtin-heavy VM work
- returns results, suspension requests, and retry requests back to the scheduler

This separation is why some latency counters are "scheduler" counters even though the task itself
is expensive. The scheduler may be fast, but the worker pool can still be saturated. Likewise, task
workers may be idle while the scheduler is delayed by coordination work.

## Database Concurrency Model

mooR's database layer is built around optimistic concurrency with serializable isolation.

The important operational points are:

- transactions read from a stable snapshot
- writes are accumulated in working sets
- commit validation checks whether concurrent changes invalidated the transaction
- conflicting transactions are retried rather than blocked behind one big global execution lock

This is a very different model from classic MOO implementations where execution and mutation were
implicitly serialized by a single-threaded runtime.

Operational consequences:

- read-heavy workloads can overlap well
- independent write workloads can also overlap until commit
- conflicting write workloads show up as retries, not just as longer wait times
- some apparent "latency" is actually retry pressure

If you are diagnosing a workload with many retries, look at the logical shape of the application as
well as the machine. Hot objects, shared counters, shared mailboxes, and other concentrated write
patterns can force serial progress even on a large machine.

## Commit and Durability Path

The write path is split into logical commit and durable flush stages.

At commit time, the database:

- validates the transaction against the current published root
- applies accepted mutations
- publishes the next root for readers
- hands the write batch off for durability work

That means the logical commit path and the durable writeback path are related but not identical.
The system can publish a new root and then rely on background infrastructure to push the queued
writes through the storage engine.

### Background Writers

mooR uses background writer infrastructure for some storage work, including a coalescing batch
writer in the Fjall-backed path.

That writer:

- receives committed write batches
- can deduplicate and coalesce pending writes
- flushes immediately under normal conditions for durability
- switches into more coalescing behavior under backpressure or slow flush conditions
- supports barrier-style synchronization when a caller needs to know a given timestamp is durable

This is important when reading performance counters:

- a fast logical commit does not always mean storage is idle
- backpressure in the batch writer can indicate the durable write path is the bottleneck
- slow barriers often mean the system is waiting for queued writes to drain

## Thread Placement and Affinity

mooR distinguishes between service/control-plane threads and task worker threads.

Service/control-plane threads include work such as:

- scheduler orchestration
- RPC/event handling
- background coordination

Task worker threads are the pool used to execute task bodies.

On systems with heterogeneous CPUs, mooR can try to reserve stronger cores for task execution while
leaving some performance-core capacity for scheduler and control-plane work. The relevant runtime
settings are documented in
[Server Configuration](server-configuration.md#task-pool-affinity-configuration).

Default behavior is:

- if the runtime detects a meaningful performance-core tier, task workers are pinned to the worker
  share of that tier
- a small number of performance cores are reserved for service/control-plane threads
- if no meaningful split is detected, the task pool is left unpinned

When affinity helps:

- the machine has a clear fast-core and efficiency-core split
- the worker pool is CPU-bound
- scheduler responsiveness matters under load

When affinity may hurt:

- the process runs in a container or VM with unusual CPU scheduling
- topology information is misleading or incomplete
- the workload is not CPU-bound and benefits more from general scheduler freedom

## Performance Counters

mooR exposes several families of internal counters through builtins:

- [`bf_counters()`](../the-moo-programming-language/built-in-functions/server.md#bf_counters)
- [`db_counters()`](../the-moo-programming-language/built-in-functions/server.md#db_counters)
- [`sched_counters()`](../the-moo-programming-language/built-in-functions/server.md#sched_counters)

These facilities are wizard-only. They are useful for operators and for world authors who have
administrative access, but they are not assumed to be available to ordinary programmers.

These return maps keyed by counter name, where each value is:

- invocation count
- cumulative duration in nanoseconds

Invocation counts are exact. Duration collection may be sampled, depending on runtime timing
configuration.

### Builtin Counters

Builtin counters measure builtin-function execution paths in the VM.

Use them when you want to understand:

- which builtins are called most often
- which builtins dominate cumulative execution time
- whether apparent "VM slowness" is really concentrated in a small set of builtins

These counters are often useful when application behavior is builtin-heavy rather than verb-heavy.

### Scheduler Counters

Scheduler counters cover orchestration and task-lifecycle work, including:

- task startup, resume, retry, and kill paths
- command parsing and verb lookup for command dispatch
- scheduler-client and task-scheduler-client round-trip latencies
- task wakeup and worker handoff timing
- garbage collection phases

These counters are useful for diagnosing:

- scheduler overload
- slow task dispatch
- wakeup delay
- task handoff latency
- task-list or checkpoint request overhead

Examples:

- High `task_submit_to_first_run_latency` suggests tasks spend time waiting before they ever get a
  worker.
- High `task_thread_handoff_latency` suggests worker-pool contention or delayed dispatch.
- High wakeup-related latencies suggest scheduler-side delay or a backlog of runnable work.

### Database Counters

Database counters cover world-state operations and the write path, including:

- object, property, and verb lookup/update operations
- provider tuple check/load paths
- commit phases such as lock wait, check, apply, and commit-result handling
- batch-writer backpressure timing

These counters are useful for diagnosing:

- expensive object/property/verb operations
- conflict-heavy workloads
- commit serialization pressure
- slow provider lock acquisition
- storage backpressure in the writer path

Examples:

- High commit lock-wait or check/apply time suggests commit-path contention.
- High provider lock-wait counters suggest the lower storage layer is contended.
- High batch-writer backpressure counters suggest durability work is falling behind foreground
  commit throughput.

## Sampling Semantics

mooR's perf counters are designed so they can remain enabled in normal operation.

By default:

- invocation counts remain exact
- many hot-path durations are sampled
- sampled durations are scaled so the cumulative totals remain useful as estimates

This is a practical tradeoff. Measuring every hot-path event exactly would distort the very paths
you are trying to observe.

The runtime timing settings in
[Server Configuration](server-configuration.md#runtime-timing-configuration) control whether timing
is enabled and how aggressively hot and medium paths are sampled.

Guidance for interpretation:

- trust invocation counts as exact
- treat cumulative duration as an estimate when sampling is enabled
- use exact timing only for focused benchmarking, profiling, or short investigation windows

## Common Tuning Scenarios

### Benchmarking or Profiling Runs

Use exact timing when you care more about measurement precision than about observer overhead.

Recommended approach:

- set perf timing sample shifts to `0`
- keep the workload otherwise as close to production as possible
- record whether affinity is enabled, since it changes worker placement

The exact settings and examples are in
[Server Configuration](server-configuration.md#runtime-timing-configuration).

### Scheduler Feels Sluggish

Look first at:

- scheduler latency counters
- wakeup and worker handoff counters
- whether too many performance cores were given to workers

Possible actions:

- increase `service_perf_cores`
- reduce task-worker pinning aggressiveness
- investigate workloads creating large numbers of short runnable tasks

For the affinity knobs themselves, see
[Server Configuration](server-configuration.md#task-pool-affinity-configuration).

### Good CPU Availability but Poor Write Throughput

Look first at:

- database commit-phase counters
- provider lock-wait counters
- batch-writer backpressure counters
- retry/conflict behavior in the workload

Possible actions:

- reduce hot write contention in application design
- inspect whether many tasks are writing the same objects or properties
- check whether the durable writer is coalescing under sustained pressure

### High Retry Pressure

Retries are not only a hardware problem. They usually mean application-level conflict.

Look for:

- many tasks touching the same objects
- concentrated mailbox or queue updates
- broad write transactions when narrower ones would suffice

More cores do not solve serializable-conflict pressure by themselves.

## What To Tune First

If you are not already operating with measurements, start here:

1. check scheduler, builtin, and DB counter maps
2. determine whether the problem is scheduler-side, worker-side, or DB-side
3. only then change affinity or timing settings

For most deployments, the defaults are the right place to start. The knobs are there to support
workload-specific tuning, not to require up-front hand tuning on day one.

## Related Reading

- [Server Configuration](server-configuration.md)
- [Server Architecture](server-architecture.md)
- [Server Assumptions About the Database](server-assumptions-about-the-database.md)
- [Controlling the Execution of Tasks](controlling-the-execution-of-tasks.md)
