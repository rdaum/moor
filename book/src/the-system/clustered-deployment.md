# Clustered Deployment

One of mooR's unique architectural capabilities is that hosts (telnet-host, web-host) and workers (curl-worker) can run
on separate machines from the daemon. This distributed architecture enables flexible deployment patterns that aren't
possible with traditional monolithic MOO servers.

## Why Deploy in a Clustered Configuration?

**Load Distribution**

While the daemon handles the majority of computational work (MOO code execution, database operations), separating hosts
and workers can help distribute network I/O and connection handling across multiple machines. This is particularly
valuable for high-traffic deployments with many simultaneous connections.

**Security Segmentation**

Running workers like `moor-curl-worker` on separate machines reduces your security blast radius. If a worker handling
processing HTTP requests is compromised, it doesn't have direct access to your MOO database. Similarly, you can isolate
public-facing telnet and web hosts from your core daemon.

**Functional Isolation**

Different components can be deployed with different security policies:

- Run the daemon on a private network with strict firewall rules
- Deploy web-host in a DMZ accessible to the public internet
- Place curl-worker on a separate network with outbound-only access
- Isolate telnet-host for traditional MUD users on different infrastructure

**Flexible Firewall Rules**

Distributed deployment allows you to define firewall rules that match your security model:

- Daemon can run in a highly restricted environment
- Only specific hosts/workers need inbound connectivity
- Each component can have tailored network policies

**Multi-Datacenter Deployments**

For redundancy and geographic distribution:

- Run the daemon in your primary datacenter
- Deploy web-hosts closer to users in different regions
- Distribute telnet-hosts geographically for better latency
- Place workers strategically based on their function

## Security Considerations

**curl-worker Isolation**

The `moor-curl-worker` is a particularly strong candidate for isolation:

- Handles untrusted external HTTP requests from MOO code
- Can be used to probe internal networks if compromised
- Benefits from running in a restricted network environment
- Can be rate-limited or sandboxed independently

**Defense in Depth**

Clustered deployment enables defense-in-depth strategies:

- Each component runs with minimal privileges
- Network segmentation limits lateral movement
- Compromise of one component doesn't compromise the entire system

## How Clustered Communication Works

In a clustered deployment, components communicate over TCP using encrypted ZeroMQ sockets instead of local Unix domain
sockets (IPC).

### TCP Mode with CURVE Encryption

All inter-component communication uses:

- **TCP Sockets**: Standard network communication (e.g., `tcp://daemon.internal:7899`)
- **CURVE Encryption**: Curve25519 elliptic curve cryptography encrypts all ZeroMQ messages
- **Enrollment-based Authentication**: Hosts/workers must register with the daemon before connecting
- **ZAP Authentication**: Zero Authentication Protocol validates all connections

### Communication Endpoints

The daemon exposes several TCP endpoints for clustered deployments:

- **RPC endpoint** (default: `tcp://0.0.0.0:7899`): Request/reply communication with hosts/workers
- **Events endpoint** (default: `tcp://0.0.0.0:7898`): Publish/subscribe for MOO events
- **Workers request** (default: `tcp://0.0.0.0:7896`): Worker task distribution
- **Workers response** (default: `tcp://0.0.0.0:7897`): Worker task responses
- **Enrollment endpoint** (default: `tcp://0.0.0.0:7900`): Host/worker registration

## Enrollment Process

Before hosts or workers can connect to a clustered daemon, they must enroll:

### 1. Generate Enrollment Token (One-Time Setup)

On the daemon host:

```bash
moor-daemon --rotate-enrollment-token
# Token saved to ${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token
```

Copy this token to each host/worker machine.

### 2. Start the Daemon with TCP Endpoints

```bash
moor-daemon \
  --rpc-listen tcp://0.0.0.0:7899 \
  --events-listen tcp://0.0.0.0:7898 \
  --workers-request-listen tcp://0.0.0.0:7896 \
  --workers-response-listen tcp://0.0.0.0:7897 \
  --enrollment-listen tcp://0.0.0.0:7900 \
  /path/to/database
```

### 3. Start Hosts/Workers with Enrollment Token

On each host/worker machine:

```bash
# For telnet-host
moor-telnet-host \
  --rpc-address tcp://daemon.internal:7899 \
  --events-address tcp://daemon.internal:7898 \
  --enrollment-address tcp://daemon.internal:7900 \
  --enrollment-token-file /path/to/enrollment-token

# For web-host
moor-web-host \
  --rpc-address tcp://daemon.internal:7899 \
  --events-address tcp://daemon.internal:7898 \
  --enrollment-address tcp://daemon.internal:7900 \
  --enrollment-token-file /path/to/enrollment-token

# For curl-worker
moor-curl-worker \
  --rpc-address tcp://daemon.internal:7899 \
  --workers-request-address tcp://daemon.internal:7896 \
  --workers-response-address tcp://daemon.internal:7897 \
  --enrollment-address tcp://daemon.internal:7900 \
  --enrollment-token-file /path/to/enrollment-token
```

### 4. Enrollment and Key Exchange

On first connection:

1. Host/worker generates its own CURVE keypair
2. Connects to the enrollment endpoint with the enrollment token
3. Registers its service type, hostname, and CURVE public key with the daemon
4. Daemon stores the public key in `${XDG_DATA_HOME:-$HOME/.local/share}/moor/allowed-hosts/{uuid}`
5. All subsequent connections use CURVE encryption with ZAP authentication

## CURVE Encryption Details

### Automatic Key Generation

Both daemon and hosts/workers automatically generate CURVE keypairs on first run. Keys are stored as Z85-encoded text in
the config directory.

### Per-Connection Encryption

Each ZeroMQ socket (RPC REQ/REP, PUB/SUB) uses CURVE encryption with ephemeral session keys. This provides:

- **Forward Secrecy**: Compromise of long-term keys doesn't compromise past sessions
- **Authentication**: Only enrolled hosts/workers with registered public keys can connect
- **Encryption**: All messages are encrypted in transit

### ZAP Authentication

After enrollment, the daemon validates all incoming ZMQ connections using the ZeroMQ Authentication Protocol (ZAP):

1. Connection attempts include the client's CURVE public key
2. Daemon checks if the public key is in the allowed-hosts directory
3. Connection is accepted only if the public key is registered
4. Invalid or unknown keys are rejected

## Example Configurations

### docker-compose.cluster.yml

The mooR repository includes `docker-compose.cluster.yml` as a reference implementation. **This configuration runs on a single host** (all containers on one machine) but demonstrates the TCP/CURVE setup you'd use for an actual multi-machine clustered deployment:

```bash
# Test clustered configuration locally on a single machine
docker compose -f docker-compose.cluster.yml up -d
```

This example configuration shows:

- Daemon configured with TCP endpoints instead of IPC
- Separate host and worker containers
- Enrollment token distribution between components
- CURVE encryption and authentication setup

**Purpose**: Use this as a reference for understanding and testing the clustered configuration locally before deploying across actual separate machines. For production multi-machine deployments, adapt the endpoint addresses to point to different hosts and configure appropriate network routing.

### Kubernetes Deployment (deploy/kubernetes/)

For a more complete example of multi-machine clustered deployment, see the Kubernetes manifests in `deploy/kubernetes/`. This configuration demonstrates:

- Health checks with daemon ping/pong verification
- Horizontal scaling of hosts and workers
- Readiness and liveness probes
- Resource limits and requests
- Service discovery and networking
- Enrollment token management via Secrets

While designed for local testing with kind/minikube, these manifests serve as a solid reference for production Kubernetes deployments. See the `deploy/kubernetes/README.md` for detailed deployment instructions.

## Managing Enrollment Tokens

### Rotating Tokens

Once the daemon is running, wizard administrators can rotate the enrollment token from inside the MOO:

```moo
token = rotate_enrollment_token();
// token is the new enrollment token string
// Also written to ${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token
```

This allows rotating secrets without shell access to the daemon host.

### Distributing Tokens

For clustered deployments, ensure the enrollment token is securely distributed:

- **Kubernetes**: Use Secrets mounted as files or environment variables
- **Configuration Management**: Use Ansible/Chef/Puppet to distribute tokens
- **Secrets Management**: Use Vault, AWS Secrets Manager, etc.

## Network Configuration

### Firewall Rules

Typical firewall configuration for clustered deployment:

**Daemon (Internal)**:

- Allow TCP 7899 (RPC) from host/worker subnets
- Allow TCP 7898 (Events) from host/worker subnets
- Allow TCP 7896-7897 (Workers) from worker subnets
- Allow TCP 7900 (Enrollment) from host/worker subnets
- No public internet access required

**Web Host (DMZ)**:

- Allow inbound HTTP/HTTPS from internet
- Allow outbound TCP 7899, 7898 to daemon
- No direct database access

**Telnet Host (DMZ or Private)**:

- Allow inbound TCP 7777 (or chosen port) from users
- Allow outbound TCP 7899, 7898 to daemon

**Curl Worker (Restricted)**:

- Allow outbound HTTP/HTTPS to internet (as needed)
- Allow outbound TCP 7896-7897 to daemon
- No inbound connections required

### DNS/Service Discovery

For multi-machine deployments:

- Use internal DNS for daemon hostname (e.g., `daemon.moo.internal`)
- Configure hosts/workers to connect via stable hostname
- Use service discovery (Consul, Kubernetes DNS) for dynamic environments

## Monitoring and Observability

Clustered deployments benefit from comprehensive monitoring:

### Health Checks

All components implement health endpoints that verify daemon connectivity:

- **Web-host**: HTTP GET on `/health` (port 8081 by default)
- **Telnet-host**: TCP socket on port 9888
- **Curl-worker**: TCP socket on port 9999

Each host and worker tracks daemon ping/pong messages and reports healthy only if a ping was received within the last 30 seconds. This ensures the component is enrolled, CURVE-authenticated, and actively communicating with the daemon.

**Testing health endpoints:**

```bash
# Web-host HTTP health check
curl http://web-host.internal:8081/health

# Telnet-host TCP health check
nc -zv telnet-host.internal 9888

# Curl-worker TCP health check
nc -zv curl-worker.internal 9999
```

In Kubernetes, these endpoints are used for liveness and readiness probes to ensure only healthy pods receive traffic.

### Metrics to Monitor

- **Connection Health**: Are all hosts/workers enrolled and connected?
- **Network Latency**: Latency between daemon and hosts/workers
- **Message Throughput**: RPC and event message rates
- **Error Rates**: Failed enrollments, disconnections, authentication failures
- **Health Check Status**: Monitor health endpoint responses for early warning of connectivity issues

### Logging

Each component logs independently:

```bash
# View daemon logs
journalctl -u moor-daemon -f

# View web-host logs (on web-host machine)
journalctl -u moor-web-host -f

# View curl-worker logs (on worker machine)
journalctl -u moor-curl-worker -f
```

For centralized logging, configure rsyslog or use a log aggregation service (ELK, Loki, etc.).

## Troubleshooting

### Hosts/Workers Can't Enroll

**Check enrollment token**:

```bash
# Verify token exists on daemon
cat ${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token

# Verify token matches on host/worker
cat /path/to/enrollment-token
```

**Check network connectivity**:

```bash
# From host/worker machine
telnet daemon.internal 7900
```

**Check daemon logs**:

```bash
journalctl -u moor-daemon -n 50 | grep enrollment
```

### Connection Drops or Timeouts

**Network issues**:

- Check firewall rules between machines
- Verify TCP endpoints are reachable
- Check for MTU issues or packet loss

**Resource exhaustion**:

- Monitor daemon CPU/memory usage
- Check ZeroMQ I/O thread count (adjust with `--num-io-threads`)

### Authentication Failures

**ZAP rejection**:

- Host/worker public key not registered with daemon
- Re-enroll the host/worker
- Check allowed-hosts directory on daemon

## Performance Considerations

### Network Overhead

TCP communication has higher latency than IPC:

- IPC: Microseconds
- TCP (same datacenter): Low milliseconds
- TCP (cross-datacenter): Tens to hundreds of milliseconds

Design your deployment topology to minimize critical-path latency.

### Connection Limits

The daemon can handle many host/worker connections, but consider:

- ZeroMQ I/O thread count (`--num-io-threads`)
- Operating system file descriptor limits
- Network bandwidth for high-traffic deployments

### Scalability

**Horizontal Scaling**:

- Hosts and workers scale horizontally (multiple instances)
- The daemon is currently a singleton (scale vertically)

**Load Balancing**:

- Use load balancers for web-host and telnet-host
- Workers automatically distribute tasks via pub/sub pattern

## When to Use Clustered Deployment

**Use clustered deployment when**:

- You need to isolate components for security
- You want to distribute network I/O across machines
- You're deploying in Kubernetes or multi-server environment
- You need geographic distribution of hosts
- You have specific firewall or network segmentation requirements

**Use single-machine IPC when**:

- Running on a single server or VM
- Using Docker Compose for development or small deployments
- Network latency would be a concern
- Simpler configuration is preferred

For most users, the default IPC configuration (Docker Compose, Debian packages) is the recommended starting point.
Graduate to clustered deployment when you have specific scaling, security, or distribution requirements.
