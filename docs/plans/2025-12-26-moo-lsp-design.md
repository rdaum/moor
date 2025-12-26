# MOO LSP Server Design

**Date:** 2025-12-26
**Status:** Draft (v2 - addresses review feedback)
**Goal:** Extend mcp-host to provide LSP support for AI-assisted MOO development

## Overview

Add an LSP endpoint to mcp-host so Serena can index and navigate MOO codebases like cowbell. The LSP server connects to the running mooR daemon, queries object definitions, and maps them to source files on disk.

## Architecture

```
┌─────────────────────────────────────────────┐
│                 mcp-host                     │
│  ┌─────────────┐       ┌─────────────────┐  │
│  │ MCP Server  │       │   LSP Server    │  │
│  │  (stdio)    │       │ (TCP, optional) │  │
│  └──────┬──────┘       └────────┬────────┘  │
│         │                       │           │
│         └───────────┬───────────┘           │
│                     │                       │
│         ┌───────────▼───────────┐           │
│         │    mooR Client        │           │
│         │  (existing RPC conn)  │           │
│         └───────────┬───────────┘           │
└─────────────────────┼───────────────────────┘
                      │
                      ▼
              ┌───────────────┐
              │  mooR Daemon  │
              │  (database)   │
              └───────────────┘
```

**Activation:** `mcp-host --lsp-port 8888` enables LSP listener on the specified TCP port.

**Client model:** Single client connection at a time.

## LSP Lifecycle Management

### Connection Handling

Single-client enforcement:
- TCP listener accepts one connection
- If a second client attempts to connect while one is active, reject with error
- On client disconnect, listener accepts new connections

### Protocol State Machine

```
                    ┌──────────────┐
      TCP Connect   │              │
─────────────────►  │  Connected   │
                    │              │
                    └──────┬───────┘
                           │ initialize request
                           ▼
                    ┌──────────────┐
                    │              │
                    │ Initializing │
                    │              │
                    └──────┬───────┘
                           │ initialized notification
                           ▼
                    ┌──────────────┐
                    │              │◄─────── Normal LSP operations
                    │    Ready     │
                    │              │
                    └──────┬───────┘
                           │ shutdown request
                           ▼
                    ┌──────────────┐
                    │              │
                    │  Shutting    │
                    │    Down      │
                    └──────┬───────┘
                           │ exit notification
                           ▼
                    ┌──────────────┐
                    │ Disconnected │
                    └──────────────┘
```

### Required Handlers

| Message | Direction | Response |
|---------|-----------|----------|
| `initialize` | Client → Server | ServerCapabilities (symbols, definition, references, hover, completion, diagnostics) |
| `initialized` | Client → Server | (notification) Begin workspace indexing |
| `shutdown` | Client → Server | null (prepare to exit) |
| `exit` | Client → Server | (notification) Close connection |

## Workspace & Symbol Model

### Workspace Scope

- **Anchors:** `#0` (system) and `player` objects
- **Files:** All `.moo` files under the workspace directory (e.g., `src/`)
- **Discovery:** Recursively follow object references from anchors

### Symbol Hierarchy

MOO concepts map to LSP symbol kinds for a class-like experience:

| MOO Concept | LSP Symbol Kind | Example |
|-------------|-----------------|---------|
| Object | Class/Module | `$string_utils` |
| Verb | Method | `$string_utils:trim` |
| Property (value) | Field | `$string_utils.cache` |
| Property (object ref) | Module reference | `$formatter.paragraph` → another object |
| Property (object array) | Directory of refs | `$formatter.handlers[0]`, `[1]`, etc. |
| Local variable | Variable | `x` inside a verb |

### Navigation Example

```
$formatter.paragraph:format()
    │         │        └── Verb on the referenced object
    │         └── Property holding object ref (follow it)
    └── Object anchored from #0.formatter
```

## File ↔ Object Mapping

### Cowbell Mapping Convention

Cowbell uses a three-part mapping system:

#### 1. `constants.moo` - Symbolic Registry

Located at `src/constants.moo`, defines symbolic names → object IDs:

```moo
// auth
define PASSWORD = #16;
define LOGIN = #17;

// events
define EVENT_RECEIVER = #4;
define EVENT = #18;

// UUID-based (runtime-created objects)
define URL_UTILS = #0000AB-9B0E8A16A0;
define HENRI = #0008CA-9A95162D6A;
```

#### 2. `import_export_id` Property - File Name Mapping

Every object has this property linking it to its source file:

```moo
object ACTOR
  parent: ROOT
  override import_export_id = "actor";    // → actor.moo
  ...
endobject
```

#### 3. `import_export_hierarchy` Property - Directory Mapping

Optional property indicating subdirectory:

```moo
object EVENT
  parent: ROOT
  override import_export_id = "event";
  override import_export_hierarchy = {"events"};  // → events/event.moo
  ...
endobject
```

### Mapping Resolution

**File → Object:**
1. Parse file path: `src/events/event.moo`
2. Extract: directory = `events`, basename = `event`
3. Find object where `import_export_id = "event"` AND `import_export_hierarchy = {"events"}`
4. Cross-reference with `constants.moo` for object ID

**Object → File:**
1. Read `import_export_id` and `import_export_hierarchy` properties
2. Construct path: `{workspace}/{hierarchy}/{id}.moo`
3. Example: `import_export_id="event"`, `hierarchy={"events"}` → `src/events/event.moo`

### Conflict Resolution Precedence

If multiple sources disagree on a mapping:

1. **`import_export_id` + `import_export_hierarchy`** on the object (highest priority - canonical)
2. **`constants.moo`** symbolic definitions
3. **`player.lsp_file_mappings`** (user overrides)
4. **Convention-based inference** from file path (lowest priority)

### Mapping States

| State | Description | LSP Behavior |
|-------|-------------|--------------|
| Mapped & Synced | File and DB match | Normal operation |
| Mapped & Drifted | File and DB differ | Diagnostic warning, offer resolution |
| Unmapped File | `.moo` file exists, no DB object | Diagnostic, prompt to create mapping |
| Unmapped Object | DB object exists, no file | Can offer to dump to file |

### Facilitating New Mappings

When LSP encounters an unmapped `.moo` file:

1. Parse file to extract `object NAME` declaration
2. Check if `import_export_id` is defined in file
3. If object exists in DB: create mapping, warn if `import_export_id` differs
4. If object doesn't exist: offer to load via objdef
5. Update `player.lsp_file_mappings` for overrides only

### Drift Detection

For mapped objects:

1. Fetch current definition from DB (objdef dump)
2. Compare with parsed file content
3. If different, emit diagnostic warning with modification timestamps

### Conflict Resolution

- **Load file → DB:** File content overwrites database
- **Dump DB → file:** Database content overwrites file
- **Show diff:** Return difference for manual merge

## LSP Operations

### Core Operations (for Serena)

| LSP Method | Implementation |
|------------|----------------|
| `textDocument/documentSymbol` | Parse `.moo` file → return objects, verbs, properties as symbol tree |
| `workspace/symbol` | Search across all mapped files + anchored objects |
| `textDocument/definition` | Resolve `$name:verb` → find source location (file + line) |
| `textDocument/references` | Search verb/property usage across workspace |
| `textDocument/hover` | Show object/verb/property info, ownership, flags, docstring |
| `textDocument/completion` | Suggest builtins, verbs, properties, variables in scope |

### Completion Support

**Trigger contexts and suggestions:**

| Context | Suggestions |
|---------|-------------|
| `$` prefix | Object names from `#0` properties (`$string_utils`, `$list_utils`) |
| `$obj:` | Verbs defined on that object |
| `$obj.` | Properties defined on that object |
| `this:` | Verbs on current object |
| `this.` | Properties on current object |
| Bare identifier | Local variables in scope, then builtins |
| Function call `(` | Builtin function signatures |

**Builtin functions:** Source from `moor_common::builtins` registry.

### Custom Operations (for mapping/drift)

| Method | Purpose |
|--------|---------|
| `moo/listMappings` | Return current file ↔ object mappings |
| `moo/createMapping` | Map unmapped file to new/existing object |
| `moo/resolveConflict` | Load file→DB or dump DB→file |
| `moo/showDiff` | Return diff between file and DB state |

## Diagnostics

### Diagnostic Types

| Severity | Type | Example Message |
|----------|------|-----------------|
| Error | Parse error | `Unexpected token 'endverb' at line 42` |
| Warning | Drift detected | `File differs from DB. DB modified 2h ago.` |
| Warning | Unmapped file | `No object mapping for this file.` |
| Info | Mapping info | `Mapped to $string_utils (#42)` |

### Push Model

Diagnostics are pushed via `textDocument/publishDiagnostics`:

| Trigger | Action |
|---------|--------|
| File opened | Parse file, publish parse errors + mapping status |
| File changed | Reparse, publish updated diagnostics |
| File saved | Recheck drift against DB, publish if changed |
| Periodic (60s) | Refresh drift status for open files |
| DB change detected | Republish drift diagnostics for affected files |

### CompileError → LSP Diagnostic Mapping

```rust
// From moor_compiler::CompileError / ParseErrorDetails
ParseErrorDetails {
    span: (start_byte, end_byte),
    expected_tokens: Vec<String>,
    notes: Vec<String>,
}

// Maps to lsp_types::Diagnostic
Diagnostic {
    range: Range {
        start: byte_to_position(span.0),
        end: byte_to_position(span.1),
    },
    severity: DiagnosticSeverity::ERROR,
    source: Some("moo"),
    message: format_error_message(),
    related_information: notes.map(|n| DiagnosticRelatedInformation { ... }),
}
```

## Configuration & Startup

### Command Line

```bash
mcp-host --lsp-port 8888 --lsp-workspace /path/to/cowbell/src
```

The player context is implicit from the existing mcp-host login.

### Startup Sequence

1. Connect to mooR daemon (existing mcp-host behavior)
2. If `--lsp-port` specified, start TCP listener
3. On LSP client connect:
   - Complete `initialize`/`initialized` handshake
   - Parse `constants.moo` for symbolic mappings
   - Query `#0` properties → build `$name` → object ID map
   - Query `player` properties → same
   - Scan `--lsp-workspace` for `.moo` files
   - For each file: resolve mapping via `import_export_id`, parse, detect drift
   - Send initial diagnostics to client

### Runtime

- Watch workspace for file changes → reparse, recheck drift
- Periodic (60s) refresh of DB state for drift detection
- On MCP objdef operations (load/reload), notify LSP to refresh affected mappings

## Implementation

### New Modules in mcp-host

| Module | Responsibility |
|--------|----------------|
| `lsp/server.rs` | TCP listener, LSP protocol handling, lifecycle state machine |
| `lsp/workspace.rs` | File scanning, mapping management, drift detection |
| `lsp/symbols.rs` | Parse files → symbol tree, handle definition/references |
| `lsp/completion.rs` | Context-aware completion suggestions |
| `lsp/diagnostics.rs` | Generate and push diagnostics |

### Dependencies

- `lsp-types` - LSP protocol types
- `tower-lsp` - Async LSP server framework (integrates with existing Tokio runtime)

### Reuse from Existing Code

**From mcp-host:**
- `moor_client.rs` - mooR connection
- `connection.rs` - Player context
- `tools/objdef.rs` - Dump/load/diff operations

**From compiler crate:**
- PEST parser (with span preservation)
- `CompileError` / `ParseErrorDetails` for diagnostics
- AST types for symbol extraction

**From common crate:**
- `builtins.rs` - Builtin function registry for completion

### Span Preservation

The compiler's `parse.rs` must preserve source spans in the AST for:
- Symbol locations (for documentSymbol)
- Go-to-definition targets
- Diagnostic ranges

Current state: Verify `moor_compiler::objdef::compile_object_definitions` returns spans. If not, extend to preserve `Span` from PEST pairs.

## Error Handling

### Connection Errors

| Error | Response |
|-------|----------|
| mooR connection lost | Attempt reconnect (existing logic), queue LSP requests |
| Reconnect failed | Send `window/showMessage` error, degrade to parse-only mode |
| Parse failure | Return empty symbols, publish diagnostic |
| DB query timeout | Return cached data if available, publish warning |

### Graceful Degradation

If mooR connection is unavailable:
- Symbol extraction from files still works (parse-only)
- Definition/references limited to current file
- Completion limited to builtins and local variables
- Drift detection disabled
- Mapping to DB objects unavailable

## Future Considerations

- Editor support (VS Code extension) - same LSP, different client
- Multiple workspace roots
- Incremental parsing for large codebases
- Semantic tokens for syntax highlighting
- `workspace/didChangeWorkspaceFolders` support
- Request cancellation (`$/cancelRequest`)
- Progress reporting (`$/progress`) for workspace indexing
