# Starting a MOO from Objdef Source

A running MOO keeps its objects, properties, and programs in a persistent database. But a new MOO
can begin as a directory of ordinary text files that you can read, edit, and keep in version
control. mooR calls these **object definition files**, or **objdef files**.

The important point is that mooR does not read those source files every time it starts. It imports
them once to create the database. After that, the database is the live world.

## Source Files and the Live Database

It helps to think of the source directory as the plans for a house and the database as the house
that was built from them:

- The **objdef source directory** contains human-readable `.moo` files. It is convenient for text
  editors, sharing, and version control.
- The **database** contains the world that the server actually runs. It includes objects, property
  values, compiled verb programs, and changes made by people inside the MOO.

They can describe the same world, but they are not kept synchronized automatically.

## What Happens on the First Start

When you start mooR with `--import` and the requested database does not exist yet, mooR:

1. Creates a new, empty database.
2. Reads the objdef files from the import directory.
3. Creates the objects, properties, and verbs described by those files.
4. Compiles the MOO code in each verb.
5. Saves the result in the new database.
6. Starts the server using that database.

```text
objdef source directory
          |
          | first import and compilation
          v
persistent database
          |
          | start the server
          v
     running MOO
```

For example, this starts a new database using the Cowbell core source included with mooR:

```bash
moor ./moor-data \
    --db world.db \
    --import cores/cowbell/src \
    --import-format objdef \
    --export ./exports
```

Here, `cores/cowbell/src` is the source directory and `./moor-data/world.db` is the database created
from it.

The provided quick-start scripts do the same thing for you. For example,
`./scripts/start-moor-cowbell.sh` selects `cores/cowbell/src` and stores the resulting database
under `run-cowbell/moor-data/`.

## What Happens on Later Starts

When mooR finds that the database already exists, it opens that database and skips the import. The
server can therefore restart without rebuilding the world from source:

```text
persistent database
          |
          | start the server
          v
     running MOO
```

The start command may still contain `--import`. This is normal. The option tells mooR what to use if
it needs to create the database; it does not replace an existing database.

This also means that editing a `.moo` file in the source directory does **not** change an existing
database, even after restarting the server. mooR does not watch the source directory for changes.

## Bringing Source Changes into a MOO

There are several ways to work, depending on what you are trying to do.

### Rebuild a Development World

During core development, you can create a fresh database and import the complete objdef directory
again. This gives you a world containing exactly what the source describes.

Creating a fresh database discards changes that exist only in the old database. Keep the old
database or export it first if those changes matter.

### Change the Live World

MOO is a live programming environment. Programmers can create objects and edit properties and verb
code from inside the running system. Those changes are saved directly in the database and survive
restarts. They do not automatically change the original source files.

### Load or Reload Particular Objects

mooR can explicitly load an objdef into a live database or replace an existing object from an
objdef. This is useful when you want to apply a selected source change without rebuilding the whole
world. See [Loading and Updating Individual Objects](object-loading.md) for the available loading
tools and their conflict-handling options.

## Exporting the Live World

Importing and exporting go in opposite directions:

```text
objdef source
      |
      | import
      v
   database
      |
      | export
      v
objdef checkpoint
```

If an export directory is configured, mooR can write checkpoints of the live database as objdef
files. An export includes changes made inside the MOO, so it can be used for backup, inspection, or
bringing live changes back into a source-controlled directory.

An export does not overwrite the original import directory. Import sources and checkpoint exports
are separate paths unless you deliberately copy or merge files between them.

## How This Differs from LambdaMOO

Classic LambdaMOO normally started from a textdump: one large database dump intended for the server
to read and write. It did not have mooR's directory of individual, human-readable objdef source
files.

mooR can still import a LambdaMOO textdump, but objdef directories make it practical to maintain a
core as ordinary source files. In both cases, the import creates the persistent database used by
later server starts.

## Where to Go Next

- [Object Definition File Format Reference](objdef-file-format.md) describes the contents of `.moo`
  files.
- [Importing and Exporting Objdef Databases](object-packaging.md) covers complete databases.
- [Loading and Updating Individual Objects](object-loading.md) covers selected object definitions.
- [Server Configuration](server-configuration.md#importexport-configuration) lists the relevant
  server options.
- [Emergency Medical Hologram Tool](moor-emh-tool.md) can load or reload objdef files while the
  regular server is stopped.

## Preparing an upstream baseline

Import installs the source you give it. That source may contain local commits and uncommitted edits,
so import alone cannot identify its upstream baseline. Ordinary directory import preserves valid
`objdef_base` metadata when supplied and leaves missing baselines unknown. Invalid baselines fail
the import. Existing databases still skip import entirely.

When importing from a Git checkout, `moor`, `moor-daemon`, and `moorc` accept
`--git-upstream origin/main` to establish the baseline from Git history. The development launcher
uses `origin/main` by default:

```bash
scripts/dev.sh --curl-worker --git-worker
```

The baseline comes from the common ancestor of `HEAD` and `origin/main`. Working files supply the
live code, including local edits. Use `--git-upstream REF` to select another remote-tracking branch.
Use `--baseline-objdef-dir PATH` to supply a baseline without Git. This replaces the Git default.
The Git worker flag controls later fetching; it does not control import baseline preparation.

Add `--clean` to discard the development database and create it again. Without `--clean`, an
existing database is reused: neither source import nor baseline preparation runs. The flag does not
repair an existing database's baselines.

The Cowbell Docker launcher accepts the same baseline options and starts both HTTP and Git workers:

```bash
scripts/start-moor-cowbell.sh --git-upstream origin/main --detach
```

Use `--core PATH` to import a prepared directory. Docker source paths must be inside the checkout,
which Compose mounts at `/db`. The launcher accepts host paths and converts them to container paths.
Use `RUN_DIR=run-cowbell-test` to create a separate installation. Restarting with the same `RUN_DIR`
preserves the database, accepted baselines, and package configuration.

The launcher passes baseline options through `MOOR_GIT_UPSTREAM` or `MOOR_BASELINE_OBJDEF_DIR`. The
npm `moor:dev` commands start both workers and accept these environment variables. The `moor`
executable also accepts these environment variables directly. Set only one; unset both for an
ordinary import. YAML baseline configuration still applies when neither variable is set.

The standard Docker image includes Git for local ancestry preparation. Git worker requests use the
built-in Git library and do not need that executable. Images built with `Dockerfile.arm64-cross`
require prepared objdefs or an explicit baseline directory because they do not include Git.

For direct startup, add `--git-upstream origin/main` alongside
`--import cores/cowbell/src --import-format objdef`. Both server executables also accept
`import_export.git_upstream` in YAML configuration. Use `--baseline-objdef-dir PATH` (or
`import_export.baseline_objdef_dir`) for an explicit base without Git. Select one baseline source;
it requires an objdef import.

`moorc` can also prepare an objdef directory for deployment elsewhere:

```bash
moorc --src-objdef-dir cores/cowbell/src \
    --git-upstream origin/main \
    --out-objdef-dir cowbell-prepared \
    --use-boolean-returns true --use-symbols-in-builtins true \
    --custom-errors true --use-uuobjids true --anonymous-objects true
```

`origin/main` must be a locally available remote-tracking branch. Fetch it beforehand if needed;
`moorc` does not fetch or alter the checkout. It finds the unique common ancestor of that branch and
`HEAD`, reads the ancestor's objdefs, and records their program hashes as baselines. The installed
programs come from the working files, including committed local work and uncommitted edits.

If upstream is behind the checkout, its commit supplies the base. If both branches have advanced,
their common ancestor supplies the base, so subsequent reviews can distinguish local edits from
upstream edits and conflicts. Missing or ambiguous ancestry fails preparation. Object identities
must match at the same object IDs; preparation does not relocate objects or guess ambiguous verb
bindings. Local objects and verbs absent from the base receive no program baseline.

Import the prepared directory when creating the world, for example:

```bash
moor --import cowbell-prepared --import-format objdef
```

This direct import preserves the prepared baselines without another Git preparation step. Select a
fresh data directory when creating a separate world. A prepared import does not repair baselines in
a database that already exists.

Each prepared program baseline includes Git provenance: repository, subtree, upstream branch, base
commit, checkout commit, resolved upstream commit, and whether the source directory had working-tree
changes. This records how the baseline was established; the hashes remain the basis for program
comparison. Prepared exports must retain baselines, so these preparation options cannot be combined
with `--include-baselines=false` when writing objdefs. Ordinary source rebuilds may continue to omit
them.

For a supplied base without Git history, use `--baseline-objdef-dir PATH` instead of
`--git-upstream`. This is an explicit choice of the accepted source, including when the working
files differ. Both sources use the same compiler options. `moorc --db-path PATH` can keep the
prepared database directly instead of exporting objdefs.

Cowbell and Snore declare their package configuration in ordinary objdef properties. Configure
`@changes` to fetch the same repository and subtree selected during preparation. Git preparation
runs before the bulk importer; the importer has no knowledge of Git, the change manager, or its
package schema.

### Source archives without Git history

The container images contain server and utility binaries. They do not contain a Cowbell source
installation or its Git history. The repository source archive contains `cores/cowbell/src`, without
prepared baseline metadata. Keep an unchanged copy of the chosen release source as the accepted
baseline. Apply installation edits to a separate copy.

For example, with both directories inside the checkout:

```bash
scripts/start-moor-cowbell.sh --core cowbell-local \
    --baseline-objdef-dir cowbell-release/cores/cowbell/src --detach
```

To produce a portable installation directory before deployment:

```bash
moorc --src-objdef-dir cowbell-local \
    --baseline-objdef-dir cowbell-release/cores/cowbell/src \
    --out-objdef-dir cowbell-prepared \
    --use-boolean-returns true --use-symbols-in-builtins true \
    --custom-errors true --use-uuobjids true --anonymous-objects true
```

Distribute `cowbell-prepared` with its baseline metadata intact. Import it without another baseline
option. This path needs neither Git history nor the Git executable on the destination. An edited
installation must not supply its own files as the baseline: that would accept local edits as
upstream.

### Startup and fetch failures

Missing or ambiguous ancestry aborts a fresh import. Fetch the required history, or supply an
explicit baseline directory, then restart. Failed import removes the newly created database so the
next attempt can import again. Existing databases skip import and baseline preparation.

If `@changes fetch` reports that no worker is available, start the required worker and retry. The
native launcher needs `--git-worker` for Git and `--curl-worker` for HTTP. The Cowbell Docker
launcher starts both. Worker startup does not require changes to package configuration. Failed
fetches do not install a partial source set; inspect `@changes status` before retrying.
