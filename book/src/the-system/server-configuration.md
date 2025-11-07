# Server Configuration

This section describes the options available for configuring and running the `moor-daemon` server binary.

## Daemon, Hosts, Workers, and RPC

The `moor-daemon` server binary provides the main server functionality, including hosting the database, handling verb
executions, and scheduling tasks. However it does _not_ handle network connections directly. Instead, special helper
processes called _hosts_ manage incoming network connections and forward them to the daemon. Likewise, outbound network
connections (or future facilities like file access) are handled by _workers_ that communicate with the daemon to perform
those activities.

To run the server, you therefore need to run not just the `moor-daemon` binary, but also one or more "hosts" (and,
optionally "workers") that will connect to the daemon.

These processes communicate over ZeroMQ sockets, with the daemon listening for RPC requests and events, and the hosts
and workers connecting to those sockets to send requests and receive responses.

Hosts and workers can be run on the same machine as the daemon (the default) or distributed across multiple machines for
clustered deployments. They are stateless and can be restarted independently of the daemon, allowing for flexible
deployment and scaling.

## Transport Modes

For single-machine deployments (the default), components communicate via **IPC (Unix domain sockets)** which use
filesystem permissions for security and require no additional configuration.

For clustered/multi-machine deployments, components communicate via **TCP with CURVE encryption**. See
the [Clustered Deployment](clustered-deployment.md) guide for complete details on distributed deployments, security
considerations, and setup instructions.

## Authentication Keys

### PASETO Keys (Ed25519) - Client/Player Authentication

PASETO tokens authenticate **clients/players** (connecting users) using Ed25519 digital signatures. These are used *
*only by the daemon** to sign and verify player session tokens.

The daemon automatically generates these keys on first run when using the `--generate-keypair` flag:

```bash
# Keys are auto-generated on first run
moor-daemon --generate-keypair <other-args>
```

This creates `moor-signing-key.pem` (private key) and `moor-verifying-key.pem` (public key) in the moor config
directory (`${XDG_CONFIG_HOME:-$HOME/.config}/moor`).

Alternatively, you can pre-generate them using `openssl`:

```bash
openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem
```

**Note**: Hosts and workers do **not** need these PEM files - they are only used by the daemon for client
authentication.

## How to set server options

In general, all options can be set either by command line arguments or by configuration file. The same option cannot be
set by both methods at the same time, and if it is set by both, the command line argument takes precedence over the
configuration.

## Configuration File Format

The configuration file uses YAML format. You can specify the path to your configuration file using the `--config-file`
command-line argument. Configuration file values can be overridden by command-line arguments.

## General Server Options

These options control the basic server behavior:

- `--config-file <PATH>`: Path to configuration (YAML) file to use. If not specified, defaults are used.
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database
- `--public-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-verifying-key.pem`): PEM encoded PASETO public key for token verification
- `--private-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-signing-key.pem`): PEM encoded PASETO private key for token signing
- `--num-io-threads <NUM>` (default: `8`): Number of ZeroMQ IO threads
- `--debug` (default: `false`): Enable debug logging

### Transport Endpoint Configuration

These options configure how the daemon communicates with hosts and workers. The defaults use IPC (Unix domain sockets) for single-machine deployments. Change these to TCP addresses (e.g., `tcp://0.0.0.0:7899`) only for clustered deployments - see [Clustered Deployment](clustered-deployment.md) for details.

| Option | Default | Description |
|--------|---------|-------------|
| `--rpc-listen` | `ipc:///tmp/moor_rpc.sock` | RPC server address |
| `--events-listen` | `ipc:///tmp/moor_events.sock` | Events publisher address |
| `--workers-request-listen` | `ipc:///tmp/moor_workers_request.sock` | Workers request pub-sub address |
| `--workers-response-listen` | `ipc:///tmp/moor_workers_response.sock` | Workers response RPC address |

### Enrollment Configuration (Clustered Deployments Only)

These options are only needed for clustered deployments with TCP transport. See [Clustered Deployment](clustered-deployment.md) for complete setup instructions.

| Option | Default | Description |
|--------|---------|-------------|
| `--enrollment-listen` | `tcp://0.0.0.0:7900` | Enrollment endpoint for host/worker registration |
| `--enrollment-token-file` | `${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token` | Path to enrollment token file |

## Database Configuration

- `<PATH>` (positional argument): Path to the database directory
- `--db <NAME>` (default: `world.db`): Name of the main database within the directory
- `--connections-file <PATH>` (default: `connections.db`): Path to connections database (relative to data directory if not absolute)
- `--tasks-db <PATH>` (default: `tasks.db`): Path to persistent tasks database (relative to data directory if not absolute)
- `--events-db <PATH>` (default: `events.db`): Path to persistent events database (relative to data directory if not absolute)

The first positional argument specifies the database directory (typically `moor-data` or similar). The daemon stores several databases within this directory by default:

- `world.db/` (or name specified by `--db`) - The main MOO database
- `connections.db` - Connection state database
- `tasks.db` - Persistent tasks database
- `events.db` - Event logging database (if event logging is enabled)

All database paths can be customized and are relative to the data directory unless specified as absolute paths.

## Language Features Configuration

These options enable or disable various MOO language features:

| Feature             | Command Line                | Default | Description                                                                      |
|---------------------|-----------------------------|---------|----------------------------------------------------------------------------------|
| Rich notify         | `--rich-notify`             | `true`  | Allow notify() to send arbitrary MOO values to players                           |
| Lexical scopes      | `--lexical-scopes`          | `true`  | Enable block-level lexical scoping with begin/end syntax and let/global keywords |
| Type dispatch       | `--type-dispatch`           | `true`  | Enable primitive-type verb dispatching (e.g., "test":reverse())                  |
| Flyweight type      | `--flyweight-type`          | `true`  | Enable flyweight types (lightweight object delegates)                            |
| Boolean type        | `--bool-type`               | `true`  | Enable boolean true/false literals                                               |
| Boolean returns     | `--use-boolean-returns`     | `false` | Make builtins return boolean types instead of integers 0/1                       |
| Symbol type         | `--symbol-type`             | `true`  | Enable symbol literals                                                           |
| Custom errors       | `--custom-errors`           | `false` | Enable error symbols beyond standard builtin set                                 |
| Symbols in builtins | `--use-symbols-in-builtins` | `false` | Use symbols instead of strings in builtins                                       |
| List comprehensions | `--list-comprehensions`     | `true`  | Enable list/range comprehensions                                                 |
| Persistent tasks    | `--persistent-tasks`        | `true`  | Enable persistent tasks between server restarts                                  |
| Event logging       | `--enable-eventlog`         | `true`  | Enable persistent event logging and history features                             |
| Anonymous objects   | `--anonymous-objects`       | `false` | Enable anonymous objects with automatic garbage collection                       |
| UUID objects        | `--use-uuobjids`            | `false` | Enable UUID object identifiers like #048D05-1234567890                           |

## Import/Export Configuration

These options control database import and export functionality:

- `--import <PATH>`: Path to a textdump or objdef directory to import
- `--export <PATH>`: Path to a textdump or objdef directory to export into
- `--import-format <FORMAT>` (default: `Textdump`): Format to import from (Textdump or Objdef)
- `--export-format <FORMAT>` (default: `Objdef`): Format to export into (Textdump or Objdef)
- `--checkpoint-interval-seconds <SECONDS>`: Interval between database checkpoints
- `--textdump-output-encoding <ENCODING>`: Encoding for textdump files (utf8 or iso8859-1)
- `--textdump-version-override <STRING>`: Version string override for textdump

## Example Configuration

Here's an example configuration file:

```yaml
# Database configuration
database_config:
  cache_eviction_interval: 300
  default_eviction_threshold: 100000000

# Language features configuration
features_config:
  persistent_tasks: true
  rich_notify: true
  lexical_scopes: true
  bool_type: true
  symbol_type: true
  type_dispatch: true
  flyweight_type: true
  list_comprehensions: true
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: true
  anonymous_objects: true

# Import/export configuration
import_export_config:
  output_encoding: "UTF8"
  checkpoint_interval: "60s"
  export_format: "Objdef"
```

## LambdaMOO Compatibility Mode

If you need to maintain compatibility with LambdaMOO 1.8, you'll need to either update your core with the changes
provided in the [Lambda-moor core](https://codeberg.org/timbran/moor/src/branch/main/cores/lambda-moor/README.md) or
disable several features. Here's a configuration that maintains LambdaMOO compatibility by disabling mooR features:

```yaml
# LambdaMOO 1.8 compatible features
features_config:
  persistent_tasks: true
  rich_notify: false
  lexical_scopes: false
  bool_type: false
  symbol_type: false
  type_dispatch: false
  flyweight_type: false
  list_comprehensions: false
  use_boolean_returns: false
  use_symbols_in_builtins: false
  custom_errors: false
  enable_eventlog: true
  use_uuobjids: false
  anonymous_objects: false

# LambdaMOO compatible import/export
import_export_config:
  output_encoding: "ISO8859_1"
```

## Anonymous Objects Configuration

The `anonymous_objects` feature flag enables a new type of object that is automatically garbage collected when no longer
referenced.
This feature is disabled by default due to performance considerations.

### Enabling Anonymous Objects

To enable anonymous objects, set the flag in your configuration file:

```yaml
features_config:
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
- **Concurrency**: Mark phase runs concurrently with normal server operations to minimize blocking but can put load
  on the system as it scans the entire database.
- **Collection Pauses**: Sweep phase can cause brief server pauses during collection cycles

The garbage collector is optimized but will impact overall server performance. Monitor your server's CPU usage and
response times when enabling this feature.

### Migration Considerations

When enabling anonymous objects on an existing MOO:

- Existing code using `create(parent, owner, 1)` will begin creating anonymous objects
- No changes needed to existing numbered or UUID object code
- Consider updating builder documentation to explain the new object type option
- Test performance impact during peak usage periods before enabling permanently
