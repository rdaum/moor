# Kubernetes Deployment for mooR

This directory contains Kubernetes manifests for deploying mooR in a clustered configuration using
TCP communication with CURVE encryption and enrollment-based authentication.

**Purpose**: These manifests are designed for local testing and development with kind/minikube,
demonstrating mooR's distributed architecture. They serve as a starting point and reference for
production deployments, which will require additional hardening, monitoring, and operational
considerations.

## Overview

This deployment demonstrates mooR's distributed architecture capabilities:

- **Stateful daemon**: Single-instance core MOO server with persistent storage
- **Scalable hosts**: Horizontally scalable telnet and web hosts
- **Isolated workers**: Distributed curl workers for security segmentation
- **TCP/CURVE communication**: Encrypted inter-component communication
- **Enrollment-based auth**: Secure host/worker registration
- **Health checks**: Daemon ping/pong verification for all components

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Kubernetes Cluster                 │
│                                                     │
│  ┌──────────────┐         ┌──────────────┐        │
│  │   Ingress    │         │   Service    │        │
│  │   (Web)      │         │  (Telnet)    │        │
│  └──────┬───────┘         └──────┬───────┘        │
│         │                        │                 │
│  ┌──────▼───────┐         ┌─────▼────────┐        │
│  │  Frontend    │         │ Telnet Host  │        │
│  │  (nginx)     │         │  (N replicas)│        │
│  └──────┬───────┘         └──────┬───────┘        │
│         │                        │                 │
│  ┌──────▼───────┐                │                 │
│  │  Web Host    │                │                 │
│  │ (N replicas) │                │                 │
│  └──────┬───────┘                │                 │
│         │                        │                 │
│         └────────┬───────────────┘                 │
│                  │                                 │
│           ┌──────▼───────┐                         │
│           │   Daemon     │                         │
│           │ (StatefulSet)│                         │
│           │      +       │                         │
│           │     PVC      │                         │
│           └──────┬───────┘                         │
│                  │                                 │
│           ┌──────▼───────┐                         │
│           │ Curl Worker  │                         │
│           │ (N replicas) │                         │
│           └──────────────┘                         │
└─────────────────────────────────────────────────────┘
```

## Prerequisites

- Kubernetes cluster (v1.24+)
- `kubectl` configured to access your cluster
- Container registry to host mooR images (or use local images)
- Storage provisioner for PersistentVolumeClaims
- Ingress controller (nginx, traefik, etc.) for web access
- (Optional) cert-manager for automatic TLS certificates

## Quick Start

### 1. Build and Push Images

First, build the mooR images and push them to your container registry:

```bash
# From the mooR root directory
docker build -t your-registry/moor:latest --target backend .
docker build -t your-registry/moor-frontend:latest --target frontend .

docker push your-registry/moor:latest
docker push your-registry/moor-frontend:latest
```

**Or for local testing with minikube/kind:**

```bash
# For minikube
eval $(minikube docker-env)
docker build -t moor:latest --target backend .
docker build -t moor-frontend:latest --target frontend .

# For kind
kind load docker-image moor:latest
kind load docker-image moor-frontend:latest
```

### 2. Configure Deployment

Edit `kustomization.yaml` to set your image registry and preferences:

```yaml
images:
  - name: moor
    newName: your-registry/moor
    newTag: latest
  - name: moor-frontend
    newName: your-registry/moor-frontend
    newTag: latest
```

Edit `configmap.yaml` to configure your deployment settings (database name, core import, etc.).

Edit `ingress.yaml` to set your domain name.

### 3. Deploy to Kubernetes

```bash
# Create namespace and deploy all resources
kubectl apply -k .

# Or apply manually
kubectl create namespace moor
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f secret.yaml
kubectl apply -f pvc.yaml
kubectl apply -f daemon.yaml
kubectl apply -f telnet-host.yaml
kubectl apply -f web-host.yaml
kubectl apply -f curl-worker.yaml
kubectl apply -f frontend.yaml
kubectl apply -f services.yaml
kubectl apply -f ingress.yaml
```

### 4. Verify Deployment

```bash
# Check pod status
kubectl get pods -n moor

# Watch rollout
kubectl rollout status statefulset/moor-daemon -n moor
kubectl rollout status deployment/moor-telnet-host -n moor
kubectl rollout status deployment/moor-web-host -n moor

# Check logs
kubectl logs -n moor -l app=moor-daemon
kubectl logs -n moor -l app=moor-telnet-host
```

### 5. Access mooR

**Telnet Access:**

```bash
# Port-forward for local testing
kubectl port-forward -n moor service/moor-telnet 8888:8888

# Or use LoadBalancer/NodePort service
telnet <external-ip> 8888
```

**Web Access:**

```bash
# Port-forward for local testing
kubectl port-forward -n moor service/moor-frontend 8080:80

# Or configure Ingress with your domain
# Visit https://your-domain.com
```

## Configuration

### Resource Limits

Default resource limits are conservative. Adjust based on your workload:

**Daemon** (in `daemon.yaml`):

- Memory: 512Mi-2Gi (adjust based on database size and user count)
- CPU: 500m-2000m (increase for more concurrent tasks)

**Hosts/Workers** (in respective files):

- Memory: 128Mi-512Mi per replica
- CPU: 100m-500m per replica

### Horizontal Scaling

Scale hosts and workers based on load:

```bash
# Scale web hosts
kubectl scale deployment/moor-web-host -n moor --replicas=5

# Scale telnet hosts
kubectl scale deployment/moor-telnet-host -n moor --replicas=3

# Scale curl workers
kubectl scale deployment/moor-curl-worker -n moor --replicas=2
```

**Or enable HorizontalPodAutoscaler** (see `hpa.yaml`):

```bash
kubectl apply -f hpa.yaml
```

### Persistent Storage

The daemon requires persistent storage for the MOO database.

**Default**: Uses dynamic provisioning with the default StorageClass.

**Custom StorageClass**:

Edit `pvc.yaml` to specify a different StorageClass:

```yaml
storageClassName: fast-ssd  # Your StorageClass
```

**Storage Size**:

Default is 10Gi. Adjust based on your expected database size:

```yaml
resources:
  requests:
    storage: 50Gi  # Increase for larger databases
```

### Enrollment Token

The enrollment token is automatically generated during deployment using a Kubernetes Job.

**To rotate the enrollment token:**

```bash
# From inside the MOO (as wizard)
token = rotate_enrollment_token();

# Or manually update the secret
kubectl create secret generic moor-enrollment-token \
  --from-literal=token=$(uuidgen) \
  --dry-run=client -o yaml | kubectl apply -n moor -f -

# Restart hosts/workers to re-enroll
kubectl rollout restart deployment/moor-telnet-host -n moor
kubectl rollout restart deployment/moor-web-host -n moor
kubectl rollout restart deployment/moor-curl-worker -n moor
```

### TLS/HTTPS Configuration

#### Option 1: cert-manager (Recommended)

Install cert-manager and configure automatic Let's Encrypt certificates:

```bash
# Install cert-manager
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml

# Apply cert-manager configuration
kubectl apply -f ingress-tls.yaml
```

The `ingress-tls.yaml` manifest includes cert-manager annotations for automatic certificate
provisioning.

#### Option 2: Manual TLS Secret

Create a TLS secret with your own certificates:

```bash
kubectl create secret tls moor-tls \
  --cert=path/to/tls.crt \
  --key=path/to/tls.key \
  -n moor

# Then use ingress-tls.yaml
kubectl apply -f ingress-tls.yaml
```

### Network Policies

For enhanced security, apply NetworkPolicies to restrict communication:

```bash
kubectl apply -f network-policy.yaml
```

This restricts:

- Daemon only accepts connections from hosts/workers
- Workers can only reach daemon (no lateral movement)
- Hosts can only reach daemon
- External access only to telnet-host and frontend

## Monitoring and Observability

### Logging

**View logs from all components:**

```bash
# Daemon logs
kubectl logs -n moor -l app=moor-daemon -f

# All moor logs
kubectl logs -n moor -l app.kubernetes.io/name=moor -f --all-containers=true
```

**Centralized logging** (with Loki, ELK, etc.):

Add appropriate annotations to pod specs for log collection.

### Metrics

For Prometheus monitoring, mooR pods can be annotated for metrics scraping.

Add to pod specs:

```yaml
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "9090"  # If metrics endpoint is added
```

### Health Checks

All components implement health checks that verify connectivity with the daemon:

- **Daemon**: TCP socket probe on RPC port (verifies port is accepting connections)
- **Web-host**: HTTP GET on `/health` endpoint (verifies recent daemon ping/pong communication)
- **Telnet-host**: TCP socket on port 9888 (verifies recent daemon ping/pong communication)
- **Curl-worker**: TCP socket on port 9999 (verifies recent daemon ping/pong communication)

Each host and worker tracks daemon ping/pong messages. Health checks verify that a ping was received
within the last 30 seconds, ensuring the component is enrolled, authenticated, and actively
communicating with the daemon over CURVE-encrypted connections.

Adjust probe timing in manifests based on your startup times and network conditions.

## Troubleshooting

### Pods Not Starting

**Check pod status:**

```bash
kubectl describe pod -n moor <pod-name>
```

**Common issues:**

1. **Image pull errors**: Verify image registry and credentials
   ```bash
   kubectl create secret docker-registry regcred \
     --docker-server=<registry> \
     --docker-username=<user> \
     --docker-password=<pass> \
     -n moor
   ```

   Then reference in pod specs:
   ```yaml
   imagePullSecrets:
     - name: regcred
   ```

2. **PVC not binding**: Check StorageClass and provisioner
   ```bash
   kubectl get pvc -n moor
   kubectl describe pvc moor-data -n moor
   ```

3. **Resource constraints**: Insufficient cluster resources
   ```bash
   kubectl describe nodes
   ```

### Hosts Can't Enroll

**Check enrollment token:**

```bash
kubectl get secret moor-enrollment-token -n moor -o jsonpath='{.data.token}' | base64 -d
```

**Check daemon logs:**

```bash
kubectl logs -n moor -l app=moor-daemon | grep -i enrollment
```

**Verify network connectivity:**

```bash
# From a host pod
kubectl exec -n moor deployment/moor-telnet-host -- \
  nc -zv moor-daemon.moor.svc.cluster.local 7900
```

### Connection Issues

**Check service endpoints:**

```bash
kubectl get endpoints -n moor
```

**Test connectivity between pods:**

```bash
# From telnet-host to daemon
kubectl exec -n moor deployment/moor-telnet-host -- \
  nc -zv moor-daemon.moor.svc.cluster.local 7899
```

**Check NetworkPolicies:**

```bash
kubectl get networkpolicy -n moor
kubectl describe networkpolicy -n moor
```

### Performance Issues

**Check resource usage:**

```bash
kubectl top pods -n moor
kubectl top nodes
```

**Increase resources:**

Edit deployment/statefulset manifests to increase resource limits.

**Scale horizontally:**

```bash
kubectl scale deployment/moor-web-host -n moor --replicas=5
```

## Advanced Configuration

### Multi-Datacenter Deployment

Deploy hosts/workers in different regions:

1. Use node affinity to place pods in specific zones
2. Configure topology spread constraints for distribution
3. Monitor inter-region latency

Example node affinity:

```yaml
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: topology.kubernetes.io/region
              operator: In
              values:
                - us-west-2
```

### Security Hardening

**Pod Security Standards:**

Apply restricted pod security:

```yaml
# In namespace.yaml
metadata:
  labels:
    pod-security.kubernetes.io/enforce: restricted
```

**Run as non-root:**

All pods already use non-root users (UID 1000).

**Read-only root filesystem:**

Add to container specs:

```yaml
securityContext:
  readOnlyRootFilesystem: true
```

Note: May require emptyDir volumes for temporary files.

### Database Backup

**Manual backup:**

```bash
# Create a snapshot of the PVC
kubectl exec -n moor moor-daemon-0 -- \
  tar czf /tmp/backup.tar.gz /data

kubectl cp moor/moor-daemon-0:/tmp/backup.tar.gz ./backup.tar.gz
```

**Automated backups:**

Use Velero or similar tools for PVC snapshots:

```bash
# Install Velero
velero install --provider <your-provider>

# Create backup schedule
velero schedule create moor-daily \
  --schedule="0 2 * * *" \
  --include-namespaces moor
```

### Custom Cores

**Import a different core:**

Edit the ConfigMap to change the import path:

```yaml
data:
  import-path: "/cores/your-core/src"
  import-format: "objdef"
```

Mount your core files:

```yaml
# In daemon.yaml
volumes:
  - name: cores
    configMap:
      name: your-core-configmap
```

## Production Checklist

Before deploying to production:

- [ ] Build release images (not debug builds)
- [ ] Configure appropriate resource limits
- [ ] Set up persistent volume backups
- [ ] Configure TLS/HTTPS with valid certificates
- [ ] Enable NetworkPolicies for security
- [ ] Configure monitoring and alerting
- [ ] Test failover scenarios
- [ ] Document custom configuration
- [ ] Set up log aggregation
- [ ] Configure HorizontalPodAutoscaler
- [ ] Review security contexts
- [ ] Enable Pod Security Standards
- [ ] Test restoration from backup

## Kustomize Support

This deployment supports Kustomize for managing environment variations:

```bash
# Development overlay
kubectl apply -k overlays/development

# Production overlay
kubectl apply -k overlays/production
```

Create overlays for different environments with specific configurations.

## Migration from Docker Compose

If migrating from Docker Compose:

1. **Export database:**
   ```bash
   docker exec moor-daemon ./moor-emh export /db/export
   ```

2. **Copy to PVC:**
   ```bash
   kubectl cp export moor/moor-daemon-0:/data/
   ```

3. **Import in Kubernetes:** Configure import in ConfigMap, restart daemon.

## Uninstalling

```bash
# Delete all resources
kubectl delete -k .

# Or manually
kubectl delete namespace moor

# Delete PVC (if not auto-deleted)
kubectl delete pvc moor-data -n moor
```

## Support

- **Issues**: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- **Documentation**: [mooR Book](https://timbran.org/book/html/)
- **Community**: [Discord](https://discord.gg/Ec94y5983z)

## Contributing

Improvements to these Kubernetes manifests are welcome! Please test thoroughly in a k8s environment
and document any changes.

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

mooR is licensed under GPL-3.0. See [LICENSE](../../LICENSE) for details.
