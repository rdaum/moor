# Kubernetes Deployment

Kubernetes deployment configurations for mooR are planned for a future release.

## Planned Features

- StatefulSet for moor-daemon with persistent volume
- Deployments for telnet-host, web-host, and curl-worker
- Service definitions for internal and external access
- ConfigMaps for configuration management
- Secrets for sensitive data (enrollment tokens, etc.)
- Ingress configuration for web access
- Horizontal pod autoscaling for hosts

## Current Status

Not yet implemented. Contributions welcome!

## In the Meantime

For container orchestration, consider:

- Using the Docker Compose configurations in `../web-basic/` or `../web-ssl/`
- Adapting these to Docker Swarm if needed
- Using the Debian packages with your own orchestration

## Contributing

If you'd like to contribute Kubernetes manifests:

1. Test thoroughly in a k8s environment
2. Document prerequisites and setup steps
3. Submit a pull request to [Codeberg](https://codeberg.org/timbran/moor)

## Support

- Issues: [Codeberg Issues](https://codeberg.org/timbran/moor/issues)
- Documentation: [mooR Book](https://timbran.org/book/html/)
- Community: [Discord](https://discord.gg/Ec94y5983z)
