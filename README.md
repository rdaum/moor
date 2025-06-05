<p align="center"><img src="./doc/porcupine-building.jpg" alt="mooR logo" width="300"/></p>

# mooR

**mooR** is a network-accessible, multi-user, programmable system for building online social environments, games, and collaborative spaces. Think of it as a virtual world where participants can not only interact with each other, but also program and modify the environment itself.

mooR is a modern, from-scratch rewrite of [LambdaMOO](https://en.wikipedia.org/wiki/MOO), maintaining full compatibility with existing LambdaMOO 1.8.x databases while adding significant enhancements.

📖 **For comprehensive documentation, see our [mooR Book](https://rdaum.github.io/moor/).**

## What Makes MOOs Special

MOOs offer a unique digital experience through:

- **Collaborative storytelling** where participants build a shared narrative
- **Live programming** - modify the world while you're in it
- **Community-driven development** through persistent interactions and relationships
- **Rich interaction** that engages users through both traditional command-line and modern web interfaces
- **Complete customizability** - everything from objects to commands can be programmed

mooR builds on the foundation of MUDs (Multi-User Dungeons) and similar multiplayer online environments that have fostered creative communities for decades. Like modern sandbox games, MMORPGs, and social platforms, MOOs provide persistent worlds where players can build, create, and collaborate. What sets MOOs apart is their emphasis on live programming and community-driven content creation - imagine if Minecraft's creative mode, Discord's community features, and a code editor all lived in the same space.

In a world of throwaway apps and walled gardens, mooR is cheerfully dragging the future into the past - taking the best ideas from decades of online community building and rebuilding them with modern technology.

## Status

mooR is approaching its 1.0 release and is currently in late alpha, with a focus on documentation, testing and performance tuning. It successfully runs databases imported from LambdaMOO, with real world workloads, and lives through our cruel stress and performance testing regimen.

Database formats and APIs may still change before the stable release, and we reserve the right to keep adding features right up until the last minute.

## Key Features & Enhancements

**Runtime improvements:**

- Fully multithreaded architecture for modern multicore systems
- Native web front end with rich content presentation
- Directory-based import/export format for version control integration
- Modular architecture for easier extension

**Language enhancements:**

- All classic MOO features plus many extensions from ToastStunt
- Maps: associative containers (`["key" -> "value"]`)
- Lexically scoped variables with `begin`/`end` blocks
- List/range comprehensions (`{x * 2 for x in [1..5]}`)
- UTF-8 strings, 64-bit integers, booleans, symbols.
- Binary values.
- Lightweight immutable objects ("flyweights")

**Modern infrastructure:**

- Fast, durable, transactional database
- Support for multiple client protocols (web, telnet)
- Easy deployment via Docker

## Quick Start

The easiest way to get started is with Docker Compose:

```bash
docker compose up
```

This starts three services:

- **moor-daemon**: The backend MOO service
- **moor-telnet-host**: Traditional telnet interface on port 8888
- **moor-web-host**: Modern web interface on port 8080

Connect via:

- **Web**: [http://localhost:8080](http://localhost:8080) (recommended for new users)
- **Telnet**: `telnet localhost 8888` (classic experience)

The server comes pre-loaded with JaysHouseCore, providing a ready-to-explore virtual world.

For more detailed setup instructions, see the [Docker Compose Setup](https://rdaum.github.io/moor/the-system/docker-compose-setup.html) section in the mooR Book.

## For Developers & Contributors

mooR offers several opportunities for contribution:

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

- **Issues**: Check our [GitHub Issues](https://github.com/rdaum/moor/issues) for current needs
- **Discussion**: Join our [Discord](https://discord.gg/Ec94y5983z) community
- **Development**: See the [mooR Book](https://rdaum.github.io/moor/) for architecture details

## Resources

- 📖 **[Complete Documentation](https://rdaum.github.io/moor/)** - The comprehensive mooR Book
- 🏗️ **[Server Architecture](https://rdaum.github.io/moor/moor-architecture.html)** - Technical overview
- 💻 **[MOO Programming Language](https://rdaum.github.io/moor/the-moo-programming-language.html)** - Language reference
- 🚀 **[Running a Server](https://rdaum.github.io/moor/the-system/running-the-server.html)** - Deployment guide

## License

mooR is licensed under the GNU General Public License v3.0. See [LICENSE](./LICENSE) for details.

This ensures the software remains open and free, in keeping with the the original LambdaMOO project.
