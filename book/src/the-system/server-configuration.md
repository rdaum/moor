# Server Configuration

This section describes the options available for configuring and running the mooR server.

For most deployments, the combined `moor` binary handles everything — database, verb execution,
telnet connections, web API, and outbound HTTP — in one process. The configuration options below
apply to that binary unless noted otherwise. Options specific to split-process or clustered
deployment are called out in their own sections.

For a deeper discussion of mooR's threading model, database concurrency model, performance counters,
and tuning guidance, see [Performance and Concurrency](performance-and-concurrency.md).

## Single-Process Configuration

The `moor` binary is the default way to run the server. It bundles the daemon, telnet host, web
host, and optional HTTP and Git worker services into one process. No worker sockets or enrollment
are needed. This is the closest analogue to running a traditional LambdaMOO or ToastStunt server.

To start the server:

```bash
moor /path/to/database --db development.db --generate-keypair
```

The same configuration file and most command-line options described in this page apply to the
combined `moor` binary. The transport endpoint and enrollment options (described below) are not
needed in single-process mode — they only apply when running components as separate processes.

Enable outbound HTTP with `--enable-curl-worker` and read-only Git access with
`--enable-git-worker`. Git requests run in isolated child processes of the same `moor` executable.
They use the same protocol and resource limits as the standalone Git worker. No Git executable is
needed for worker requests. Import preparation with `--git-upstream` separately requires Git and
local repository history.

You can configure the Git service in YAML:

```yaml
services:
  git_worker:
    enabled: true
    work_dir: git-jobs
    max_concurrent_requests: 4
    limits:
      max_seconds: 30
      max_entries: 4096
      max_file_bytes: 4194304
      max_total_bytes: 16777216
      max_fetch_bytes: 268435456
      max_memory_bytes: 1073741824
```

Relative job directories resolve under the server data directory. Job directories are temporary and
removed when each request finishes. Limits apply to individual requests; excess concurrent work
receives a busy response. Invalid limits or inaccessible job storage prevent startup before database
import. Shutdown terminates active Git child processes before the worker detaches.

## PostgreSQL requirements

The optional `postgres` Cargo feature requires a thread-safe libpq 16 or newer. Supported servers
are PostgreSQL 16, 17, and 18. The client and server major versions can differ. Default builds use
Fjall and do not require libpq.

On Debian or Ubuntu, install the build dependency with:

```bash
sudo bash scripts/install-libpq.sh build
```

The installer retains compatible installed libraries. If development headers are absent, it uses
compatible distribution packages. It adds the PostgreSQL package repository only when necessary. For
runtime images, use `runtime` instead of `build`. PostgreSQL package variants depend on
`libpq5 (>= 16)`.

Exact client versions are for compatibility tests. For example, this command can downgrade an
existing client installation:

```bash
sudo bash scripts/install-libpq.sh build "$(dpkg --print-architecture)" 16
```

The adapter checks `PQisthreadsafe()` before connection setup. Each connection belongs to one worker
thread. The connection options include `require_auth`, which requires libpq 16. The adapter uses
nonblocking connections, parameterized queries, and single-row results. It does not require the
cancellation or chunk APIs added in later versions. See the PostgreSQL documentation for
[thread safety](https://www.postgresql.org/docs/16/libpq-threading.html) and
[connection options](https://www.postgresql.org/docs/16/libpq-connect.html).

The schema uses domains, JSONB, advisory locks, and conflict handling supported by PostgreSQL 16.
The compatibility suite exercises schema creation, startup, prepared writes, recovery, and both SQL
commit policies.

For a disposable local fixture with TLS and SCRAM authentication, run:

```bash
scripts/test-postgres-adapter.sh 16 native
```

Set `PG_BIN` if the server tools are outside `/usr/lib/postgresql/16/bin`. The fixture creates a
private temporary cluster and removes it after the tests. It also tests server crashes. It does not
use the Cowbell cluster. Use `17` or `18` to test those server versions. Omit `native` for Docker.
Set `MOOR_PG_TEST_CLI=1` to include builds and configuration checks for all four storage-aware
tools. Set `LD_LIBRARY_PATH` to test a separately installed libpq runtime.

## Daemon, Hosts, Workers, and RPC (Advanced)

For split-process or clustered deployment, the server is broken into separate binaries:

The `moor-daemon` binary provides the core server functionality — hosting the database, handling
verb executions, and scheduling tasks. It does _not_ handle network connections directly. Special
helper processes called _hosts_ manage incoming network connections and forward them to the daemon.
Likewise, outbound network connections (or future facilities like file access) are handled by
_workers_ that communicate with the daemon to perform those activities.

To run a split-process deployment, you therefore need to run not just the `moor-daemon` binary, but
also one or more "hosts" (and, optionally "workers") that will connect to the daemon.

These processes communicate over ZeroMQ sockets, with the daemon listening for RPC requests and
events, and the hosts and workers connecting to those sockets to send requests and receive
responses.

Hosts and workers can be run on the same machine as the daemon (using IPC) or distributed across
multiple machines for clustered deployments (using TCP with CURVE encryption). They are stateless
and can be restarted independently of the daemon, allowing for flexible deployment and scaling.

See [Clustered Deployment](clustered-deployment.md) for complete details on split-process and
multi-machine setups.

## Transport Modes

For single-process deployment (the default), all components run inside the `moor` binary and no
transport configuration is needed.

For split-process same-machine deployments, components run as separate processes and communicate via
**IPC (Unix domain sockets)**, which use filesystem permissions for security and require no
additional configuration.

For clustered/multi-machine deployments, components communicate via **TCP with CURVE encryption**.
See the [Clustered Deployment](clustered-deployment.md) guide for complete details on distributed
deployments, security considerations, and setup instructions.

## Authentication Keys

### PASETO Keys (Ed25519) - Client/Player Authentication

PASETO tokens authenticate **clients/players** (connecting users) using Ed25519 digital signatures.
These are used _only by the daemon_ to sign and verify player session tokens.

The daemon automatically generates these keys on first run when using the `--generate-keypair` flag:

```bash
# Keys are auto-generated on first run
moor --generate-keypair <other-args>
```

This creates `moor-signing-key.pem` (private key) and `moor-verifying-key.pem` (public key) in the
moor config directory (`${XDG_CONFIG_HOME:-$HOME/.config}/moor`).

Alternatively, you can pre-generate them using `openssl`:

```bash
openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem
```

**Note**: Hosts and workers do **not** need these PEM files - they are only used by the daemon for
client authentication.

## How to set server options

In general, all options can be set either by command line arguments or by configuration file. The
same option cannot be set by both methods at the same time, and if it is set by both, the command
line argument takes precedence over the configuration.

## Configuration File Format

The configuration file uses YAML format. You can specify the path to your configuration file using
the `--config-file` command-line argument. Configuration file values can be overridden by
command-line arguments.

## General Server Options

These options control the basic server behavior:

- `--config-file <PATH>`: Path to configuration (YAML) file to use. If not specified, defaults are
  used.
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database
- `--public-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-verifying-key.pem`):
  PEM encoded PASETO public key for token verification
- `--private-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-signing-key.pem`):
  PEM encoded PASETO private key for token signing
- `--num-io-threads <NUM>` (default: `8`): Number of ZeroMQ IO threads
- `--debug` (default: `false`): Enable debug logging

### Transport Endpoint Configuration (Split-Process Only)

These options configure how the daemon communicates with hosts and workers when running as separate
processes. They do not apply to the combined `moor` binary, which uses in-process endpoints. The
defaults use IPC (Unix domain sockets) for same-machine split-process deployments. Change these to
TCP addresses (e.g., `tcp://0.0.0.0:7899`) only for clustered deployments - see
[Clustered Deployment](clustered-deployment.md) for details.

| Option                      | Default                                 | Description                     |
| --------------------------- | --------------------------------------- | ------------------------------- |
| `--rpc-listen`              | `ipc:///tmp/moor_rpc.sock`              | RPC server address              |
| `--events-listen`           | `ipc:///tmp/moor_events.sock`           | Events publisher address        |
| `--workers-request-listen`  | `ipc:///tmp/moor_workers_request.sock`  | Workers request pub-sub address |
| `--workers-response-listen` | `ipc:///tmp/moor_workers_response.sock` | Workers response RPC address    |

### Enrollment Configuration (Clustered Deployments Only)

These options are only needed for clustered deployments with TCP transport. See
[Clustered Deployment](clustered-deployment.md) for complete setup instructions.

| Option                    | Default                                                   | Description                                      |
| ------------------------- | --------------------------------------------------------- | ------------------------------------------------ |
| `--enrollment-listen`     | `tcp://0.0.0.0:7900`                                      | Enrollment endpoint for host/worker registration |
| `--enrollment-token-file` | `${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token` | Path to enrollment token file                    |

## Database Configuration

- `<PATH>` (positional argument): Path to the database directory
- `--db <NAME>` (default: `world.db`): Name of the main database within the directory
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database (relative to
  data directory if not absolute)
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database (relative to data
  directory if not absolute)
- `--events-db <PATH>` (default: `events.db`): Path to persistent events database (relative to data
  directory if not absolute)

The first positional argument specifies the database directory (typically `moor-data` or similar).
The daemon stores several databases within this directory by default:

- `world.db/` (or name specified by `--db`) - The main MOO database
- `connections.db` - Connection state database
- `tasks.db` - Persistent tasks database
- `events.db` - Event logging database (if event logging is enabled)

All database paths can be customized and are relative to the data directory unless specified as
absolute paths.

## Language Features Configuration

These options enable or disable runtime and language features. Lexical scopes and list/range
comprehensions are always enabled; older config files may still contain those keys, but they are
ignored.

| Feature             | Command Line                | Default | Description                                                     |
| ------------------- | --------------------------- | ------- | --------------------------------------------------------------- |
| Rich notify         | `--rich-notify`             | `true`  | Allow notify() to send arbitrary MOO values to players          |
| Type dispatch       | `--type-dispatch`           | `true`  | Enable primitive-type verb dispatching (e.g., "test":reverse()) |
| Flyweight type      | `--flyweight-type`          | `true`  | Enable flyweight types (lightweight object delegates)           |
| Boolean type        | `--bool-type`               | `true`  | Enable boolean true/false literals                              |
| Boolean returns     | `--use-boolean-returns`     | `false` | Make builtins return boolean types instead of integers 0/1      |
| Symbol type         | `--symbol-type`             | `true`  | Enable symbol literals                                          |
| Custom errors       | `--custom-errors`           | `false` | Enable error symbols beyond standard builtin set                |
| Symbols in builtins | `--use-symbols-in-builtins` | `false` | Use symbols instead of strings in builtins                      |
| Persistent tasks    | `--persistent-tasks`        | `true`  | Enable persistent tasks between server restarts                 |
| Event logging       | `--enable-eventlog`         | `true`  | Enable persistent event logging and history features            |
| Anonymous objects   | `--anonymous-objects`       | `false` | Enable anonymous objects with automatic garbage collection      |
| UUID objects        | `--use-uuobjids`            | `false` | Enable UUID object identifiers like #048D05-1234567890          |

## Import/Export Configuration

These options control database import and checkpoint export functionality:

- `--import <PATH>`: Path to a textdump or objdef directory to import
- `--export <PATH>`: Path to export checkpoints into (always uses objdef format)
- `--import-format <FORMAT>` (default: `Textdump`): Format to import from (Textdump or Objdef)
- `--checkpoint-interval-seconds <SECONDS>`: Interval between database checkpoints

`--import` is used only when creating a new database. If the requested database already exists, mooR
opens it and skips the import. The import directory is not watched for later source changes. See
[Starting a MOO from Objdef Source](bootstrapping-from-source.md) for the full lifecycle.

## Runtime Timing Configuration

These options control latency duration sampling for internal performance counters. Invocation counts
remain exact; these settings only affect duration collection.

These counters are used to observe hot runtime paths such as scheduler wakeups, lock waits, database
commit stages, builtin execution, and other VM execution activity. In the normal server
configuration, the system does not record a full timestamp pair for every hot-path event. Instead,
it samples durations and scales the totals back up. This keeps the counters cheap enough to leave
enabled in regular use.

In practice, these settings are mostly useful in three situations:

- You are benchmarking and want exact latency measurements rather than sampled estimates.
- You are chasing a performance regression and want denser timing data from hot paths.
- You want to reduce timing overhead further and are willing to trade away duration fidelity.

| Setting               | Command Line                         | Default | Description                                                   |
| --------------------- | ------------------------------------ | ------- | ------------------------------------------------------------- |
| Perf timing enabled   | `--perf-timing-enabled <BOOL>`       | `true`  | Enable or disable latency duration collection globally        |
| Hot-path sample shift | `--perf-timing-hot-path-shift <NUM>` | `6`     | Sampling shift for hot paths. `0` means exact, `6` means 1/64 |

In YAML, set these under `runtime:`:

```yaml
runtime:
  gc_interval: "30s"
  scheduler_tick_duration: "10ms"
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 6
```

For exact timing during benchmarking or profiling runs:

```yaml
runtime:
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 0
```

To disable duration timing entirely while keeping invocation counters:

```yaml
runtime:
  perf_timing_enabled: false
```

Guidance:

- Leave the defaults alone for normal deployments. They are intended to keep timing overhead low
  while still producing useful long-run aggregates.
- Use `0` for the sample shift during focused benchmarking or profiling runs where exact timing is
  more important than hot-path overhead.
- Set `perf_timing_enabled: false` if you only care about invocation counts and do not want duration
  timing at all.

## Task Pool Affinity Configuration

Task worker affinity is configured under `runtime:` and can also be overridden on the command line.

The daemon has two broad thread classes:

- service or control-plane threads, such as the scheduler, RPC/event handling, and coordination work
- task worker threads, which execute verbs and other task bodies in the task pool

This is an important architectural difference from LambdaMOO-style servers. In LambdaMOO, task
execution is effectively serialized through one main execution path. In moor, runnable tasks are
dispatched onto a worker pool so independent task execution can proceed concurrently across multiple
cores. The scheduler remains responsible for orchestration, wakeups, and queue management, while the
task pool provides the actual parallel execution capacity.

That means thread placement matters more here than in a single-threaded MOO. A poor affinity choice
can leave the scheduler contending with task execution on the same high-performance cores, while an
appropriate split can preserve both throughput and responsiveness.

On systems with heterogeneous CPUs, especially recent x86 and ARM systems, not all cores are equal.
Some cores are tuned for throughput and sustained performance, while others are tuned for efficiency
or background work. The affinity settings let the daemon reserve stronger cores for task execution
while leaving some capacity for the scheduler and other control-plane work.

If the runtime can identify a distinct performance-core tier, the default `auto` mode tries to use
that tier for task workers. If it cannot identify a meaningful split, the task pool is left
unpinned.

| Setting                     | Command Line                 | Default        | Description                                                                                       |
| --------------------------- | ---------------------------- | -------------- | ------------------------------------------------------------------------------------------------- |
| Task pool pinning           | `--task-pool-pinning <MODE>` | `auto`         | Controls whether task worker threads are pinned to detected performance cores                     |
| Reserved service perf cores | `--service-perf-cores <NUM>` | topology-based | Reserves detected performance cores for non-task service threads before assigning worker affinity |

`task_pool_pinning` accepts:

- `auto`: Use the runtime's default policy.
- `performance`: Pin task workers to detected performance cores when available.
- `none`: Do not pin task worker threads.

`service_perf_cores` must be a non-negative integer. It reserves that many detected performance
cores for service threads. The value is clamped so that, when possible, at least one performance
core remains available for task workers.

When `service_perf_cores` is not set, the reservation defaults are:

- `0` for systems with `0..=2` detected performance cores
- `1` for systems with `3..=7` detected performance cores
- `2` for systems with `8+` detected performance cores

Examples:

```yaml
runtime:
  task_pool_pinning: performance
  service_perf_cores: 2
```

```bash
# Force performance-core pinning for task workers
moor-daemon --task-pool-pinning performance ...

# Reserve two detected performance cores for scheduler / control-plane work
moor-daemon --service-perf-cores 2 ...

# Disable task-worker pinning
moor-daemon --task-pool-pinning none ...
```

Guidance:

- Leave this on `auto` unless you have measured a reason to override it.
- `performance` is useful when you know the machine has a meaningful fast-core tier and you want
  task execution to stay there even if automatic detection would otherwise fall back.
- `none` is useful inside containers, VMs, or unusual schedulers where explicit pinning hurts more
  than it helps.
- Increase `service_perf_cores` if the scheduler, RPC handling, or other daemon-side coordination
  work becomes a bottleneck while worker threads are saturating the faster cores.
- Decrease `service_perf_cores` if the machine has only a few performance cores and you want to
  maximize task execution throughput.

## Captured Verb Call Configuration

Some clients ask the daemon to run a verb and hand back its return value together with the narrative
output the call produced, instead of streaming that output to a connection. The web host's verb
endpoint and the welcome-message endpoint both work this way. Those calls occupy an RPC worker while
they wait, so the daemon puts an upper bound on how long a caller may ask it to wait.

| Setting              | Command Line                            | Default | Description                                                    |
| -------------------- | --------------------------------------- | ------- | -------------------------------------------------------------- |
| Max capture deadline | `--max-capture-deadline-seconds <SECS>` | `60`    | Longest deadline a client may request for a captured verb call |

A client may request a shorter deadline, but not a longer one: a request above the maximum is
refused rather than quietly shortened, so the client's own receive timeout still matches what it
asked for. A deadline of zero is not valid. When the deadline passes, the task is cancelled and the
caller gets a time-limit task error.

```yaml
runtime:
  max_capture_deadline: "30s"
```

```bash
moor-daemon --max-capture-deadline-seconds 30 ...
```

## Example Configuration

Here's an example configuration file:

```yaml
# Database configuration
database:
  object_verbs:
    max_memtable_size: 536870912

# Language features configuration
features:
  persistent_tasks: true
  rich_notify: true
  bool_type: true
  symbol_type: true
  type_dispatch: true
  flyweight_type: true
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: true
  anonymous_objects: true

# Import/export configuration
import_export:
  checkpoint_interval: "60s"

# Runtime timing configuration
runtime:
  perf_timing_enabled: true
  perf_timing_hot_path_shift: 6
  task_pool_pinning: auto
  service_perf_cores: 1
```

## LambdaMOO Compatibility Mode

If you need to import LambdaMOO 1.8 content, use the
[Lambda-moor core](https://github.com/timbran-project/moor/tree/main/cores/lambda-moor) or disable
the remaining compatibility-sensitive features. Lexical scopes and list/range comprehensions are
part of the language now and cannot be disabled.

```yaml
# LambdaMOO 1.8 compatible features
features:
  persistent_tasks: true
  rich_notify: false
  bool_type: false
  symbol_type: false
  type_dispatch: false
  flyweight_type: false
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: false
  anonymous_objects: false

# LambdaMOO textdump import is supported
# Checkpoints always export in objdef format
```

## Anonymous Objects Configuration

The `anonymous_objects` feature flag enables a new type of object that is automatically garbage
collected when no longer referenced. This feature is disabled by default due to performance
considerations.

### Enabling Anonymous Objects

To enable anonymous objects, set the flag in your configuration file:

```yaml
features:
  anonymous_objects: true
```

Or use the command line flag: `--anonymous-objects`

### When to Enable Anonymous Objects

**Consider enabling if:**

- Your MOO creates many temporary objects (game pieces, UI elements, etc.)
- You have developers who struggle with manual object cleanup
- You want to reduce the burden of object lifecycle management
- Your server has sufficient CPU resources for garbage collection overhead

**Keep disabled if:**

- Your MOO has strict performance requirements with minimal latency tolerance
- Your builders are experienced with manual object lifecycle management
- Your server runs on resource-constrained hardware
- You need maximum predictable performance without GC pauses

### Performance Implications

Anonymous objects use a mark-and-sweep garbage collector with the following characteristics:

- **CPU Overhead**: The GC thread runs continuously, consuming CPU cycles even when not collecting
- **Memory Usage**: Same storage costs as regular objects until collection occurs
- **Concurrency**: Mark phase runs concurrently with normal server operations to minimize blocking
  but can put load on the system as it scans the entire database.
- **Collection Pauses**: Sweep phase can cause brief server pauses during collection cycles

The garbage collector is optimized but will impact overall server performance. Monitor your server's
CPU usage and response times when enabling this feature.

### Migration Considerations

When enabling anonymous objects on an existing MOO:

- Existing code using `create(parent, owner, 1)` will begin creating anonymous objects
- No changes needed to existing numbered or UUID object code
- Consider updating builder documentation to explain the new object type option
- Test performance impact during peak usage periods before enabling permanently

## Git worker

`moor-git-worker` provides read-only Git repository access through `worker_request('git, ...)`. It
runs as a separate process and uses the existing worker sockets and CURVE enrollment. The daemon
does not need Git installed. The worker uses the Rust `gix` library.

Build and start the worker with the same socket addresses as the daemon:

```bash
cargo build -p moor-git-worker
./target/debug/moor-git-worker --work-dir=/tmp/moor-git-jobs
```

The default addresses match the daemon's default IPC addresses. Use `--help` for connection options.
For TCP connections, use the standard host enrollment options and a separate worker identity
directory. The `deploy/clustered/web-basic` deployment includes an optional `git-worker` profile:

```bash
COMPOSE_PROFILES=git-worker ./start.sh
```

The worker accepts public HTTP and HTTPS repository URLs. It rejects embedded credentials, URL query
strings, fragments, local paths, and other transports. It does not follow HTTP redirects. Use the
repository's final URL. Private repository credentials and persistent repository caches are not
supported in this version.

Each request uses a disposable bare repository. The worker does not check out files, run repository
hooks, follow symlinks, fetch submodules, or apply content filters. Job processes have an empty
inherited environment and isolated Git configuration. A request cannot select a host filesystem
path. The worker uses Linux resource limits for each job. An operator can also limit the worker's
network access and temporary storage through its container or service configuration.

### Git request protocol

Requests contain an operation and a schema-1 map. Field names and operation names accept strings or
symbols. Unknown fields and duplicate normalized field names are errors.

```moo
response = worker_request('git, {'snapshot, [
    "schema" -> 1,
    "repository" -> "https://github.com/timbran-project/moor.git",
    "revision" -> ["ref" -> "refs/heads/main"],
    "path" -> "cores/cowbell/src"
]}, ["timeout_seconds" -> 30.0]);
```

The caller needs wizard authority or an explicit grant for `worker_request()`. The worker does not
interpret the returned files or write to the world database.

| Operation      | Request fields, in addition to `schema`                          | Result fields                                                                                                      |
| -------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `capabilities` | None                                                             | `operations`, `transports`, `object_formats`, `limits`, `max_concurrent_requests`, `content_type`, `path_encoding` |
| `refs`         | `repository`, optional `limits`                                  | `refs`                                                                                                             |
| `tree`         | `repository`, `revision`, optional `path`, `recursive`, `limits` | `commit`, `tree`, `path`, `entries`                                                                                |
| `read`         | `repository`, `revision`, `path`, optional `limits`              | `commit`, `entry`                                                                                                  |
| `snapshot`     | `repository`, `revision`, optional `path`, `limits`              | `commit`, `tree`, `path`, `entries`                                                                                |

A revision contains exactly one full ref name or full commit ID:

```moo
["ref" -> "refs/tags/v1.2.3"]
["commit" -> "sha1:0123456789abcdef0123456789abcdef01234567"]
```

Schema 1 initially supports SHA-1 repositories. Annotated tags resolve to commits. Branch and tag
short names, abbreviated IDs, and revision expressions are rejected. A requested commit must be
available from the remote. Some servers restrict fetching commits that no advertised ref names. A
failed commit fetch never falls back to the current branch head.

The worker resolves the revision once for each request. All returned entries come from that commit.
Use the returned commit ID in later requests to keep the same revision when a branch moves. `refs`
lists advertised branches and tags without fetching their object contents. Each ref has a `name` and
`oid`. Annotated tags also have a `peeled` ID when the server provides it.

Paths are UTF-8 strings, relative to the repository root. An empty path selects the root directory.
Absolute paths, empty components, dot segments, backslashes, and NUL bytes are rejected. A
repository entry with an unsupported name fails the listing instead of receiving a lossy replacement
name.

`tree` lists immediate children by default. Set `recursive` to `true` to include descendants.
`snapshot` always includes descendants. Returned listing paths are relative to the selected
directory and sorted by their UTF-8 bytes. A `read` entry retains the requested repository-relative
path.

Every entry has `path`, `kind`, and an algorithm-prefixed `oid`:

| Kind        | Additional fields                        | Behavior                                                                |
| ----------- | ---------------------------------------- | ----------------------------------------------------------------------- |
| `file`      | `size`, `executable`, optional `content` | Exact blob bytes and executable mode.                                   |
| `symlink`   | `size`, optional `content`               | Link-target bytes. The target is never followed.                        |
| `directory` | None                                     | The `oid` identifies the directory tree.                                |
| `submodule` | None                                     | The `oid` identifies the recorded submodule commit. No recursive fetch. |

`read` and `snapshot` include `content` for files and symlinks. Content is a MOO binary value.
`tree` returns metadata only. The worker does not decode text, change line endings, or expand Git
LFS pointers. Git LFS pointer files remain ordinary file contents.

### Results, errors, and limits

Successful responses have this envelope:

```moo
["schema" -> 1, "ok" -> true, "result" -> result_map]
```

Application failures have this envelope:

```moo
["schema" -> 1, "ok" -> false,
 "error" -> ["code" -> "path_not_found", "message" -> "Requested path does not exist."]]
```

Callers should branch on `code`, not the message. Codes include `invalid_request`,
`unsupported_schema`, `unsupported_operation`, `unsupported_transport`, `revision_not_found`,
`path_not_found`, `wrong_object_type`, `unsupported_path`, `authentication_required`,
`fetch_failed`, `invalid_repository`, `limit_exceeded`, `busy`, and `job_failed`. `job_failed`
includes child process failures such as memory exhaustion. Unavailable workers, disconnection,
request deadlines, and worker infrastructure failures use the existing raised `worker_request()`
errors.

| Worker option               | Default    | Meaning                                                                                   |
| --------------------------- | ---------- | ----------------------------------------------------------------------------------------- |
| `--max-concurrent-requests` | 4          | Active Git jobs. Further requests return `busy`.                                          |
| `--max-entries`             | 4096       | Entries or refs in one result, including directories.                                     |
| `--max-file-bytes`          | 4194304    | Bytes in one returned file or symlink.                                                    |
| `--max-total-bytes`         | 16777216   | Combined returned file and symlink contents.                                              |
| `--max-fetch-bytes`         | 268435456  | Git HTTP response headers and bodies across the request. Also limits each temporary file. |
| `--max-memory-bytes`        | 1073741824 | Virtual address space for each job process.                                               |
| `--max-seconds`             | 30         | Maximum job duration, including connection and download.                                  |

A request can lower `max_entries`, `max_file_bytes`, and `max_total_bytes` through its `limits` map.
It cannot raise the operator's limits. Metadata has a separate bounded response allowance. The
effective deadline is the smaller of `timeout_seconds` and `--max-seconds`.

A successful snapshot is complete for the selected directory. Exceeding a limit fails the whole
request, without a partial result. Deadlines terminate and reap the child before releasing its slot.
Normal completion, failure, and graceful shutdown remove temporary repositories. After an abrupt
worker or machine termination, an operator can remove leftover `job-*` directories while the worker
is stopped. These directories contain no review state and are not required by later requests.
