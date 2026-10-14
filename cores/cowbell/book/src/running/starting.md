# Create a world and start it

This walkthrough starts a Cowbell world on your own computer. You will connect, create an object,
stop the server, and find the object again after a restart.

The commands assume Linux, a shell, Docker with Compose, and Node.js/npm. Use mooR's `main` source:
Cowbell needs features beyond the stable 1.0 server. The first launch builds the server and Meadow,
so it takes longer than a restart.

## Source, database, and running world

The files in `cores/cowbell/src/` describe the starting world. On its first start, the server
imports those files into a database. From then on, the database holds your world, including objects
people create and code they edit.

Restarting opens that database again. It does not reimport the source files. Editing the source
checkout therefore does not update an existing world. Later, you can use [`@changes`](updates.md) to
review and apply object updates.

## Prepare a local installation

If you do not have a mooR checkout, obtain one:

```sh
git clone https://github.com/timbran-project/moor.git
cd moor
```

Otherwise, open a shell in your existing repository root. The commands in this part of the book run
from that directory.

The supplied Compose configuration publishes ports on all interfaces. For this local walkthrough,
create `compose.local.yaml` with these contents:

```yaml
services:
  moor:
    ports: !override
      - "127.0.0.1:8888:8888"
      - "127.0.0.1:8081:8081"
  moor-frontend:
    ports: !override
      - "127.0.0.1:8080:80"
```

`!override` replaces the original port list instead of adding another list. It requires Compose
2.24.4 or later. See Docker's
[merge rules](https://docs.docker.com/reference/compose-file/merge/#replace-value).

Make a copy of the server configuration for your installation:

```sh
cp moor-dev.yaml moor-local.yaml
export RUN_DIR=run-cowbell
export MOOR_CONFIG_FILE=moor-local.yaml
export COMPOSE_FILE=docker-compose.yml:compose.local.yaml
```

Keep these three environment variables in each shell used to manage this installation. `RUN_DIR`
selects its saved data. If `run-cowbell/` already contains a world, this procedure reopens it. For a
new experiment, choose an unused relative directory inside the checkout.

Start the world:

```sh
./scripts/start-moor-cowbell.sh --git-upstream origin/main --detach
docker compose ps
```

The baseline option records which upstream code your initial source came from. That helps `@changes`
distinguish later local edits from upstream updates. It uses local Git history and runs only during
the initial import. An existing database is unchanged by that option.

If your source has no Git history, see
[baseline preparation](updates.md#where-the-baseline-comes-from) before choosing an import method.

The two services are `moor`, which runs the world, and `moor-frontend`, which serves Meadow. Read
their logs if either service exits:

```sh
docker compose logs --tail=100 moor moor-frontend
```

## Connect and make something

Open [Meadow at localhost:8080](http://localhost:8080). A fresh Cowbell world supplies the account
`archwizard`, with password `test`. A terminal client can connect to `localhost:8888` instead:

```text
connect archwizard test
```

This is a wizard account with authority over the whole world. Change its password before opening the
installation to other people. In the world, replace `YOUR-NEW-PASSWORD` with your chosen password:

```text
@password test YOUR-NEW-PASSWORD
```

The reply is `New password set.` The command accepts the old and new passwords as arguments. Treat
any command history containing them as sensitive. Cowbell also supplies a `Test_Player` account;
review the initial accounts before public use.

Create a reminder that this is your world:

```text
@create $thing named "restart marker",marker
@describe marker as This object belongs to my test world.
look marker
```

The marker starts in your inventory. Remember its name and description for the restart check. You
can also type `examples` in the First Room to visit the travel examples.

## Stop and restart the same world

In the shell, stop both services and allow time for shutdown:

```sh
docker compose stop -t 60
docker inspect --format '{{.State.Status}} {{.State.ExitCode}}' moor
```

A clean stop reports `exited 0`. Investigate another exit status in the logs before using this stop
as the basis of a backup.

Start the existing containers again:

```sh
docker compose start
```

Reconnect with your new password and enter `look marker`. The object and its description should
still be there. This restart uses the existing image and database, without a rebuild.

If you removed the containers, run the launcher again with the same environment variables. The
database remains in `run-cowbell/`. Do not remove that directory to solve a startup problem.

## Know what you have created

| Location                   | Purpose                                                                |
| -------------------------- | ---------------------------------------------------------------------- |
| `cores/cowbell/src/`       | Initial object definitions, used when creating the database.           |
| `run-cowbell/moor-data/`   | World database and runtime storage, including tasks and event history. |
| `run-cowbell/config/`      | Keys and configuration files generated by the running components.      |
| `run-cowbell/local-share/` | Additional persistent component data.                                  |
| `run-cowbell/export/`      | Object checkpoints written by the server.                              |
| `moor-local.yaml`          | Your server configuration, outside the runtime directory.              |
| `compose.local.yaml`       | The local port configuration used in this lesson.                      |

The default Compose command names the world database `development.db`. Its name does not make it
temporary. Treat the whole runtime directory as saved world data.

Next, read [connections and workers](configuration.md) and make your first
[backup and restore check](backups.md).
