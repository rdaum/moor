# From a starting core to a maintained world

Running a world means keeping people's creations and conversations available over time. You need a
way to start it, control access, preserve its saved data, and maintain it as the software changes.

A **wizard** is a privileged account inside the world. A **world operator** also manages the server,
storage, configuration, and recovery. One person can do both jobs, but wizard commands cover only
part of running a world.

## Start with a small installation

[Create a world and start it](starting.md) walks through a local Compose installation. You connect,
make an object, stop the server, and find the object again after restarting. That establishes the
distinction between the starting source and the database holding your own world.

[Connections, clients, and workers](configuration.md) explains the components and their
configuration. [Accounts and authority](accounts.md) introduces player registration, building and
programming privileges, and the settings to review before inviting visitors.

## Learn recovery before making updates

[Back up and restore](backups.md) makes a stopped-world archive and opens it in a separate
directory. It explains why an object export is only one part of preserving an installation.

[`@changes`](updates.md) then introduces source comparisons, local customizations, review choices,
and apply results. [Upgrading](upgrades.md) brings the procedures together in a rehearsal before
changing the working world.

For everyday diagnosis, [monitoring and recovery](operations.md) covers logs, connections, tasks,
schedules, and useful evidence for a bug report.

## Scope of these procedures

The shell walkthroughs use the supplied single-process Compose arrangement and its default embedded
storage. They assume Linux and a local test installation. Public deployment, PostgreSQL backups, and
split processes require additional configuration. Use the
[mooR server manual](https://timbran.org/book/2.0-dev/html/the-system/running-the-server.html) for
server configuration beyond this walkthrough.

The startup and review commands are grounded in the launcher and core tests. A separate server check
verifies a stopped-directory restore. The full Compose deployment, Meadow-history recovery, and
version-to-version upgrade rehearsal are not yet automated book tests.

These limits matter when adapting the examples to an existing community. Keep the installation's
actual paths, storage backend, credentials, and version requirements alongside its operating notes.
