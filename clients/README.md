# Client Applications

This directory contains scripts for fetching client applications that are developed
in separate repositories but are needed for local development builds.

## Why Not Git Submodules?

We use fetch scripts instead of git submodules for several reasons:

1. **Independent development**: Submodules pin to specific commits, requiring updates
   in the parent repo whenever the submodule changes. Fetch scripts always pull the
   latest code, which is appropriate for development workflows.

2. **Simpler contributor experience**: Submodules add complexity to git operations
   (`git clone --recursive`, `git submodule update --init`, etc.). Fetch scripts
   are explicit and self-documenting.

3. **Flexible overrides**: Developers actively working on a client can easily point
   to their own checkout via environment variables, without modifying any tracked files.

4. **No coupling of release cycles**: The mooR backend and client applications are
   released independently. Submodules would create artificial coupling between their
   version histories.

## Meadow Web Client

[Meadow](https://codeberg.org/timbran/meadow) is the official web client for mooR.
It is maintained in a separate repository to allow independent release cycles and
issue tracking.

### Fetching Meadow

To clone or update the Meadow web client:

```bash
./clients/fetch-meadow.sh
```

This will clone the repository to `clients/meadow/` (or update it if already present).

### Using a Custom Meadow Location

If you have Meadow checked out elsewhere (e.g., for active development), you can
override the path using the `MEADOW_PATH` environment variable:

```bash
export MEADOW_PATH=/path/to/your/meadow
docker compose up --build
```

Or inline:

```bash
MEADOW_PATH=../my-meadow-fork docker compose up --build
```

### Production Deployments

For production, the deployment configurations in `deploy/` use pre-built Docker
images from `codeberg.org/timbran/meadow:latest` rather than building from source.
