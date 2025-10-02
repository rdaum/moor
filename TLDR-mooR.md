- mooR is a 21st century implementation of LambdaMOO, on steroids
  - See: https://en.wikipedia.org/wiki/MOO for a TLDR on that

- mooR consists of:
  - a virtual machine / compiler / runtime for a custom language ("MOO") based on the LambdaMOO
    "MOO" language
  - a shared object database with serializable transational isolation
  - a prototype object model with "properties" and "verbs" (methods & commands)
  - a simple command parser that breaks commands into pieces to execute verbs
  - a scheduler which manages task / verb / command executions
  - a ZeroMQ "RPC" server which submits to said scheduler, using FlatBuffers for serialization over
    ZMQ
  - various "host" processes which turn inbound network protocols into RPC calls

- It is written in Rust
- It is broken into a Cargo workspace, with the crate structure:
  - crates/var - the enumerated variant for value types
  - crates/common - various common model objects and utilities + program/opcode structure
  - crates/compiler - compiler/lexer, syntax tree, decompiler
  - crates/kernel - virtual machine, builtin functions, and task scheduler
  - crates/daemon - rpc host that talks to the kernel's scheduler to do things
  - crates/telnet-host - exposes a line-oriented TCP stream that RPCs to the daemon
  - crates/web-host - exposes WebSocket and REST API endpoints for web clients
  - crates/rpc/* - shared rpc common bits.
  - web-client/ - Web-based MOO client, which communicates with the web services defined by
    `web-host`

* For more details there is a mdbook under book/, which can be explored
* The backend uses Rust 1.88.0, the frontend uses modern TypeScript/Node.js/Vite
* We aim for concise, clean, and poetic code, with plenty of comments describing intent / desire as
  breadcrumbs for our futureselves
* Our MOO dialect has a number of powerful additions to bring it up to 21st century standards:
  lexical scopes, maps (dictionaries), a lightweight immutable object type, list comprehensions
* We would eventually like to be able to host languages other than MOO (such as e.g. JavaScript or
  Lua) in our runtime, but have not done this yet
* Backwards compatibility with LambdaMOO is very important, and we've also brought in some
  compatibility with ToastStunt (a LambdaMOO fork) but we are not fully compatible with it
* However our ultimate goal is a much richer front-end experience than line-based telnet.

Finally: We're enot aiming for nostalgia / replication of the past, but its modernization.
