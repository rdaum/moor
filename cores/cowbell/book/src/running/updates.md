# Keep local work and review updates with `@changes`

Suppose you changed a room's description and improved one of its verbs. A newer Cowbell version now
changes that verb too. You need to see both versions before deciding what belongs in your world.

`@changes` fetches source, compares it with the running world, and lets you review what to install.
Fetching and editing a review do not themselves change the live programs. Applying a reviewed
selection does.

This chapter covers the ordinary update workflow and the meaning of its decisions. Use a restored
test world for your first update, with a [recoverable backup](backups.md).

## Three versions, not two

The comparison uses three sources:

| Version      | Meaning                                                  |
| ------------ | -------------------------------------------------------- |
| **Live**     | The program currently installed in your world.           |
| **Incoming** | The source fetched or uploaded for this review.          |
| **Baseline** | The upstream version previously accepted for comparison. |

For example, your live verb might contain a custom greeting while upstream adds a permission check.
Comparing both with the baseline identifies two separate edits. You can preserve the greeting and
incorporate the check in an edited version.

Existing descriptions and other property values are world data. Ordinary program updates preserve
existing property values and object attributes. New properties and objects have separate checks.
`@changes` does not perform arbitrary data migrations or delete objects omitted from incoming
source.

## Where the baseline comes from

A normal import installs the source you supply. That source can already contain local changes, so
the importer cannot assume that it is an unmodified upstream release.

The [starting walkthrough](starting.md) uses `--git-upstream origin/main`. During the first import,
this finds the common ancestor of the checkout and that local remote-tracking reference. Working
files become live code; the ancestor supplies the baseline.

For source without Git history, the launcher also accepts `--baseline-objdef-dir PATH`. The path
must contain the actual source version you intend to use as the baseline. Select one baseline
method, and keep source directories inside the checkout for the Docker launcher.

These options do not repair an existing database. If its baselines are unknown, stop and establish
which source version belongs to that world. `@changes adopt PACKAGE` prepares a separate history
repair review. Applying adoption records baselines without replacing live programs. Choosing the
wrong source makes future comparisons misleading.

A known baseline whose text is unavailable is a different situation. The baseline identity exists,
but the review needs its source text. `@changes base ROW` can retrieve available Git history. It
does not require treating today's live code as the old baseline.

## Inspect the package and its source

A **package** tells the change manager which objects belong to a review and where their source comes
from. It does not represent a second database. Cowbell supplies a package named `cowbell`.

As a wizard, enter:

```text
help @changes
@changes packages
```

The supplied package reads `cores/cowbell/src` from the mooR Git repository's `refs/heads/main`.
Existing worlds can have different settings. Read the configured source before fetching.

To explicitly select that source:

```text
@changes upstream cowbell git https://github.com/timbran-project/moor.git refs/heads/main cores/cowbell/src
```

A full commit identifier can replace `refs/heads/main` when you want a fixed source version. A
moving branch can produce a different update on a later fetch.

Git fetching requires an actual wizard and an available Git worker. Delegated access to `@changes`
does not grant every underlying operation. The standard Cowbell launcher starts both Git and HTTP
workers. See [configuration](configuration.md) for other installations.

## Fetch and read the comparison

```text
@changes fetch
@changes status
@changes diff
```

Fetching runs in the background. Wait for it to finish before reading the comparison. If the package
already has an active review, open that review instead of creating another one.

The comparison numbers its rows. Use a number from the displayed page to inspect that item:

```text
@changes compare 1
@changes source 1 live
@changes source 1 incoming
@changes source 1 base
```

Here, `1` is an example row number. Select the row you mean. `@changes next` shows another page.
`@changes details` shows source provenance and limitations.

Short commands remember the review and rows shown on your connection. If another session changes the
review, read `@changes diff` again. `@changes status` reports progress but does not accept a new row
list on your behalf.

## Choose what happens to a program

| Command                 | Live program after apply       | Baseline after apply               |
| ----------------------- | ------------------------------ | ---------------------------------- |
| `@changes incoming ROW` | Incoming program.              | Incoming version.                  |
| `@changes keep ROW`     | Your existing live program.    | Incoming version is acknowledged.  |
| `@changes later ROW`    | Your existing live program.    | Previous baseline stays unchanged. |
| `@changes edit ROW`     | Your validated edited program. | Incoming version is acknowledged.  |

**Keep** is a decision about this incoming version: you prefer your local program. **Later** leaves
the question open. Neither decision installs upstream code, but they produce different comparisons
next time.

In the greeting example, inspect the permission change before choosing. To combine it with your
greeting, edit a draft. Do not select incoming merely to clear a conflict marker.

Code already present locally is already running. It is not an incoming addition just because the
same review also contains a new helper elsewhere. Explicitly grouped changes still need a complete
selection, as described next.

## Edit a draft

In Meadow, the review editor lets you edit a proposed program and save the choice. From a terminal:

```text
@changes edit 1
```

The terminal editor appends submitted lines to its saved buffer. Use `.show` to inspect it and
`.clear` before pasting a complete replacement. A line containing only `.` validates and saves the
edited choice. `@abort` leaves the editor while retaining the buffer.

Saving a draft does not install it. A compiler error keeps the draft available for correction. After
reconnecting, `@changes drafts` lists saved buffers. Open the indicated review, then return to the
matching row.

## Keep related changes together

An updated verb can depend on a new helper verb or property. A group records that those changes
belong together. The system cannot infer every dependency from the code.

For example, if displayed rows `2` and `3` are a caller and its helper:

```text
@changes group greeting 2 3
@changes diff
@changes select-group greeting
@changes diff
```

Declaring a group does not select its members. Group selection preserves explicit edited choices and
drafts. Read the resulting comparison before applying it. Groups can also declare prerequisite
groups with `requires=NAME,NAME`.

A new property needs an explicit decision about its initial value. `@changes initialize ROW` permits
that value where the property does not exist. The `descendants` form also permits the displayed
inheritance effect. Read that effect before approving it. Existing values are preserved.

New object creation requires explicit absent object addresses and wizard authority. Preview does not
reserve an address, and creation does not run initialization hooks. Stop for an author-supplied
installation procedure when the review needs object setup or a data migration it cannot express.

## Preview, apply, and read the result

After making your choices:

```text
@changes preview
```

Read the planned effects across the whole review, including pages you have not opened. Resolve any
blockers. If the selection matches your intention:

```text
@changes apply
@changes status
@changes result
```

Apply runs in the background. Its initial response is not the completion result. `result` records
what actually committed. A change to the selection or comparison invalidates an earlier prepared
apply; preview again after reviewing that change.

Exercise the updated commands in the world. A successful apply means the selected changes were
installed. It does not prove that the new behaviour suits your world.

After a partial apply, `@changes diff` lets you continue reviewing deferred work. Alternatively:

```text
@changes finish
```

Finishing keeps committed changes and closes the remaining review. Deferred contents and their old
baselines stay unchanged. A later fetch can offer that work again.

## When a review gets stuck

| Situation                                               | Next step                                                                                             |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Fetch failed because a worker or source is unavailable. | Correct the cause, read `@changes status`, then use `@changes retry`.                                 |
| A live verb changed while you reviewed it.              | Use `@changes refresh`, read the new comparison, and check `@changes recovery`.                       |
| A saved decision no longer matches the evidence.        | Inspect the recovery entry and make a new decision. `recovery-source` can show its saved edited code. |
| You want to abandon pending work.                       | Use `@changes discard`. It leaves earlier committed changes installed.                                |
| You need an earlier result.                             | Use `@changes history` and `@changes result ID`. Retained history has size limits.                    |

There is no general rollback command. Finishing, discarding, or refreshing a review does not restore
the world to its state before an earlier apply. That is why the operating procedure includes a
tested backup and an upgrade rehearsal.

Meadow presents the same workflow through **Check for updates**, **Review planned effects**,
**Confirm apply**, and **Results**. Its **Minimize review** action hides the panel without closing
the review. The commands remain available when you use a terminal.
