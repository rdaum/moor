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

# Use custom core database (defaults to cores/JHCore-DEV-2.db)
MOOR_CORE=MyCore.db npm run daemon:dev

# Build for production
npm run build

# Type checking
npm run typecheck

# Linting
npm run lint
```

For more details on the overall system architecture, see the
[server architecture documentation](../book/src/the-system/server-architecture.md).
