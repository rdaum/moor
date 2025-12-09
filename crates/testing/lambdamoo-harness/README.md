# lambdamoo-harness

Test harness that embeds the original LambdaMOO C implementation for comparative
testing and benchmarking against mooR.

This crate is **not built by default**. It requires fetching external LambdaMOO
sources before it can be compiled.

## Setup

1. Run the setup script to fetch LambdaMOO sources:

   ```bash
   ./crates/testing/lambdamoo-harness/setup-lambdamoo.sh
   ```

   This will:
   - Clone `wrog/lambdamoo` from GitHub (pinned to a specific commit)
   - Apply patches from `patches/` for modern compiler compatibility
   - Run `configure` to generate `config.h`

2. Build the harness:

   ```bash
   cargo build -p lambdamoo-harness
   ```

## Usage

### Load Testing

The `lambdamoo-load-test` binary measures verb dispatch performance:

```bash
# Basic verb dispatch benchmark
cargo run --release -p lambdamoo-harness --bin lambdamoo-load-test -- \
    --db-path lambdamoo/Minimal.db \
    --num-invocations 100 \
    --num-verb-iterations 1000

# Opcode throughput benchmark (raw interpreter speed)
cargo run --release -p lambdamoo-harness --bin lambdamoo-load-test -- \
    --db-path lambdamoo/Minimal.db \
    --opcode-mode \
    --loop-iterations 100000
```

### Rust API

The `LambdaMooHarness` struct provides a safe Rust wrapper:

```rust
use lambdamoo_harness::LambdaMooHarness;

let harness = LambdaMooHarness::new(Path::new("path/to/db.db"))?;
let conn = harness.create_connection(player_objid)?;
let output = harness.execute_command(&conn, "look")?;
```

## What's Included

- `LambdaMooHarness` - Rust wrapper for initializing and interacting with LambdaMOO
- `lambdamoo-load-test` - Binary for comparative load testing against mooR
- `patches/` - Patches for modern glibc compatibility and build configuration
- `src/net_harness.c` - Custom network layer that captures output for testing

## License Note

**LambdaMOO is NOT GPL.** It is licensed under the
[Xerox License](https://spdx.org/licenses/Xerox.html), which is permissive but
requires compliance with US export control laws. This makes it GPL-incompatible.

This harness is a development/testing tool kept separate from the main mooR
distribution. The LambdaMOO sources are not included in the moor repository and
must be fetched separately using the setup script.
