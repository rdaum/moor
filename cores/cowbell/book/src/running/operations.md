# Monitor, diagnose, and recover

A report such as “the command does nothing” can describe several different failures. The browser can
lose its connection, the MOO can reject a permission, or a background worker can fail after the
command was accepted.

Start with the result the person expected, their account, the command, and the time. Preserve the
actual error text. Then check the component responsible for that part of the action.

## Check the installation from the shell

For the Compose installation, use the environment from [the starting lesson](starting.md):

```sh
docker compose ps
docker compose logs --tail=100 moor
docker compose logs --tail=100 moor-frontend
```

The `moor` log includes the world runtime and its bundled hosts and workers. The frontend log helps
with browser delivery and proxy failures. A working frontend page does not prove the world behind it
is accepting connections.

If ports or paths look wrong, inspect `docker compose config`. Compare its resolved `RUN_DIR` and
configuration path with the installation you intended to start.

## Separate connection failures from world failures

| Symptom                             | Check next                                                                                        |
| ----------------------------------- | ------------------------------------------------------------------------------------------------- |
| Neither client can connect.         | Backend process, listening ports, and server startup log.                                         |
| Telnet works, but Meadow does not.  | Frontend, web host, proxy configuration, and browser connection error.                            |
| Login works, but one command fails. | Full MOO error, account privileges, target object, and the verb named in the traceback.           |
| The browser shows old output.       | Connection state and whether the narrative is scrolled into history. Compare with a fresh `look`. |
| A building command is denied.       | Builder/programmer role, object ownership, and room or area grants.                               |
| An update fetch fails later.        | `@changes status`, its source settings, and the required worker.                                  |

Try the same intended action as the affected user. A successful wizard test does not explain why an
ordinary user's permission check failed.

## Tasks and recurring schedules

A **task** is a running or suspended execution of MOO code. A **schedule** arranges future
executions. One schedule can produce many tasks over time.

Programmers can inspect their own work; wizards can inspect other owners' work. The useful commands
include:

```text
@ps
@schedules
@schedule SCHEDULE-ID
```

Replace `SCHEDULE-ID` with an identifier from the schedule list. Read its owner, callback, and
reported status before changing it.

To stop future recurrence:

```text
@stop-schedule SCHEDULE-ID
```

Stopping a schedule does not kill a task already running from one of its firings. If that task also
needs to stop, identify its task ID with `@ps`, then use `@kill-task TASK-ID`.

Task IDs and schedule IDs are different. Do not substitute one for the other. Restarting the server
is not a reliable way to cancel persistent scheduled work.

## Recover a review

`@changes` keeps progress and review results. A fetch can fail without changing live code. An apply
can commit some work and leave other work for review.

Read `@changes status` and `@changes result` before retrying or discarding anything. The
[update chapter](updates.md#when-a-review-gets-stuck) explains refresh, saved drafts, deferred work,
and recovery.

## Collect enough evidence for a repair

For a reproducible problem, record:

- The server version and the relevant core update or apply result.
- The exact command and error, including its traceback.
- Whether the account is a visitor, builder, programmer, or wizard.
- Whether it happens through Meadow, telnet, or both.
- The object involved and any local customization relevant to the failure.
- What changed immediately before the problem began.

Remove passwords, credentials, and private conversation from shared reports. Retain the original
details privately if they are needed for diagnosis.

A restart can restore availability, but it does not explain the failure. Check the affected
interaction afterward and keep the evidence needed for a permanent fix.
