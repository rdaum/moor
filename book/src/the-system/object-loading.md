# Loading and Updating Individual Objects

Use `dump_object()` to export one object. Use `load_object()` to create or merge an object, or
`reload_object()` to replace an existing definition. These functions receive objdef text in memory.
They do not read files, fetch URLs, or watch source directories.

For a complete initial database import, see
[Starting a MOO from objdef Source](bootstrapping-from-source.md). For reviewed program updates, use
the review APIs described below.

## Dump an object

```moo
definition = dump_object(#123);
definition = dump_object(#123, ["constants" -> true]);
source = dump_object(#123, ["include_baselines" -> false]);
```

The result is a list of source lines. The optional `constants` boolean selects symbolic object
names. The dump includes pending changes in the calling task. It does not create a separate database
snapshot. A client can save the returned lines as a `.moo` file.

The `include_baselines` boolean defaults to true. Set it to false for source exports that do not
need the reserved `objdef_base` annotations. Other metadata and the database's stored baselines
remain unchanged. Directory import leaves the baseline unknown when those annotations are absent.

For directory source exports, use `moorc --include-baselines=false --out-objdef-dir DIRECTORY` with
the usual input options. Core rebuilds use this setting. Backup and checkpoint exports retain
baselines so restoration preserves the distinction between accepted programs and local edits.

Dumps preserve explicit local values, clear states, local permissions, and metadata independently.
An inherited property remains inherited even when its current value equals an explicit local value
elsewhere.

### Clear Property Values

The `clear` clause removes a property's local value so it inherits from its parent. It follows any
permissions and metadata, without an equals sign:

```objdef
override description (owner: #3, flags: "rw") [note -> "updated"] clear;
```

The same clause works in a `property` declaration. During a merge, declarations without a value or a
`clear` clause preserve the existing value. A new property without a value starts clear.

`clear` remains a valid constant, property name, and identifier in verb code. An assignment such as
`override description = clear;` still refers to a constant named `clear`.

Exports use the clause for clear values, so merging an export also restores those clear values.
Older importers cannot read this clause. The new importer still accepts existing objdef files.

## Direct import options

```text
obj load_object(str-or-list source [, map options])
obj reload_object(str-or-list source [, map options])
```

Both functions require wizard authority or an explicit builtin grant. Each source must define
exactly one object. Both return the affected object address. Source can be a string or a list of
strings.

| Option       | Meaning                                                                                      |
| ------------ | -------------------------------------------------------------------------------------------- |
| `constants`  | Map of constant substitutions. Defaults to empty.                                            |
| `target`     | Explicit target address. Defaults to the source address. Reload requires an existing target. |
| `allocation` | Load only: `source`, `next`, `anonymous`, or `uuid`. Defaults to `source`.                   |

`target` requires `allocation: source`. Anonymous and UUID allocation require their runtime
features. Unknown options, duplicate normalized keys, invalid values, and unsupported combinations
fail before writing. The old third argument and conflict controls are removed. Use
`preview_objdef_changes()` for review.

```moo
loaded = load_object(definition);
loaded = load_object(definition, ["target" -> #456]);
created = load_object(template, ["allocation" -> "next"]);
created = load_object(template, ["allocation" -> "anonymous"]);
created = load_object(template, ["allocation" -> "uuid"]);
replaced = reload_object(definition, ["target" -> #456, "constants" -> constants]);
```

A target override changes the declaration address only. It does not rewrite object references in
programs, values, owners, or metadata. Source self-references that require relocation are rejected.
Bind those references explicitly to the intended target before importing.

## Merge and replacement

`load_object()` applies supplied declarations. Omitted attributes, properties, verbs, and ordinary
metadata entries remain unchanged. Explicit default values still apply: `readable: false` clears the
flag, and `location: #-1` clears the location. Case-only string changes remain meaningful.

A verb matches an existing declaration only when its alias list and argument specification match
exactly. The merge preserves that definition's UUID and lookup position. New declarations append in
source order. An overlapping alias alone does not identify an existing declaration. Ambiguous or
duplicate matches fail.

`reload_object()` replaces the definition of an existing object. It removes absent local properties,
ordinary metadata, and inherited local overrides. It recreates verbs in source order. Missing
attributes receive creation defaults: empty name, `NOTHING` references, and unset flags. Inherited
property definitions remain available through the selected parent.

During replacement, a property declaration without a value restores a clear local value. During
merge, clearing requires the explicit `clear` clause described above.

The reserved `objdef_base` metadata key is ignored in direct input. Merge preserves existing
tracking on surviving definitions. Replacement invalidates tracking for removed or recreated
definitions. Direct import never acknowledges incoming source as an accepted baseline.

## Constants and failures

```moo
constants = parse_objdef_constants(constants_source);
loaded = load_object(definition, ["constants" -> constants]);
```

Constants and programs use the runtime's configured language features. Memory source labels do not
authorize filesystem includes. Source is limited to 16 MiB.

Parsing and option errors raise `E_INVARG` or `E_TYPE` before mutation. If an import fails after a
write begins, the entire task transaction rolls back. A MOO `try` block cannot retain a partial
import. Transaction conflicts retry through the normal runtime mechanism.

The [Meadow object browser](../web-client/authoring-tools.md#exporting-and-reloading-objdef-files)
uses replacement for uploaded definitions. The [moor-emh tool](moor-emh-tool.md#object-importexport)
can import local files while the regular server is stopped.

## Inspecting local and incoming contents

`preview_objdef_changes()` also accepts `operation: "inspect"` with selected `fields` from `object`,
`attribute`, `property`, and `program`. This compares current database contents with the supplied
source inside the requested object scope. It includes objects and verbs present on only one side,
property definitions and overrides, and object attributes. Source definitions are interpreted as a
complete import: omitted attributes take their creation defaults, and properties without values are
clear. This is a content comparison, not the partial-merge semantics of `load_object()`. Matched
verb programs remain part of the program review described below.

Property and attribute rows use the same classifications as verb programs: `local`, `upstream`,
`conflict`, `converged`, or `unbased`. Import preparation records ancestor hashes for these fields
in object metadata, alongside the existing per-verb baselines. Unsupported values whose contents
cannot be fingerprinted remain unbased. No object graph or additional database format is stored.

Items present on only one side are `local_only` or `incoming_only`; absence alone does not establish
whether something was added or deleted. Properties distinguish stored values, clear values,
permissions, and metadata. Meadow includes all item types in the same change filters and review
list. These comparisons do not advance field baselines when program updates are applied.

Rows have `read_only: true`, explicit `live_present` and `incoming_present` flags, and no choices.
`details` requests text for selected opaque row IDs. Large or unsupported values carry an
`inspection_error` instead of preventing inspection of other fields. A report revision lets clients
reject pagination across changing local contents. Inspection produces no apply evidence and
`apply_objdef_changes()` rejects inspection requests.

Cowbell and Snore expose this through `$change_manager:inspection()`. Meadow combines those rows
with program reviews in its initial **All differences** list. Local-only members have a read-only
source pane; properties and attributes present on both sides have a comparison view. These reads
observe current local state against the saved incoming source and do not change pending choices.

## Read-only program review

`preview_objdef_changes(sources, request [, choices])` compares existing programs and proposed verb
and property additions without changing content or baseline metadata. It requires wizard authority
or an explicit builtin grant, plus permission to read the selected definitions.

Each source unit has a diagnostic `label` and `text`. Text can be a string or a list of lines.
Labels grant no filesystem access. Source input is limited to 4,096 units and 16 MiB in total.
Reports contain at most 8,192 change rows.

```moo
sources = {["label" -> "login.moo", "text" -> incoming_lines]};
request = ["schema" -> 1, "operation" -> "update",
           "objects" -> {$login}, "fields" -> {"program"}];
report = preview_objdef_changes(sources, request);
```

`objects` explicitly selects installation-local object addresses. Optional `constants` supplies
constant substitutions. Source export identities must match live identities when supplied. Ambiguous
verb declarations remain unsupported. Unknown keys and duplicate normalized keys fail.

Each row contains an opaque `id`, target `object`, verb `names`, source label, and field name. It
also contains `base`, `live`, and `incoming` fingerprints, classification, eligibility, allowed
choices, default action, and blockers. An empty base list means that the baseline is unknown.
Classifications are `unchanged`, `upstream`, `local`, `converged`, `conflict`, and `unbased`.
Unbased updates require adoption. An `adopt` request inspects source-derived baseline enrollment.
Neither operation writes during preview.

Optional `details` selects row IDs whose live and incoming program text the report includes. Detail
selection does not change review evidence. Clients must compare returned evidence with the saved
review before showing newly read text under an old approval.

Optional `base_sources` uses the same source-unit format as incoming objdefs. It requires selected
program `details`. Each selected historical member must match the installed object binding, verb
names, argument specification, and accepted program fingerprint. Supplied object identities must
also match. A mismatch rejects the request. Verified details include `base_text` and
`base_text_available`. Historical input does not change apply evidence or install source.

Optional `provenance` is a map limited to 8192 bytes. It provides retrieval hints, which are part of
the guarded request. Adoption and accepted update choices retain these hints in baseline `source`
metadata. The native operation sets `label` and `digest` from the actual incoming source. Keeping
local code or installing an edited result still accepts incoming provenance. Deferral preserves the
previous baseline. Detail rows expose existing hints as `base_source`. These hints never replace
fingerprint verification.

The report's `evidence` binds the source, scope, compilation options, target identities, authority,
live programs, and baselines. Evidence grants no permission. Existing property values and definition
fields remain unchanged. Diagnostics identify unsupported declarations and fields.

Choices are a map from row ID to a map containing `choice`. Supported values are `incoming`,
`local`, `edited`, and `defer`. Adoption permits only `incoming` and `defer`. An edited choice also
contains `program`, a string or list of lines. Preview compiles the draft and returns `validation`
rows with a fingerprint and validation token, or compiler diagnostics with result-pane coordinates.
Drafts remain separate from incoming source. Changing draft text invalidates its validation token.

Program baselines use the metadata key `objdef_base`, with `schema` and `program` entries and an
optional `source` provenance map. The schema is `objdef-v1:program:sha256`. Its fingerprint uses
decompiled program structure and separate typed literals, encoded with CBOR and hashed with SHA-256.
Layout is ignored. String case, literal types, execution order, finite floating-point bits, and
installed object references remain significant. Symbols use Unicode case-folded names. Map entries
sort by encoded key; flyweight slots sort by folded name. Captured lambda values and non-finite
floats are unsupported.

These rules describe content equality, not behavioral equivalence. A hash cannot recover old source.
Unknown baseline schemas block updates until explicit re-adoption. Automatic eligibility requires
administrator-owned objects and verbs whose content and metadata are not publicly writable. An
explicit `trusted_owners` list can additionally name non-wizard owners trusted by the administrator.
This policy is part of the guarded request and does not bypass object or verb access permissions.
Cowbell and Snore declare their system `Hacker` owner in their package data. Publicly writable
targets remain ineligible even when their owner is trusted.

## Apply a reviewed decision

```moo
receipt = apply_objdef_changes(sources, request, report["evidence"], choices);
```

Apply reparses the saved input and compares current state with the original evidence. A stale review
fails before writing. A transaction retry must use that same evidence. Evidence grants no authority.

Missing choices use each row's default. Conflicts require an explicit choice. Unknown rows and
unsupported choices fail. For `edited`, include the exact draft and its successful preview token:

```moo
choices[row_id] = ["choice" -> "edited", "program" -> draft,
                   "validation" -> validation["validation"]];
```

Adoption writes source-derived baselines without changing programs. An incoming choice installs the
incoming program. Local preserves the live program; edited installs the validated draft. All three
record incoming as the accepted baseline. Defer writes nothing.

Updates to existing programs preserve definition UUIDs, aliases, owners, flags, lookup order, and
ordinary metadata. The receipt contains decisions and hashes, without historical source. It remains
uncommitted until the calling task commits. If a write fails, the whole task rolls back. Core
applications must save their completion record in that same transaction and abort if that
bookkeeping fails.

### Add verbs with explicit change groups

Cowbell can add a verb to an existing managed object. The new verb uses the supplied owner, flags,
argument specification, program, and ordinary metadata. Its owner must meet the review's trust
policy. A local or inherited alias collision blocks creation. Uncertain wildcard overlaps also block
creation. Preview allocates no verb UUID. Apply allocates it only within the transaction.

A new helper often needs an update to the caller that uses it. A change group declares that these
changes must be installed together. The service cannot discover every dependency from program text.
The administrator must declare those dependencies.

For example, an update to `store:put` calls a new `store:validate_item` helper. Put both changes in
a group named `store-update`. Selecting only the caller would leave broken code, so apply rejects
that selection. Selecting the group installs the helper and updates the caller in one transaction.

In Meadow:

1. Open the review and select **Create group**.
2. Enter a name and select all required changes.
3. If another group must apply too, select it under **Other groups required by this group**.
4. Save the group. This clears previous choices in the review without changing running code.
5. Select the group, review each program, then apply.

For an existing program, **Keep local** or an edited result remains available after selecting the
group. A new verb uses the supplied definition and program. **Skip GROUP** defers the whole group.
Apply rejects a partly selected group, a missing member, or an unselected required group.

The terminal equivalents are:

```text
@changes group ID GENERATION NAME ROW ... [requires=NAME,NAME]
@changes select-group ID GENERATION NAME
@changes skip-group ID GENERATION NAME
```

Use row numbers from the review and a single word for the command-line group name. Changing a group
clears saved choices, so use the new generation for subsequent commands.

If another task creates a conflicting verb after preview, apply rejects the old review. It leaves
the caller and other changes untouched. Refresh the review to see the current definitions.
Successful apply records the new member and its accepted program baseline with the completion
receipt.

Existing property values remain unchanged. Group membership does not authorize unsupported
operations.

The native request's optional `policy` has schema 1 and a `groups` list. Each group has `name`,
`members`, and optional `requires` entries. `members` contains opaque membership keys from report
rows. These keys identify object, verb names, and argument specification across initial creation and
later reviews. `requires` contains other group names. Duplicate membership, unknown prerequisites,
and dependency cycles are rejected. Limits are 32 groups, 8,192 members, and 1 MiB of policy data.

Cowbell stores this policy in package configuration. Uploaded source cannot change it. For reviews
with proposed additions, ungrouped program updates remain blocked. Preview returns `groups` and
`group_validation` alongside individual choices. Apply rechecks group selection before any write.

### Initialize new properties

A new property can hold a schema version, an empty collection, or another initial value. That value
applies only when the property is absent. It never replaces existing application data.

For example, `store:validate_item` can need a new `store.item_schema` property. Include the
property, helper, and caller in one group. Apply creates the property and helper before the
transaction commits. Other tasks see the complete change after commit.

In Meadow:

1. Select the proposed property in the review.
2. Read its initial value, owner, flags, and metadata.
3. If the object has descendants, decide whether they can inherit this new property.
4. Select **Permit initialization and clear choices**.
5. Add the property and all dependent programs to a change group.
6. Select the group, then apply it.

The terminal command is `@changes initialize ID GENERATION ROW`. To permit inheritance by existing
descendants, add `descendants` after ROW. Use `@changes source ID GENERATION ROW incoming` to read
the initial value and permissions first.

Initialization permission becomes a package rule for this member. Saving it clears previous choices
and advances the review generation. It does not write the value. Apply still requires a complete
group, current authority, and unchanged review evidence.

A local, inherited, or descendant property with the same name blocks creation. Apply rechecks
absence and inheritance. If another task creates a conflicting property, refresh the review before
making another choice. A failed apply leaves the entire selected group unchanged.

After installation, group selection keeps the property's current value, permissions, metadata, and
accepted baseline. The comparison can still show local data changes or a different upstream initial
value. Neither change authorizes replacement. Property overrides, clear operations, and data
migrations remain outside guarded apply.

Property flags have these meanings:

- `r` permits reading by other users.
- `w` permits writing by other users.
- `c` gives the owner of each descendant ownership of its inherited property.
- `o` permits concurrent writes to the value. The last committed write wins.

Native requests include `property` alongside `program` in `fields`. The policy's `initialization`
list contains `member`, `mode: "absent"`, and a boolean `descendants` for each permitted property.
New-property rows permit `incoming` or `defer`. Installed-property rows permit `local` or `defer`.
For installed properties, `local` preserves the accepted baseline as well as the live state. The
service methods are `set_group(ID, GENERATION, NAME, MEMBERS, REQUIRES)` and
`resolve_group(ID, GENERATION, NAME, CHOICE)`. An empty member list removes a group. Selection
choices are `incoming` and `defer`.

## Review packages with `@changes`

Cowbell and Snore expose the same schema-1 service at `$change_manager`. `help @changes` lists the
terminal commands. Snore requires a wizard. Cowbell also accepts current administrator delegation
with an `@changes` allowlist entry. Each API call and background job checks that authority again.
Source, drafts, and saved reviews are private administrator data.

In Meadow, **Changes** appears in the navigation bar when the core exposes an accessible, compatible
service. Select a package to configure its upstream, fetch source, or upload objdef files. **New
package** accepts explicit managed objects, object constant bindings, and trusted owners. Git
configuration accepts a full ref or commit and a repository subdirectory.

Select **Adopt supplied source as the baseline** when supplying original upstream source. Adoption
creates a review; applying it records baselines without replacing live programs. The workspace also
opens saved reviews, discards pending reviews after confirmation, and lists retained completed
reviews. Configuration saves use the generation observed when the form opened. A concurrent
configuration change rejects the save and preserves the form for inspection.

The review opens on **Changes**. **Upstream source** identifies the repository, revision, or URL.
**Left unchanged** groups unsupported fields by reason. **Groups** appears only when groups exist.
**Results** lists the choices from a completed apply. These views leave the comparison area
available for code. Edited drafts show compiler diagnostics in the Monaco editor. Reopening an
invalid saved draft restores its diagnostics. Changing the draft clears markers for the previous
text.

Meadow protects unsaved drafts during navigation and browser reload. After reconnect, it reads the
current review state. If another client changed the review, Meadow retains unsaved text and requires
an explicit reload before further choices. Copy that text before discarding it. Saved drafts remain
in the service across disconnects and server restarts.

Each core declares its default package and object bindings in its own source. Prepare upstream
baselines with `moorc` before a fresh import, as described in
[Preparing an upstream baseline](bootstrapping-from-source.md#preparing-an-upstream-baseline).
Cowbell defaults to the mooR GitHub repository, `refs/heads/main`, and the `cores/cowbell/src`
subtree. With `moor-git-worker` running, a wizard can use `@changes fetch` without configuring an
upstream. Existing databases retain their saved package settings.

Cowbell accepts this command to select a Git upstream:

```text
@changes upstream git https://github.com/timbran-project/moor.git refs/heads/main cores/cowbell/src
```

Supply a package name before `git` to configure another package. The revision must be a full ref or
a `sha1:` commit ID. The optional path selects a subtree. Git staging requires an actual wizard;
Cowbell administrator delegation does not grant Git fetch access.

Git staging collects `.moo` files recursively and retains their relative paths. It uses the
package's installed constants and skips `constants.moo`. Symlinks, submodules, and invalid UTF-8
source cause staging to fail. The saved review and receipt record the repository, requested
revision, resolved commit, tree, subtree, and source digest.

Both cores also accept HTTP bundle URLs. Configure one and stage an update:

```text
@changes upstream https://example.org/releases/core.moo
@changes stage
@changes status 1
@changes diff 1
@changes apply 1 1
```

Use the review ID and generation returned by your own commands. `stage`, `adopt`, and `upstream`
default to the installed core's package. Supply a package name to select an additional application.
Use `@changes package NAME #OBJECT ...` to configure a separate application. Packages cannot share
managed objects. This prevents two upstreams from changing the same program baselines.

For existing content without a known baseline, `@changes adopt NAME` records supplied program
fingerprints without changing live programs. An HTTP upstream must return one self-contained UTF-8
objdef text bundle. The curl worker must be available. Directory URLs, archives, and filesystem
includes are not supported. Effective URL, ETag, and the source digest are saved as provenance.

`diff` lists stable row IDs, classifications, eligibility, choices, and blockers. Inspect a selected
program with `@changes source ID GENERATION ROW live` or `incoming`. Add an offset to page through
its decompiled lines. Both panes use decompiled coordinates. Structured details also contain exact
incoming body text and its file position.

Cowbell can retrieve historical objdefs from the accepted Git commit or accept an upload. Meadow
offers **Fetch accepted source** when Git provenance and wizard authority are available. Use
**Upload accepted objdefs** for other sources. Retrieval uses the saved commit and repository
subtree, even if the configured branch now points elsewhere. The native operation verifies the
selected member against its baseline before the service retains the source.

The terminal commands are `@changes base ID GENERATION ROW` for Git retrieval and
`@changes source ID GENERATION ROW base` to read verified historical text.

Meadow then offers a three-way comparison of the accepted baseline, local code, and upstream code.
The edited result remains separate. Save unsaved edits before loading historical source. If source
is unavailable, the two-way comparison remains usable.

The Cowbell service method `base_source(ID, GENERATION, ROW, SOURCES)` accepts uploaded source
units. Omit `SOURCES` to retrieve the pinned Git source. The response contains verified details and
the next generation. Saved choices remain intact. Each member can use a different baseline revision.
Source remains in the pending review across restarts. Refresh, apply, and discard remove the
historical cache. Completion receipts do not contain historical source.

Resolve a row with `@changes resolve ID GENERATION ROW incoming`, `local`, or `defer`. For a short
edited program, use `@changes resolve ID GENERATION ROW edited PROGRAM`. The service validates the
draft without changing live code. Apply uses the new generation returned after each decision. Edited
and local resolutions acknowledge the incoming fingerprint as the baseline.

Multiline uploads and drafts use the same programmatic service:

```moo
sources = {["label" -> "example.moo", "text" -> source_lines]};
staged = $change_manager:stage("example", sources);
page = $change_manager:review(staged["review_id"], staged["generation"]);
validated = $change_manager:resolve(staged["review_id"], staged["generation"], row_id,
                                   "edited", program_lines);
```

`capabilities()` publishes limits and supported operations. `packages()` returns bindings and
upstream settings. `configure()` and `upstream()` require the expected package generation when
replacing settings. `review()` accepts a generation-bound cursor and optional classification filter.
It returns counts and up to 50 rows within 256 KiB. `diagnostics()` pages through reported issues.
`details()` checks original evidence before returning selected text and a saved draft, within 512
KiB. Compile errors identify the source or row, pane, and line/column range. Service errors carry a
schema-1 map in the raised error value.

Cowbell also exposes `history(package)`. It returns summaries from the retained receipts for that
package. A wizard can list all receipts; a delegated administrator can list only their own.
`status(review_id)` returns the corresponding receipt. Clients check the advertised operations
before using this extension.

In Cowbell, the upstream argument also accepts a Git source map:

```moo
source = ["transport" -> "git", "repository" -> "https://github.com/timbran-project/moor.git",
          "revision" -> ["ref" -> "refs/heads/main"], "path" -> "cores/cowbell/src"];
package = $change_manager:packages()["packages"]["cowbell"];
$change_manager:upstream("cowbell", source, package["generation"]);
```

Only one review can be active per package. The service permits eight pending reviews, 4 MiB of
source per review, and 32 MiB of pending storage. Choice writes, refresh, discard, and apply require
the displayed review generation. Refresh creates new evidence and clears old approvals. It uses the
saved source without fetching again. To review newer upstream source, discard and stage again.

Fetch and apply return promptly and run as background tasks. Poll `status()` for the committed
result. Apply writes programs, baselines, and its receipt in one transaction. Repeated apply calls
return the existing job or receipt. A partial result retains source for deferred rows. Completion
and discard remove working source and drafts. The last 64 completed receipts retain counts and up to
50 decisions, without program bodies.

After a restart or interrupted task, query status. An `interrupted` review requires an explicit
refresh or discard. Rejection leaves installed content unchanged. A changed live program makes the
original evidence stale; refresh before making new decisions.

### Bind and create objects

A new object needs an explicit numeric ID before preview. Its objdef constant maps to that ID in
package configuration. Source cannot select another address or move an existing object.

In Meadow:

1. Select the package.
2. If a review is active, finish or discard it first.
3. Select **Object bindings and source scope**.
4. Enter one binding per line, such as `HELPER = #9600`.
5. Choose whether the source lists every member of each managed object.
6. Save the configuration, then fetch or upload source.
7. Read the new object's full definition, including ownership, flags, properties, and programs.
8. Put the new object and its dependent programs in a change group.
9. Select the group, then apply it.

A new package can start with no existing managed objects. Add its proposed bindings before fetching
source. Only wizards can propose or create objects.

The terminal equivalent for the bindings is:

```text
@changes bindings PACKAGE GENERATION [HELPER=#9600 OTHER=#9601] [complete]
```

This command replaces the package's pending object bindings. Installed bindings remain in place.
Omit bindings to remove all pending proposals. Omit `complete` to use partial source. Use the
generation shown by `@changes packages`. Ordinary objdef constants bind references inside programs
and property values to these IDs. Uploaded source must not redeclare configured constant names. Git
fetching omits `constants.moo` and uses package bindings.

The new-object row includes the complete initial definition. Selecting that row permits these
contents only at an absent address. Properties require explicit initial values. Inherited overrides
apply only to the new object. Apply does not run `initialize`, movement hooks, or other MOO verbs.
Programs can call initialization code later through normal, explicit administration.

Preview creates nothing and reserves no IDs. Apply checks the addresses again. It allocates all
selected objects before connecting their parents and locations. Then it installs their contents. A
new child can therefore inherit a property from a new parent in the same transaction. Required new
parents, owners, and locations must be selected too. A missing selection rejects apply.

If an ID is occupied:

1. Discard the review.
2. Choose another explicit ID in **Object bindings and source scope**.
3. Fetch or upload the source again.
4. Update the change group to include the newly bound object.
5. Review the new comparison before applying.

The occupied object stays unchanged. Apply never relocates an object or guesses its identity from
its name. A concurrent creation or late error rolls back all selected changes, including program
updates, object contents, membership, baselines, and the receipt.

After creation, the package retains a creation receipt for that address. Later reviews compare the
installed object but cannot replace its structure. **Keep local** preserves that structure and its
creation receipt. Separate program rows control program updates. Existing property data remains
unchanged. A missing or different creation receipt blocks the object row.

### Complete and partial source

Partial source makes no statement about omitted members. This is the default. A file that contains
one changed method does not request deletion of the other methods.

Complete source declares that it lists every member of the managed objects. An omitted installed
member then appears as an unsupported removal. Apply still preserves that member. Deletion,
renaming, structural updates to existing objects, and data migrations need separate support.

The scope declaration is a package rule. Fetching source cannot change it. Native policy can name
complete objects individually. Meadow's checkbox declares all managed objects complete or leaves all
of them partial. Changes to bindings or scope require a fresh review.

Native requests include `object` in `fields` for creation. Policy `creations` entries contain an
explicit `object` and `mode: "absent"`. After apply, the core records the native creation hash as
`installed` in that entry. Policy `complete` lists objects with complete source. All policy
addresses must belong to the request's managed scope. Source outside that scope remains unapplied.

The object baseline records `creation` and `creation_schema` alongside field baselines. The schema
is `objdef-v1:creation-text:sha256`. The hash covers normalized objdef text with reserved baselines
removed. It identifies the accepted initial definition. It does not authorize replacing live data.
