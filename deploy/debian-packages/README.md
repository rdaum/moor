# Debian Package Deployment

This guide covers building and deploying mooR using Debian packages (.deb files). This approach is
suitable for traditional Linux server deployments using systemd service management.

## Overview

mooR can be packaged into several Debian packages:

- **moor-daemon**: Core MOO server with systemd service
- **moor-telnet-host**: Telnet server with systemd service
- **moor-web-host**: Web API server with systemd service
- **meadow**: Web client static files (architecture-independent, managed in Meadow repository)

## Use Case

This approach is ideal for:

- Traditional Linux server deployments
- Debian/Ubuntu based systems
- Deployments managed with systemd
- Integration with existing system management tools
- Development or testing on native systems

## Prerequisites

- Debian-based Linux (Debian 12+, Ubuntu 22.04+, or similar)
- Rust toolchain 1.92.0 or later
- Node.js 20+ (for web client)
- cargo-deb tool
- Standard build tools (gcc, make, pkg-config)

### Install Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cargo-deb
cargo install cargo-deb

# Install Node.js (if building web client)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs

# Install system dependencies
sudo apt-get install -y \
    clang-16 \
    libclang-16-dev \
    swig \
    python3-dev \
    cmake \
    libc6 \
    git \
    libsodium-dev \
    pkg-config \
    libssl-dev
```

## Building Packages

### Quick Build All Packages

Use the provided script to build all packages at once:

```bash
# From the mooR repository root
cd deploy/debian-packages
./build-all-packages.sh
```

This will create .deb files in the repository root:

- `moor-daemon_*.deb`
- `moor-telnet-host_*.deb`
- `moor-web-host_*.deb`
- `moor-web-client_*.deb`

### Building Individual Packages

#### Rust Binary Packages (daemon, telnet-host, web-host)

```bash
# Build in release mode first
cargo build --release --workspace

# Build daemon package
cargo deb -p moor-daemon --no-build

# Build telnet host package
cargo deb -p moor-telnet-host --no-build

# Build web host package
cargo deb -p moor-web-host --no-build
```

The `--no-build` flag uses the existing release binaries. Omit it to rebuild from source.

#### Web Client Package

Meadow is managed in a [separate repository](https://codeberg.org/timbran/meadow). To build its
Debian package:

```bash
cd ../meadow
npm install
npm run build:deb
```

## Installing Packages

### Option 1: Install from APT Repository (Recommended)

The easiest way to install mooR is from the Codeberg package repository:

```bash
# Add the repository signing key
sudo curl https://codeberg.org/api/packages/timbran/debian/repository.key \
    -o /etc/apt/keyrings/timbran-moor.asc

# Add the repository (for Debian Bookworm / Ubuntu 22.04+)
echo "deb [signed-by=/etc/apt/keyrings/timbran-moor.asc] https://codeberg.org/api/packages/timbran/debian bookworm main" \
    | sudo tee /etc/apt/sources.list.d/moor.list

# Update and install
sudo apt update
sudo apt install moor-daemon moor-telnet-host moor-web-host meadow
```

This handles dependencies automatically and makes future upgrades simple with `apt upgrade`.

### Option 2: Install from Release Downloads

Download `.deb` packages from the [releases page](https://codeberg.org/timbran/moor/releases) and
install manually:

```bash
# 1. Install daemon first (core service)
sudo dpkg -i moor-daemon_*.deb

# 2. Install hosts (depend on daemon being installed)
sudo dpkg -i moor-telnet-host_*.deb
sudo dpkg -i moor-web-host_*.deb

# 3. Install web client (optional, needs nginx or similar)
sudo dpkg -i meadow_*.deb

# Fix any missing dependencies
sudo apt-get install -f
```

### Option 3: Install Locally-Built Packages

If you built packages yourself (see Building Packages above):

```bash
sudo dpkg -i ../../target/debian/moor-*.deb
sudo apt-get install -f  # Fix any missing dependencies
```

## Post-Installation Configuration

### 1. Configure the Daemon

Edit the daemon configuration:

```bash
sudo nano /etc/moor/daemon-config.yaml
```

Key settings:

- `rpc_listen`: RPC endpoint address (default: `tcp://127.0.0.1:7899`)
- `events_listen`: Events endpoint address (default: `tcp://127.0.0.1:7898`)
- Database path (default: `/var/spool/moor-daemon/moor.db`)

### 2. Configure Hosts

Telnet host configuration (if different from defaults):

```bash
sudo systemctl edit moor-telnet-host
```

Web host configuration (if different from defaults):

```bash
sudo systemctl edit moor-web-host
```

### 3. Initialize Database

On first install, initialize the database:

```bash
# The daemon will auto-import the lambda-moor core on first run
sudo systemctl start moor-daemon

# Check logs to verify initialization
sudo journalctl -u moor-daemon -f
```

### 4. Start Services

Enable and start the services:

```bash
# Enable services to start on boot
sudo systemctl enable moor-daemon
sudo systemctl enable moor-telnet-host
sudo systemctl enable moor-web-host

# Start services
sudo systemctl start moor-daemon
sudo systemctl start moor-telnet-host
sudo systemctl start moor-web-host
```

### 5. Verify Services

Check that all services are running:

```bash
sudo systemctl status moor-daemon
sudo systemctl status moor-telnet-host
sudo systemctl status moor-web-host
```

## Web Client Setup with nginx

If you installed the web client package, configure nginx to serve it:

### Install nginx

```bash
sudo apt-get install nginx
```

### Configure nginx Site

Use the provided configuration template:

```bash
# Copy the example configuration
sudo cp nginx-for-debian.conf /etc/nginx/sites-available/moor

# Edit with your domain/settings
sudo nano /etc/nginx/sites-available/moor

# Enable the site
sudo ln -s /etc/nginx/sites-available/moor /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

The web client files are installed to `/usr/share/moor/web-client/`.

### SSL/TLS Setup (Recommended for Production)

For production deployments, use Let's Encrypt:

```bash
# Install certbot
sudo apt-get install certbot python3-certbot-nginx

# Obtain certificate (replace with your domain)
sudo certbot --nginx -d your-domain.com

# Certbot will automatically configure nginx for HTTPS
```

Certificates auto-renew via systemd timer.

## Service Management

### Start/Stop/Restart Services

```bash
# Daemon
sudo systemctl start moor-daemon
sudo systemctl stop moor-daemon
sudo systemctl restart moor-daemon

# Telnet host
sudo systemctl start moor-telnet-host
sudo systemctl stop moor-telnet-host
sudo systemctl restart moor-telnet-host

# Web host
sudo systemctl start moor-web-host
sudo systemctl stop moor-web-host
sudo systemctl restart moor-web-host
```

### View Logs

```bash
# Follow daemon logs
sudo journalctl -u moor-daemon -f

# Follow telnet host logs
sudo journalctl -u moor-telnet-host -f

# Follow web host logs
sudo journalctl -u moor-web-host -f

# View recent logs for all moor services
sudo journalctl -u 'moor-*' --since today
```

### Service Status

```bash
# Check all moor services
systemctl list-units 'moor-*'

# Detailed status
sudo systemctl status moor-daemon moor-telnet-host moor-web-host
```

## File Locations

### Binaries

- `/usr/bin/moor-daemon`
- `/usr/bin/moor-telnet-host`
- `/usr/bin/moor-web-host`
- `/usr/bin/moorc` (compiler utility, installed with daemon)
- `/usr/bin/moor-emh` (database utility, installed with daemon)

### Configuration

- `/etc/moor/daemon-config.yaml` (daemon configuration)
- `/etc/moor/telnet-host-config.yaml` (telnet host configuration, if customized)
- `/etc/moor/web-host-config.yaml` (web host configuration, if customized)

### Data

- `/var/spool/moor-daemon/` (database directory)
- `/var/spool/moor-daemon/moor.db/` (actual database)

### Web Client

- `/usr/share/moor/web-client/` (static files)

### Systemd Services

- `/lib/systemd/system/moor-daemon.service`
- `/lib/systemd/system/moor-telnet-host.service`
- `/lib/systemd/system/moor-web-host.service`

### Cores

- `/usr/share/moor/cores/lambda-moor/` (default MOO core)

## Backup and Restore

### Backup

```bash
# Stop the daemon first
sudo systemctl stop moor-daemon

# Backup the database
sudo tar czf moor-backup-$(date +%Y%m%d).tar.gz /var/spool/moor-daemon/

# Restart the daemon
sudo systemctl start moor-daemon
```

### Restore

```bash
# Stop the daemon
sudo systemctl stop moor-daemon

# Restore from backup
sudo tar xzf moor-backup-20250101.tar.gz -C /

# Start the daemon
sudo systemctl start moor-daemon
```

## Upgrading

### If Using APT Repository (Recommended)

```bash
# Backup your database first (see Backup and Restore above)

# Upgrade all mooR packages
sudo apt update
sudo apt upgrade

# Services will be automatically restarted
```

### If Using Manual Package Installation

1. **Backup your database** (see above)

2. **Stop services**:
   ```bash
   sudo systemctl stop moor-telnet-host moor-web-host moor-daemon
   ```

3. **Install new packages**:
   ```bash
   sudo dpkg -i moor-daemon_*.deb
   sudo dpkg -i moor-telnet-host_*.deb
   sudo dpkg -i moor-web-host_*.deb
   sudo dpkg -i moor-web-client_*.deb  # if using web client
   ```

4. **Restart services**:
   ```bash
   sudo systemctl start moor-daemon
   sudo systemctl start moor-telnet-host
   sudo systemctl start moor-web-host
   ```

5. **Verify**:
   ```bash
   sudo systemctl status moor-daemon moor-telnet-host moor-web-host
   ```

## Uninstalling

To completely remove mooR:

```bash
# Stop services
sudo systemctl stop moor-telnet-host moor-web-host moor-daemon

# Remove packages (--purge removes config files too)
sudo apt-get remove --purge meadow moor-web-host moor-telnet-host moor-daemon

# Manually remove data if desired
sudo rm -rf /var/spool/moor-daemon/
```

## Testing and Validation

After installation and configuration, validate your deployment to ensure everything is working
correctly.

### 1. Verify Services Are Running

Check that all services are active:

```bash
sudo systemctl status moor-daemon moor-telnet-host moor-web-host
```

All services should show `active (running)` status.

### 2. Test Telnet Access

If running the telnet host, test connectivity:

```bash
telnet localhost 8888
```

You should see a connection prompt. Try basic commands:

```
connect wizard
look
quit
```

### 3. Test Web Access

If running the web host with nginx, test HTTP access:

```bash
# Test that nginx is serving the web client
curl -I http://localhost/

# If using a domain, test from another machine
curl -I http://your-domain.com/
```

You should see a `200 OK` response with HTML content.

### 4. Test WebSocket Connection

For web deployments, verify WebSocket connectivity:

```bash
# Check that the web host is accessible
curl http://localhost:8080/health || echo "Web host may not have health endpoint"
```

Test from a browser by opening the web client and checking the browser console for WebSocket
connection messages.

### 5. Check Logs for Errors

Examine logs for any unexpected errors:

```bash
# Check daemon logs
sudo journalctl -u moor-daemon -n 50 --no-pager

# Check telnet host logs
sudo journalctl -u moor-telnet-host -n 50 --no-pager

# Check web host logs
sudo journalctl -u moor-web-host -n 50 --no-pager

# Check nginx logs (if using)
sudo tail -50 /var/log/nginx/error.log
```

### 6. Verify Database Initialization

Check that the database was created and initialized:

```bash
# Check database file exists
sudo ls -lh /var/spool/moor-daemon/*.db

# Check database size (should be > 0 bytes)
sudo du -h /var/spool/moor-daemon/
```

### 7. Test Basic MOO Operations

Connect via telnet or web client and verify:

- Can connect as a wizard/user
- Can execute basic commands (`look`, `@who`, etc.)
- Objects and rooms are accessible
- Database changes persist across connections

### Manual Testing Checklist

For production deployments, verify:

- [ ] All systemd services start without errors
- [ ] Services restart automatically after reboot
- [ ] Telnet connections work (if enabled)
- [ ] Web client loads in browser (if enabled)
- [ ] WebSocket connections establish (check browser console)
- [ ] SSL certificates are valid (if using HTTPS)
- [ ] Firewall rules allow necessary ports
- [ ] Log rotation is working
- [ ] Backups can be created and restored
- [ ] Database changes persist across service restarts

### Automated Testing (Development)

For development and testing, you can use a test VM or container:

```bash
# Using Multipass (Ubuntu VMs)
multipass launch --name moor-test
multipass shell moor-test

# Build and install packages
# (copy .deb files to VM and install)

# Run validation tests
sudo systemctl status moor-daemon
telnet localhost 8888
```

See the main [deploy/README.md](../README.md) for Docker-based testing alternatives.

## Troubleshooting

### Services won't start

1. **Check logs**:
   ```bash
   sudo journalctl -u moor-daemon -n 50
   ```

2. **Verify configuration**:
   ```bash
   sudo cat /etc/moor/daemon-config.yaml
   ```

3. **Check file permissions**:
   ```bash
   ls -la /var/spool/moor-daemon/
   ```

4. **Verify user exists**:
   ```bash
   id moor
   ```

### Database initialization fails

1. Check that lambda-moor core is installed:
   ```bash
   ls -la /usr/share/moor/cores/lambda-moor/
   ```

2. Check daemon has write access:
   ```bash
   sudo -u moor touch /var/spool/moor-daemon/test
   ```

### Hosts can't connect to daemon

1. Verify daemon is running:
   ```bash
   sudo systemctl status moor-daemon
   ```

2. Check enrollment token was created:
   ```bash
   sudo ls -la /etc/moor/enrollment-token
   sudo ls -la /var/lib/moor-daemon/enrollment-token
   ```

3. Check network bindings in config:
   ```bash
   sudo grep listen /etc/moor/daemon-config.yaml
   ```

### nginx can't find web client files

1. Verify package is installed:
   ```bash
   dpkg -l | grep meadow
   ```

2. Check files exist:
   ```bash
   ls -la /usr/share/moor/web-client/
   ```

3. Verify nginx config path:
   ```bash
   grep root /etc/nginx/sites-enabled/moor
   ```

## Development Use

For development, you can install packages locally without systemd:

```bash
# Extract package contents
dpkg -x moor-daemon_*.deb ./local-install/

# Run manually
./local-install/usr/bin/moor-daemon --help
```

Or use the binaries directly from the `target/release/` directory after `cargo build --release`.

## Support

- Issues: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- Documentation: [mooR Book](https://timbran.org/book/html/)
- Community: [Discord](https://discord.gg/Ec94y5983z)
