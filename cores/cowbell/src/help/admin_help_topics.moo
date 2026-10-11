object ADMIN_HELP_TOPICS [
  import_export_id -> "admin_help_topics",
  import_export_hierarchy -> {"help"}
]
  name: "Admin Help Topics"
  parent: HELP_SOURCE
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property topic_administration_changes (owner: ARCH_WIZARD, flags: "rc") = {
    "@changes",
    "Review and apply program updates",
    "Fetch upstream code, compare it with your running MOO, then choose what to apply.\n\nStart with `@changes fetch`. When it finishes, open `@changes diff <review ID>`. In Meadow, select an item to compare local and upstream contents. Save your choices, then apply them.\n\nThe review shows its ID and generation. Use the current generation after saving a choice. ROW is the number beside a verb in command output. Add an OFFSET to diff, details, or source to see another page.\n\n## Review\n\n* `@changes fetch` — Fetch the default package and create a review.\n* `@changes diff <review ID>` — Review changes.\n* `@changes status <review ID>` — Check progress or return to a review.\n* `@changes details <review ID>` — See the source and upgrade checks.\n* `@changes apply <review ID> <generation>` — Apply the saved choices.\n* `@changes discard <review ID> <generation>` — Remove a review.\n\nThe accepted baseline is the upstream version you last accepted. Your local code can differ from it. A three-way comparison shows that baseline, your local code, and the newly fetched code. Fetching historical source does not change any running code.\n\n## Choices from the command line\n\n* `@changes source <review ID> <generation ROW live|incoming|base [OFFSET]>` — Read a program.\n* `@changes base <review ID> <generation ROW>` — Fetch and verify the accepted Git source (wizards only).\n* `@changes resolve <review ID> <generation ROW incoming|local|defer>` — Use upstream, keep local, or skip a program.\n* `@changes resolve <review ID> <generation ROW edited PROGRAM>` — Use an edited program.\n* `@changes refresh <review ID> <generation>` — Compare the fetched source again and clear the saved choices.\n\n## Changes that belong together\n\nAn updated program can need a new helper verb. Put the caller and helper in the same group. The review cannot discover every dependency from program text. You must declare them.\n\nIn Meadow, choose Create a change group. Name the group, select every required change, and save it. Saving a group clears the review's previous choices. It does not change running code.\n\nSelect the group to choose all its changes. You can then keep or edit an existing caller program. A new verb uses the supplied definition and program. Skip the whole group to defer it. Apply rejects an incomplete group or a missing required group.\n\n* `@changes group ID GENERATION NAME ROW ... [requires=NAME,NAME]` — Create or replace a group. ROW is a number from the review. Use a single word for NAME.\n* `@changes select-group ID GENERATION NAME` — Select every change in the group.\n* `@changes skip-group ID GENERATION NAME` — Skip every change in the group.\n\nFor example, put an updated store:put and its new store:validate_item helper in one group. Both changes commit together. If another task creates the helper first, apply stops and asks for a new review. It leaves the caller unchanged.\n\nNew verbs cannot reuse conflicting local or inherited names. Wildcard names can also conflict. Choose a distinct name instead. Existing properties keep their current values. This version can add verbs. It cannot yet add properties or objects.\n\n## Package setup\n\n* `@changes packages` — List packages and active reviews.\n* `@changes fetch <package>` — Fetch another package.\n* `@changes package <name> <objects>` — Register package objects.\n* `@changes upstream <package> <HTTP bundle URL>` — Set an HTTP source.\n* `@changes upstream <package> git <repository FULL-REF-OR-COMMIT [PATH]>` — Set a Git source (wizards only).\n* `@changes adopt <package>` — Record an upstream baseline without changing live programs.",
    {"changes", "upgrades"},
    'administration,
    {"@sudo"}
  };
  property topic_administration_dump_database (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@dump-database", .content = "Usage: `@dump-database`\n\nManually triggers a database dump to disk.\n\nOnly admins can use this command.", .aliases = {"dump", "checkpoint", "save"}, .category = 'administration, .summary = "Trigger database dump", .see_also = {}>;
  property topic_administration_overview (owner: ARCH_WIZARD, flags: "rc") = {
    "administration",
    "Administration commands",
    "Commands for delegated administration and auditing:\n\n`@sudo`, `@sudo-grant`, `@sudo-allow`, `@sudo-revoke`, `@sudo-show`, `@sudo-who`, `@sudo-log`, `@dump-database`\n\nTypical flow: `@sudo-grant` -> `@sudo-allow` -> validate with `@sudo-show` -> audit with `@sudo-log`.\n\nImportant: `@sudo` is an allowlisted command-dispatch facility, not a universal permission elevator. Some privileged operations still require dedicated admin verbs.",
    {"admin", "sudo", "management"},
    'administration,
    {"@sudo", "@sudo-grant", "@sudo-allow", "@sudo-show", "@sudo-log"}
  };
  property topic_administration_sudo (owner: ARCH_WIZARD, flags: "rc") = {
    "@sudo",
    "Run an allowlisted delegated admin command",
    "Usage: `@sudo <command>`\n\nRuns an allowlisted command through delegated admin dispatch.\n\nImportant limitations:\n- This is not universal command elevation.\n- `set_task_perms()` scope does not make arbitrary downstream command execution fully transitive.\n- Some operations (for example direct property/verb mutation commands) may still fail unless exposed as dedicated admin verbs.\n\nAllowlist entries can be plain verb names (for example `@llm-budget`) or object-scoped tokens (`#11::@dig`).\n\nTroubleshooting:\n- `@sudo @rmprop ...` returning permission denied usually means that operation is outside current delegated-dispatch guarantees.\n- Check your grant and allowlist with `@sudo-show <player>`.\n- Check active entries with `@sudo-who` and recent audit events with `@sudo-log`.",
    {"sudo", "@sudo", "sudo-cmd"},
    'administration,
    {"@sudo-grant", "@sudo-allow", "@sudo-show", "@sudo-who", "@sudo-log"}
  };
  property topic_administration_sudo_allow (owner: ARCH_WIZARD, flags: "rc") = {
    "@sudo-allow",
    "Set sudo allowlist",
    "Usage: `@sudo-allow <player> to <verb|obj::verb,...>`\n\nSets the list of command verbs a player may run via `@sudo`.\n\nExamples:\n- `@sudo-allow Ryan to @llm-budget,@dump-database`\n- `@sudo-allow Ryan to #11::@dig`\n- `@sudo-allow Ryan to *`\n\nLeast-privilege guidance:\n- Prefer `obj::verb` entries over plain verb names.\n- Avoid `*` except for tightly controlled temporary situations.\n- Keep lists focused on commands with expected delegated behavior.",
    {"sudo-allow", "allow sudo"},
    'administration,
    {"@sudo-grant", "@sudo-revoke", "@sudo-show", "@sudo-log"}
  };
  property topic_administration_sudo_grant (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@sudo-grant", .content = "Usage: `@sudo-grant <player> as <wizard_player>`\n\nConfigures a player to execute commands as a specified wizard delegate via `@sudo`. If the player has no allowlist yet, a default seed is created.", .aliases = {"sudo-grant", "grant sudo", "sudo-grant"}, .category = 'administration, .summary = "Grant sudo delegation", .see_also = {"@sudo-revoke", "@sudo-allow"}>;
  property topic_administration_sudo_log (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@sudo-log", .content = "Usage: `@sudo-log [N]`\n\nShows the most recent N sudo audit entries (default 20).", .aliases = {"sudo-log", "sudo audit", "sudo-audit"}, .category = 'administration, .summary = "Show sudo audit log", .see_also = {"@sudo-who"}>;
  property topic_administration_sudo_revoke (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@sudo-revoke", .content = "Usage: `@sudo-revoke <player>`\n\nRemoves sudo delegation, allowlist entries, and clears active sudo task markers for that player.", .aliases = {"sudo-revoke", "revoke sudo", "sudo-revoke"}, .category = 'administration, .summary = "Revoke sudo delegation", .see_also = {"@sudo-grant", "@sudo-allow"}>;
  property topic_administration_sudo_show (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@sudo-show", .content = "Usage: `@sudo-show <player>`\n\nShows delegate mapping, allowlist, and active sudo task entries for a player.", .aliases = {"sudo-show", "show sudo", "sudo-show"}, .category = 'administration, .summary = "Show sudo state for a player", .see_also = {"@sudo-who", "@sudo-allow"}>;
  property topic_administration_sudo_who (owner: ARCH_WIZARD, flags: "rc") = <HELP, .name = "@sudo-who", .content = "Usage: `@sudo-who`\n\nLists active sudo tasks and recent sudo audit log entries.", .aliases = {"sudo-who", "sudo active", "sudo-active"}, .category = 'administration, .summary = "Show active sudo and recent audit", .see_also = {"@sudo-show", "@sudo-log"}>;

  override topic_order (owner: ARCH_WIZARD, flags: "rc") = {
    'topic_administration_changes,
    'topic_administration_overview,
    'topic_administration_sudo,
    'topic_administration_sudo_grant,
    'topic_administration_sudo_allow,
    'topic_administration_sudo_revoke,
    'topic_administration_sudo_show,
    'topic_administration_sudo_who,
    'topic_administration_sudo_log,
    'topic_administration_dump_database
  };
endobject
