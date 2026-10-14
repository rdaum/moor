# Back up a world and restore it

A backup needs to preserve the world you have now: people's objects, code, accounts, and the
supporting state needed to reopen it. Keeping a fresh copy of Cowbell's source does not preserve
those changes.

This procedure covers the default local Compose installation from
[the starting walkthrough](starting.md). It uses the default embedded databases and stops the world
before copying files. A PostgreSQL installation or storage outside the runtime directory needs a
backend-specific backup procedure.

## Object exports and installation backups

An **object export** writes the world as objdef source. It is useful for inspecting objects, sharing
source, and reconstructing object state. It is not a copy of the complete installation.

An **installation backup** also needs runtime storage, keys, and configuration. Event history and
persisted task state live outside the main object database. The standard launcher puts those data
under `run-cowbell/`.

Preserve the whole directory, including `config/` and `local-share/`. Keys are part of the
installation's identity. Replacing them can invalidate signed credentials or other saved authority.
The files `moor-local.yaml` and `compose.local.yaml` are outside that directory and need copying
too.

## Make a stopped-world backup

This example assumes `RUN_DIR=run-cowbell` and the two configuration files from the starting lesson.
If you chose other paths, substitute them consistently.

1. Tell visitors that the world will stop for a backup. Finish any update already being applied.
2. Stop the services from the repository root:

   ```sh
   docker compose stop -t 60
   docker inspect --format '{{.State.Status}} {{.State.ExitCode}}' moor
   ```

   Continue when the result is `exited 0`. If shutdown timed out or failed, inspect the logs first.

3. Create a new backup directory and archive:

   ```sh
   backup_dir="$HOME/cowbell-backups/$(date +%Y%m%d-%H%M%S)"
   mkdir -p "$backup_dir"
   chmod 700 "$backup_dir"
   tar -czf "$backup_dir/world.tar.gz" \
     run-cowbell moor-local.yaml compose.local.yaml
   git rev-parse HEAD > "$backup_dir/source-revision.txt"
   docker inspect --format '{{.Image}}' moor > "$backup_dir/server-image.txt"
   tar -tzf "$backup_dir/world.tar.gz" > "$backup_dir/contents.txt"
   ```

   The revision and image identifier record what ran this world. Retain the corresponding source and
   image as well. An identifier alone cannot recreate an unavailable image or uncommitted source.

4. Start the existing installation again:

   ```sh
   docker compose start
   ```

Store another copy of the archive away from this computer. The archive contains accounts, keys, and
private world data. Additional files you introduce, such as certificates or environment files, also
need to be included in your backup procedure.

Do not copy live database directories with this recipe. Stopping all writers makes the files a
consistent recovery point without requiring filesystem snapshot support.

## Restore into a separate directory

Use the same server version for your first restore check. Test an upgrade separately after you know
that the backup opens correctly.

The supplied Compose installation uses fixed ports and container names. Stop the original world
before starting this copy. Keep its runtime directory intact.

Before restoring a world with external integrations, plan how to isolate its network access and
disable those integrations in the copy. Saved schedules and resumed work can perform real actions. A
copied database is still a working world. The commands here assume the local example world.

From the repository root, with `backup_dir` still pointing to your archive:

```sh
docker compose stop -t 60
mkdir restore-check
tar -xzf "$backup_dir/world.tar.gz" -C restore-check
export RUN_DIR=restore-check/run-cowbell
export MOOR_CONFIG_FILE=restore-check/moor-local.yaml
export COMPOSE_FILE=docker-compose.yml:restore-check/compose.local.yaml
./scripts/start-moor-cowbell.sh --detach
```

Use an unused directory for `restore-check`. This command reopens the copied database. Its import
option does not overwrite that database.

Reconnect and check more than the welcome banner:

- Log in with the password that was current at backup time.
- Find an object created after the original import, such as the restart marker.
- Check its description and a local program you changed.
- Inspect `@changes packages` for your saved package configuration.
- Inspect `@schedules` for expected recurring work.
- If you use Meadow history, unlock it and read entries from before the backup.

History recovery still requires the player's history encryption credentials. A backup does not
replace a forgotten encryption password.

To return to the original installation, stop the copy and restore the original environment values:

```sh
docker compose stop -t 60
export RUN_DIR=run-cowbell
export MOOR_CONFIG_FILE=moor-local.yaml
export COMPOSE_FILE=docker-compose.yml:compose.local.yaml
./scripts/start-moor-cowbell.sh --detach
```

Keep the restored directory until you have checked the result. Work performed in the restored copy
does not automatically appear in the original world.

## Request an object checkpoint

Inside the MOO, an administrator with the required authority can request an export:

```text
@dump-database
```

The command waits for the checkpoint to finish before reporting success. If the export fails, it
reports failure instead. Read the server log for details before trying again.

Exports appear under the configured export path, `run-cowbell/export/` in this installation. Keep
their baseline metadata when using them for recovery. A source-only export stripped of baselines
cannot preserve the same update history.

The stopped-directory restore procedure has been checked with an isolated Cowbell server, including
login, an authored object, a stored property, and a signed token. The full Compose and
Meadow-history restore exercise still needs an end-to-end test.
