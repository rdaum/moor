# mooR Web Client

A rich web client for connecting to mooR servers via the `moor-web-host` API service.

## Overview

This web client provides a browser interface for interacting with mooR worlds, communicating with
the backend through WebSocket connections and RESTful API calls handled by the `moor-web-host`
binary.

## Development

```bash
# Start development server
npm run dev

# Start daemon in development
npm run daemon:dev

# Start web-host in development
npm run web-host:dev

# Start daemon, web-host, and web client together
npm run full:dev

# Use custom core database (defaults to cores/lambda-moor)
MOOR_CORE=MyCore.db npm run daemon:dev

# Build for production
npm run build

# Type checking
npm run typecheck

# Linting
npm run lint
```

## Dependencies

Running the daemon, host, and client requires the same dependencies for those listed in the
Dockerfile. Specifically you'll need Rust, NodeJS, npm, and those packages the Dockerfile gets with
apt install such as clang. After installing them, run openssl commands in the same Dockerfile to
generate the keypair, run npm install, and then you are ready to run the commands above such as _npm
run full:dev_.

## Development on Windows

Various packages the daemon depends on do not support Windows, but work fine from within a linux
file system in WSL 2. You must install the dependencies in the linux file system and not the Windows
file system that gets mounted automatically under /mnt/c. Happily, there is a WSL Extension for VS
Code that lets you interact normally with the linux files. It's worth reviewing the
[Windows developer documentation for node in WSL](https://learn.microsoft.com/en-us/windows/dev-environment/javascript/nodejs-on-wsl)
and learning how
[WSL and Windows files and commands interoperate](https://learn.microsoft.com/en-us/windows/wsl/filesystems)
to avoid surprises.

For more details on the overall system architecture, see the
[server architecture documentation](https://timbran.codeberg.page/moor-book-html/the-system/server-architecture.html).
