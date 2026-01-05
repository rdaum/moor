# Textdump Load Test Generator Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a reusable test-only DB builder and a test-only textdump writer to generate large, structured dumps for read-path performance testing.

**Architecture:** Build a deterministic test DB via a `TestDbBuilder` (configurable inheritance + override ratios) and dump it to a minimal, valid Moor-format textdump. The writer streams from a snapshot, avoiding in-memory full object graphs.

**Tech Stack:** Rust tests in `crates/db/tests`, `moor-db`, `moor-common`, `moor-var`, `moor-textdump`.

---

### Task 1: Add reusable test builder API

**Files:**
- Create: `crates/db/tests/support/mod.rs`
- Create: `crates/db/tests/support/builder.rs`

**Step 1: Write a failing compile-time test scaffold**

```rust
#[test]
fn builds_configured_database() {
    let _ = support::builder::DbBuildConfig::default();
    // TODO: use builder in later tasks
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: FAIL with module/config not found.

**Step 3: Implement `DbBuildConfig` + `TestDbBuilder`**

```rust
#[derive(Clone, Debug)]
pub struct DbBuildConfig {
    pub object_count: usize,
    pub inheritance_stride: usize,
    pub props_per_object: usize,
    pub verbs_per_object: usize,
    pub prop_override_ratio: f32,
    pub verb_override_ratio: f32,
    pub rng_seed: u64,
}

pub struct GeneratedDb {
    pub db: Arc<TxDB>,
    pub object_ids: Vec<Obj>,
}

pub struct TestDbBuilder {
    config: DbBuildConfig,
}

impl TestDbBuilder {
    pub fn new(config: DbBuildConfig) -> Self { /* ... */ }

    pub fn build(&self) -> GeneratedDb {
        // 1) create #0 system object
        // 2) create objects with parent determined by inheritance_stride
        // 3) define props + verbs on each object
        // 4) apply overrides based on ratios
    }
}
```

**Step 4: Ensure deterministic inheritance and overrides**

- Use `StdRng::seed_from_u64(rng_seed)`.
- Track per-object property names and verb names so overrides can reuse the same names on descendants.

**Step 5: Run test to verify it passes**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: PASS for compile (test will still be placeholder).

---

### Task 2: Add test-only textdump writer

**Files:**
- Create: `crates/db/tests/support/textdump_writer.rs`

**Step 1: Write a failing unit test that dumps and reloads**

```rust
#[test]
fn generated_textdump_round_trips() {
    // TODO: builder -> dump -> read_textdump -> verify counts
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: FAIL due to missing writer and load path.

**Step 3: Implement a minimal Moor-format writer**

```rust
pub struct TextdumpWriteConfig {
    pub version: semver::Version,
    pub compile_options: CompileOptions,
    pub encoding: EncodingMode,
}

pub fn write_textdump(
    snapshot: &dyn SnapshotInterface,
    out: &mut dyn std::io::Write,
    config: &TextdumpWriteConfig,
) -> Result<(), std::io::Error> {
    // 1) write version line
    // 2) compute object list (sorted by Obj id)
    // 3) compute verb count
    // 4) write counts + users (0)
    // 5) write objects (object header + props/verbs)
    // 6) write verb programs (empty program with '.')
    // 7) write task queue with zeros
}
```

**Step 4: Keep writer output minimal and valid for `read_textdump`**

- Use `TextdumpVersion::Moor(...).to_version_string()` for the first line.
- For each object:
  - `#<id>` line, name line, `ohandles` line (write `0`).
  - flags/owner/location/contents/next/parent/child/sibling (use `-1` for NOTHING).
  - verbdefs from `get_object_verbs` (name string, owner, flags, prep).
  - propdefs from `get_object_properties` where `definer == obj`.
  - propvals from `get_all_property_values` (self then parents), emitting type + data + owner + flags.
- Verb programs: header `#<objid>:<verbnum>` and then `.` only.
- Task queue lines: `0 clocks`, `0 queued tasks`, `0 suspended tasks`.

**Step 5: Run test to verify it passes**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: PASS for writer round-trip.

---

### Task 3: Wire integration test using builder + writer + reader

**Files:**
- Create: `crates/db/tests/textdump_generated_read.rs`
- Modify: `crates/db/Cargo.toml` (dev-dependency)

**Step 1: Add test that builds DB, writes textdump, reads it back**

```rust
#[path = "support/mod.rs"]
mod support;

#[test]
fn generated_textdump_round_trips() {
    let config = support::builder::DbBuildConfig { /* larger numbers */ };
    let generated = support::builder::TestDbBuilder::new(config).build();

    let mut dump = Vec::new();
    support::textdump_writer::write_textdump(
        generated.db.create_snapshot().unwrap().as_ref(),
        &mut dump,
        &support::textdump_writer::TextdumpWriteConfig::default(),
    ).unwrap();

    // Load dump into a fresh DB using read_textdump
    let (db, _) = TxDB::open(None, DatabaseConfig::default());
    let mut loader = db.loader_client().unwrap();
    let cursor = std::io::BufReader::new(std::io::Cursor::new(dump));

    read_textdump(
        loader.as_mut(),
        cursor,
        Version::new(0, 1, 0),
        CompileOptions::default(),
        TextdumpImportOptions::default(),
    ).unwrap();

    let snapshot = db.create_snapshot().unwrap();
    assert_eq!(snapshot.get_objects().unwrap().len(), generated.object_ids.len());
}
```

**Step 2: Add dev-dependency for `moor-textdump`**

```toml
[dev-dependencies]
moor-textdump = { path = "../textdump" }
```

**Step 3: Run test to verify it passes**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: PASS.

**Step 4: Add a couple of asserts for overrides**

- Choose a few known objects and properties/verbs that were overridden.
- Verify values/program presence via snapshot APIs.

**Step 5: Run test to verify it still passes**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: PASS.

---

### Task 4: Add simple doc comment + config defaults

**Files:**
- Modify: `crates/db/tests/support/builder.rs`

**Step 1: Add Rustdoc for builder and config**

```rust
/// Builder for generating deterministic inheritance graphs with optional overrides.
```

**Step 2: Provide a sane `Default` config for local tests**

```rust
impl Default for DbBuildConfig {
    fn default() -> Self {
        Self {
            object_count: 500,
            inheritance_stride: 3,
            props_per_object: 5,
            verbs_per_object: 3,
            prop_override_ratio: 0.2,
            verb_override_ratio: 0.2,
            rng_seed: 42,
        }
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p moor-db --test textdump_generated_read -v`
Expected: PASS.

---

Plan complete and saved to `docs/plans/2026-01-06-textdump-load-test-generator.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?
