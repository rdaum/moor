# Server Architecture

Understanding mooR's architecture is key to successfully running and maintaining a mooR server. Unlike traditional MOO
servers that consisted of a single executable, mooR is designed as a modular system with multiple components working
together.

## Component Overview

A complete mooR installation consists of several specialized components:

### Core Components

**moor-daemon**
: The heart of the system. This component manages the MOO database, executes verbs, handles object manipulation, and
coordinates all MOO operations. Think of it as the "brain" that understands MOO code and maintains the virtual world's
state.

**moor-telnet-host**
: Provides traditional telnet access for players. This component handles the classic MOO experience that players
familiar with LambdaMOO expect - text-based connections over port 8888 (by default).

**moor-web-host**
: Provides RESTful API endpoints and WebSocket connections for web clients. This component handles authentication,
property access, verb execution, and real-time communication via WebSockets.

**moor-frontend**
: The web client application. This is served by nginx in production or Vite in development, providing the browser-based
interface that communicates with moor-web-host.

**curl-worker**
: Handles outbound HTTP requests from MOO code. When your MOO needs to fetch data from external APIs, send webhooks, or
interact with web services, this component manages those network operations safely.

## How Components Communicate

All components communicate through authenticated RPC (Remote Procedure Call) connections:

- The **daemon** acts as the central coordinator
- **Hosts** (telnet and web) connect to the daemon to relay player commands and receive responses
- **Workers** (like curl-worker) connect to the daemon to handle specific tasks
- Transport security uses CURVE encryption (for TCP) with enrollment-based authentication
- Client/player authentication uses PASETO tokens signed by the daemon

## Advantages of This Design

**Flexibility**: You can run different components on different machines or scale them independently based on your needs.

**Security**: Network operations are isolated in separate workers, reducing security risks to the core MOO environment.

**Modernization**: The modular design allows adding new connection types (like web interfaces) without changing the core
MOO logic.

**Reliability**: If a host crashes, only that connection type is affected - the core MOO world continues running.

## Build and Performance Considerations

### Build Profiles

mooR supports configurable build profiles to balance compilation time with runtime performance:

**Debug Profile (Default)**

- Optimized for fast builds during development
- Includes debug symbols for troubleshooting
- Suitable for development, testing, and small-scale deployments
- Significantly faster Docker builds (minutes vs. tens of minutes)

**Release Profile**

- Optimized for production performance
- Aggressive compiler optimizations and link-time optimization (LTO)
- Smaller binary sizes and maximum runtime performance
- Longer build times due to optimization passes

The frontend (web-based) component always uses optimized builds via Vite, regardless of the backend build profile.

### Deployment Approaches

This modular architecture means there are more pieces to coordinate compared to traditional MOO servers. However, mooR
provides several approaches to make this manageable:

- **Docker Compose**: Orchestrates all components automatically (recommended for most users)
- **Debian Packages**: Handles system integration for Debian-based systems
- **Manual Setup**: For custom deployments or development environments

**Docker Compose Examples:**

```bash
# Development build (fast compilation)
docker compose up

# Production build (optimized performance)
BUILD_PROFILE=release docker compose up
```

The next sections cover each of these deployment approaches in detail.
