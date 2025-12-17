# Quick Start Guide

Get the Python echo worker running in under 5 minutes.

## Prerequisites

- Python 3.10+
- mooR daemon running with workers enabled

## Step 1: Install Dependencies

```bash
cd tools/example-python-worker
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

## Step 2: Verify Setup

```bash
python3 verify_setup.py
```

Expected output:

```
Checking Python version... OK (3.x.x)
Checking pyzmq... OK (with CURVE support)
Checking flatbuffers... OK
All checks passed!
```

## Step 3: Start the Daemon

Start the mooR daemon with worker support:

```bash
# From the moor root directory
cargo run --bin moord -- --workers-enabled
```

## Step 4: Run the Worker

**Option A: IPC Mode (Local, No Encryption)**

For local development, use IPC sockets (no authentication needed):

```bash
./run_worker.sh
```

**Option B: TCP Mode (Network, With CURVE Encryption)**

For network deployment or Docker, use TCP with CURVE encryption:

```bash
# Get the enrollment token from the daemon startup output
export MOOR_ENROLLMENT_TOKEN=your-token-here

# Override addresses for TCP
WORKER_REQUEST_ADDR=tcp://localhost:7899 \
WORKER_RESPONSE_ADDR=tcp://localhost:7898 \
./run_worker.sh
```

## Step 5: Test from MOO

Connect to your MOO and run:

```moo
;worker_request("echo", {"Hello", "World", 42})
```

Expected result:

```
=> {"echo_response", "Hello", "World", 42}
```

## Troubleshooting

### "Connection refused" errors

- Check the daemon is running with `--workers-enabled`
- Check the socket addresses match between daemon and worker
- For TCP: ensure the enrollment token is correct

### "CURVE authentication failed"

- For TCP mode: ensure `MOOR_ENROLLMENT_TOKEN` is set correctly
- Delete `.moor-worker-data/` and retry to re-enroll

### "ModuleNotFoundError: No module named 'moor_schema'"

Make sure you're in the `tools/example-python-worker` directory and the `PYTHONPATH` includes the
current directory (the run_worker.sh script does this).

### Worker not receiving requests

- Verify the daemon logs show worker registration
- Check that you're using the correct worker type ("echo")
- Ensure the subscription topic matches ("workers")

## Next Steps

- Read [README.md](README.md) for detailed documentation
- Create custom workers by copying and modifying `echo_worker.py`
- Explore the [curl-worker](../../crates/curl-worker/) Rust implementation
