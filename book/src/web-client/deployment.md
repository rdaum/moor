# Deployment

The web client is optional, but when enabled it typically sits behind a proxy that serves static assets and forwards
API/WebSocket traffic to `moor-web-host`.

## Typical Topology

A common deployment flow is:

1. Browser loads static web client assets (HTML/CSS/JS).
2. The proxy forwards `/api` and WebSocket traffic to `moor-web-host`.
3. `moor-web-host` connects to the daemon over RPC and relays FlatBuffer events to the client.

## Proxy Examples

Use the deployment templates as starting points:

- `deploy/web-basic/`: HTTP-only Docker setup with nginx proxying to `moor-web-host`.
- `deploy/web-ssl/`: HTTPS setup with Let's Encrypt and nginx TLS termination.
- `deploy/debian-packages/nginx-for-debian.conf`: Example nginx config for packaged installs.
- `deploy/kubernetes/`: Ingress + frontend deployment for Kubernetes.

### Minimal nginx mapping (HTTP)

Based on `deploy/web-basic/nginx.conf`, the proxy must forward API and WebSocket traffic to `moor-web-host`:

```nginx
location /api/ { proxy_pass http://moor_api; }
location /auth/ { proxy_pass http://moor_api; }
location /fb/ { proxy_pass http://moor_api; }
location /ws/ {
    proxy_pass http://moor_api;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

The frontend is served as static files with a fallback to `index.html` for client-side routing.

### HTTPS notes

For SSL, see `deploy/web-ssl/nginx-ssl.conf` for the TLS termination and HTTP->HTTPS redirect configuration.

## Development

During development, Vite serves the web client and proxies API/WebSocket requests to the web host.

Related docs:

- [Server Architecture](../the-system/server-architecture.md)
- [Running a mooR Server](../the-system/running-the-server.md)
