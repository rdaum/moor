# mooR Python Worker Example

This is a proof-of-concept Python worker implementation for mooR, demonstrating cross-language
worker support using FlatBuffers and ZeroMQ with CURVE encryption.

## Purpose

This example demonstrates that:

- FlatBuffer schemas work across languages (Rust <-> Python)
- The worker protocol can be implemented in any language
- CURVE authentication works cross-language for TCP connections
- ZeroMQ messaging is language-agnostic

## Architecture

```
+-------------+                                    +--------------+
|   mooR      |  ZMQ PUB/SUB (daemon->worker)      |   Python     |
|   Daemon    +------------------------------------>   Worker     |
|   (Rust)    |  ipc:///tmp/moor_workers_request   |              |
|             |                                    |              |
|             |  ZMQ REQ/REP (worker->daemon)      |              |
|             |<-----------------------------------+              |
+-------------+  ipc:///tmp/moor_workers_response  +--------------+
       ^                                                  |
       |                                                  |
       +-------- FlatBuffer Messages ---------------------+
                  + CURVE Authentication (TCP only)
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
    common.fbs var.fbs moor_program.fbs moor_rpc.fbs
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
# For IPC (local development, no encryption)
WORKER_REQUEST_ADDR=ipc:///tmp/moor_workers_request.sock \
WORKER_RESPONSE_ADDR=ipc:///tmp/moor_workers_response.sock \
./run_worker.sh

# For TCP (requires enrollment)
MOOR_ENROLLMENT_TOKEN=your-enrollment-token \
WORKER_REQUEST_ADDR=tcp://localhost:7899 \
WORKER_RESPONSE_ADDR=tcp://localhost:7898 \
./run_worker.sh
```

**Manual invocation:**

With default IPC sockets (recommended for local development, no encryption needed):

```bash
python3 echo_worker.py \
    --request-address ipc:///tmp/moor_workers_request.sock \
    --response-address ipc:///tmp/moor_workers_response.sock
```

With TCP addresses (for Docker or networked setups, uses CURVE encryption):

```bash
# Set enrollment token
export MOOR_ENROLLMENT_TOKEN=your-enrollment-token

python3 echo_worker.py \
    --request-address tcp://localhost:7899 \
    --response-address tcp://localhost:7898 \
    --enrollment-address tcp://localhost:7900 \
    --data-dir ./.moor-worker-data
```

### Authentication

#### IPC Mode (Local Development)

When using IPC sockets (`ipc://` addresses), no authentication is required. This is recommended for
local development.

#### TCP Mode (Network Deployment)

When using TCP sockets (`tcp://` addresses), the worker automatically uses CURVE encryption for
secure communication. This requires:

1. **Enrollment token**: The daemon generates an enrollment token that workers use to register
   themselves. Set via:
   - `MOOR_ENROLLMENT_TOKEN` environment variable
   - `--enrollment-token-file` argument
   - Default XDG location: `~/.config/moor/enrollment-token`

2. **Enrollment process**: On first connection, the worker:
   - Generates a CURVE25519 keypair
   - Sends an enrollment request with its public key
   - Receives the daemon's public key
   - Saves identity to disk for future runs

3. **Subsequent runs**: The worker loads its saved identity and keys.

## Implementation Status

### Completed

- [x] Project structure and dependencies
- [x] FlatBuffer Python bindings generation
- [x] CURVE key generation and management
- [x] Enrollment client for daemon registration
- [x] ZMQ socket setup (REQ + SUB) with CURVE encryption
- [x] Worker registration and ping/pong
- [x] WorkerRequest parsing
- [x] WorkerResult response building
- [x] Basic Var type conversion (string, int, float, list)
- [x] Echo worker returning list with prepended response
- [x] Integration tested with Rust daemon

### Limitations

- Var copying supports only string, int, float, and list types
- Other types return placeholder strings
- No proper error handling for malformed requests
- No timeout support
- No graceful shutdown on daemon disconnect

### Future Enhancements

- Complete Var type support (map, obj, err, symbol)
- Add more worker examples (HTTP, math, string manipulation)
- Comprehensive error handling
- Integration tests
- Type hints throughout
- Packaging (setup.py)

## Technical Details

### CURVE Authentication

The worker uses ZMQ CURVE encryption for TCP connections:

- **Key generation**: CURVE25519 keypairs (40-character Z85-encoded)
- **Enrollment**: Worker sends public key to daemon, receives daemon's public key
- **Encryption**: All subsequent messages are encrypted end-to-end
- **Identity persistence**: Keys and identity saved to `{data-dir}/{service-type}-*.{key,pub,json}`

### FlatBuffer Message Flow

1. **Enroll** (TCP only): Worker -> Daemon
   - Message: `EnrollmentRequest { enrollment_token, curve_public_key, service_type, hostname }`
   - Response: `EnrollmentResponse { success, service_uuid, daemon_curve_public_key }`

2. **Attach**: Worker -> Daemon
   - Message: `AttachWorker { worker_id, worker_type }`
   - Multipart: `[worker_id_bytes, flatbuffer_payload]`
   - Response: `WorkerAttached { worker_id }` or error

3. **Subscribe**: Worker listens on PUB/SUB channel (topic: "workers")

4. **Work Request**: Daemon -> Worker
   - Message: `WorkerRequest { worker_id, id, perms, request, timeout_ms }`

5. **Work Response**: Worker -> Daemon
   - Message: `RequestResult { worker_id, request_id, result }` or `RequestError { ... }`

## Current Behavior

The echo worker:

1. Connects to the daemon and registers as an "echo" worker
2. Responds to ping requests to maintain connection
3. Receives work requests with arguments
4. Returns a list containing `"echo_response"` followed by the original arguments
5. Supports string, integer, float, and list Var types (other types return placeholders)

Test from MOO:

```moo
;worker_request("echo", {"Hello", "World", 123})
=> {"echo_response", "Hello", "World", 123}
```

## Files

- **`requirements.txt`**: Python dependencies (pyzmq, flatbuffers)
- **`run_worker.sh`**: Helper script to run worker with venv activation
- **`moor_worker.py`**: Core worker protocol implementation with CURVE auth
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
3. Create a custom `_build_request_result()` method in your worker subclass
4. Add support for additional Var types in `_copy_var()` as needed
5. Update `_handle_request()` to call your processing function

## License

Same as mooR (GPL-3.0)

## References

- [FlatBuffers Python Tutorial](https://flatbuffers.dev/flatbuffers_guide_tutorial.html)
- [PyZMQ Documentation](https://pyzmq.readthedocs.io/)
- [ZMQ CURVE Security](http://curvezmq.org/)
- [mooR Worker Protocol](../../crates/curl-worker/src/main.rs)
