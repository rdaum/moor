# Alternative Installation Methods

While Docker Compose is the recommended approach for most users, mooR provides several other installation methods for different use cases and environments.

## Debian Packages

For Debian-based systems (including Ubuntu), mooR provides native `.deb` packages that integrate cleanly with your system's package management and systemd service management.

### About Debian Packages

mooR packages include:

- **moor-daemon**: Core MOO server with systemd service
- **moor-telnet-host**: Telnet server with systemd service
- **moor-web-host**: Web API server with systemd service
- **moor-curl-worker**: HTTP request worker with systemd service
- **moor-web-client**: Static web client files (architecture-independent)

All packages integrate with systemd, create necessary users and directories, and include the LambdaMOO-based lambda-moor core database by default.

### Installation Options

**Option 1: Install from APT Repository (Recommended)**

Add the Codeberg package repository to install and update mooR packages with `apt`:

```bash
# Add the repository signing key
sudo curl https://codeberg.org/api/packages/timbran/debian/repository.key \
    -o /etc/apt/keyrings/timbran-moor.asc

# Add the repository (for Debian Bookworm / Ubuntu 22.04+)
echo "deb [signed-by=/etc/apt/keyrings/timbran-moor.asc] https://codeberg.org/api/packages/timbran/debian bookworm main" \
    | sudo tee /etc/apt/sources.list.d/moor.list

# Update and install
sudo apt update
sudo apt install moor-daemon moor-telnet-host moor-web-host moor-web-client
```

**Option 2: Download from Releases**

Download pre-built `.deb` packages from the [mooR Codeberg releases page](https://codeberg.org/timbran/moor/releases):

```bash
sudo dpkg -i moor-*.deb
sudo apt-get install -f  # Install any missing dependencies
```

**Option 3: Build Locally**

Build packages yourself using the provided scripts in `deploy/debian-packages/`:

```bash
cd deploy/debian-packages
./build-all-packages.sh
sudo dpkg -i ../../target/debian/moor-*.deb
```

### Comprehensive Documentation

For detailed installation, configuration, service management, testing, and troubleshooting:

**→ See [`deploy/debian-packages/README.md`](https://codeberg.org/timbran/moor/src/branch/main/deploy/debian-packages/README.md)**

This includes:
- Complete installation and post-installation configuration
- Service management with systemd
- nginx setup for the web client
- Backup and restore procedures
- Automated testing
- Troubleshooting guide

### When to Use Debian Packages

Debian packages are ideal when:
- You're running a Debian-based Linux distribution
- You want system-level integration (systemd services, standard file locations)
- You prefer traditional package management over containers
- You're setting up a production server on bare metal or VPS
- You want to deploy on multiple separate machines

## Building from Source

For developers, custom deployments, or platforms without pre-built packages, you can compile mooR from source code.

### Prerequisites

You'll need the Rust toolchain installed. The recommended way is using `rustup`:

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow the installation prompts, then restart your shell or run:
source ~/.cargo/env
```

### Building Process

1. **Clone the repository**:
   ```bash
   git clone https://codeberg.org/timbran/moor.git
   cd moor
   ```

2. **Build all components**:
   ```bash
   cargo build --release --all-targets
   ```

   This will take some time as Rust compiles all dependencies and mooR components.

3. **Find your binaries**:
   After building, you'll find the executables in `target/release/`:
   - `moor-daemon`
   - `moor-telnet-host`
   - `moor-web-host`
   - `curl-worker`

### Manual Configuration

When building from source, you'll need to manually set up:

- **PASETO authentication keys**: The daemon auto-generates these keys with the `--generate-keypair` flag (creates `moor-signing-key.pem` and `moor-verifying-key.pem`)
- **Configuration files**: Create appropriate configuration for each component
- **Core database**: Install and configure your chosen MOO core
- **Service coordination**: Ensure all components can communicate properly (see [Server Architecture](server-architecture.md#communication-transport))
  - Default: IPC (Unix domain sockets) - simplest, no encryption needed
  - Clustered: TCP with CURVE encryption requires enrollment tokens (see `--rotate-enrollment-token` flag)

The `docker-compose.yml` and `process-compose.yaml` files provide excellent examples of how to configure each component.

### When to Build from Source

Source builds are best for:
- Development and testing
- Platforms without Debian package support
- Custom configurations requiring code modifications
- Learning how mooR works internally
- Contributing to the project

## Configuration Reference

Regardless of your installation method, you'll need to configure mooR's components. The arguments and options for the server executables are documented in the [Server Configuration](server-configuration.md) chapter.

## Choosing Your Method

| Method | Best For | Pros | Cons |
|--------|----------|------|------|
| **Docker Compose** | Most users, quick setup | Easy, complete environment, works everywhere | Requires Docker knowledge |
| **Debian Packages** | Production Linux servers | System integration, familiar package management | Limited to Debian-based systems |
| **Source Build** | Developers, custom needs | Full control, latest code, all platforms | Complex setup, manual configuration |

## Getting Help

For installation issues:
- Check the mooR Codeberg repository for the latest installation instructions
- Review the `docker-compose.yml` file for configuration examples
- Consult the community forums or Discord for platform-specific guidance

Remember that regardless of your installation method, you'll also need to choose and install a MOO core database - see [Understanding MOO Cores](understanding-moo-cores.md) for guidance on that crucial next step.
