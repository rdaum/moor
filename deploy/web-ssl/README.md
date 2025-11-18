# Web-SSL Deployment (HTTPS with Let's Encrypt)

This deployment configuration provides a complete production-ready mooR setup with HTTPS via Let's
Encrypt SSL/TLS certificates. It includes the web client, API server, telnet access, and automatic
certificate management.

## Use Case

This setup is ideal for:

- Production internet-facing deployments
- Public MOO/MUD servers with web access
- Deployments requiring secure HTTPS connections
- Sites needing trusted SSL certificates

## Services

This configuration runs:

- **moor-daemon**: Core MOO database and virtual machine
- **moor-telnet-host**: Telnet server for client connections (port 8888)
- **moor-web-host**: Web API and WebSocket server
- **moor-frontend**: nginx with SSL/TLS serving the web client (ports 80, 443)
- **moor-curl-worker**: Worker for outbound HTTP requests from MOO code
- **certbot**: Let's Encrypt certificate management (run separately)
- **init-enrollment**: One-time setup for authentication tokens

## Prerequisites

- Docker and Docker Compose installed
- **Domain name** pointing to your server's public IP
- Ports 80 (HTTP), 443 (HTTPS), and optionally 8888 (telnet) accessible from internet
- At least 1GB RAM recommended
- Valid email address for Let's Encrypt notifications

## Quick Start

### Step 1: Initial Setup

1. **Copy this directory** to your deployment location:
   ```bash
   cp -r deploy/web-ssl /path/to/deployment
   cd /path/to/deployment
   ```

2. **Configure your domain and email** in `.env` (copy from `.env.example`):
   ```bash
   cp .env.example .env
   nano .env
   ```

   Set these **required** values:
   ```bash
   DOMAIN_NAME=your-domain.com
   CERTBOT_EMAIL=your-email@example.com
   ```

3. **Update nginx configuration** with your domain:
   ```bash
   nano nginx-ssl.conf
   ```

   Replace `YOUR_DOMAIN_HERE` and `YOUR_DOMAIN` with your actual domain name.

### Using Pre-built Images or Local Builds

The `docker-compose.yml` is configured to use pre-built Docker images from the Codeberg container
registry.

- **For x86_64 systems** (default): Images are automatically pulled from Codeberg
- **For ARM64 systems**: Change the image tag from `latest-x86_64` to `latest-aarch64` in
  `docker-compose.yml`
- **For local builds**: Edit `docker-compose.yml` and uncomment the `build:` sections for services
  (replacing `image:` lines), then ensure you're in the mooR source directory before running
  commands

Note: Both backend and frontend images are pre-built and available from Codeberg. Local builds are
only necessary if you're developing or need to customize the deployment.

### Step 2: Obtain SSL Certificate

Before starting the main services, you need to obtain an SSL certificate from Let's Encrypt:

1. **Start only the frontend temporarily** (for ACME challenge):
   ```bash
   # Create directories for certbot
   mkdir -p letsencrypt certbot-webroot

   # Start a temporary nginx for certificate validation
   docker compose up -d moor-frontend
   ```

2. **Run certbot to obtain certificate**:
   ```bash
   docker compose -f docker-compose.yml -f docker-compose.certbot.yml run --rm certbot certonly \
     --webroot \
     --webroot-path=/var/www/certbot \
     --email your-email@example.com \
     --agree-tos \
     --no-eff-email \
     -d your-domain.com
   ```

3. **Verify certificate was created**:
   ```bash
   ls -la letsencrypt/live/your-domain.com/
   ```

   You should see `fullchain.pem` and `privkey.pem`.

### Step 3: Start All Services

1. **Stop temporary frontend**:
   ```bash
   docker compose down
   ```

2. **Start all services**:
   ```bash
   docker compose up -d
   ```

3. **Check logs** to verify startup:
   ```bash
   docker compose logs -f
   ```

4. **Access your site**:
   - Open your browser to `https://your-domain.com`
   - HTTP traffic on port 80 will automatically redirect to HTTPS

## Certificate Renewal

Let's Encrypt certificates expire after 90 days. Set up automatic renewal:

### Manual Renewal

```bash
docker compose -f docker-compose.yml -f docker-compose.certbot.yml run --rm certbot renew
docker compose restart moor-frontend
```

### Automatic Renewal with Cron

Add this to your crontab (`crontab -e`):

```bash
# Renew Let's Encrypt certificates daily at 2am
0 2 * * * cd /path/to/deployment && docker compose -f docker-compose.yml -f docker-compose.certbot.yml run --rm certbot renew && docker compose restart moor-frontend >> /var/log/moor-certbot.log 2>&1
```

Or use a systemd timer (see appendix below).

## First-Time MOO Setup

Default wizard credentials (if using lambda-moor core):

- Username: `Wizard`
- Password: (none - press enter)

**IMPORTANT**: Change the wizard password immediately after first login:

```
@password newpassword
```

## Configuration

### Environment Variables

See `.env.example` for all options. Required settings:

- **DOMAIN_NAME**: Your domain name (e.g., `moo.example.com`)
- **CERTBOT_EMAIL**: Email for Let's Encrypt notifications
- **TELNET_PORT**: Port for telnet connections (default: 8888)
- **DATABASE_NAME**: Database file name (default: production.db)

### nginx SSL Configuration

The `nginx-ssl.conf` file is pre-configured with:

- HTTP to HTTPS redirect
- SSL certificate paths
- Modern TLS protocols (TLSv1.2, TLSv1.3)
- WebSocket support
- Gzip compression

**Important**: Replace placeholder domain names in `nginx-ssl.conf`:

- Line 44, 61: `YOUR_DOMAIN_HERE` → your actual domain
- Lines 66-67: `YOUR_DOMAIN` → your actual domain (in certificate paths)

### Data Persistence

Data is stored in Docker volumes and local directories:

- `./moor-data/`: Main database directory
- `./letsencrypt/`: SSL certificates
- `./certbot-webroot/`: ACME challenge directory
- `./moor-telnet-host-data/`: Telnet host state
- `./moor-web-host-data/`: Web host state
- `./moor-curl-worker-data/`: Curl worker state
- Docker volumes for enrollment and host authorization

**Backup Strategy**:

```bash
# Backup database
tar czf moor-backup-$(date +%Y%m%d).tar.gz moor-data/

# Backup certificates (optional, can regenerate)
tar czf certs-backup-$(date +%Y%m%d).tar.gz letsencrypt/
```

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
docker compose logs -f moor-frontend
```

### Restart after config changes

```bash
docker compose restart moor-frontend  # After nginx config changes
docker compose restart                # All services
```

### Rebuild after mooR updates

```bash
docker compose build --no-cache
docker compose up -d
```

## Firewall Configuration

Ensure these ports are accessible:

```bash
# UFW example
sudo ufw allow 80/tcp    # HTTP (for ACME challenges and redirect)
sudo ufw allow 443/tcp   # HTTPS
sudo ufw allow 8888/tcp  # Telnet (optional, can restrict by IP)
```

## Troubleshooting

### Certificate generation fails

1. **Verify DNS**: Ensure your domain resolves to your server:
   ```bash
   dig your-domain.com
   nslookup your-domain.com
   ```

2. **Check port 80 accessibility**: Let's Encrypt needs port 80 for validation:
   ```bash
   curl http://your-domain.com/.well-known/acme-challenge/test
   ```

3. **Check certbot logs**:
   ```bash
   docker compose -f docker-compose.yml -f docker-compose.certbot.yml logs certbot
   ```

4. **Rate limits**: Let's Encrypt has rate limits. If you hit them, wait or use staging:
   ```bash
   # Use staging server for testing
   docker compose -f docker-compose.yml -f docker-compose.certbot.yml run --rm certbot certonly \
     --webroot --webroot-path=/var/www/certbot \
     --staging \
     --email your-email@example.com \
     --agree-tos -d your-domain.com
   ```

### HTTPS not working

1. **Check certificate paths** in `nginx-ssl.conf` match your domain

2. **Verify certificates exist**:
   ```bash
   ls -la letsencrypt/live/your-domain.com/
   ```

3. **Check nginx logs**:
   ```bash
   docker compose logs moor-frontend
   ```

4. **Test nginx config**:
   ```bash
   docker compose exec moor-frontend nginx -t
   ```

### WebSocket connection fails over HTTPS

1. Ensure WebSocket upgrade headers are configured in `nginx-ssl.conf`
2. Check browser console for mixed content warnings
3. Verify `X-Forwarded-Proto` is set correctly

### Cannot access via HTTP after setup

This is expected - HTTP (port 80) redirects to HTTPS (port 443). If you need HTTP access for
testing, temporarily modify `nginx-ssl.conf`.

## Security Considerations

1. **Strong passwords**: Change all default passwords immediately

2. **Firewall**: Use a firewall to restrict access to necessary ports only

3. **Regular updates**: Keep mooR and Docker images updated

4. **Monitor certificates**: Ensure certificate renewal is working

5. **Backup encryption**: Consider encrypting backups if they contain sensitive data

6. **Telnet security**: Consider disabling telnet or restricting by IP if not needed

7. **Rate limiting**: Consider adding rate limiting in nginx for public endpoints

## Monitoring

Consider setting up monitoring for:

- Certificate expiration (Let's Encrypt expires in 90 days)
- Disk space (database can grow)
- Service health (all containers running)
- Log rotation

Example health check:

```bash
#!/bin/bash
# Save as /usr/local/bin/moor-health-check.sh

cd /path/to/deployment
if ! docker compose ps | grep -q "Up"; then
    echo "mooR services not running!" | mail -s "mooR Alert" admin@example.com
    docker compose up -d
fi
```

## Upgrading

To upgrade to a newer version of mooR:

1. **Backup everything**:
   ```bash
   tar czf moor-full-backup-$(date +%Y%m%d).tar.gz moor-data/ letsencrypt/
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
   docker compose up -d
   ```

4. **Check logs for issues**:
   ```bash
   docker compose logs -f
   ```

## Appendix: Systemd Timer for Certificate Renewal

Create `/etc/systemd/system/moor-certbot-renew.service`:

```ini
[Unit]
Description=Renew mooR Let's Encrypt Certificates
After=docker.service
Requires=docker.service

[Service]
Type=oneshot
WorkingDirectory=/path/to/deployment
ExecStart=/usr/bin/docker compose -f docker-compose.yml -f docker-compose.certbot.yml run --rm certbot renew
ExecStartPost=/usr/bin/docker compose restart moor-frontend
```

Create `/etc/systemd/system/moor-certbot-renew.timer`:

```ini
[Unit]
Description=Renew mooR Let's Encrypt Certificates Daily
Requires=moor-certbot-renew.service

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable moor-certbot-renew.timer
sudo systemctl start moor-certbot-renew.timer
```

## Support

- Issues: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- Documentation: [mooR Book](https://timbran.org/book/html/)
- Community: [Discord](https://discord.gg/Ec94y5983z)
