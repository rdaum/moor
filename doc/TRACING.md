# Tracing Support in Moor

Moor includes comprehensive Chrome Trace Event Format tracing for performance diagnostics and
debugging. This allows you to capture detailed performance data about task execution, VM operations,
and database transactions.

## Overview

The tracing system generates Chrome Trace Event Format JSON files that can be visualized in:

- Chrome DevTools (chrome://tracing)
- Perfetto UI (https://ui.perfetto.dev)
- Other compatible trace viewers

## Local Development (Bacon)

For local development with bacon, use the pre-configured tracing job:

```bash
bacon daemon-debug-traced
```

This builds with the `trace_events` feature and outputs traces to `moor-trace.json`.

## NPM Scripts

For convenience, npm scripts are available for tracing:

```bash
# Daemon with tracing
npm run daemon:traced

# Full development stack with tracing
npm run full:dev-traced
```

These scripts build with the `trace_events` feature and output traces to `moor-trace.json`.

## Docker with Tracing

### Option 1: Using Docker Compose Override

The easiest way to enable tracing in Docker is using the tracing override file:

```bash
# Create traces directory for output
mkdir -p traces

# Start with tracing enabled
docker compose -f docker-compose.yml -f docker-compose.tracing.yml up
```

This will:

- Build all services with the `trace_events` feature
- Mount the `./traces` directory to `/moor/traces` in the container
- Output trace events to `/moor/traces/moor-trace.json`
- Enable full backtraces for debugging

### Option 2: Custom Docker Build

You can also build a custom tracing-enabled image:

```bash
# Build with tracing enabled
docker build --build-arg TRACE_EVENTS=true -t moor-tracing .

# Run with trace output
docker run -v $(pwd)/traces:/moor/traces moor-tracing \
  ./moor-daemon /db/moor-data --trace-output=/moor/traces/moor-trace.json
```

## Using Trace Output

### Viewing Traces

1. **Chrome DevTools**: Open `chrome://tracing` and load the JSON file
2. **Perfetto UI**: Visit https://ui.perfetto.dev and upload the JSON file

### Trace Events Captured

The tracing system captures:

- **Task Lifecycle**: Creation, start, completion, suspension, resumption
- **VM Execution**: Verb calls, builtin execution, opcode counters
- **Database Transactions**: Begin, check, apply, commit, rollback
- **Scheduler Activity**: Active/queued task counts

### Example Analysis

Common performance issues you can identify:

- Long-running verbs or builtins
- Transaction conflicts in the database
- Task scheduling bottlenecks
- Memory allocation patterns

## Performance Impact

When the `trace_events` feature is disabled (default), there is **zero runtime cost** - all tracing
macros expand to no-ops.

When enabled, there is minimal overhead as events are batched and processed in a background thread.
The system uses:

- Non-blocking channel communication
- Periodic file flushing (every 5 seconds)
- Background thread processing

## Configuration

### Command Line Options

When built with tracing enabled, the daemon supports:

```bash
./moor-daemon --trace-output=path/to/trace.json
```

### Environment Variables

For debugging, you may want to enable:

```bash
RUST_BACKTRACE=full  # Full backtraces for debugging
```

## Troubleshooting

### No Trace Output

- Ensure the `trace_events` feature is enabled during build
- Check that the output directory is writable
- Verify the `--trace-output` argument is provided

### Large Trace Files

Trace files can grow large over time. Consider:

- Running tracing only during specific test periods
- Using the `--checkpoint-interval-seconds` to limit runtime
- Processing and analyzing traces periodically

### Build Issues

If building with tracing fails:

- Ensure all dependencies are up to date
- Check that the `trace_events` feature is properly configured
- Verify the kernel crate has the feature enabled
