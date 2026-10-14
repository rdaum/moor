# From a starting core to a maintained world

Running a world means keeping people's creations and conversations available over time. You need a
way to start it, control access, protect its saved data, and maintain it as the software changes.

A **wizard** is a privileged account inside the world. A **world operator** also manages the server,
storage, configuration, and recovery. One person might do both jobs, but wizard commands cover only
part of running a world.

This part follows the world's life from its first import to ongoing maintenance. The detailed
chapters are still to be written. The sections below describe their scope and point to existing
instructions where available.

## Create a world

Cowbell's source defines the starting objects. Importing that source creates a database which then
holds the running world's objects and programs. As people build, their work becomes part of that
database. The source directory and the live database serve different purposes.

The first chapter needs a complete route from an empty directory to a working world. It must show
where the database lives, how to connect for the first time, and how to stop and restart the same
world. It must also explain initial account setup and when an import occurs.

For the current local launcher, see
[Try Cowbell](https://github.com/timbran-project/moor/blob/main/cores/cowbell/README.md#try-cowbell).
That is a local starting point. The
[mooR operator documentation](https://timbran.org/book/2.0-dev/html/the-system/running-the-server.html)
covers the server. This book needs to connect those instructions to Cowbell's initial configuration.

## Configure access and optional components

A world needs addresses through which people can connect. Meadow, a web host, and a telnet host have
different roles in that connection. Workers provide optional external operations, such as Git access
or model requests.

The configuration chapter must distinguish a local experiment from a world open to other people. It
needs a component diagram, configuration examples, and checks for each connection. It must also
explain which workers a chosen Cowbell feature needs and how to recognize a missing worker.

## Look after accounts and the community

The account chapter needs to cover account creation, profiles, builder and programmer privileges,
wizard access, and delegated administrative commands. It also needs everyday procedures for helping
someone who cannot connect or use a command.

Community policy belongs here too: what the world records, who can inspect it, and which information
optional model tools send elsewhere. The supplied help includes world-specific policy text; it needs
review when setting up a new community.

## Back up and restore

A source export and a database backup preserve different things. The backup chapter must say what
each contains and identify any additional keys, configuration, or history storage needed for
recovery.

Its example must restore into a separate instance and verify the result. A backup procedure is
incomplete until the operator knows how to use the backup. The book does not yet provide that
verified procedure.

## Keep your work while Cowbell changes

People can change a running world while Cowbell's source also develops. Updating a server executable
and updating objects in that world are separate operations.

Cowbell's `@changes` tools support reviewing and applying object changes. They belong in the
maintenance workflow alongside source configuration, local work, and backups. The chapter needs a
worked example that starts with a local edit, obtains an upstream change, reviews their differences,
and resolves the result.

It must explain groups, conflicts, diagnostics, application results, and recovery using the current
commands. It must distinguish changes already applied locally from incoming work. Until that guide
exists, start with `help @changes` in the world. Core programmers can inspect the
[change manager](https://github.com/timbran-project/moor/blob/main/cores/cowbell/src/change_manager.moo)
and its tests.

## Upgrade and keep the world healthy

The upgrade chapter must join these procedures: check compatibility, take a recoverable backup,
update the server, review core changes, restart when needed, and verify the world. It must explain
recovery without promising that an object update can always be undone.

The operations chapter needs practical checks for logs, running tasks, recurring schedules, worker
failures, and stalled model requests. It must show how to identify the affected component before
changing settings or restarting services.

These chapters are the largest documentation gap. They need tested procedures on disposable worlds,
in addition to examples of commands inside the MOO.
