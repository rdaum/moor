# mooR Python Worker Example

This is a proof-of-concept Python worker implementation for mooR, demonstrating cross-language
worker support using FlatBuffers and ZeroMQ.

## Purpose

This example demonstrates that:

- FlatBuffer schemas work across languages (Rust ↔ Python)
- The worker protocol can be implemented in any language
- PASETO authentication works cross-language
- ZeroMQ messaging is language-agnostic

## Architecture

```
┌─────────────┐                                    ┌──────────────┐
│   mooR      │  ZMQ PUB/SUB (daemon→worker)       │   Python     │
│   Daemon    ├───────────────────────────────────►│   Worker     │
│   (Rust)    │  ipc:///tmp/moor_workers_request   │              │
│             │                                    │              │
│             │  ZMQ REQ/REP (worker→daemon)       │              │
│             │◄───────────────────────────────────┤              │
└─────────────┘  ipc:///tmp/moor_workers_response  └──────────────┘
       ▲                                                  │
       │                                                  │
       └──────── FlatBuffer Messages ────────────────────┘
                  + PASETO Authentication
```

## Setup

### Prerequisites

- Python 3.10 or later
- pip or pip3

### Installation

1. Install dependencies:

```bash
cd tools/example-python-worker
pip install -r requirements.txt
```

2. Generate FlatBuffer bindings (already done, but for reference):

```bash
cd ../../crates/schema/schema
flatc --python -o ../../../tools/example-python-worker/moor_schema/ \
    common.fbs var.fbs moor_program.fbs moor_rpc.fbs moor_event_log.fbs task.fbs
```

## Usage

### Running the Echo Worker

The echo worker is a simple example that returns its arguments unchanged.

**Quick start with helper script:**

```bash
./run_worker.sh
```

Override defaults with environment variables:

```bash
WORKER_REQUEST_ADDR=tcp://localhost:7896 \
WORKER_RESPONSE_ADDR=tcp://localhost:7897 \
./run_worker.sh
```

**Manual invocation:**

With default IPC sockets (recommended for local development):

```bash
source venv/bin/activate
python3 echo_worker.py \
    --public-key /path/to/moor-verifying-key.pem \
    --private-key /path/to/moor-signing-key.pem \
    --request-address ipc:///tmp/moor_workers_request.sock \
    --response-address ipc:///tmp/moor_workers_response.sock
```

Or with TCP addresses (for Docker or networked setups):

```bash
source venv/bin/activate
python3 echo_worker.py \
    --public-key /path/to/public_key.pem \
    --private-key /path/to/private_key.pem \
    --request-address tcp://localhost:7896 \
    --response-address tcp://localhost:7897
```

### Using the Same Keypair as Rust Workers

The Python worker uses the same Ed25519 keypair format as the Rust workers. You can use the same key
files:

```bash
python3 echo_worker.py \
    --public-key ../path/to/rust/worker/public_key.pem \
    --private-key ../path/to/rust/worker/private_key.pem
```

## Implementation Status

### ✅ Completed

- [x] Project structure and dependencies
- [x] FlatBuffer Python bindings generation
- [x] PASETO v4.public token creation
- [x] Ed25519 keypair loading
- [x] ZMQ socket setup (REQ + SUB)
- [x] Worker registration and ping/pong
- [x] WorkerRequest parsing
- [x] WorkerResult response building
- [x] Basic Var type conversion (string, int)
- [x] Echo worker returning list with prepended response
- [x] Integration tested with Rust daemon

### 🚧 Limitations

- Var copying supports only string and int types
- Other types return placeholder strings
- No proper error handling for malformed requests
- No timeout support
- No graceful shutdown on daemon disconnect

### 📋 Future Enhancements

- Complete Var type support (float, list, map, obj, err)
- Add more worker examples (HTTP, math, string manipulation)
- Comprehensive error handling
- Integration tests
- Type hints throughout
- Packaging (setup.py)

## Technical Details

### PASETO Token Format

The worker creates a PASETO v4.public token with:

- **Version**: v4 (modern, secure)
- **Purpose**: public (asymmetric, Ed25519)
- **Payload**: Worker UUID as UTF-8 string
- **Footer**: `key-id:moor_worker`
- **Signature**: Ed25519 private key

Example token structure:

```
v4.public.<base64-payload>.<base64-signature>?key-id:moor_worker
```

### FlatBuffer Message Flow

1. **Attach**: Worker → Daemon
   - Message: `AttachWorker { worker_token, worker_type, worker_id }`
   - Response: Acknowledgment

2. **Subscribe**: Worker listens on PUB/SUB channel

3. **Work Request**: Daemon → Worker
   - Message: `WorkerRequest { request_id, worker_type, perms, arguments, timeout }`

4. **Work Response**: Worker → Daemon
   - Message: `WorkerResult { request_id, result }` or `WorkerError { request_id, error }`

## Current Behavior

The echo worker:

1. Connects to the daemon and registers as an "echo" worker
2. Responds to ping requests to maintain connection
3. Receives work requests with arguments
4. Returns a list containing `"echo_response"` followed by the original arguments
5. Supports string and integer Var types (other types return placeholders)

Test from MOO:

```moo
;worker_request("echo", {"Hello", "World", 123})
=> {"echo_response", "Hello", "World", 123}
```

## Files

- **`requirements.txt`**: Python dependencies
- **`run_worker.sh`**: Helper script to run worker with venv activation
- **`moor_worker.py`**: Core worker protocol implementation
- **`echo_worker.py`**: Echo worker example
- **`verify_setup.py`**: Dependency verification script
- **`moor_schema/`**: Generated FlatBuffer Python bindings
- **`README.md`**: This file

## Debugging

Enable debug output:

```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

Check ZMQ connection:

```bash
# Monitor ZMQ traffic (requires zmq_monitor)
python3 -c "import zmq; print(zmq.zmq_version())"
```

## Extending

To create your own worker:

1. Copy `echo_worker.py` as a starting point
2. Change the `worker_type` to your worker name
3. Modify `_build_request_result()` in `moor_worker.py` to process arguments
4. Add support for additional Var types in `_copy_var()` as needed
5. Update `_handle_request()` to call your processing function

## License

Same as mooR (GPL-3.0)

## References

- [FlatBuffers Python Tutorial](https://flatbuffers.dev/flatbuffers_guide_tutorial.html)
- [PyZMQ Documentation](https://pyzmq.readthedocs.io/)
- [pyseto Documentation](https://pyseto.readthedocs.io/)
- [mooR Worker Protocol](../../crates/curl-worker/src/main.rs)
