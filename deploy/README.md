# mooR Deployment Guide

This directory contains deployment configurations and guides for various mooR deployment scenarios.
Choose the approach that best fits your needs.

## Single-Machine vs Multi-Machine Deployments

**Important**: These examples use IPC (Unix domain sockets) for communication between mooR
components because they're designed for single-machine deployments.

mooR **can** be deployed across multiple machines using TCP sockets with CURVE encryption and
enrollment tokens for security. However, these examples don't show that configuration because:

- **Single-machine deployments** (Docker Compose, process-compose, Debian packages) use IPC for
  simplicity, security, and performance
- **Multi-machine deployments** (Kubernetes, manual multi-host setups) would use TCP with CURVE
  enrollment
- The complexity of enrollment tokens and CURVE key management is unnecessary when all services run
  on the same host

If you need multi-machine deployment examples, see the Kubernetes section or consider contributing
to expand these examples.

## Quick Decision Guide

**Are you...**

- **Just getting started or developing?** → Use the [root docker-compose.yml](../docker-compose.yml)
  for quick development setup (IPC-based, no enrollment tokens)
- **Testing TCP/CURVE enrollment flows?** → Use
  [docker-compose.cluster.yml](../docker-compose.cluster.yml) for multi-machine deployment testing

- **Deploying with web access on a local network?** → See [web-basic/](web-basic/) for HTTP-only
  Docker deployment

- **Running a production web-enabled MOO on the internet?** → See [web-ssl/](web-ssl/) for HTTPS
  Docker deployment with Let's Encrypt

- **Setting up a classic telnet-only MOO?** → See [telnet-only/](telnet-only/) for minimal Docker
  deployment

- **Installing on a traditional Linux server?** → See [debian-packages/](debian-packages/) for
  systemd-based deployment

- **Need Kubernetes?** → See [kubernetes/](kubernetes/) (contributions welcome, you masochist!)

## Deployment Options

### 1. Development Options

Several tools are available for local development and testing:

#### Docker Compose (Containerized)

**Location**: [../docker-compose.yml](../docker-compose.yml)

**Purpose**: Quick containerized development setup

Uses fast debug builds without optimization, running all services in containers isolated from the
host system. **Not recommended for production** deployments.

**Quick start**:

```bash
docker compose up
```

#### process-compose (Native)

**Location**: [../process-compose.yaml](../process-compose.yaml) and
[../process-compose-dev.yaml](../process-compose-dev.yaml)

**Purpose**: Run all mooR services natively on your host using process orchestration

Runs all mooR services natively on your host with no Docker overhead, managing them as local
processes using IPC for inter-service communication. The `process-compose-dev.yaml` variant uses
debug builds for faster iteration, while `process-compose.yaml` uses release builds (slow build
times but optimized runtime performance).

**Prerequisites**: Install [process-compose](https://github.com/F1bonacc1/process-compose)

**More info**:
[process-compose documentation](https://f1bonacc1.github.io/process-compose/launcher/)

**Quick start**:

```bash
# Development mode (debug builds)
process-compose -f process-compose-dev.yaml up

# Production-like mode (release builds)
process-compose up
```

#### bacon (File-Watching Development)

**Location**: [../bacon.toml](../bacon.toml)

**Purpose**: File-watching development with automatic restarts

Watches source files for changes and automatically rebuilds and restarts services. Provides separate
jobs for daemon, telnet, and web host, making it ideal for rapid iteration when working on a single
service.

**Prerequisites**: Install bacon (`cargo install bacon`)

**More info**: [bacon documentation](https://dystroy.org/bacon/)

**Available jobs**:

```bash
bacon daemon          # Run daemon with file watching (release build)
bacon daemon-debug    # Run daemon with file watching (debug build)
bacon daemon-debug-traced  # Run daemon with tracing enabled
bacon telnet          # Run telnet host with file watching
bacon web             # Run web host with file watching
bacon test            # Run tests with file watching
bacon curl-worker     # Run curl worker with file watching
```

#### npm Scripts (Web Client Development)

**Location**: [../package.json](../package.json)

**Purpose**: Workflows for web client development in [web-client/](../web-client/)

Starts daemon, web-host, and Vite dev server together using concurrently, providing hot module
reloading for web client changes. Includes tracing variants for debugging backend issues while
working on the UI. **Primary use case is active development on the web client UI** in `web-client/`.

**Available scripts**:

```bash
# Development servers
npm run dev                  # Web client dev server only (port 3000)
npm run daemon:dev           # Daemon only (debug build)
npm run daemon:traced        # Daemon with tracing enabled
npm run web-host:dev         # Web host only

# Full stack
npm run full:dev             # All services: web client + daemon + web-host
npm run full:dev-traced      # All services with daemon tracing

# Build
npm run build                # Build web client
npm run full:build           # Build web client + web host (release)
```

**Recommended Development Workflow**:

1. **First time / Quick demo**: Use Docker Compose
   ```bash
   docker compose up
   ```

2. **Active web client development** (working in `web-client/`): Use npm scripts
   ```bash
   npm run full:dev      # Starts daemon, web-host, and Vite dev server with HMR
   ```

3. **Active backend development** (working on Rust code): Use bacon for file watching
   ```bash
   bacon daemon-debug    # Terminal 1: daemon with file watching
   bacon telnet          # Terminal 2: telnet host with file watching (optional)
   npm run dev           # Terminal 3: web client dev server only
   ```

4. **Testing full stack natively**: Use process-compose
   ```bash
   process-compose -f process-compose-dev.yaml up
   ```

---

### 2. Web-Basic Deployment (HTTP)

**Location**: [web-basic/](web-basic/)

**Purpose**: Full-featured deployment with web client, HTTP only

Provides a modern web interface with WebSocket support alongside traditional telnet access, running
over HTTP without SSL/TLS encryption. Suitable for local network deployments, running behind an
external reverse proxy that handles SSL, development/testing environments, or as a quick web-enabled
setup.

**Quick start**:

```bash
cd web-basic
cp .env.example .env
docker compose up -d
# Visit http://localhost:8080
```

[Read full guide →](web-basic/README.md)

---

### 3. Web-SSL Deployment (HTTPS)

**Location**: [web-ssl/](web-ssl/)

**Purpose**: Production internet-facing deployment with HTTPS

Production-type configuration with automatic Let's Encrypt SSL certificates, modern TLS, and HTTP to
HTTPS redirect. Includes both web client and telnet access. Designed for public internet-facing MOO
servers and production deployments requiring trusted SSL certificates.

Obviously for running on your own host you will need to use this as a starting place, since every
hosting situation is a little bit different, and everybody has their own preferences for what
packages to use and how to configure them. But this should give you an idea of how the pieces fit
together.

**Prerequisites**: Domain name pointing to your server, with ports 80 and 443 accessible from the
internet.

**Quick start**:

```bash
cd web-ssl
cp .env.example .env
# Edit .env with your domain and email
docker compose up -d
# Obtain certificate (see guide)
```

[Read full guide →](web-ssl/README.md)

---

### 4. Telnet-Only Deployment

**Location**: [telnet-only/](telnet-only/)

**Purpose**: Traditional MUD/MOO server without web interface

Minimal resource deployment providing a classic telnet-only experience with production-ready release
builds and no web dependencies. Ideal for traditional MOO/MUD communities, users who prefer telnet
clients, or environments with limited server resources.

**Quick start**:

```bash
cd telnet-only
cp .env.example .env
docker compose up -d
telnet localhost 8888
```

[Read full guide →](telnet-only/README.md)

---

### 5. Debian Package Deployment

**Location**: [debian-packages/](debian-packages/)

**Purpose**: Traditional Linux installation with systemd services

Native Linux installation using standard Debian/Ubuntu package management with systemd service
control. Provides separate packages for each component: `moor-daemon` (core MOO server),
`moor-telnet-host` (telnet server), `moor-web-host` (web API server), and `moor-web-client` (static
web files). Best suited for traditional Linux server administration, integration with existing
system management tools, and users comfortable with systemd on Debian/Ubuntu based systems.

**Quick start**:

```bash
cd debian-packages
./build-all-packages.sh
sudo dpkg -i moor-daemon_*.deb
sudo dpkg -i moor-telnet-host_*.deb
sudo dpkg -i moor-web-host_*.deb
sudo systemctl start moor-daemon
```

[Read full guide →](debian-packages/README.md)

---

### 6. Kubernetes Deployment

**Location**: [kubernetes/](kubernetes/)

**Status**: Planned for future release

**Contributions welcome!** If you'd like to contribute Kubernetes manifests, please see the
[kubernetes README](kubernetes/README.md).

---

## Architecture Overview

All deployment options use the same mooR architecture:

```
┌─────────────────┐
│   Web Client    │ (Optional: Browser-based interface)
│  (nginx + JS)   │
└────────┬────────┘
         │ HTTP/WebSocket
┌────────▼────────┐
│  moor-web-host  │ (Optional: Web API server)
└────────┬────────┘
         │
         │ ZeroMQ RPC
┌────────▼────────┐        ┌──────────────┐
│  moor-daemon    │◄───────┤ Telnet users │
│  (Core MOO VM)  │        └──────────────┘
└────────┬────────┘             ▲
         │                      │
    ┌────▼───────┐     ┌────────┴────────┐
    │ moor-curl- │     │ moor-telnet-host│
    │   worker   │     │   (Telnet API)  │
    └────────────┘     └─────────────────┘
```

**Components**:

- **moor-daemon**: Core MOO server (database, VM, task scheduler)
- **moor-telnet-host**: Traditional telnet interface
- **moor-web-host**: Web API and WebSocket server
- **moor-frontend**: Static web client (HTML/CSS/JS)
- **moor-curl-worker**: Handles outbound HTTP from MOO code

All components communicate via ZeroMQ using IPC (Unix domain sockets) for single-machine
deployments.

## Common Configuration

### Ports

Default ports used across deployments:

- **8080**: Web interface (HTTP)
- **443**: Web interface (HTTPS, SSL deployments only)
- **8888**: Telnet interface
- **8081**: Web API server (internal)

**Note**: Internal communication between mooR components uses IPC (Unix domain sockets) for
single-machine deployments, so no additional TCP ports are exposed.

### Environment Variables

Common environment variables across deployments:

- `BUILD_PROFILE`: `debug` or `release` (Docker only)
- `DATABASE_NAME`: Database filename (default: `production.db` for production, `development.db` for
  dev)
- `RUST_BACKTRACE`: Rust backtrace level for debugging (`0`, `1`, or `full`)
- `TELNET_PORT`: Telnet listen port (default: `8888`)
- `WEB_PORT`: Web listen port (default: `8080`, or `80`/`443` with SSL)

## Data Management

### Backups

All deployments store data in similar locations:

**Docker deployments**:

```bash
# Backup
tar czf moor-backup-$(date +%Y%m%d).tar.gz ./moor-data/

# Restore
tar xzf moor-backup-YYYYMMDD.tar.gz
```

**Debian packages**:

```bash
# Backup
sudo systemctl stop moor-daemon
sudo tar czf moor-backup-$(date +%Y%m%d).tar.gz /var/spool/moor-daemon/
sudo systemctl start moor-daemon
```

### Restore from Export

All deployments can use the [restore-from-export.sh](scripts/restore-from-export.sh) script to
restore from a mooR export snapshot.

See [scripts/restore-from-export.sh](scripts/restore-from-export.sh) documentation.

## Upgrading

### Docker Deployments

```bash
# 1. Backup data
tar czf backup-$(date +%Y%m%d).tar.gz moor-data/

# 2. Pull latest changes (if using git)
git pull

# 3. Rebuild and restart
docker compose down
docker compose build --no-cache
docker compose up -d
```

### Debian Package Deployments

```bash
# 1. Backup data
sudo systemctl stop moor-daemon
sudo tar czf backup-$(date +%Y%m%d).tar.gz /var/spool/moor-daemon/
sudo systemctl start moor-daemon

# 2. Install new packages
sudo dpkg -i moor-daemon_*.deb
sudo dpkg -i moor-telnet-host_*.deb
sudo dpkg -i moor-web-host_*.deb

# 3. Restart services
sudo systemctl restart moor-daemon moor-telnet-host moor-web-host
```

## Security Considerations

### General Recommendations

Always change the wizard password after first login, and use a firewall to restrict access to
necessary ports. Keep mooR and system packages updated, implement regular automated backups, and
monitor logs for suspicious activity.

### Docker-Specific

1. **Limit port exposure**: Only expose necessary ports to host
2. **Use secrets**: Store sensitive data in Docker secrets (not in compose files)
3. **Network isolation**: Use Docker networks to isolate services
4. **Read-only volumes**: Mount sensitive data as read-only where possible

### Production Deployments

1. **SSL/TLS**: Always use HTTPS for internet-facing deployments
2. **Certificate monitoring**: Monitor certificate expiration
3. **Rate limiting**: Implement rate limiting on web endpoints
4. **Intrusion detection**: Consider IDS/IPS for public servers
5. **Regular audits**: Audit user permissions and access logs

## Utilities and Scripts

### Available in This Directory

- [scripts/restore-from-export.sh](scripts/restore-from-export.sh) - Restore database from export
  snapshot
- [debian-packages/build-all-packages.sh](debian-packages/build-all-packages.sh) - Build all Debian
  packages
- [debian-packages/build-web-client-deb.sh](debian-packages/build-web-client-deb.sh) - Build web
  client package

### Other Files

- [Dockerfile-forgejo-builder](Dockerfile-forgejo-builder) - CI/CD builder image (for Forgejo
  Actions)

## Getting Help

### Documentation

- **mooR Book**: [https://timbran.org/book/html/](https://timbran.org/book/html/)
- **Repository**: [https://codeberg.org/timbran/moor](https://codeberg.org/timbran/moor)
- **Issues**: [https://codeberg.org/timbran/moor/issues](https://codeberg.org/timbran/moor/issues)

### Community

- **Discord**: [https://discord.gg/Ec94y5983z](https://discord.gg/Ec94y5983z)

### Reporting Issues

When reporting deployment issues, please include:

1. Deployment method used (Docker, Debian packages, etc.)
2. Operating system and version
3. mooR version (from logs or `moor-daemon --version`)
4. Relevant error messages or logs
5. Steps to reproduce

## Contributing

Contributions to deployment configurations are welcome! Please:

1. Test thoroughly in your environment
2. Document prerequisites and setup steps
3. Follow existing style and structure
4. Submit pull requests to [Codeberg](https://codeberg.org/timbran/moor)

See [CONTRIBUTING.md](../CONTRIBUTING.md) for more details.

## License

mooR is free software licensed under GPL-3.0. See [../LICENSE](../LICENSE) for details.

Note: Core databases in `../cores/` have separate licensing. See
[../cores/LICENSING.md](../cores/LICENSING.md).
