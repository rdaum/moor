# Telnet-Only Deployment

This deployment configuration provides a traditional MUD/MOO setup with telnet access only. It does
not include the web host or web client components.

## Use Case

This setup is ideal for:

- Traditional MUD/MOO users who prefer telnet clients
- Minimal deployments without web interface requirements
- Lower resource usage compared to full web-enabled setups
- Classic MOO experience

## Services

This configuration runs:

- **moor-daemon**: Core MOO database and virtual machine
- **moor-telnet-host**: Telnet server for client connections

Communication between services uses IPC (Unix domain sockets) with filesystem permissions for
security.

## Prerequisites

- Docker and Docker Compose installed
- Port 8888 available for telnet connections
- At least 512MB RAM recommended
- Basic understanding of Docker volumes for data persistence

## Quick Start

### Using Pre-built Images (Recommended)

The `docker-compose.yml` is configured to use pre-built Docker images from the Codeberg container
registry:

1. **Copy this directory** to your deployment location:
   ```bash
   cp -r deploy/telnet-only /path/to/deployment
   cd /path/to/deployment
   ```

2. **Start the services** using the start script:
   ```bash
   ./start.sh
   ```

That's it! The start script handles user permissions and directory setup, then pulls and starts the
containers.

### Using Locally-Built Images

If you prefer to build images locally on your machine:

1. Follow step 1 above
2. Edit `docker-compose.yml` and uncomment the `build:` sections for services (replacing `image:`
   lines)
3. Ensure you're in the mooR source directory (parent of `deploy/`)
4. Run: `./start.sh`

### For ARM64 Systems

If running on aarch64 systems, change the image tag from `latest-x86_64` to `latest-aarch64` in
`docker-compose.yml`, or use local builds instead.

## Verifying Your Setup

1. **Check logs** to verify startup:
   ```bash
   docker compose logs -f
   ```

2. **Connect via telnet**:
   ```bash
   telnet localhost 8888
   ```

## First-Time Setup

On first run, the system will:

1. Import the default LambdaMOO core database (if no existing database)
2. Start the telnet listener on port 8888

Default wizard credentials (if using lambda-moor core):

- Username: `Wizard`
- Password: (none - press enter)

**IMPORTANT**: Change the wizard password immediately after first login:

```
@password newpassword
```

## Configuration

### Customizing Ports and Settings

To change ports or other settings, edit `docker-compose.yml` directly:

- **Telnet port**: Change the `ports:` mapping for `moor-telnet-host` (default: `8888:8888`)
- **Database name**: Modify the `--db=` argument in the daemon command (default: `production.db`)

### Data Persistence

Data is stored in local directories:

- `./moor-data/`: Main database directory (created on first run)
- `./moor-telnet-host-data/`: Telnet host state (created on first run)
- `moor-ipc`: Docker volume for IPC socket communication

**Backup Strategy**: Regularly backup the `./moor-data/` directory to preserve your database.

## Management Commands

### Start services

```bash
./start.sh
```

### Stop services

```bash
docker compose stop
```

### View logs

```bash
docker compose logs -f
docker compose logs -f moor-daemon
docker compose logs -f moor-telnet-host
```

### Restart after changes

```bash
docker compose restart
```

### Rebuild after mooR updates

```bash
docker compose build --no-cache
docker compose up -d
```

## Upgrading

To upgrade to a newer version of mooR:

1. **Backup your data**:
   ```bash
   tar czf moor-data-backup-$(date +%Y%m%d).tar.gz moor-data/
   ```

2. **Pull latest images** (if using pre-built images):
   ```bash
   docker compose pull
   ```

   Or **rebuild containers** (if using local builds):
   ```bash
   docker compose build --no-cache
   ```

3. **Restart services**:
   ```bash
   docker compose down
   ./start.sh
   ```

## Troubleshooting

### Cannot connect via telnet

1. Check that services are running:
   ```bash
   docker compose ps
   ```

2. Check logs for errors:
   ```bash
   docker compose logs moor-daemon
   docker compose logs moor-telnet-host
   ```

3. Verify port is exposed:
   ```bash
   docker compose port moor-telnet-host 8888
   ```

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

### Host cannot connect to daemon

If the telnet host cannot connect to the daemon:

1. Check that both services are in the same Docker network:
   ```bash
   docker compose ps
   ```

2. Check IPC socket permissions:
   ```bash
   docker compose exec moor-daemon ls -la /var/run/moor/
   ```

3. Restart both services:
   ```bash
   docker compose restart
   ```

## Security Considerations

1. **Change default passwords**: The default core may have default wizard credentials
2. **Firewall configuration**: Ensure port 8888 is only accessible to intended users
3. **Regular backups**: Backup `moor-data/` directory regularly
4. **Update regularly**: Keep mooR updated with latest security fixes
5. **Network isolation**: Consider running on a private network or behind a firewall

## Next Steps

- Connect via telnet client to localhost:8888
- Change wizard password
- Explore the MOO environment
- Read the [mooR Book](https://timbran.org/book/html/) for programming guides
- Consider adding monitoring and automated backups

## Support

- Issues: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- Documentation: [mooR Book](https://timbran.org/book/html/)
- Community: [Discord](https://discord.gg/Ec94y5983z)
