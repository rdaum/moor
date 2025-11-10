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

# Bring up mooR in a local kind cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test-helpers.sh"

CLUSTER_NAME="moor"
NAMESPACE="moor"

log_info "Starting mooR Kubernetes cluster"

# Check prerequisites
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

log_info "✓ Prerequisites satisfied"

# Check if cluster already exists
if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    log_info "Cluster '$CLUSTER_NAME' already exists"
    read -p "Delete and recreate? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Deleting existing cluster..."
        kind delete cluster --name "$CLUSTER_NAME"
    else
        log_info "Using existing cluster"
        kubectl cluster-info --context kind-$CLUSTER_NAME
        exit 0
    fi
fi

# Create kind cluster with port mappings
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

log_info "✓ Cluster created"

# Wait for cluster to be ready
log_info "Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=60s
sleep 5
log_info "✓ Cluster ready"

# Build images
log_info "Building mooR images..."
cd "$SCRIPT_DIR/../.."

# Build base backend image first
if ! docker image inspect moor:base >/dev/null 2>&1; then
    log_info "Building base backend image (this may take a few minutes)..."
    docker build -t moor:base --target backend .
fi

# Build image with cores included for local development
log_info "Building image with cores..."
cat > Dockerfile.with-cores <<'EOF'
FROM moor:base

# Copy cores into the image
COPY cores/ /cores/
EOF

docker build -f Dockerfile.with-cores -t moor:latest .
rm -f Dockerfile.with-cores

if docker image inspect moor-frontend:latest >/dev/null 2>&1; then
    log_info "Using existing moor-frontend:latest image"
else
    log_info "Building frontend image..."
    docker build -t moor-frontend:latest --target frontend .
fi

log_info "✓ Images built"

# Load images into cluster
log_info "Loading images into kind cluster..."
kind load docker-image moor:latest --name "$CLUSTER_NAME"
kind load docker-image moor-frontend:latest --name "$CLUSTER_NAME"
log_info "✓ Images loaded"

# Deploy manifests
log_info "Deploying mooR to cluster..."
cd "$SCRIPT_DIR"

# Create namespace first
kubectl apply -f namespace.yaml

# Create enrollment token secret directly (the Job approach doesn't work without kubectl in the container)
log_info "Creating enrollment token..."
kubectl create secret generic moor-enrollment-token \
  -n moor \
  --from-literal=token="$(openssl rand -base64 32)" \
  --dry-run=client -o yaml | kubectl apply -f -

# Deploy everything else
kubectl apply -k .
log_info "✓ Manifests deployed"

# Wait for pods to be created
log_info "Waiting for pods to start..."
sleep 5

# Show status
log_info ""
log_info "Cluster: $CLUSTER_NAME"
log_info "Namespace: $NAMESPACE"
log_info ""
log_info "Pods status:"
kubectl get pods -n moor

log_info ""
log_info "Services:"
log_info "  Telnet:        telnet localhost 8888"
log_info "  Web frontend:  http://localhost:8080"
log_info ""
log_info "Useful commands:"
log_info "  kubectl get pods -n moor              # Show pod status"
log_info "  kubectl logs -n moor -l app=moor-daemon --tail=50  # Daemon logs"
log_info "  kubectl logs -n moor -l app=moor-telnet-host       # Telnet host logs"
log_info "  kubectl logs -n moor -l app=moor-web-host          # Web host logs"
log_info "  kubectl delete -k deploy/kubernetes    # Stop everything"
log_info "  kind delete cluster --name $CLUSTER_NAME  # Delete cluster"
log_info ""
log_info "Note: Initial startup may take a minute while pods initialize"
log_info "      Use 'kubectl get pods -n moor' to check status"
