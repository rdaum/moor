# MOO LSP Server Design

**Date:** 2025-12-26
**Status:** Draft
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

### Mapping Storage

Mappings are stored in the MOO world on the player object:

```moo
player.lsp_file_mappings = ["src/foo.moo" -> #42, "src/bar.moo" -> #57, ...]
```

### Mapping Sources

1. Query `#0` properties for `$name` → object ID mappings
2. Query `player` properties similarly
3. Parse `src/constants.moo` for additional mappings
4. Read `player.lsp_file_mappings` for explicit file → object mappings
5. Walk source files for bidirectional mapping properties

### Mapping States

| State | Description | LSP Behavior |
|-------|-------------|--------------|
| Mapped & Synced | File and DB match | Normal operation |
| Mapped & Drifted | File and DB differ | Diagnostic warning, offer resolution |
| Unmapped File | `.moo` file exists, no DB object | Diagnostic, prompt to create mapping |
| Unmapped Object | DB object exists, no file | Can offer to dump to file |

### Facilitating New Mappings

When LSP encounters an unmapped `.moo` file:

1. Parse file to extract object name/structure
2. Present options: map to existing object or create new
3. If creating: use objdef load via mooR client
4. Update `player.lsp_file_mappings`

### Drift Detection

For mapped objects:

1. Fetch current definition from DB (objdef dump)
2. Compare with parsed file content
3. If different, emit diagnostic warning

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

### Custom Operations (for mapping/drift)

| Method | Purpose |
|--------|---------|
| `moo/listMappings` | Return current file ↔ object mappings |
| `moo/createMapping` | Map unmapped file to new/existing object |
| `moo/resolveConflict` | Load file→DB or dump DB→file |
| `moo/showDiff` | Return diff between file and DB state |

## Diagnostics

| Severity | Type | Example Message |
|----------|------|-----------------|
| Error | Parse error | `Unexpected token 'endverb' at line 42` |
| Warning | Drift detected | `File differs from DB. DB modified 2h ago.` |
| Warning | Unmapped file | `No object mapping for this file.` |
| Info | Mapping info | `Mapped to $string_utils (#42)` |

Drift warnings appear as LSP diagnostics so Serena sees them when reading files.

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
   - Query `#0` properties → build `$name` → object ID map
   - Query `player` properties → same
   - Query `player.lsp_file_mappings` → build file → object map
   - Scan `--lsp-workspace` for `.moo` files
   - For each file: check if mapped, parse, detect drift
   - Send initial diagnostics to client

### Runtime

- Watch workspace for file changes → reparse, recheck drift
- Periodically or on-demand refresh DB state for drift detection

## Implementation

### New Modules in mcp-host

| Module | Responsibility |
|--------|----------------|
| `lsp_server.rs` | TCP listener, LSP protocol handling (JSON-RPC) |
| `lsp_workspace.rs` | File scanning, mapping management, drift detection |
| `lsp_symbols.rs` | Parse files → symbol tree, handle definition/references |
| `lsp_diagnostics.rs` | Generate diagnostics (errors, drift, unmapped) |

### Dependencies

- `lsp-types` - LSP protocol types
- `tower-lsp` or `lsp-server` - Protocol handling

### Reuse from Existing Code

**From mcp-host:**
- `moor_client.rs` - mooR connection
- `connection.rs` - Player context
- `tools/objdef.rs` - Dump/load/diff operations

**From compiler crate:**
- PEST parser
- `CompileError` → LSP Diagnostic translation
- AST for symbol extraction

## Future Considerations

- Editor support (VS Code extension) - same LSP, different client
- Multiple workspace roots
- Incremental parsing for large codebases
- Semantic tokens for syntax highlighting
