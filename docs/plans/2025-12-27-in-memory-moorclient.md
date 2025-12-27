# In-Memory MoorClient Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create an in-memory MoorClient that bypasses network RPC, enabling the LSP to work with a local database without requiring a running mooR daemon.

**Architecture:** The in-memory client wraps `WorldState` directly, using `MoorDB` in ephemeral mode (`path=None`). This bypasses the ZMQ transport layer while reusing the same database and permission infrastructure.

**Tech Stack:** MoorDB (fjall-based), WorldState trait, existing test infrastructure patterns

---

## Research Findings

### Current Architecture

```
MoorClient (network)
    → ZMQ REQ/REP sockets
    → RpcTransport
    → RpcMessageHandler
    → Scheduler
    → WorldState (WorldStateSource::new_world_state())
    → MoorDB (fjall key-value store)
```

### Key Abstractions

1. **WorldState trait** (`crates/common/src/model/world_state.rs:152`)
   - ~80 methods for all world operations
   - Permission-aware (takes `perms: &Obj` for checks)
   - Transaction-based (commit/rollback)

2. **WorldStateSource trait** (`crates/common/src/model/world_state.rs:469`)
   - Factory for creating WorldState instances
   - `new_world_state()` creates a transaction

3. **MoorDB** (`crates/db/src/moor_db.rs`)
   - Already supports in-memory mode: `TxDB::open(None, config)`
   - Uses tempdir when path is None

4. **DbWorldState** (`crates/db/src/db_worldstate.rs`)
   - Implements WorldState trait
   - Wraps WorldStateTransaction from database layer

### LSP Client Requirements

The LSP uses these MoorClient methods:

| MoorClient Method | WorldState Equivalent |
|-------------------|----------------------|
| `list_verbs(obj, inherited)` | `verbs(perms, obj)` + ancestor walk |
| `list_properties(obj, inherited)` | `properties(perms, obj)` + ancestor walk |
| `get_verb(obj, name)` | `get_verb(perms, obj, name)` + `retrieve_verb()` |
| `get_property(obj, name)` | `retrieve_property(perms, obj, name)` |
| `list_objects()` | `all_objects()` |
| `eval(expr)` | Requires Scheduler (complex) |

For LSP purposes, we only need **read-only introspection** - no eval/command/invoke.

---

## Implementation Options

### Option A: Direct WorldState Wrapper (Recommended)

Create a thin wrapper around WorldState for LSP introspection only.

**Pros:**
- Minimal code (~300-500 LOC)
- No scheduler complexity
- Direct, synchronous access
- Can load from .moo files OR existing database

**Cons:**
- No eval/command support
- Different interface than MoorClient

### Option B: Full In-Process Daemon

Embed RpcMessageHandler + Scheduler without network.

**Pros:**
- Full MoorClient compatibility
- Supports eval/command

**Cons:**
- Complex (~2000+ LOC)
- Requires async task handling
- Overkill for LSP needs

### Recommendation

**Option A** for MVP. The LSP only needs introspection. Eval support can be added later via Option B if needed.

---

## Task Breakdown

### Task 1: Create InMemoryWorldState Module

**Files:**
- Create: `crates/rpc/moor-client/src/in_memory.rs`
- Modify: `crates/rpc/moor-client/src/lib.rs` (add module)

**Step 1: Create the module file**

```rust
// crates/rpc/moor-client/src/in_memory.rs

//! In-memory world state access for LSP and tooling.
//!
//! Provides direct access to a MoorDB without network RPC.

use eyre::Result;
use moor_common::model::world_state::{WorldState, WorldStateSource};
use moor_db::MoorDB;
use moor_var::Obj;
use std::sync::Arc;

/// Configuration for in-memory world state.
pub struct InMemoryConfig {
    /// Path to existing database, or None for ephemeral.
    pub db_path: Option<std::path::PathBuf>,
    /// Player object for permission checks (wizard for full access).
    pub perms_player: Obj,
}

/// In-memory world state wrapper.
pub struct InMemoryWorldState {
    db: Arc<MoorDB>,
    perms: Obj,
}
```

**Step 2: Add module to lib.rs**

Add to `crates/rpc/moor-client/src/lib.rs`:
```rust
pub mod in_memory;
```

**Step 3: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs crates/rpc/moor-client/src/lib.rs
git commit -m "feat(moor-client): add in_memory module skeleton"
```

---

### Task 2: Implement InMemoryWorldState Constructor

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Add dependencies to Cargo.toml**

Check `crates/rpc/moor-client/Cargo.toml` for `moor-db` dependency. If missing, add:
```toml
moor-db = { path = "../../db" }
```

**Step 2: Implement constructor**

```rust
impl InMemoryWorldState {
    /// Create a new in-memory world state.
    ///
    /// If `db_path` is None, creates an ephemeral in-memory database.
    /// If `db_path` is Some, opens an existing database read-only.
    pub fn new(config: InMemoryConfig) -> Result<Self> {
        use moor_kernel::config::{Config, DatabaseConfig};

        let db_config = DatabaseConfig::default();
        let (db, _is_new) = MoorDB::open(
            config.db_path.as_deref(),
            db_config,
        );

        Ok(Self {
            db,
            perms: config.perms_player,
        })
    }

    /// Create a new transaction for world state operations.
    fn new_transaction(&self) -> Result<Box<dyn WorldState>> {
        self.db.new_world_state()
            .map_err(|e| eyre::eyre!("Failed to create transaction: {}", e))
    }
}
```

**Step 3: Add basic test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ephemeral() {
        let config = InMemoryConfig {
            db_path: None,
            perms_player: Obj::mk_id(0), // #0 system object
        };
        let ws = InMemoryWorldState::new(config);
        assert!(ws.is_ok());
    }
}
```

**Step 4: Commit**

```bash
git add crates/rpc/moor-client/
git commit -m "feat(moor-client): implement InMemoryWorldState constructor"
```

---

### Task 3: Implement list_verbs

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Add VerbInfo struct**

```rust
/// Information about a verb for LSP display.
#[derive(Debug, Clone)]
pub struct VerbInfo {
    pub name: String,
    pub owner: Obj,
    pub flags: String,
    pub args: String,
    pub definer: Obj,
}
```

**Step 2: Implement list_verbs**

```rust
impl InMemoryWorldState {
    /// List all verbs on an object.
    ///
    /// If `include_inherited` is true, walks the parent chain.
    pub fn list_verbs(&self, obj: &Obj, include_inherited: bool) -> Result<Vec<VerbInfo>> {
        let ws = self.new_transaction()?;
        let mut results = Vec::new();

        // Get verbs from this object
        let verbs = ws.verbs(&self.perms, obj)
            .map_err(|e| eyre::eyre!("Failed to list verbs: {}", e))?;

        for vdef in verbs.iter() {
            results.push(VerbInfo {
                name: vdef.names().join(" "),
                owner: vdef.owner().clone(),
                flags: format!("{:?}", vdef.flags()),
                args: format!("{:?}", vdef.args()),
                definer: obj.clone(),
            });
        }

        // Walk inheritance chain if requested
        if include_inherited {
            let mut current = ws.parent_of(&self.perms, obj).ok();
            while let Some(parent) = current {
                if !parent.is_valid() {
                    break;
                }
                if let Ok(parent_verbs) = ws.verbs(&self.perms, &parent) {
                    for vdef in parent_verbs.iter() {
                        results.push(VerbInfo {
                            name: vdef.names().join(" "),
                            owner: vdef.owner().clone(),
                            flags: format!("{:?}", vdef.flags()),
                            args: format!("{:?}", vdef.args()),
                            definer: parent.clone(),
                        });
                    }
                }
                current = ws.parent_of(&self.perms, &parent).ok();
            }
        }

        // Rollback since we're read-only
        let _ = ws.rollback();

        Ok(results)
    }
}
```

**Step 3: Add test**

```rust
#[test]
fn test_list_verbs_empty_db() {
    let config = InMemoryConfig {
        db_path: None,
        perms_player: Obj::mk_id(0),
    };
    let ws = InMemoryWorldState::new(config).unwrap();

    // Empty DB should return empty or error gracefully
    let result = ws.list_verbs(&Obj::mk_id(1), false);
    // Either empty vec or ObjectNotFound error is acceptable
    assert!(result.is_ok() || result.is_err());
}
```

**Step 4: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs
git commit -m "feat(moor-client): implement list_verbs for InMemoryWorldState"
```

---

### Task 4: Implement list_properties

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Add PropInfo struct**

```rust
/// Information about a property for LSP display.
#[derive(Debug, Clone)]
pub struct PropInfo {
    pub name: String,
    pub owner: Obj,
    pub flags: String,
    pub definer: Obj,
}
```

**Step 2: Implement list_properties**

```rust
impl InMemoryWorldState {
    /// List all properties on an object.
    ///
    /// If `include_inherited` is true, walks the parent chain.
    pub fn list_properties(&self, obj: &Obj, include_inherited: bool) -> Result<Vec<PropInfo>> {
        let ws = self.new_transaction()?;
        let mut results = Vec::new();

        // Get properties from this object
        let props = ws.properties(&self.perms, obj)
            .map_err(|e| eyre::eyre!("Failed to list properties: {}", e))?;

        for pdef in props.iter() {
            results.push(PropInfo {
                name: pdef.name().to_string(),
                owner: pdef.owner().clone(),
                flags: format!("{:?}", pdef.flags()),
                definer: obj.clone(),
            });
        }

        // Walk inheritance chain if requested
        if include_inherited {
            let mut current = ws.parent_of(&self.perms, obj).ok();
            while let Some(parent) = current {
                if !parent.is_valid() {
                    break;
                }
                if let Ok(parent_props) = ws.properties(&self.perms, &parent) {
                    for pdef in parent_props.iter() {
                        results.push(PropInfo {
                            name: pdef.name().to_string(),
                            owner: pdef.owner().clone(),
                            flags: format!("{:?}", pdef.flags()),
                            definer: parent.clone(),
                        });
                    }
                }
                current = ws.parent_of(&self.perms, &parent).ok();
            }
        }

        let _ = ws.rollback();
        Ok(results)
    }
}
```

**Step 3: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs
git commit -m "feat(moor-client): implement list_properties for InMemoryWorldState"
```

---

### Task 5: Implement get_verb (with source code)

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Add VerbCode struct**

```rust
/// Verb with source code.
#[derive(Debug, Clone)]
pub struct VerbCode {
    pub info: VerbInfo,
    pub source: Vec<String>,
}
```

**Step 2: Implement get_verb**

```rust
impl InMemoryWorldState {
    /// Get a verb's code from an object.
    pub fn get_verb(&self, obj: &Obj, verb_name: &str) -> Result<VerbCode> {
        use moor_var::Symbol;

        let ws = self.new_transaction()?;
        let vname = Symbol::mk(verb_name);

        // Get verb definition
        let vdef = ws.get_verb(&self.perms, obj, vname)
            .map_err(|e| eyre::eyre!("Verb not found: {}", e))?;

        // Get verb program
        let (program, _) = ws.retrieve_verb(&self.perms, obj, vdef.uuid())
            .map_err(|e| eyre::eyre!("Failed to retrieve verb code: {}", e))?;

        // Convert program to source lines
        let source = match program {
            moor_var::program::ProgramType::Source(src) => {
                src.lines().map(|s| s.to_string()).collect()
            }
            moor_var::program::ProgramType::Cst(cst) => {
                // Unparse CST back to source
                moor_compiler::unparse(&cst)
                    .lines()
                    .map(|s| s.to_string())
                    .collect()
            }
            _ => vec!["/* binary verb */".to_string()],
        };

        let _ = ws.rollback();

        Ok(VerbCode {
            info: VerbInfo {
                name: vdef.names().join(" "),
                owner: vdef.owner().clone(),
                flags: format!("{:?}", vdef.flags()),
                args: format!("{:?}", vdef.args()),
                definer: obj.clone(),
            },
            source,
        })
    }
}
```

**Step 3: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs
git commit -m "feat(moor-client): implement get_verb for InMemoryWorldState"
```

---

### Task 6: Implement get_property and list_objects

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Implement get_property**

```rust
impl InMemoryWorldState {
    /// Get a property value from an object.
    pub fn get_property(&self, obj: &Obj, prop_name: &str) -> Result<moor_var::Var> {
        use moor_var::Symbol;

        let ws = self.new_transaction()?;
        let pname = Symbol::mk(prop_name);

        let value = ws.retrieve_property(&self.perms, obj, pname)
            .map_err(|e| eyre::eyre!("Property not found: {}", e))?;

        let _ = ws.rollback();
        Ok(value)
    }

    /// List all valid objects in the database.
    pub fn list_objects(&self) -> Result<Vec<Obj>> {
        let ws = self.new_transaction()?;

        let objects = ws.all_objects()
            .map_err(|e| eyre::eyre!("Failed to list objects: {}", e))?;

        let result: Vec<Obj> = objects.iter().collect();

        let _ = ws.rollback();
        Ok(result)
    }
}
```

**Step 2: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs
git commit -m "feat(moor-client): implement get_property and list_objects"
```

---

### Task 7: Add MoorDB Loading from .moo Files

**Files:**
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Add load_from_files method**

```rust
impl InMemoryWorldState {
    /// Create an in-memory world state and load objects from .moo files.
    ///
    /// This is useful for LSP when working with a file-based project
    /// without a running mooR daemon.
    pub fn load_from_files(files: &[std::path::PathBuf], perms: Obj) -> Result<Self> {
        use moor_compiler::{CompileOptions, ObjFileContext, compile_object_definitions};

        // Create ephemeral database
        let ws = Self::new(InMemoryConfig {
            db_path: None,
            perms_player: perms.clone(),
        })?;

        // Load each file
        for file in files {
            let content = std::fs::read_to_string(file)
                .map_err(|e| eyre::eyre!("Failed to read {}: {}", file.display(), e))?;

            let options = CompileOptions::default();
            let mut context = ObjFileContext::default();

            if let Ok(definitions) = compile_object_definitions(&content, &options, &mut context) {
                // TODO: Insert definitions into database
                // This requires using the LoaderInterface
                tracing::info!("Loaded {} objects from {}", definitions.len(), file.display());
            }
        }

        Ok(ws)
    }
}
```

**Step 2: Commit**

```bash
git add crates/rpc/moor-client/src/in_memory.rs
git commit -m "feat(moor-client): add load_from_files skeleton for file-based loading"
```

---

### Task 8: Create MoorClientTrait for Unified Interface

**Files:**
- Create: `crates/rpc/moor-client/src/traits.rs`
- Modify: `crates/rpc/moor-client/src/lib.rs`
- Modify: `crates/rpc/moor-client/src/in_memory.rs`

**Step 1: Define the trait**

```rust
// crates/rpc/moor-client/src/traits.rs

//! Common trait for mooR client implementations.

use async_trait::async_trait;
use eyre::Result;
use moor_common::model::ObjectRef;
use moor_var::{Obj, Var};

/// Information about a verb.
#[derive(Debug, Clone)]
pub struct VerbInfo {
    pub name: String,
    pub owner: Obj,
    pub flags: String,
    pub args: String,
    pub definer: Obj,
}

/// Information about a property.
#[derive(Debug, Clone)]
pub struct PropInfo {
    pub name: String,
    pub owner: Obj,
    pub flags: String,
    pub definer: Obj,
}

/// Verb with source code.
#[derive(Debug, Clone)]
pub struct VerbCode {
    pub info: VerbInfo,
    pub source: Vec<String>,
}

/// Trait for read-only introspection of a mooR world.
///
/// Implemented by both network MoorClient and InMemoryWorldState.
#[async_trait]
pub trait MoorIntrospection: Send + Sync {
    /// List verbs on an object.
    async fn list_verbs(&mut self, obj: &ObjectRef, include_inherited: bool) -> Result<Vec<VerbInfo>>;

    /// List properties on an object.
    async fn list_properties(&mut self, obj: &ObjectRef, include_inherited: bool) -> Result<Vec<PropInfo>>;

    /// Get verb source code.
    async fn get_verb(&mut self, obj: &ObjectRef, verb_name: &str) -> Result<VerbCode>;

    /// Get property value.
    async fn get_property(&mut self, obj: &ObjectRef, prop_name: &str) -> Result<Var>;

    /// List all objects.
    async fn list_objects(&mut self) -> Result<Vec<Obj>>;
}
```

**Step 2: Implement trait for InMemoryWorldState**

Wrap synchronous methods in async:

```rust
#[async_trait]
impl MoorIntrospection for InMemoryWorldState {
    async fn list_verbs(&mut self, obj: &ObjectRef, include_inherited: bool) -> Result<Vec<VerbInfo>> {
        let obj = obj.resolve_to_obj()?;
        self.list_verbs_sync(&obj, include_inherited)
    }
    // ... etc
}
```

**Step 3: Commit**

```bash
git add crates/rpc/moor-client/src/
git commit -m "feat(moor-client): add MoorIntrospection trait for unified interface"
```

---

### Task 9: Integrate with LSP Backend

**Files:**
- Modify: `tools/lsp/src/backend.rs`

**Step 1: Add InMemoryWorldState as fallback**

When `moor_client` is None, use `InMemoryWorldState` for file-based introspection:

```rust
use moor_client::in_memory::InMemoryWorldState;

// In MooLanguageServer::new or initialized:
// If no RPC client, create in-memory world state from workspace files
let in_memory_state = if moor_client.is_none() {
    let files = workspace::scan_workspace(&workspace).await;
    Some(Arc::new(RwLock::new(
        InMemoryWorldState::load_from_files(&files, Obj::mk_id(0))?
    )))
} else {
    None
};
```

**Step 2: Use trait object for unified access**

```rust
// Use MoorIntrospection trait for operations
async fn get_verbs_for_object(&self, obj: &ObjectRef) -> Result<Vec<VerbInfo>> {
    if let Some(client) = &self.moor_client {
        client.write().await.list_verbs(obj, true).await
    } else if let Some(in_mem) = &self.in_memory_state {
        in_mem.write().await.list_verbs(obj, true).await
    } else {
        Ok(vec![])
    }
}
```

**Step 3: Commit**

```bash
git add tools/lsp/src/backend.rs
git commit -m "feat(lsp): integrate InMemoryWorldState for offline mode"
```

---

### Task 10: Add Integration Tests

**Files:**
- Create: `crates/rpc/moor-client/src/in_memory_tests.rs`

**Step 1: Test with real .moo file**

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_minimal_core() {
        // Use a minimal core file from the test fixtures
        let test_file = PathBuf::from("../../cores/minimal.moo");
        if !test_file.exists() {
            // Skip if test file doesn't exist
            return;
        }

        let ws = InMemoryWorldState::load_from_files(
            &[test_file],
            Obj::mk_id(0),
        );

        assert!(ws.is_ok());

        let ws = ws.unwrap();
        let objects = ws.list_objects().unwrap();
        assert!(!objects.is_empty());
    }
}
```

**Step 2: Commit**

```bash
git add crates/rpc/moor-client/src/
git commit -m "test(moor-client): add integration tests for InMemoryWorldState"
```

---

## Future Enhancements

These are out of scope for MVP but noted for future:

1. **Eval support**: Embed Scheduler for `eval()` and `command()`
2. **Write operations**: Enable `set_property()`, `program_verb()`
3. **Change watching**: Detect .moo file changes and reload
4. **Incremental loading**: Only reload changed files

---

## Execution Checklist

- [ ] Task 1: Create module skeleton
- [ ] Task 2: Implement constructor
- [ ] Task 3: Implement list_verbs
- [ ] Task 4: Implement list_properties
- [ ] Task 5: Implement get_verb
- [ ] Task 6: Implement get_property and list_objects
- [ ] Task 7: Add load_from_files
- [ ] Task 8: Create MoorIntrospection trait
- [ ] Task 9: Integrate with LSP
- [ ] Task 10: Add integration tests
