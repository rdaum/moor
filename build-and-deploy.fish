#!/usr/bin/env fish
# Build and deploy moor images to timbran.org

set -l CONTEXT "timbran"

echo "🔨 Building moor images for $CONTEXT..."
echo ""

# Build backend services (daemon, web-host, telnet-host, curl-worker)
echo "📦 Building backend services..."
docker --context $CONTEXT build \
    --target backend \
    -t moor-moor-daemon:latest \
    -t moor-moor-telnet-host:latest \
    -t moor-moor-web-host:latest \
    -t moor-moor-curl-worker:latest \
    .

if test $status -ne 0
    echo "❌ Backend build failed!"
    exit 1
end

echo ""
echo "✅ Backend services built successfully"
echo ""

# Build frontend nginx image
echo "📦 Building frontend nginx image..."
docker --context $CONTEXT build \
    --target frontend \
    -t moor-moor-frontend:latest \
    .

if test $status -ne 0
    echo "❌ Frontend build failed!"
    exit 1
end

echo ""
echo "✅ Frontend built successfully"
echo ""
echo "🎉 All images built and pushed to $CONTEXT!"
echo ""
echo "To restart services, run:"
echo "  ssh moor@timbran.org 'cd ~/timbran && docker-compose down && docker-compose up -d'"
