# Docker Compose Setup

Docker Compose is the recommended way to deploy a mooR server. It handles all the complexity of coordinating multiple
components, making it easy to get a complete MOO environment running with minimal configuration.

## What is Docker Compose?

Docker Compose is a tool that helps you define and run multi-container applications. Instead of starting each mooR
component manually and configuring their connections, Docker Compose lets you manage everything as a single unit with
simple commands.

## Deployment Configurations

mooR provides several Docker Compose configurations to suit different needs:

### Production Deployments

The `deploy/` directory contains production-ready configurations:

**`deploy/telnet-only/`**
: Minimal configuration with just the daemon and telnet host. Ideal for traditional MUD usage without web components.

**`deploy/web-basic/`**
: Full web-enabled deployment with HTTP. Use behind a reverse proxy that handles SSL, or for internal networks.

**`deploy/web-ssl/`**
: Full web-enabled deployment with HTTPS/TLS using Let's Encrypt certificates. Recommended for internet-facing production servers.

**`deploy/debian-packages/`**
: Native systemd services for Debian/Ubuntu without Docker.

Each deployment includes:
- Release builds optimized for performance
- Automated testing scripts
- Detailed README with setup instructions
- Services run as your user to avoid permission issues

**→ For production use, choose one of the above configurations and follow its README.**

### Development & Quick Start

**`docker-compose.yml`** (repository root)
: For development, testing, and quick evaluation. Uses debug builds for faster compilation. Services communicate via IPC (Unix domain sockets).

**`docker-compose.cluster.yml`**
: For testing multi-machine deployments with TCP and CURVE encryption.

## Prerequisites

- Docker and Docker Compose installed (most modern Docker installations include Compose by default)
- At least 1GB RAM recommended
- Ports 8080 (web) and/or 8888 (telnet) available, depending on configuration

You can verify your Docker installation with:

```bash
docker --version
docker compose version
```

## Production Deployment Setup

### Choosing a Configuration

1. **Traditional MUD** (telnet only): Use `deploy/telnet-only/`
2. **Web-enabled, behind reverse proxy**: Use `deploy/web-basic/`
3. **Web-enabled, internet-facing**: Use `deploy/web-ssl/`
4. **Native packages** (no Docker): Use `deploy/debian-packages/`

### Deployment Steps

All production configurations follow the same basic steps:

1. **Copy the deployment directory** to your server:
   ```bash
   cp -r deploy/web-basic /path/to/deployment
   cd /path/to/deployment
   ```

2. **Start the services** using the included start script:
   ```bash
   ./start.sh
   ```

   The start script handles user permissions and directory setup automatically.

3. **Verify deployment**:
   ```bash
   docker compose ps
   docker compose logs -f
   ```

4. **Test the deployment** (optional but recommended):
   ```bash
   ./test.sh
   ```

Each configuration's README provides specific instructions and customization options.

### Service Components

A complete mooR deployment includes:

**moor-daemon**
: The core MOO server handling database, task scheduling, and execution.

**moor-telnet-host**
: Traditional telnet interface (port 8888 by default).

**moor-web-host**
: REST API and WebSocket server for web clients.

**moor-frontend**
: nginx serving the web client and proxying API requests (web deployments only).

**moor-curl-worker**
: Handles outbound HTTP requests from MOO code.

Services communicate via Unix domain sockets (IPC) and run as your user to avoid permission issues.

## Development Quick Start

For development, testing, and quick evaluation, mooR provides two pre-configured start scripts in the repository root. These scripts automatically handle Docker permissions, core-specific feature flags, and resource isolation.

### 1. Choose a Core

**Cowbell** (Modern core with web-native features):
```bash
./scripts/start-moor-cowbell.sh
```

**LambdaCore** (Classic LambdaMOO core, 1.8.x compatible):
```bash
./scripts/start-moor-lambdacore.sh
```

### 2. Isolated Environments

Each script uses its own isolated runtime directory:
- `run-cowbell/`
- `run-lambda-moor/`

This ensures that you can switch between cores without database or keypair "pollution". Each environment maintains its own persistent database, configuration, and host keys.

### 3. Build Profiles

By default, these scripts use a high-performance **release** build (`release-fast`). For a faster initial compile during development, you can use the `--debug` flag:

```bash
./scripts/start-moor-cowbell.sh --debug
```

Access the system via:
- **Web Client**: http://localhost:8080
- **Telnet Interface**: `telnet localhost 8888`

## Common Operations

These commands work for all Docker Compose configurations:

### Viewing Logs

```bash
# View logs from all services
docker compose logs -f

# View logs from a specific service
docker compose logs -f moor-daemon
docker compose logs -f moor-telnet-host
```

The `-f` flag "follows" the logs, showing new output as it appears.

### Stopping Services

If running in the foreground, press `Ctrl+C`. For background services:

```bash
docker compose down
```

This stops and removes containers but preserves data directories.

### Restarting After Changes

```bash
docker compose restart
```

### Rebuilding After Updates

```bash
docker compose build --no-cache
docker compose up -d
```

## Data Persistence

All Docker Compose configurations store data in local directories. For the development scripts, these are consolidated under core-specific runtime directories:

- `./run-cowbell/` or `./run-lambda-moor/`
  - `moor-data/` - Main database directory
  - `*-host-data/` - Host-specific state and keys
  - `config/` - Server cryptographic keypairs
  - `ipc/` - Unix domain sockets for inter-service communication

**Important**: These directories are created with your user permissions. Always backup your data regularly.

### Automatic Database Exports

The daemon automatically exports the database at regular intervals (configured via `--export-interval` CLI argument in your docker-compose configuration). These exports are written in **[objdef format](objdef-file-format.md)** - a human-readable, text-based representation of your database.

**Objdef exports are your most valuable backup:**

- **Human-readable and editable**: You can read, understand, and manually edit the exported files
- **Version control friendly**: Text format works well with git, allowing you to track changes over time
- **Compression-friendly**: Objdef files compress extremely well, making archives space-efficient
- **Format-stable**: While the binary database format may change between mooR versions, objdef remains stable and portable

The binary database (`moor.db/`) is optimized for consistency and instant startup, but the objdef exports in `moor-data/` are the "gold standard" backup format. Copy these exports regularly to safe storage, compress them, and consider putting them in revision control for change tracking.

## Customization

You can modify `docker-compose.yml` files to suit your needs:

- **Change ports**: Edit the `ports:` mappings
- **Configure services**: Add environment variables or command-line flags
- **Scale workers**: Run multiple curl-worker instances for high-traffic scenarios

For nginx configuration (web deployments), edit `nginx.conf` and restart the frontend:

```bash
docker compose restart moor-frontend
```

## Troubleshooting

### Common Issues

**Port conflicts**
: If ports 8080 (web) or 8888 (telnet) are already in use, modify the port mappings in the compose file.

**Permission denied errors**
: Use the `./start.sh` script which handles user permissions automatically. If running `docker compose` directly, ensure you've exported `USER_ID` and `GROUP_ID` environment variables and pre-created the data directories.

**Services won't start**
: Check logs with `docker compose logs <service-name>`. Verify all required directories exist and are accessible.

**Build failures**
: Ensure you have enough disk space and memory. Rust compilation requires substantial resources.

**Connection issues**
: Verify all services are running with `docker compose ps`. Check that `moor-ipc/` directory is accessible.

**Database won't import**
: First startup imports the core database, which can take several minutes. Check `docker compose logs moor-daemon` for progress.

### Testing Your Deployment

Production configurations in `deploy/` include test scripts:

```bash
cd deploy/web-basic
./test.sh
```

These validate that services are running correctly and can communicate.

### Getting Help

- **Docker Compose docs**: [docs.docker.com/compose/](https://docs.docker.com/compose/)
- **mooR issues**: [codeberg.org/timbran/moor/issues](https://codeberg.org/timbran/moor/issues)
- **Community**: [Discord](https://discord.gg/Ec94y5983z)

## Advanced: Multi-Machine Deployments

For running services across multiple machines, see `docker-compose.cluster.yml` which demonstrates:
- TCP with CURVE encryption for inter-service communication
- Enrollment token setup for host authentication
- Network configuration for distributed deployments

This is an advanced configuration. Most users should use single-machine IPC-based deployments.
