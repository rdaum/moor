# Web-Basic Deployment (HTTP, No SSL)

This deployment configuration provides a complete mooR setup with web client, telnet access, and all
backend services. It runs over HTTP without SSL/TLS encryption.

## Use Case

This setup is ideal for:

- Local development or testing environments
- Deployments behind an external reverse proxy that handles SSL
- Internal network deployments where SSL is not required
- Getting started quickly with the web interface

**Not recommended for**: Production internet-facing deployments without an external SSL termination
proxy.

## Services

This configuration runs:

- **moor-daemon**: Core MOO database and virtual machine
- **moor-telnet-host**: Telnet server for client connections (port 8888)
- **moor-web-host**: Web API and WebSocket server
- **moor-frontend**: nginx serving the web client (port 8080)
- **moor-curl-worker**: Worker for outbound HTTP requests from MOO code
- **init-enrollment**: One-time setup for authentication tokens

## Prerequisites

- Docker and Docker Compose installed
- Ports 8080 (web), 8888 (telnet) available
- At least 1GB RAM recommended
- Basic understanding of Docker volumes for data persistence

## Quick Start

1. **Copy this directory** to your deployment location:
   ```bash
   cp -r deploy/web-basic /path/to/deployment
   cd /path/to/deployment
   ```

2. **Review and customize** the `.env` file (copy from `.env.example`):
   ```bash
   cp .env.example .env
   # Edit .env with your preferred settings
   ```

3. **Start the services**:
   ```bash
   docker compose up -d
   ```

4. **Check logs** to verify startup:
   ```bash
   docker compose logs -f
   ```

5. **Access the web interface**:
   - Open your browser to `http://localhost:8080`

6. **Or connect via telnet**:
   ```bash
   telnet localhost 8888
   ```

## First-Time Setup

On first run, the system will:

1. Generate an enrollment token for secure host-daemon communication
2. Import the default LambdaMOO core database (if no existing database)
3. Start all services (daemon, telnet host, web host, frontend, curl worker)

Default wizard credentials (if using lambda-moor core):

- Username: `Wizard`
- Password: (none - press enter)

**IMPORTANT**: Change the wizard password immediately after first login:

```
@password newpassword
```

## Configuration

### Environment Variables

See `.env.example` for available configuration options. Key settings:

- **WEB_PORT**: Port for web interface (default: 8080)
- **TELNET_PORT**: Port for telnet connections (default: 8888)
- **DATABASE_NAME**: Name of the database file (default: production.db)

### nginx Configuration

The `nginx.conf` file configures the web frontend. It handles:

- Serving static web client files
- Proxying API requests to moor-web-host
- WebSocket connection upgrades
- Gzip compression

To customize, edit `nginx.conf` and restart the frontend:

```bash
docker compose restart moor-frontend
```

### Data Persistence

Data is stored in Docker volumes and local directories:

- `./moor-data/`: Main database directory (created on first run)
- `./moor-telnet-host-data/`: Telnet host state
- `./moor-web-host-data/`: Web host state
- `./moor-curl-worker-data/`: Curl worker state
- `moor-enrollment`: Docker volume for authentication tokens
- `moor-allowed-hosts`: Docker volume for host authorization

**Backup Strategy**: Regularly backup the `./moor-data/` directory to preserve your database.

## Management Commands

### Start services

```bash
docker compose up -d
```

### Stop services

```bash
docker compose stop
```

### View logs

```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f moor-daemon
docker compose logs -f moor-web-host
docker compose logs -f moor-frontend
```

### Restart after config changes

```bash
docker compose restart
```

### Rebuild after mooR updates

```bash
docker compose build --no-cache
docker compose up -d
```

## Using Behind a Reverse Proxy

If you're running this behind an external reverse proxy (e.g., Caddy, Traefik, external nginx):

1. **Keep this deployment on HTTP** (as configured)
2. **Configure your reverse proxy** to:
   - Handle SSL/TLS termination
   - Forward HTTP traffic to this deployment on port 8080
   - Set appropriate headers (X-Real-IP, X-Forwarded-For, X-Forwarded-Proto)
   - Support WebSocket upgrades for `/ws/` paths

3. **Update nginx.conf** if needed to trust proxy headers

Example Caddy configuration:

```
your-domain.com {
    reverse_proxy localhost:8080
}
```

Example Traefik labels (add to moor-frontend service):

```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.moor.rule=Host(`your-domain.com`)"
  - "traefik.http.routers.moor.tls.certresolver=letsencrypt"
```

## Upgrading

To upgrade to a newer version of mooR:

1. **Backup your data**:
   ```bash
   tar czf moor-data-backup-$(date +%Y%m%d).tar.gz moor-data/
   ```

2. **Pull latest changes** (if using git clone):
   ```bash
   git pull
   ```

3. **Rebuild containers**:
   ```bash
   docker compose build --no-cache
   ```

4. **Restart services**:
   ```bash
   docker compose down
   docker compose up -d
   ```

## Troubleshooting

### Cannot access web interface

1. Check that services are running:
   ```bash
   docker compose ps
   ```

2. Check logs for errors:
   ```bash
   docker compose logs moor-frontend
   docker compose logs moor-web-host
   ```

3. Verify port is exposed:
   ```bash
   docker compose port moor-frontend 80
   ```

4. Try accessing directly: `http://localhost:8080`

### WebSocket connection fails

1. Check web-host logs:
   ```bash
   docker compose logs moor-web-host
   ```

2. Verify nginx WebSocket configuration in `nginx.conf`

3. Check browser console for WebSocket errors

### Database won't start

1. Check disk space:
   ```bash
   df -h
   ```

2. Check permissions on moor-data directory:
   ```bash
   ls -la moor-data/
   ```

3. Check daemon logs for specific error:
   ```bash
   docker compose logs moor-daemon
   ```

### Services can't communicate

1. Check that all services are on the same Docker network:
   ```bash
   docker network inspect web-basic_moor_net
   ```

2. Check enrollment token was generated:
   ```bash
   docker compose logs init-enrollment
   ```

## Security Considerations

1. **HTTP Only**: This configuration uses HTTP without encryption
   - Only use on trusted networks or behind SSL-terminating proxy
   - Do not expose directly to the internet without SSL

2. **Change default passwords**: The default core may have default wizard credentials

3. **Firewall configuration**:
   - Restrict access to ports 8080 and 8888 as appropriate
   - Consider binding to localhost if behind a reverse proxy

4. **Regular backups**: Backup `moor-data/` directory regularly

5. **Update regularly**: Keep mooR updated with latest security fixes

## Next Steps

- Access web interface at http://localhost:8080
- Connect via telnet client to localhost:8888
- Change wizard password
- Explore the MOO environment
- Read the [mooR Book](https://timbran.org/book/html/) for programming guides
- Consider setting up SSL (see `../web-ssl/` deployment)

## Support

- Issues: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- Documentation: [mooR Book](https://timbran.org/book/html/)
- Community: [Discord](https://discord.gg/Ec94y5983z)
