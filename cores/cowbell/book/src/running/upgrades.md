# Upgrade the server and the core

Your installation has several versions: the server executable, Cowbell's installed objects, Meadow,
and any separately deployed hosts or workers. They do not all change when you run one command.

Rebuilding a server image changes the executable. Reopening an existing database preserves its
objects. Applying an `@changes` review changes the selected objects in that database.

## Rehearse on a restored copy

Before changing the working installation:

1. Read the [server changelog](https://github.com/timbran-project/moor/blob/main/ChangeLog.md) and
   [Cowbell changelog](https://github.com/timbran-project/moor/blob/main/cores/cowbell/ChangeLog.md)
   for the intended versions.
2. Record the current source revision, image, configuration, and any local source modifications.
3. Make a [backup](backups.md) and restore it into a separate directory.
4. Verify that the restored world works with the existing version before attempting the upgrade.

If the copy already fails before the upgrade, resolve that failure first. Otherwise, you cannot tell
whether a later failure comes from the update or the recovery procedure.

## Choose the order from the compatibility requirements

Incoming Cowbell code can need builtins or syntax unavailable in an older server. A newer server can
also require a database conversion. Read the release instructions for those requirements before
choosing an order.

For a compatible server update followed by a core update, the rehearsal is:

1. Stop the restored instance cleanly.
2. Build or install the chosen server version and any required matching client or worker versions.
3. Start it with the restored database and the saved configuration.
4. Check login and a few existing interactions before changing core objects.
5. Configure a fixed incoming source version for the rehearsal.
6. [Review and apply the core update](updates.md), preserving the intended local work.
7. Read the apply results and exercise the affected behaviour.

The launcher builds from the current checkout. `docker compose start` uses existing containers and
does not rebuild them. Neither command automatically synchronizes an existing world's objects with
the checked-out Cowbell source.

## Check the world that matters to your community

Use both an administrator and an ordinary account. Useful checks include:

- Connecting through each supported client.
- Visiting a locally built room and using its exits.
- Using an object with a local verb or customized message.
- Confirming a saved property value that upstream also defines.
- Checking scheduled work and worker-backed features you use.
- Reading Meadow history, if enabled.
- Inspecting the completed `@changes` result and any deferred work.

Keep the intended source version fixed between the rehearsal and the real update. Fetching a moving
branch later can produce a different change set.

## Apply the rehearsed procedure

Arrange a maintenance period and make a fresh backup of the working installation. Repeat the
procedure that succeeded on the copy. If people changed the live world in the meantime, review the
new comparison rather than reusing old row numbers or decisions blindly.

Once the update completes, repeat the relevant checks and retain the backup and old executable.
Record deferred work so it does not disappear merely because the main update succeeded.

## Recovery has a boundary

`@changes discard` abandons pending review work. It does not undo changes already committed.
Likewise, replacing the server executable does not undo a database conversion or an object update.

A recovery to the pre-upgrade state uses the saved database, required configuration and keys, and a
compatible old executable. It also loses changes made after that backup. Account for that loss
before reopening a recovered world to visitors.

Do not assume an older executable can open storage last written by a newer version. Establish that
compatibility from the release's instructions or restore the old backup with the old version.
