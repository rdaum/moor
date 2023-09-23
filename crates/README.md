Directory layout for `crates/`

Binaries:
  * `daemon` - the actual server runtime. Brings up the database, VM, task scheduler, etc, and provides an interface
     to them over a 0MQ based RPC interface, exposing any external network protocol to the outside world. 
     Instead, that functionality is provided by...
  * `telnet-host` - a binary which connects to `daemon` and provides a classic LambdaMOO-style telnet interface.
     The idea being that the `daemon` can go up and down, or be located on a different physical machine from the  
     network `host`s
  * `web-host` - like the above, but hosts an HTTP server which provides a websocket interface to the system.
     as well as various web APIs.
  * `console-host` - console host which connects as a user to the `daemon` and provides a readline-type interface to the
     system.

Libraries:
  * `values` - crate that implements the core MOO discriminated union (`Var`) value type,
     plus all associated types and traits.
  * `kernel` - the actual implementation of the system: database, compiler, virtual machine,
     task scheduler, implementations of all builtin functions, etc.
  * `rpc-common` - provides types & functions used by both `daemon` and each host binary, for the RPC interface
  * `regexpr-binding` - provides bindings to the old regular expressions library used by
     the LambdaMOO server, for compatibility with existing cores. This is a temporary measure until
     this can be reworked with use of the `regex` crate and some compatibility translation
