object PROG_HELP_TOPICS [
  import_export_id -> "prog_help_topics",
  import_export_hierarchy -> {"help"}
]
  name: "Programmer Help Topics"
  parent: HELP_SOURCE
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override content_kind = 'djot_blocks;

  property topic_programming_chmod (owner: ARCH_WIZARD, flags: "rc") = {
    "@chmod",
    "@chmod - Show/change permissions",
    "@chmod TARGET [PERMS]\n\nPERMS accepts absolute (`rw`, `rxd`) and symbolic add/remove (`+w`, `-d`) forms.\nTargets: object, object.property, object:verb.",
    {"chmod"},
    "programming",
    {}
  };
  property topic_programming_grep (owner: ARCH_WIZARD, flags: "rc") = {
    "@grep",
    "@grep - Search verb code",
    "@grep [options] PATTERN [OBJECT]\n\nOptions: `-r/--regex`, `-s/--case-sensitive`, `-i`, `--owner OBJECT`, `--limit N`.",
    {"grep"},
    "programming",
    {}
  };
  property topic_programming_move (owner: ARCH_WIZARD, flags: "rc") = {
    "@cpverb",
    "@cpverb/@mvverb - Copy or move verbs",
    "`@cpverb SRC:VERB to DEST[:NEW]` copies a verb.\n`@mvverb SRC:VERB to DEST[:NEW] --confirm` moves a verb.\nUse `--dry-run` with @mvverb to preview.",
    {"@mvverb", "cpverb", "mvverb"},
    "programming",
    {}
  };
  property topic_programming_overview (owner: ARCH_WIZARD, flags: "rc") = {
    "programming",
    "Programmer commands",
    "You need the programmer bit to use these commands. If you don't have one, ask a wizard to grant you the programmer bit.\n\nCommands for examining and modifying code:\n\n`@list`, `@program`, `@verb`, `@rmverb`, `@cpverb`, `@mvverb`, `@which`, `@where-defined`, `@verbs`, `@properties`, `@property`, `@rmproperty`, `@clear-property`, `@args`, `@show`, `@chmod`, `@grep`, `@doc`, `@edit`, `@browse`, `@codepaste`, `eval`\n\nTasks and schedules: `@ps`, `@schedules`, `@schedule`, `@stop-schedule`, `@kill`.\n\nUse `help <command>` for details on each command.",
    {"prog", "code", "verbs"},
    "programming",
    {"building"}
  };
  property topic_programming_program (owner: ARCH_WIZARD, flags: "rc") = {
    "@program",
    "@program - Program a verb",
    "@program <object>:<verb> -- Program a verb via input.\n\n*Forms:*\n\n- `@program OBJ:VERB`                   Interactive mode\n- `@program OBJ:VERB DOBJ PREP IOBJ`    Match by argspec\n- `@program# OBJ:NUMBER`                Program verb by index\n- `@program OBJ:VERB = CODE_LINE`       Inline one-line mode\n\nInteractive special lines: `.` finish, `@abort` cancel, `.<text>` literal leading dot.",
    {"@program#", "program"},
    "programming",
    {}
  };
  property topic_programming_show (owner: ARCH_WIZARD, flags: "rc") = {
    "@show",
    "@show - Display object info",
    "@show <object>[selectors] -- Display object information.\n\n*Selectors:*\n\n- `.`   Local properties only\n- `..`  All properties (including inherited)\n- `:`   Local verbs only\n- `::`  All verbs (including inherited)\n\n*Examples:*\n\n- `@show #1`       Show summary with counts and hints\n- `@show #1.`      Show local properties\n- `@show #1..`     Show all properties (+ inherited)\n- `@show #1:`      Show local verbs\n- `@show #1::`     Show all verbs (+ inherited)\n- `@show #1.name`  Show specific property\n- `@show #1:tell`  Show specific verb\n- `@show #1.:`     Show local props + local verbs\n- `@show #1..:`    Show all props + local verbs\n- `@show #1.::`    Show local props + all verbs\n- `@show #1..::`   Show all props + all verbs\n\nAlias: `@display`",
    {"@display", "show", "display"},
    "programming",
    {}
  };
  property topic_programming_tasks (owner: ARCH_WIZARD, flags: "rc") = {
    "@ps",
    "@ps - Tasks and schedules",
    "`@ps` (alias `@tasks`) lists active tasks, queued tasks, and live schedules.\n`@schedules` lists schedules only. Programmers see their own schedules; wizards see all owners.\n\n`@schedule ID` shows callback arguments, timing, running task, and fault diagnostics.\n`@stop-schedule ID` (alias `@kill-schedule`) stops future firings. It does not kill a firing already running. Use `@kill TASK-ID` for that task. Schedule IDs and task IDs are separate.\n\nOnly a schedule's owner or a wizard may inspect or stop it. A stopped or expired ID is harmless to stop again.",
    {
      "@tasks",
      "@schedules",
      "@schedule",
      "@stop-schedule",
      "@kill-schedule",
      "@kill",
      "@kill-task",
      "tasks",
      "schedules"
    },
    "programming",
    {}
  };
  property topic_programming_which (owner: ARCH_WIZARD, flags: "rc") = {
    "@which",
    "@which - Show verb definition source",
    "`@which OBJ:VERB` (alias `@where-defined`) shows definer, owner, flags, argspec, and code line count.",
    {"@where-defined", "which", "where-defined"},
    "programming",
    {}
  };

  override topic_order (owner: ARCH_WIZARD, flags: "rc") = {
    'topic_programming_overview,
    'topic_programming_show,
    'topic_programming_program,
    'topic_programming_grep,
    'topic_programming_chmod,
    'topic_programming_move,
    'topic_programming_which,
    'topic_programming_tasks
  };
endobject
