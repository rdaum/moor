object BUILDER_HELP_TOPICS [
  import_export_id -> "builder_help_topics",
  import_export_hierarchy -> {"help"}
]
  name: "Builder Help Topics"
  parent: HELP_SOURCE
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property topic_building_dig (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "@dig", .content = "Create a passage to an existing room in the same area.\n\nUsage:\n\n- `@dig [--dry-run] [--allow-parallel] [oneway] DIR[|RETURNDIR] to ROOM`\n\nExamples:\n\n- `@dig north to Library`\n- `@dig n,ne|s,sw to #123`\n- `@dig oneway hatch to Maintenance`\n- `@dig --dry-run north to Lobby`\n\nNotes:\n\n- Without `|RETURNDIR`, opposite directions are inferred when possible.\n- `--allow-parallel` allows multiple passages between the same room pair.", .aliases = {"@tunnel", "dig", "tunnel", "passage"}, .category = 'building, .summary = "Create a passage", .see_also = {"@undig", "@passage", "@set-passage"}>;
  property topic_building_overview (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "building", .content = "You will need builder or programmer privileges to use these commands. If you don't have builder or programmer privileges, ask a wizard to grant builder privileges.\n\nCommands for creating and organizing the world:\n\n`@create`, `@recycle`, `@dig`, `@undig`, `@passage`, `@set-passage`, `@rename`, `@describe`, `@edit-description`, `@parent`, `@children`, `@integrate`, `@move`, `@messages`, `@rules`, `@reactions`, `@show-reaction`, `@parents`, `@set-thumbnail`, `@grant`, `@audit`, `@build`, `#`\n\nUse `help <command>` for details on each command.", .aliases = {"build", "create", "world"}, .category = 'building, .summary = "Builder commands", .see_also = {"programming"}>;
  property topic_building_passage (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "@passage", .content = "Show detailed information for a passage from the current room.\n\nUsage:\n\n- `@passage DIRECTION`\n\nShows:\n\n- label and aliases\n- description and prose style\n- departure/arrival phrases\n- destination and return side info\n- open/locked state", .aliases = {"@passage-info", "@pinfo", "passage info"}, .category = 'building, .summary = "Show passage details", .see_also = {"@set-passage", "@dig", "@undig"}>;
  property topic_building_set_passage (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "@set-passage", .content = "Edit properties of a passage from the current room.\n\nUsage:\n\n- `@set-passage DIRECTION PROPERTY VALUE`\n\nProperties:\n\n- `description`\n- `departure`\n- `arrival`\n- `style` (`sentence` or `fragment`)\n- `aliases` (comma-separated)\n- `is_door` (`true/false`, also `yes/no`, `1/0`, `on/off`)\n\n`is_door` controls whether `open/close/lock/unlock` treat the exit as a door.", .aliases = {"@setp", "set passage", "passage edit", "is_door"}, .category = 'building, .summary = "Edit passage properties", .see_also = {"@passage", "@dig", "@undig"}>;
  property topic_building_undig (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "@undig", .content = "Remove a passage from the current room.\n\nUsage:\n\n- `@undig DIRECTION`\n- `@undig ROOM`\n\nExamples:\n\n- `@undig north`\n- `@undig Library`\n\nThe command checks builder permissions on the current room.", .aliases = {"@remove-exit", "@delete-passage", "undig", "remove exit"}, .category = 'building, .summary = "Remove a passage", .see_also = {"@dig", "@passage"}>;
  property topic_hash_lookup (owner: ARCH_WIZARD, flags: "rc") = <HELP, .content_kind = 'djot_blocks, .name = "#", .content = "Usage: `#<name>[.<property>...] [= <value>] [exit|player|inventory] [for <code>]`\n\nQuickly look up objects and their properties. Supports chained property access and assignment.\n\n*Basic usage:*\n\n- `#lamp` - Find object named 'lamp' in current room\n- `#lamp.description` - Get the description property\n- `#lamp.location.name` - Chained property access\n\n*Assignment (programmers only):*\n\n- `#me.description = \"A tall wizard.\"` - Set a property\n- `#lamp.brightness = 10` - Set numeric value\n- `#box.contents = {}` - Set to empty list\n\n*Scope modifiers:*\n\n- `#sword inventory` - Find in your inventory\n- `#north exit` - Find exit/passage info\n- `#bob player` - Find player by name\n\n*Code evaluation (programmers only):*\n\n- `#lamp for %#.owner` - Evaluate code with `%#` as the result\n- `#bob.location for length(%#.contents)` - Chain + eval", .aliases = {"lookup", "find", "object-lookup", "hash"}, .category = 'building, .summary = "Quick object lookup", .see_also = {}>;

  override topic_order (owner: ARCH_WIZARD, flags: "rc") = {
    'topic_building_overview,
    'topic_hash_lookup,
    'topic_building_dig,
    'topic_building_undig,
    'topic_building_passage,
    'topic_building_set_passage
  };
endobject
