<p align="center"><img src="./doc/porcupine-building.jpg" alt="mooR logo" width="300"/></p>

# mooR

[![Sponsor](https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink)](https://github.com/sponsors/rdaum)

**mooR** is a network-accessible, multi-user, programmable system for building online social
environments, games, and collaborative spaces. Think of it as a virtual world where participants can
not only interact with each other, but also program and modify the environment itself.

mooR is a modern, from-scratch rewrite of [LambdaMOO](https://en.wikipedia.org/wiki/MOO),
maintaining full compatibility with existing LambdaMOO 1.8.x databases while adding significant
enhancements.

📖 **For comprehensive documentation, see our [mooR Book](https://timbran.org/book/html/).**

> **📦 Repository Migration Notice (August 2025):** We've recently moved our primary repository from
> GitHub to [Codeberg](https://codeberg.org/timbran/moor). If you're viewing this on GitHub, please
> consider switching to Codeberg for the latest updates, issue tracking, and contributions.

## What Makes MOOs Special

MOOs offer a unique digital experience through:

- **Collaborative storytelling** where participants build a shared narrative
- **Live programming** - modify the world while you're in it
- **Community-driven development** through persistent interactions and relationships
- **Rich interaction** that engages users through both traditional command-line and modern web
  interfaces
- **Complete customizability** - everything from objects to commands can be programmed

mooR builds on the foundation of MUDs (Multi-User Dungeons) and similar multiplayer online
environments that have fostered creative communities for decades. Like modern sandbox games,
MMORPGs, and social platforms, MOOs provide persistent worlds where players can build, create, and
collaborate. What sets MOOs apart is their emphasis on live programming and community-driven content
creation - imagine if Minecraft's creative mode, Discord's community features, and a code editor all
lived in the same space.

In a world of throwaway apps and walled gardens, mooR is cheerfully dragging the future into the
past - taking the best ideas from decades of online community building and rebuilding them with
modern technology.

## Status

mooR is approaching its 1.0 release and is currently in late alpha, with a focus on documentation,
testing and performance tuning. It successfully runs databases imported from LambdaMOO, with real
world workloads, and lives through our cruel stress and performance testing regimen.

Database formats and APIs may still change before the stable release, and we reserve the right to
keep adding features right up until the last minute.

**Repository**: The primary mooR repository is hosted on
[Codeberg](https://codeberg.org/timbran/moor) with a mirror on GitHub.

## Key Features & Enhancements

**Runtime improvements:**

- Fully multithreaded architecture for modern multicore systems
- Web frontend client
- Directory-based import/export format for version control integration
- Modular architecture for easier extension

**Language enhancements:**

- UTF-8 strings, 64-bit integers, binary values
- Proper boolean values (`true`/`false`)
- Maps: associative containers (`["key" -> "value"]`)
- Lexically scoped variables with `begin`/`end` blocks
- List/range comprehensions (`{x * 2 for x in [1..5]}`)
- Lambda functions: anonymous functions with closures (`{x, y} => x + y`)
- Symbol literals ('mysymbol) like Lisp/Scheme (optional)
- UUID object identifiers (optional)
- Anonymous objects with automatic garbage collection (optional)
- Lightweight immutable objects ("flyweights") (optional)

**Modern infrastructure:**

- Fast, durable, transactional database
- Support for multiple client protocols (web, telnet)
- Easy deployment via Docker

## Quick Start

The easiest way to get started is with Docker Compose:

```bash
docker compose up
```

For faster builds during development, the default configuration uses debug builds. For production
deployment with optimized performance, use:

```bash
BUILD_PROFILE=release docker compose up
```

**Note**: Debug builds compile significantly faster (especially on resource-constrained systems like
Docker Desktop on macOS) while still providing good performance for development and testing.

This starts four services:

- **moor-daemon**: The backend MOO service
- **moor-telnet-host**: Traditional telnet interface on port 8888
- **moor-web-host**: REST API and WebSocket server for web clients
- **moor-frontend**: Web client served via nginx on port 8080

Connect via:

- **Web**: [http://localhost:8080](http://localhost:8080)
- **Telnet**: `telnet localhost 8888`

The server comes pre-loaded with an extraction of LambdaCore, providing a ready-to-explore virtual
world.

For more detailed setup instructions, see the
[Docker Compose Setup](https://timbran.org/book/html/the-system/docker-compose-setup.html) section
in the mooR Book.

### Alternative: Frontend Development Setup

For frontend development and testing, you can run just the daemon and web client without Docker:

```bash
npm run full:dev
```

This starts the moor-daemon and web development server, accessible at
[http://localhost:3000](http://localhost:3000). This setup excludes telnet and provides
hot-reloading for frontend development, but it requires installing some dependencies. See
[the web client's readme](https://codeberg.org/timbran/moor/src/branch/main/web-client#readme) for
details.

## For Developers & Contributors

mooR offers several opportunities for contribution. For detailed contribution guidelines, see
[CONTRIBUTING.md](CONTRIBUTING.md).

**Core Development** (Rust):

- Server architecture improvements
- New builtin functions
- Performance optimizations
- Protocol extensions

**World Building** (MOO language):

- Creating new cores and experiences
- Porting existing MOO content
- Building modern web-enabled interfaces

**Documentation & Testing**:

- Expanding the mooR Book
- Creating tutorials and examples
- Stress testing and bug reports

### Getting Involved

- **Issues**: Check our [Codeberg Issues](https://codeberg.org/timbran/moor/issues) for current
  needs
- **Discussion**: Join our [Discord](https://discord.gg/Ec94y5983z) community
- **Development**: See the [mooR Book](https://timbran.org/book/html/) for architecture details
- **Support**: Consider [sponsoring the project](https://github.com/sponsors/rdaum) to help with
  ongoing development

## Bug Reports

Found a bug or have a feature request? Please file an issue on our
[Codeberg issue tracker](https://codeberg.org/timbran/moor/issues).

When reporting bugs, please include:

- Steps to reproduce the issue
- Expected vs actual behavior
- Your system information (OS, Docker version if applicable)
- Relevant log output or error messages

## Resources

- 📖 **[Complete Documentation](https://timbran.org/book/html/)** - The comprehensive mooR Book
- 🏗️ **[Server Architecture](https://timbran.org/book/html/moor-architecture.html)** - Technical
  overview
- 💻
  **[MOO Programming Language](https://timbran.org/book/html/the-moo-programming-language.html)** -
  Language reference
- 🚀 **[Running a Server](https://timbran.org/book/html/the-system/running-the-server.html)** -
  Deployment guide

## License

mooR is licensed under the GNU General Public License v3.0. See [LICENSE](./LICENSE) for details.

This ensures the software remains open and free, in keeping with the the original LambdaMOO project.
