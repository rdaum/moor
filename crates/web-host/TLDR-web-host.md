(Read root TLDR-mooR.md for context first)

- This crate contains RESTful web services and WebSocket connectivity to the mooR RPC daemon
- It provides a pure API server with no static file serving or frontend bundling
- The frontend is now a separate TypeScript/Node.js project in the `web-client/` directory
- In development, Vite dev server proxies API calls to this service
- In production, nginx serves static files and proxies API calls to this service
