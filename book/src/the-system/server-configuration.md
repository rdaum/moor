# Server Configuration

This section describes the options available for configuring and running the `moor-daemon` server binary.

## Daemon, Hosts, Workers, and RPC

The `moor-daemon` server binary provides the main server functionality, including hosting the database, handling verb
executions, and scheduling tasks. However it does _not_ handle network connections directly. Instead, special helper
processes called _hosts_ manage incoming network connections and forward them to the daemon. Likewise, outbound network
connections (or future facilities like file access) are handled by _workers_ that communicate with the daemon to perform
those activities.

To run the server, you therefore need to run not just the `moor-daemon` binary, but also one or more "hosts" (and,
optionally "workers")
that will connect to the daemon.

These processes communicate over ZeroMQ sockets, with the daemon listening for RPC requests and events, and the hosts
and workers connecting to those sockets to send requests and receive responses.

Hosts and workers do *not* need to be run on the same machine as the daemon, and can be distributed across multiple
machines
or processes. They are stateless and can be clustered for high availability and load balancing. They can also be
restarted
independently of the daemon, allowing for flexible deployment and scaling, including live upgrades of the daemon without
restarting running connections.

When located on the same machine, the default addresses for the daemon's RPC and events sockets use IPC (Inter-Process
Communication) endpoints (e.g., `ipc:///tmp/moor_rpc.sock`), which are fast and efficient Unix domain sockets that
rely on filesystem permissions for security.

When running in a distributed environment across multiple machines, you must use TCP endpoints (e.g.,
`tcp://0.0.0.0:7899`). TCP endpoints automatically enable CURVE encryption to protect communication over the network.

Examples of running the daemon and hosts using TCP connections can be found in the `docker-compose.yml` file in the
`moor` repository.

## Encryption and Authentication Keys

The mooR system uses two separate key systems for security:

### 1. PASETO Authentication Keys (Ed25519) - Client/Player Authentication

PASETO tokens authenticate **clients/players** (connecting users) using Ed25519 digital signatures. These are used **only by the daemon** to sign and verify player session tokens.

The daemon automatically generates these keys on first run when using the `--generate-keypair` flag:

```bash
# Keys are auto-generated on first run
moor-daemon --generate-keypair <other-args>
```

This creates `moor-signing-key.pem` (private key) and `moor-verifying-key.pem` (public key) in the moor config directory (`${XDG_CONFIG_HOME:-$HOME/.config}/moor`).

Alternatively, you can pre-generate them using `openssl`:

```bash
openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem
```

**Note**: Hosts and workers do **not** need these PEM files - they are only used by the daemon for client authentication.

### 2. CURVE Transport Encryption (Curve25519) - Daemon↔Host/Worker Transport

When using **TCP endpoints**, all ZeroMQ communication between the daemon and hosts/workers is encrypted using CurveZMQ (Curve25519 elliptic curve cryptography). When using **IPC endpoints** (local Unix domain sockets), CURVE encryption is **disabled** since communication never leaves the local machine and is protected by filesystem permissions.

#### TCP Mode (Encrypted)

For distributed deployments using TCP endpoints:

- **Automatic Key Generation**: The daemon generates a CURVE keypair on first run (stored as Z85-encoded text)
- **Enrollment Process**: Hosts/workers must enroll before connecting:
  1. Generate their own CURVE keypair on first run
  2. Connect to the enrollment endpoint (default: `tcp://0.0.0.0:7900`) with an enrollment token
  3. Register their service type, hostname, and CURVE public key with the daemon
  4. Daemon stores the public key in `${XDG_DATA_HOME:-$HOME/.local/share}/moor/allowed-hosts/{uuid}` for ZAP authentication
- **ZAP Authentication**: After enrollment, the daemon validates all incoming ZMQ connections using ZeroMQ Authentication Protocol (ZAP)
- **Per-Connection Encryption**: Each ZMQ socket (RPC REQ/REP, PUB/SUB) uses CURVE with ephemeral session keys

#### IPC Mode (No Encryption)

For local deployments using IPC endpoints:

- **No CURVE Encryption**: IPC endpoints are exempt from CURVE encryption
- **Filesystem Security**: Unix domain sockets rely on filesystem permissions for access control
- **No Enrollment Required**: Hosts/workers connect directly without enrollment
- **Performance**: IPC mode offers lower latency and higher throughput than encrypted TCP

#### Enrollment Token

TCP mode requires an **enrollment token** for CURVE enrollment:

```bash
# Generate enrollment token (one-time setup)
moor-daemon --rotate-enrollment-token

# Token is saved to ${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token
# Hosts/workers and the daemon will use this path by default (no flag needed)
```

The enrollment token is a shared secret used during the CURVE enrollment process to authorize hosts/workers to register their public keys with the daemon.

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
- `--rpc-listen <ADDR>` (default: `ipc:///tmp/moor_rpc.sock`): RPC server address (CURVE-encrypted if TCP)
- `--events-listen <ADDR>` (default: `ipc:///tmp/moor_events.sock`): Events publisher listen address (CURVE-encrypted if TCP)
- `--workers-response-listen <ADDR>` (default: `ipc:///tmp/moor_workers_response.sock`): Workers server RPC address (CURVE-encrypted if TCP)
- `--workers-request-listen <ADDR>` (default: `ipc:///tmp/moor_workers_request.sock`): Workers server pub-sub address (CURVE-encrypted if TCP)
- `--enrollment-listen <ADDR>` (default: `tcp://0.0.0.0:7900`): ZMQ REP socket for host/worker enrollment (not CURVE-encrypted)
- `--enrollment-token-file <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token`): Path to enrollment token file
- `--public-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-verifying-key.pem`): PEM encoded PASETO public key for token verification
- `--private-key <PATH>` (default: `${XDG_CONFIG_HOME:-$HOME/.config}/moor/moor-signing-key.pem`): PEM encoded PASETO private key for token signing
- `--num-io-threads <NUM>` (default: `8`): Number of ZeroMQ IO threads
- `--debug` (default: `false`): Enable debug logging

## Database Configuration

These options control the database behavior:

- `<PATH>` (positional argument): Path to the database file to use or create
- `--cache-eviction-interval-seconds <SECONDS>`: Rate to run cache eviction cycles
- `--default-eviction-threshold <SIZE>`: Default memory threshold for cache eviction

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
| UUID objects        | `--use-uuobjids`            | `false` | Enable UUID object identifiers like #048D05-1234567890                          |

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

If you need to maintain compatibility with LambdaMOO 1.8, you'll need to either update your core with the changes provided in the [Lambda-moor core](https://codeberg.org/timbran/moor/src/branch/main/cores/lambda-moor/README.md) or disable several features. Here's a configuration that maintains LambdaMOO compatibility by disabling mooR features:

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

The `anonymous_objects` feature flag enables a new type of object that is automatically garbage collected when no longer referenced.
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
