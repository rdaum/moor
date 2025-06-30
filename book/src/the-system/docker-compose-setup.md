# Docker Compose Setup

Docker Compose is the recommended way to run a mooR server. It handles all the complexity of coordinating multiple components, making it easy to get a complete MOO environment running with minimal configuration.

## What is Docker Compose?

Docker Compose is a tool that helps you define and run multi-container applications. For mooR, it uses a file called `docker-compose.yml` (found in the root of the repository) to describe how each component should run, what environment variables or files it needs, and how the parts connect to each other.

Instead of starting each mooR component manually and configuring their connections, Docker Compose lets you manage everything as a single unit with simple commands.

## Prerequisites

Make sure you have Docker and Docker Compose installed. Most modern Docker installations include Compose by default. You can verify your installation with:

```bash
docker --version
docker compose version
```

## Understanding the Configuration

The `docker-compose.yml` file in the mooR repository defines all the components needed for a complete MOO server:

### Service Definitions

**moor-daemon**
: Configured with authentication keys (`moor-signing-key.pem` and `moor-verifying-key.pem`) and set up to listen for RPC requests from other components.

**moor-telnet-host**
: Connected to the daemon using the same authentication keys, listening on port 8888 by default for traditional telnet connections.

**moor-web-host**
: Connected to the daemon with matching authentication, providing RESTful API endpoints and WebSocket connections for web clients.

**moor-frontend**
: An nginx container serving the TypeScript/VanJS web application and proxying API calls to moor-web-host. Accessible via web browser on port 8080.

**curl-worker**
: Connected to the daemon to handle outbound HTTP requests from MOO code, enabling your MOO to interact with external web services.

## Basic Operations

### Starting Your Server

To start all services, open a terminal in the root of the mooR repository and run:

```bash
docker compose up
```

This will:
1. Build the Docker images (if needed)
2. Start all containers
3. Display logs from all services in your terminal

### Running in the Background

For production or if you want to continue using your terminal for other tasks, run in detached mode:

```bash
docker compose up -d
```

This starts the containers in the background and returns you to the command prompt.

### Viewing Logs

To monitor what's happening with your server:

```bash
# View logs from all services
docker compose logs -f

# View logs from a specific service
docker compose logs -f moor-daemon
docker compose logs -f moor-telnet-host
docker compose logs -f moor-frontend
```

The `-f` flag "follows" the logs, showing new output as it appears.

### Stopping Your Server

If running in the foreground, press `Ctrl+C`. For background services:

```bash
docker compose down
```

This stops and removes the containers but preserves your data volumes.

## Current Configuration Notes

The provided `docker-compose.yml` file is set up to build mooR from the source code in the repository. When you run `docker compose up`, Docker will compile the Rust backend code and build the TypeScript frontend, creating all necessary images.

> **Future Plans**: The configuration may be updated in future releases to use pre-built tagged images, making it faster to run stable releases without building from source.

## Customization

You can modify the `docker-compose.yml` file to suit your needs:

- **Change ports**: Modify the port mappings if you need different external ports
- **Add environment variables**: Configure additional settings for each component
- **Mount volumes**: Persist data or configuration files outside the containers
- **Scale services**: Run multiple instances of hosts or workers for high-traffic scenarios

## Learning from the Configuration

The `docker-compose.yml` file serves as excellent documentation for understanding how to run mooR components manually. Open it in a text editor to see:

- What command-line arguments each binary uses
- What environment variables are needed
- How authentication keys are configured
- How components connect to each other

This information is invaluable if you ever need to set up a custom deployment or debug connection issues.

## Troubleshooting

### Common Issues

**Port conflicts**: If ports 8080 (web) or 8888 (telnet) are already in use, modify the port mappings in the compose file.

**Build failures**: Ensure you have enough disk space and memory for the Rust compilation process.

**Connection issues**: Check that the authentication keys are properly mounted and accessible to all services.

### Getting Help

For more information about Docker Compose itself, see the [official Docker Compose documentation](https://docs.docker.com/compose/).

For mooR-specific issues, check the project's GitHub repository or community forums.
