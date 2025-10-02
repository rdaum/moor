### What this is

mooR uses [FlatBuffers](https://flatbuffers.dev/) for serialization across persistence and RPC boundaries.

**Why FlatBuffers?**

- **Zero-copy deserialization**: Read data directly from the buffer without parsing/unpacking
- **Schema evolution**: Forward/backward compatible - can add optional fields, deprecate old
  ones ([evolution guide](https://flatbuffers.dev/evolution/))
- **Language interoperability**: Generated bindings for C++, Python, JavaScript, etc. enable polyglot clients
- **Performance**: Faster than parsing formats like JSON or even binary formats like bincode that require full
  deserialization

Previously mooR used `bincode`, which has several disadvantages compared to FlatBuffers:

- Requires full deserialization before accessing any field
- Poor schema evolution - adding fields breaks compatibility
- Rust-specific - difficult to build clients in other languages
- No random access to nested data

**Where we use FlatBuffers:**

* **`moor-db`**: Storing structured entities in the database
* **`moor-daemon`**: Event log and task list persistence in custom DBs
* **RPC wire format**: All communication between daemon, hosts (`web-host`, `telnet-host`), workers (`curl-worker`), and
  tools

### Schema Files

- **`common.fbs`**: Shared core types (Var, Obj, Symbol, UUID, errors, events, etc.)
- **`moor_rpc.fbs`**: RPC message types for host↔daemon, client↔daemon, worker↔daemon communication
- **`db.fbs`**: Database persistence types (objects, properties, verbs, etc.)
- **`all_schemas.fbs`**: Top-level schema that includes all the above

### Structure

Because of the way planus manages includes, it's not possible to split interdependent messages across crates, so we're
lumping them all in here together.

### Code Organization

The generated FlatBuffer code lives in `crates/common/src/schema/schemas_generated.rs`, which is kept private. We expose
the types through domain-specific modules that re-export from the generated namespaces:

- **`crates/common/src/schema/rpc.rs`**: Re-exports `MoorRpc` namespace types for RPC messages
- **`crates/common/src/schema/common.rs`**: Re-exports `MoorCommon` namespace for shared core types (Var, Obj, errors,
  etc.)
- **`crates/common/src/schema/event_log.rs`**: Re-exports event log related types
- **`crates/common/src/schema/program.rs`**: Re-exports program/verb types

This organization keeps the massive generated file private while providing clean, semantic access through modules like:

```rust
use moor_common::schema::rpc::DaemonToWorkerReply;
use moor_common::schema::common::VarBytes;
```

### Generating Code

**Important:** We check in the generated code (`schemas_generated.rs`) rather than regenerating it automatically.

When you modify any `.fbs` schema file, regenerate using:

```shell
planus rust -o ./crates/common/src/schema/schemas_generated.rs ./crates/common/schema/all_schemas.fbs
```

### Migration Status

**Note:** Not everything has been migrated to FlatBuffers yet. The following still use `bincode`:

- **`crates/var/`**: Core Var type serialization (used extensively throughout the system)
- **`crates/kernel/src/tasks/tasks_db.rs`**: Some task persistence structures

These are candidates for future migration to FlatBuffers to gain the benefits of zero-copy deserialization and better
schema evolution.

