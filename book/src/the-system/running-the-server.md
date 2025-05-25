# Running a mooR server installation

Running a mooR server installation is a bit more complicated than the classical LambdaMOO server, which consisted of
a single executable file. mooR is a collection of several components, each of which can be configured and run
independently.

## Docker Compose

However, the most common way to run a mooR server is to simply to use `docker compose` based on the `docker-compose.yml`
file
in the root of the mooR repository. This file defines the various components of the server and their configurations, and
contains comments explaining each component.

### What's in the `docker-compose.yml` file?

The `docker-compose.yml` file defines the following components:

`moor-daemon`: The main server component that hosts the database and handles verb executions. It is configured to use
the `moor-signing-key.pem` and `moor-verifying-key.pem` files for authentication, and listens for RPC requests.

`moor-telnet-host`: A host that provides a Telnet interface for players to connect to the MOO. It connects to the daemon
and
forwards player commands to it. It is configured to use the same keys as the daemon for authentication.
It listens on port 8888 by default.

`moor-web-host`: A host that provides a web interface for players to connect to the MOO. It connects to the daemon and
forwards player commands to it. It presents a web-based client that can be used in a browser, and also exposes a
WebSocket
interface for real-time communication in a manner similar to the Telnet host. It is also configured to use the same keys
as the daemon for authentication.

`curl-worker`: A worker that can be used to make outbound network requests, such as HTTP requests. It connects to the
daemon
and can be used to perform tasks that require network access, such as fetching data from external APIs or sending
notifications.

## Debian packages

Another method is to use the `debian` packages built by the `debian` directory in various mooR repositories, and
provided on the releases page of the mooR GitHub repository. These packages can be installed on a Debian-based
system (like Ubuntu or Debian itself) and will set up the server components for you, after which you can install
the core database you wish to start from and run the server.

The set of arguments to the `mooR` server executable are documented in the
chapter [Server Configuration](server-configuration.md).

## Rolling your own...

If you want to run a mooR server without using Docker or Debian packages, you can do so by compiling the source code
and running the `moor-daemon` binary directly. You will need to create the necessary configuration files and directories
for the server to run, and you will need to set up the necessary keys for authentication. Following the examples in the
`docker-compose.yml` file to see how to set up the various components and their configurations is a good starting point.

### Building from source

To build the mooR server from source, you will need to have the Rust toolchain installed. You can install it using
the `rustup` tool, which is the recommended way to install Rust. Once you have Rust installed, you can clone the mooR
repository from GitHub and build the server using the following commands:

```bash
cargo build --release --all-targets
```

Which will run for some time. After the build is complete, you will find the `moor-daemon` binary, along with the
`moor-telnet-host` and `moor-web-host` binaries, in the `target/release/` directory.
