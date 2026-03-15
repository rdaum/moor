Directory layout for `crates/`

Binaries:

- `daemon` - the actual server runtime. Brings up the database, VM, task scheduler, etc, and
  provides an interface to them over a 0MQ based RPC interface, not exposing any external network
  protocol to the outside world. Instead, that functionality is provided by...
- `telnet-host` - a binary which connects to `daemon` and provides a classic LambdaMOO-style telnet
  interface. The idea being that the `daemon` can go up and down, or be located on a different
  physical machine from the\
  network `host`s
- `web-host` - like the above, but hosts an HTTP server which provides a websocket interface to the
  system. as well as various web APIs.
- `testing/load-tools` - tools for inducing load for transactional consistency test (via jepsen's
  `elle` tool), or for performance testing.
- `testing/moot` - a comprensive test suite for verifying the correctness of the MOO implementation,
  including a battery of tests ported from ToastStunt.

Libraries:

- `var` - implements the basic moor/MOO value types and exports common constants and error structs
  associated with them
- `common` - common model objects and utilities such as WorldState, command matching, and utilities
- `db` - implementation of the `WorldState` object database overtop of `rdb`
- `compiler` - the MOO language grammar, parser, AST, and codegen, as well as the decompiler &
  unparser
- `vm` - the VM execution core: stack frames, activations, environment storage, and unwind logic.
  Pure types with no host/scheduler dependencies.
- `kernel` - the kernel of the MOO driver: task scheduler, builtin functions, and host services that
  wire the VM into the transactional database
- `rpc/rpc-common` - provides types & functions used by both `daemon` and each host binary, for the
  RPC interface
- `rpc/rpc-async-client` - provides an async RPC client for the `daemon`'s RPC interface