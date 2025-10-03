# Quick Start Guide

Get the Python worker example running in 5 minutes.

## 1. Install Dependencies

### Option A: Using Virtual Environment (Recommended)

```bash
cd tools/example-python-worker

# Install venv if not already installed (Debian/Ubuntu)
sudo apt install python3-venv

# Create and activate virtual environment
python3 -m venv venv
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

### Option B: System-Wide Installation (Not Recommended)

```bash
cd tools/example-python-worker
pip install -r requirements.txt --break-system-packages
```

### Option C: Using System Packages

```bash
# Install via apt (Debian/Ubuntu)
sudo apt install python3-zmq python3-flatbuffers python3-cryptography

# Note: pyseto may not be available, would need pip or pipx
pipx install pyseto
```

## 2. Verify Setup

Run the verification script to ensure everything is installed correctly:

```bash
python3 verify_setup.py
```

You should see all checks pass with green ✓ marks.

## 3. Get Keypair Files

You need Ed25519 keypair files in PEM format. You can either:

### Option A: Use Existing Rust Worker Keys

If you already have keys for the Rust curl-worker, use those:

```bash
# Example paths - adjust to your setup
PUBLIC_KEY=~/.moor/public_key.pem
PRIVATE_KEY=~/.moor/private_key.pem
```

### Option B: Generate New Keys

Generate a new Ed25519 keypair using OpenSSL:

```bash
# Generate private key
openssl genpkey -algorithm ED25519 -out private_key.pem

# Extract public key
openssl pkey -in private_key.pem -pubout -out public_key.pem
```

## 4. Start the mooR Daemon

In another terminal, start the mooR daemon (if not already running):

```bash
cd ../../
cargo run --bin moor-daemon
```

The daemon uses IPC sockets by default:
- Workers request: `ipc:///tmp/moor_workers_request.sock`
- Workers response: `ipc:///tmp/moor_workers_response.sock`

For TCP (Docker), the ports are typically:
- Workers request: `tcp://localhost:7896`
- Workers response: `tcp://localhost:7897`

## 5. Run the Echo Worker

### Using the Helper Script (Recommended)

The easiest way to run the worker:

```bash
./run_worker.sh
```

This script handles venv activation and uses sensible defaults. You can override settings with environment variables:

```bash
WORKER_PUBLIC_KEY=/path/to/key.pem WORKER_PRIVATE_KEY=/path/to/key.pem ./run_worker.sh
```

### Manual Invocation

If you prefer to run directly (requires bash-compatible shell):

```bash
source venv/bin/activate
python3 echo_worker.py \
    --public-key /home/ryan/moor/moor-verifying-key.pem \
    --private-key /home/ryan/moor/moor-signing-key.pem \
    --request-address ipc:///tmp/moor_workers_request.sock \
    --response-address ipc:///tmp/moor_workers_response.sock
```

For TCP addresses (e.g., Docker):

```bash
source venv/bin/activate
python3 echo_worker.py \
    --public-key public_key.pem \
    --private-key private_key.pem \
    --request-address tcp://localhost:7896 \
    --response-address tcp://localhost:7897
```

## 6. Test from MOO

Once connected, you can test the echo worker from MOO code:

```moo
; Call the echo worker
worker_request("echo", {"Hello", "World", 123})
```

This should return the arguments unchanged: `{"Hello", "World", 123}`.

## Troubleshooting

### Import errors

```bash
pip install -r requirements.txt
```

### Connection refused

- Make sure the daemon is running
- Check that the addresses match (--request-address, --response-address)
- Verify the daemon is listening on the worker ports

### Authentication failed

- Ensure you're using the correct keypair files
- The public key must be registered with the daemon
- Check that the PEM files are readable

### No FlatBuffer schemas

If you get import errors for `moor_schema.*`:

```bash
cd ../../crates/schema/schema
flatc --python -o ../../../tools/example-python-worker/moor_schema/ \
    common.fbs var.fbs moor_program.fbs moor_rpc.fbs moor_event_log.fbs task.fbs
```

## Next Steps

- Read the [README.md](README.md) for implementation details
- Examine [moor_worker.py](moor_worker.py) to understand the protocol
- Create your own worker by copying [echo_worker.py](echo_worker.py)
- Complete the FlatBuffer message serialization/deserialization

## Current Status

This implementation is working with basic functionality:

- ✓ Connects to daemon
- ✓ Authenticates with PASETO tokens
- ✓ Sets up ZMQ sockets
- ✓ Parses WorkerRequest messages
- ✓ Builds WorkerResult responses
- ✓ Handles string and int Var types
- ✓ Returns list with "echo_response" prepended to arguments
- ⚠ Other Var types return placeholders

The worker successfully demonstrates cross-language FlatBuffer support and is functional for basic use cases!
