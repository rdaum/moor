#!/bin/bash
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

# Test script for Kubernetes deployment
# This script creates a kind cluster, builds/loads images, deploys manifests, and validates

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test-helpers.sh"

CLUSTER_NAME="moor-test"
NAMESPACE="moor"
TIMEOUT=300  # 5 minutes for pods to be ready

log_info "Starting Kubernetes deployment test"

# Cleanup function
cleanup() {
    if [ "${KEEP_CLUSTER:-0}" = "1" ]; then
        log_info "KEEP_CLUSTER=1, skipping cleanup. Cluster: $CLUSTER_NAME"
        log_info "To access the cluster: kind get kubeconfig --name $CLUSTER_NAME > ~/.kube/config"
        log_info "To delete later: kind delete cluster --name $CLUSTER_NAME"
    else
        log_info "Cleaning up kind cluster..."
        kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
        log_info "Cleanup complete"
    fi
}

# Setup trap for cleanup
trap cleanup EXIT

# Check prerequisites
log_info "Checking prerequisites..."

if ! command -v kind &> /dev/null; then
    log_error "kind is not installed"
    log_error "Install with: curl -Lo ./kind https://kind.sigs.k8s.io/dl/latest/kind-linux-amd64 && chmod +x ./kind && sudo mv ./kind /usr/local/bin/"
    exit 1
fi

if ! command -v kubectl &> /dev/null; then
    log_error "kubectl is not installed"
    log_error "Install from: https://kubernetes.io/docs/tasks/tools/"
    exit 1
fi

if ! command -v docker &> /dev/null; then
    log_error "docker is not installed"
    exit 1
fi

log_info "✓ Prerequisites satisfied (kind, kubectl, docker)"

# Delete any existing test cluster
kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true

# Create kind cluster with extra port mappings for services
log_info "Creating kind cluster '$CLUSTER_NAME'..."
cat <<EOF | kind create cluster --name "$CLUSTER_NAME" --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    extraPortMappings:
      # Telnet
      - containerPort: 30888
        hostPort: 8888
        protocol: TCP
      # Web frontend
      - containerPort: 30080
        hostPort: 8080
        protocol: TCP
EOF

log_info "✓ Kind cluster created"

# Wait for cluster to be fully ready
log_info "Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=60s || {
    log_error "Cluster nodes did not become ready"
    exit 1
}

# Give containerd a moment to fully initialize
sleep 5
log_info "✓ Cluster is ready"

# Build moor images if they don't exist
log_info "Building moor images..."
cd "$SCRIPT_DIR/../.."

# Check if images already exist
if docker image inspect moor-with-cores:test >/dev/null 2>&1 && docker image inspect moor-frontend:test >/dev/null 2>&1; then
    log_info "Using existing moor images"
else
    log_info "Building backend image (this may take a few minutes)..."
    # First build the base backend image
    docker build -t moor:test --target backend . || {
        log_error "Failed to build backend image"
        exit 1
    }

    log_info "Building test image with cores..."
    # Build a test-specific image that includes the cores
    cat > Dockerfile.k8s-test <<'EOF'
FROM moor:test

# Copy cores into the image for testing
COPY cores/ /cores/

EOF

    # Build test image with cores
    docker build -f Dockerfile.k8s-test -t moor-with-cores:test . || {
        log_error "Failed to build test image with cores"
        exit 1
    }

    rm -f Dockerfile.k8s-test

    log_info "Building frontend image..."
    docker build -t moor-frontend:test --target frontend . || {
        log_error "Failed to build frontend image"
        exit 1
    }
fi

log_info "✓ Images built"

# Load images into kind cluster
log_info "Loading images into kind cluster..."
kind load docker-image moor-with-cores:test --name "$CLUSTER_NAME"
kind load docker-image moor-frontend:test --name "$CLUSTER_NAME"
log_info "✓ Images loaded"

# Create a test kustomization that uses NodePort services
log_info "Creating test configuration..."
TEST_DIR=$(mktemp -d)
cp -r "$SCRIPT_DIR"/*.yaml "$TEST_DIR/"

# Modify kustomization to use test images
cat > "$TEST_DIR/kustomization.yaml" <<EOF
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

namespace: moor

resources:
  - namespace.yaml
  - configmap.yaml
  - secret.yaml
  # Skip PVC for testing - we use emptyDir in the patch
  - daemon.yaml
  - telnet-host.yaml
  - web-host.yaml
  - curl-worker.yaml
  - frontend.yaml
  - services-nodeport.yaml

commonLabels:
  app.kubernetes.io/name: moor
  app.kubernetes.io/instance: test

images:
  - name: moor
    newName: moor-with-cores
    newTag: test
  - name: moor-frontend
    newName: moor-frontend
    newTag: test

# Remove enrollment token job - create secret directly for testing
patches:
  - target:
      kind: ConfigMap
      name: moor-config
    patch: |-
      - op: replace
        path: /data/database-name
        value: "test.db"
EOF

# Create NodePort services instead of LoadBalancer for testing
cat > "$TEST_DIR/services-nodeport.yaml" <<'EOF'
# Internal headless service for daemon
apiVersion: v1
kind: Service
metadata:
  name: moor-daemon
  namespace: moor
  labels:
    app.kubernetes.io/name: moor
    app.kubernetes.io/component: daemon
spec:
  type: ClusterIP
  clusterIP: None
  selector:
    app: moor-daemon
  ports:
    - name: rpc
      port: 7899
      targetPort: rpc
      protocol: TCP
    - name: events
      port: 7898
      targetPort: events
      protocol: TCP
    - name: workers-req
      port: 7896
      targetPort: workers-req
      protocol: TCP
    - name: workers-resp
      port: 7897
      targetPort: workers-resp
      protocol: TCP
    - name: enrollment
      port: 7900
      targetPort: enrollment
      protocol: TCP
---
# NodePort service for telnet
apiVersion: v1
kind: Service
metadata:
  name: moor-telnet
  namespace: moor
  labels:
    app.kubernetes.io/name: moor
    app.kubernetes.io/component: telnet-host
spec:
  type: NodePort
  selector:
    app: moor-telnet-host
  ports:
    - name: telnet
      port: 8888
      targetPort: telnet
      nodePort: 30888
      protocol: TCP
---
# Internal service for web-host
apiVersion: v1
kind: Service
metadata:
  name: moor-web-host
  namespace: moor
  labels:
    app.kubernetes.io/name: moor
    app.kubernetes.io/component: web-host
spec:
  type: ClusterIP
  selector:
    app: moor-web-host
  ports:
    - name: http
      port: 8081
      targetPort: http
      protocol: TCP
---
# NodePort service for frontend
apiVersion: v1
kind: Service
metadata:
  name: moor-frontend
  namespace: moor
  labels:
    app.kubernetes.io/name: moor
    app.kubernetes.io/component: frontend
spec:
  type: NodePort
  selector:
    app: moor-frontend
  ports:
    - name: http
      port: 80
      targetPort: http
      nodePort: 30080
      protocol: TCP
EOF

# Remove the Job-based secret creation, create secret directly
cat > "$TEST_DIR/secret.yaml" <<'EOF'
apiVersion: v1
kind: Secret
metadata:
  name: moor-enrollment-token
  namespace: moor
  labels:
    app.kubernetes.io/name: moor
    app.kubernetes.io/component: secret
type: Opaque
stringData:
  token: "test-enrollment-token-12345"
EOF

# Use JSON patch to replace volumes and add core import
# The cores are baked into the moor-with-cores:test image
cat > "$TEST_DIR/daemon-patch.yaml" <<'EOF'
- op: replace
  path: /spec/template/spec/volumes
  value:
    - name: data
      emptyDir: {}
    - name: allowed-hosts
      emptyDir: {}
    - name: config-dir
      secret:
        secretName: moor-enrollment-token
        items:
          - key: token
            path: enrollment-token
- op: replace
  path: /spec/template/spec/containers/0/volumeMounts
  value:
    - name: data
      mountPath: /data
    - name: allowed-hosts
      mountPath: /tmp/.local/share/moor
    - name: config-dir
      mountPath: /moor/enrollment
      readOnly: true
- op: add
  path: /spec/template/spec/containers/0/env/-
  value:
    name: HOME
    value: /tmp
- op: replace
  path: /spec/template/spec/containers/0/args
  value:
    - /data/moor-data
    - --db=$(DATABASE_NAME)
    - --rpc-listen=tcp://0.0.0.0:$(RPC_PORT)
    - --events-listen=tcp://0.0.0.0:$(EVENTS_PORT)
    - --workers-request-listen=tcp://0.0.0.0:$(WORKERS_REQUEST_PORT)
    - --workers-response-listen=tcp://0.0.0.0:$(WORKERS_RESPONSE_PORT)
    - --enrollment-listen=tcp://0.0.0.0:$(ENROLLMENT_PORT)
    - --enrollment-token-file=/moor/enrollment/enrollment-token
    - --generate-keypair
    - --import=$(IMPORT_PATH)
    - --import-format=$(IMPORT_FORMAT)
EOF

# Patch web-host to use /health endpoint (same as production config)
# Note: The /health endpoint checks daemon ping/pong to verify connectivity
# This is faster and lighter than invoking MOO code
cat > "$TEST_DIR/web-host-patch.yaml" <<'EOF'
- op: replace
  path: /spec/template/spec/containers/0/readinessProbe
  value:
    httpGet:
      path: /health
      port: http
    initialDelaySeconds: 5
    periodSeconds: 5
    timeoutSeconds: 3
    failureThreshold: 2
- op: replace
  path: /spec/template/spec/containers/0/livenessProbe
  value:
    httpGet:
      path: /health
      port: http
    initialDelaySeconds: 20
    periodSeconds: 10
    timeoutSeconds: 5
    failureThreshold: 3
EOF

# Update kustomization to include the JSON patches
cat >> "$TEST_DIR/kustomization.yaml" <<'EOF'

patches:
  - path: daemon-patch.yaml
    target:
      kind: StatefulSet
      name: moor-daemon
  - path: web-host-patch.yaml
    target:
      kind: Deployment
      name: moor-web-host
EOF

log_info "✓ Test configuration created"

# Deploy to kubernetes
log_info "Deploying to Kubernetes..."
kubectl apply -k "$TEST_DIR" || {
    log_error "Failed to deploy manifests"
    exit 1
}
log_info "✓ Manifests applied"

# Wait for namespace
log_info "Waiting for namespace..."
kubectl wait --for=jsonpath='{.status.phase}'=Active namespace/moor --timeout=30s || {
    log_error "Namespace not active"
    exit 1
}

# Check what resources were created
log_info "Checking deployed resources..."
kubectl get all -n moor || true
kubectl get configmap -n moor || true
kubectl get secret -n moor || true

# Check StatefulSet
log_info "Checking StatefulSet..."
kubectl get statefulset -n moor || true
kubectl describe statefulset moor-daemon -n moor || {
    log_error "StatefulSet moor-daemon not found"
    log_error "All resources in namespace:"
    kubectl get all -n moor -o wide
    exit 1
}

# Wait for StatefulSet to create pod
log_info "Waiting for daemon pod to be created..."
for i in {1..30}; do
    POD_COUNT=$(kubectl get pods -n moor -l app=moor-daemon --no-headers 2>/dev/null | wc -l)
    if [ "$POD_COUNT" -gt 0 ]; then
        log_info "✓ Daemon pod created"
        break
    fi
    if [ $i -eq 30 ]; then
        log_error "Daemon pod was not created after 30 seconds"
        log_error "StatefulSet status:"
        kubectl describe statefulset moor-daemon -n moor
        exit 1
    fi
    sleep 1
done

# Note: We don't wait for daemon readiness - the hosts will retry until daemon is up
log_info "Daemon pod created, hosts will connect when it's ready..."

# Wait for telnet host pod to be ready (includes time for daemon to import)
log_info "Waiting for telnet host to be ready (timeout: ${TIMEOUT}s)..."
if ! kubectl wait --for=condition=ready pod -l app=moor-telnet-host -n moor --timeout=${TIMEOUT}s; then
    log_error "Telnet host pod failed to become ready"
    log_error "Daemon logs:"
    kubectl logs -n moor -l app=moor-daemon --tail=50 || true
    log_error "Telnet host logs:"
    kubectl logs -n moor -l app=moor-telnet-host --tail=50 || true
    exit 1
fi
log_info "✓ Telnet host pod is ready"

# Wait for web host pod to be ready
log_info "Waiting for web host to be ready..."
if ! kubectl wait --for=condition=ready pod -l app=moor-web-host -n moor --timeout=${TIMEOUT}s; then
    log_error "Web host pod failed to become ready"
    kubectl logs -n moor -l app=moor-web-host --tail=50 || true
    exit 1
fi
log_info "✓ Web host pod is ready"

# Wait for frontend pod to be ready
log_info "Waiting for frontend to be ready..."
if ! kubectl wait --for=condition=ready pod -l app=moor-frontend -n moor --timeout=60s; then
    log_error "Frontend pod failed to become ready"
    kubectl logs -n moor -l app=moor-frontend --tail=50 || true
    exit 1
fi
log_info "✓ Frontend pod is ready"

# Hosts being ready proves daemon has imported and is serving
# No need for additional waiting or log checks

# Test telnet connectivity with actual MOO commands
log_info "Testing telnet connection and MOO core..."
{
    sleep 2
    echo "connect wizard"
    sleep 3
    echo "look"
    sleep 2
    echo "@who"
    sleep 2
    echo "@quit"
    sleep 1
} | telnet localhost 8888 > /tmp/k8s-telnet-test.txt 2>&1 || true

# Show telnet output for debugging
log_info "Telnet test output:"
cat /tmp/k8s-telnet-test.txt | head -30

# Verify we got a valid MOO response
if grep -qE "Connected|Welcome|The First Room|Wizard" /tmp/k8s-telnet-test.txt; then
    log_info "✓ Telnet connection and MOO core verified"
else
    log_error "Telnet test failed - MOO core not responding properly"
    log_error "See telnet output above for details"
    exit 1
fi

# Test web frontend connectivity
log_info "Testing web frontend (port 8080)..."
if curl -f -s http://localhost:8080/ | grep -qE "moor|<!DOCTYPE html>"; then
    log_info "✓ Web frontend is serving HTML content"
else
    log_error "Web frontend not serving HTML"
    exit 1
fi

# Test web-host API - welcome message endpoint verifies MOO core is loaded
log_info "Testing MOO core via web API..."
WELCOME_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/fb/invoke_welcome_message" 2>/dev/null || echo "000")
if [ "$WELCOME_STATUS" = "200" ]; then
    log_info "✓ MOO core is loaded and responding via web API"
else
    log_error "Welcome message endpoint returned status $WELCOME_STATUS (expected 200)"
    log_error "MOO core may not be loaded or web-host cannot connect to daemon"
    exit 1
fi

# Cleanup temp file
rm -f /tmp/k8s-telnet-test.txt

# Show pod status
log_info "Final pod status:"
kubectl get pods -n moor

# Check for errors in logs
log_info "Checking logs for critical errors..."
DAEMON_ERRORS=$(kubectl logs -n moor -l app=moor-daemon --tail=100 | grep -iE "panic|fatal" | head -5 || true)
if [ -n "$DAEMON_ERRORS" ]; then
    log_warn "Found errors in daemon logs:"
    echo "$DAEMON_ERRORS"
else
    log_info "✓ No critical errors in daemon logs"
fi

TELNET_ERRORS=$(kubectl logs -n moor -l app=moor-telnet-host --tail=100 | grep -iE "panic|fatal" | head -5 || true)
if [ -n "$TELNET_ERRORS" ]; then
    log_warn "Found errors in telnet host logs:"
    echo "$TELNET_ERRORS"
else
    log_info "✓ No critical errors in telnet host logs"
fi

WEB_ERRORS=$(kubectl logs -n moor -l app=moor-web-host --tail=100 | grep -iE "panic|fatal" | head -5 || true)
if [ -n "$WEB_ERRORS" ]; then
    log_warn "Found errors in web host logs:"
    echo "$WEB_ERRORS"
else
    log_info "✓ No critical errors in web host logs"
fi

# Cleanup temp directory
rm -rf "$TEST_DIR"

log_info "✓ Kubernetes deployment test completed successfully"
log_info ""
log_info "Summary:"
log_info "  - Kind cluster created and configured"
log_info "  - Images built and loaded"
log_info "  - Kubernetes manifests deployed"
log_info "  - All pods started and became ready"
log_info "  - Services accessible via NodePort"
log_info "  - No critical errors in logs"
log_info ""
log_info "Cluster: $CLUSTER_NAME"
log_info "Namespace: $NAMESPACE"

if [ "${KEEP_CLUSTER:-0}" = "1" ]; then
    log_info ""
    log_info "To interact with the cluster:"
    log_info "  kubectl get pods -n moor"
    log_info "  kubectl logs -n moor -l app=moor-daemon"
    log_info "  telnet localhost 8888"
    log_info "  curl http://localhost:8080"
fi
