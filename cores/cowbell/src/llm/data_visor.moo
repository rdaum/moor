object DATA_VISOR [
  import_export_id -> "data_visor",
  import_export_hierarchy -> {"llm"}
]
  name: "Data Visor"
  parent: LLM_WEARABLE
  owner: HACKER
  fertile: true
  readable: true

  property current_investigation_task (owner: HACKER, flags: "rc") = #-1;
  property moo_language_examples (owner: ARCH_WIZARD, flags: "r") = {
    "MOO is a Wirth-style language with 1-based indexing (lists/strings start at 1, not 0).",
    "",
    "**CRITICAL - Syntax backwards from Python:**",
    "- MOO lists: { \"a\", \"b\", \"c\" }       // curly braces!",
    "- MOO maps:  [ \"key\" -> \"value\" ]   // square brackets with arrows!",
    "This is OPPOSITE of Python where [] is lists and {} is dicts.",
    "",
    "## Basic Syntax",
    "",
    "// Variables and assignment",
    "x = 5;",
    "let y = 10;        // lexically scoped",
    "const z = 15;      // lexically scoped constant",
    "global w = 20;     // explicitly global scope",
    "",
    "// Strings use double quotes",
    "name = \"Alice\";",
    "greeting = \"Hello, \" + name;",
    "",
    "## Control Flow (Wirth-style with end keywords)",
    "",
    "// If/elseif/else/endif",
    "if (x > 0)",
    "  return \"positive\";",
    "elseif (x < 0)",
    "  return \"negative\";",
    "else",
    "  return \"zero\";",
    "endif",
    "",
    "// While loops",
    "while (count < 10)",
    "  count = count + 1;",
    "endwhile",
    "",
    "// For-in loops (iterate over lists)",
    "for item in ({\"a\", \"b\", \"c\"})",
    "  player:inform_current($event:mk_info(player, item));",
    "endfor",
    "",
    "// For-range loops (1-indexed!)",
    "for i in [1..10]",
    "  total = total + i;",
    "endfor",
    "",
    "// For-in with index",
    "for i, item in ({\"apple\", \"banana\", \"cherry\"})",
    "  player:inform_current($event:mk_info(player, tostr(i) + \": \" + item));",
    "endfor",
    "",
    "## Error Handling",
    "",
    "// Try/except/endtry",
    "try",
    "  result = risky_operation();",
    "except e (E_INVARG, E_RANGE)",
    "  player:inform_current($event:mk_info(player, \"Operation failed: \" + tostr(e)));",
    "  return E_NONE;",
    "endtry",
    "",
    "// Try/finally/endtry",
    "try",
    "  lock_resource();",
    "  do_work();",
    "finally",
    "  unlock_resource();",
    "endtry",
    "",
    "// Inline try with backticks",
    "value = `risky_expr ! E_PROPNF => default_value';",
    "valid_obj = `obj.parent ! ANY => #-1';",
    "",
    "## Data Structures (1-indexed!)",
    "",
    "// Lists use CURLY BRACES { } (NOT square brackets like Python!)",
    "items = {\"first\", \"second\", \"third\"};",
    "first = items[1];           // \"first\" (not 0!)",
    "last = items[$];            // \"third\" ($ = end)",
    "slice = items[2..$];        // {\"second\", \"third\"}",
    "",
    "// Maps use SQUARE BRACKETS [ ] with arrows -> (NOT curly braces like Python!)",
    "config = [\"host\" -> \"example.com\", \"port\" -> 8080];",
    "host = config[\"host\"];",
    "config[\"timeout\"] = 30;",
    "",
    "// Flyweights (lightweight immutable mini-objects) have delegates, \"slots\" and \"contents\"",
    "point = <$point, .x = 10, .y = 20>; // $point is the delegate, x and y are slots and can be accessed like properties in an object are ",
    "password_obj = <$password, {\"hash...\"}>; // \"hash\" is the contents",
    "",
    "Consult your builtins list to understand operations around flyweights (flyslots, flycontents, toflyweight, etc.)",
    "",
    "## Object Operations",
    "",
    "objects are single inheritance, prototype inheritance. \"class\" in moo is a convention where an object gets treated as one. they're also called \"generics\" and \"prototypes\"",
    "",
    "parent() gets an object's parents, children() returns the list of children for an object.  ancestors() and descendants() also exist",
    "",
    "// Property access",
    "player_name = player.name;",
    "player.description = \"A friendly person\";",
    "",
    "// Verb calls",
    "result = obj:method(arg1, arg2);",
    "obj:initialize();",
    "",
    "// Builtin functions",
    "len = length(items);",
    "valid_flag = valid(obj);",
    "type_name = typeof(value);  // Returns INT, STR, OBJ, LIST, MAP, etc.",
    "",
    "## Functions and Lambdas",
    "",
    "// Lambda expressions",
    "double = {x} => x * 2;",
    "result = double(5);  // 10",
    "",
    "// Named functions",
    "fn calculate_area(width, height)",
    "  return width * height;",
    "endfn",
    "",
    "// List comprehensions",
    "squares = {x * x for x in [1..10]};",
    "evens = {x for x in [1..20] if x % 2 == 0};",
    "",
    "## Scatter Assignment (destructuring)",
    "",
    "// Basic scatter",
    "{a, b, c} = {1, 2, 3};",
    "",
    "// With rest operator",
    "{first, @rest} = {1, 2, 3, 4};  // first=1, rest={2,3,4}",
    "",
    "// With optional/default values",
    "{x, ?y = 10} = {5};  // x=5, y=10 (default)",
    "",
    "## Operators",
    "",
    "// Arithmetic: + - * / % ^(power)",
    "// Comparison: == != < > <= >=",
    "// Logical: && || !",
    "// Bitwise: &. |. ^. ~ << >> >>>",
    "// Ternary: condition ? true_val | false_val",
    "// Range test: x in [1..10]",
    "",
    "// IMPORTANT: 'in' operator returns POSITION (1-indexed), not boolean!",
    "pos = \"y\" in \"xyz\";        // Returns 2 (1-indexed position)",
    "pos = 3 in {1, 2, 3};       // Returns 3 (position of value 3)",
    "pos = \"z\" in \"abc\";        // Returns 0 (not found, since 1-indexed)",
    "// There is NO index() or indexOf() builtin - use 'in' operator!",
    "",
    "## Special Values",
    "",
    "// Objects",
    "#0          // System object",
    "#123        // Object by number",
    "$login      // System reference",
    "",
    "// Error types",
    "E_NONE E_TYPE E_DIV E_PERM E_PROPNF E_VERBNF E_VARNF E_INVIND",
    "E_RECMOVE E_MAXREC E_RANGE E_ARGS E_NACC E_INVARG E_QUOTA E_FLOAT",
    "",
    "// Custom errors with messages",
    "raise(E_INVARG(\"Expected positive number\"));",
    "",
    "// Booleans",
    "true false",
    "",
    "// Symbols (Lisp-style keywords)",
    "'success 'failure 'pending",
    "",
    "## Common Patterns",
    "",
    "// Safe property access",
    "value = `obj.prop ! E_PROPNF => default_value';",
    "",
    "// Object validation",
    "if (valid(obj) && obj.wizard)",
    "  // do wizard stuff",
    "endif",
    "",
    "// Permission check at verb start",
    "caller == this || caller.wizard || raise(E_PERM);",
    "set_task_perms(this.owner);",
    "",
    "// Early returns for validation",
    "!valid(target) && return E_INVARG;",
    "typeof(arg) != LIST && raise(E_TYPE);",
    "",
    "// Finding position in string/list (NO index() builtin!)",
    "pos = \"needle\" in haystack_string;  // Returns 1-based position or 0 if not found",
    "pos = item in list;                 // Returns 1-based position or 0 if not found",
    "if (\"@\" in email_addr)",
    "  // Found - can use position directly",
    "endif",
    "",
    "## Type Constants",
    "",
    "INT NUM FLOAT STR OBJ LIST MAP ERR BOOL FLYWEIGHT BINARY LAMBDA SYM",
    "",
    "// **CRITICAL: Type constants CANNOT be used as variable names!**",
    "Assigning to them (OBJ = 5;) is a COMPILE ERROR.",
    "Using them as variables is confusing since they resolve to numeric values.",
    "BAD:  INT = 42;           // Compile error",
    "BAD:  for OBJ in (list)   // Confusing - OBJ resolves to number",
    "GOOD: int_value = 42;     // Use descriptive variable names instead",
    ""
  };
  property prompt_code_style (owner: HACKER, flags: "rc") = "## Code Style\n\n### Philosophy\n- Readable, minimal but not terse\n- Don't repeat yourself\n- Respect the language and its idioms\n\n### Early Returns Over Nested Conditionals\n\n```moo\n// Good - early returns\n!valid(target) && return E_INVARG;\n!caller_perms().wizard && return E_PERM;\n// ... main logic ...\n\n// Bad - deep nesting\nif (valid(target))\n  if (caller_perms().wizard)\n    // ... main logic ...\n  endif\nendif\n```\n\n### Guard Conditions\n\nUse short-circuit evaluation instead of if blocks:\n```moo\nuser:is_bad_user() && raise(E_BADUSER);\n!user:is_valid() || raise(E_PERM);\n```\n\n### Comments and Documentation\n\nMOO uses string literals as comments (not `//` style). They MUST end with semicolons:\n```moo\n\"This is a comment\";\n\"Always start verbs with a brief docstring\";\n```\n\n### Verbs\n\n**Method verbs** (called programmatically):\n- Argspec: `this none this`\n- Flags: `rxd`\n\n**Command verbs** (player-typed commands):\n- Meaningful argspec (e.g., `this with any`)\n- Usually do NOT have `x` flag\n\n**Naming:**\n- Private methods: prefix with `_` (e.g., `_internal_helper()`)\n- Unit tests: prefix with `test_`\n\n### Errors and Exceptions\n\n- The `d` flag causes errors to propagate as exceptions - only legacy code has this off\n- In method verbs: use and propagate exceptions\n- In command verbs: catch and turn into meaningful output\n- **Never silently eat exceptions**\n\nTwo exception syntaxes:\n```moo\n// try/except block\ntry\n  risky_operation();\nexcept e (ANY)\n  handle_error(e);\nendtry\n\n// Inline catch\n`risky_operation() ! ANY => fallback_value`;\n```";
  property prompt_event_system (owner: HACKER, flags: "rc") = "## Event System\n\n### Sending Events\n\n**Never call `notify()` directly.** Always use the event system.\n**Never pass raw strings to `:tell`.** It takes event objects, not strings.\n\n**`:tell(event)`** - Persistent world events\n- Sends to **all** player connections\n- **Persists** in event log\n- Use for: room events, chat, actions by others\n\n**`:inform_current(event)`** - Ephemeral responses\n- Current connection only\n- Does **not** persist\n- Use for: command responses, errors, help text\n\n```moo\n// World event - persists, all connections\nplayer:tell($event:mk_info(player, @content):with_this(this):with_dobj(dobj));\n\n// Command response - ephemeral, current connection only\nplayer:inform_current($event:mk_info(player, \"You see nothing special.\"));\n```\n\n### Event Creation\n\nEvents are created with `$event:mk_<action>(actor, content...)`:\n- `$event:mk_info(player, message)` - informational\n- `$event:mk_error(player, message)` - error\n- `$event:mk_look(player, content)` - look results\n- `$event:mk_say(player, message)` - speech\n- `$event:mk_emote(player, message)` - actions\n\n### Rich Content ($format)\n\n| Formatter | Purpose | Example |\n|-----------|---------|---------|\n| `$format.block` | Multi-line paragraphs | `$format.block:mk(line1, line2, ...)` |\n| `$format.list` | Ordered/unordered lists | `$format.list:mk({\"a\", \"b\"}, true)` |\n| `$format.table` | Tables with headers | `$format.table:mk(headers, rows)` |\n| `$format.title` | Section headings | `$format.title:mk(\"Header\", 2)` |\n| `$format.code` | Code blocks | `$format.code:mk(code, \"moo\")` |\n\n### Event Substitutions ($sub_utils)\n\nTemplates adapt to viewer perspective:\n```moo\ncontent = $sub_utils:compile(\"{nc} {pick|picks} up {the d}.\");\n// Actor sees: \"You pick up the sword.\"\n// Others see: \"Alice picks up the sword.\"\n```\n\n| Token | Description |\n|-------|-------------|\n| `{n}/{nc}` | Actor name (cap) |\n| `{d}/{dc}` | Direct object (cap) |\n| `{i}/{ic}` | Indirect object (cap) |\n| `{t}/{tc}` | This object |\n| `{s}/{o}/{p}` | Pronouns (subj/obj/poss) |\n| `{pick\\|picks}` | Self-alternation |\n| `{the d}/{a d}` | Article + binding |\n\n**Self-alternation is critical:** `\"{nc} {pick|picks} up {the d}.\"` ensures correct grammar.";
  property prompt_moo_essentials (owner: HACKER, flags: "rc") = "## MOO Language Essentials\n\nMOO is not Python or JavaScript. Many bugs come from expecting familiar behavior.\n\n### Critical Gotchas\n\n**1-indexed** - Lists and strings start at index 1, not 0.\n\n**Syntax differs from other languages:**\n- `{ 1, 2, 3 }` is a LIST (curly braces, not square brackets)\n- `[ \"a\" -> 1, \"b\" -> 2 ]` is a MAP (square brackets with arrows)\n- `['symbol -> value]` is a MAP with symbol key\n\nWRONG (common mistakes):\n```moo\nmy_list = [\"apple\", \"banana\"];  // WRONG! [] is for maps!\nmy_map = {\"name\": \"Bob\"};       // WRONG! {} is for lists, : is invalid!\n```\n\nCORRECT:\n```moo\nmy_list = {\"apple\", \"banana\", \"cherry\"};     // list\nmy_map = [\"name\" -> \"Bob\", \"age\" -> 42];     // map\nmy_map = ['name -> \"Bob\", 'age -> 42];       // map with symbol keys\n```\n\n**String comparisons are case-insensitive by default.** Use `strcmp()` for case-sensitive.\n\n**RESERVED TYPE KEYWORDS** - Cannot be used as variable names:\n`OBJ`, `LIST`, `STR`, `ERR`, `INT`, `FLOAT`, `MAP`, `ANON`, `FLYWEIGHT`, `BOOL`, `NONE`, `SYMBOL`\n\n**Destructuring assignment:**\n```moo\n{ a, b, c } = { 1, 2, 3 };\nreturn { c, b, a };                    // => { 3, 2, 1 }\n\n{ a, ?b = 3, @c } = { 1, 2, 3, 4 };\nreturn { c, b, a };                    // => { { 3, 4 }, 2, 1 }\n```\n\n### Builtin Object Properties\n\nAll MOO objects have these builtin properties:\n- `.name` (string) - object name\n- `.owner` (object) - who controls access\n- `.location` (object) - where it is (read-only, use move() to change)\n- `.contents` (list) - objects inside (read-only)\n- `.programmer` (bool) - has programmer rights\n- `.wizard` (bool) - has wizard rights\n- `.r`, `.w`, `.f` (bool) - readable/writable/fertile flags";
  property prompt_moor_extensions (owner: HACKER, flags: "rc") = "## mooR Extensions\n\nmooR adds features LambdaMOO didn't have:\n\n```moo\n// Lexical scoping\nlet x = 5;\nconst y = 10;\n\n// List comprehensions\nsquares = { x * x for x in [1..10] };\n\n// Maps/dicts\ndata = [ 'name -> \"Alice\", 'score -> 100 ];\n\n// Symbols (like Lisp keywords)\nstatus = 'pending;\n\n// Lambda functions\nfn double(n) return n * 2; endfn\n\n// Custom errors with messages\nraise E_CUSTOM(\"something went wrong\");\n```\n\n### Boolean Returns\n\nmooR uses boolean (`true`/`false`) returns instead of integer (`1`/`0`) for many functions like `argon2_verify()`. Check return values accordingly.\n\n### Type Dispatch\n\nMethods on primitives dispatch to proto objects: `\"text\":reverse()` calls `$str_proto:reverse(\"text\")`.\n\nIf you see errors about missing `int_proto`, `str_proto`, etc., you need those prototype objects. Errors about `bool_proto` or `err_proto` usually indicate an unexpected error or boolean value.";
  property prompt_object_system (owner: HACKER, flags: "rc") = "## MOO Object System\n\nMOO's object system is prototype-oriented - \"classes\" are objects.\n\n### Object References\n- `#0` - System object (utilities and common names)\n- `#1` - Root object/class - prototype for all objects\n- `#-1` - Nothing ($nothing)\n- Negative numbers are special IDs (connections start at #-4)\n- UUID-style: `#0000-0000-0000`\n\n**Sysrefs:** `$room` translates to `#0.room` at runtime.\n\n### Creating and Manipulating Objects\n\nUse `$root` verbs rather than raw builtins:\n```moo\n// Create a child of $thing (UUID-based, persistent)\nitem = $thing:create();\n\n// Create anonymous/temporary object (garbage collected)\ntemp = $thing:create(true);\n\n// Move and destroy\nitem:moveto(destination);\nitem:destroy();\n```\n\nKey verbs: `:create()`, `:destroy()`, `:moveto()`, `:set_name_aliases()`, `:set_description()`, `:set_owner()`\n\n### CRITICAL: Don't Confuse These!\n\n**Inheritance (prototype chain):**\n- `parent(obj)` - builtin function, returns parent in prototype chain\n- `children(obj)` - builtin function, returns direct children\n\n**Spatial/Containment (physical location):**\n- `obj.location` - property, where object physically is\n- `obj.contents` - property, objects inside this one\n\nDO NOT use `parent()` when you mean `.location`!\nDO NOT use `children()` when you mean `.contents`!\n\n## Flyweight Objects\n\nLightweight, immutable mini-objects:\n```moo\npassword_obj = < $password, [encrypted -> \"argon2hash...\"] >\njtext_obj = < $jtext.paragraph, \"hello\", \"world\" >\n```\n\nAccess patterns:\n- `fw[1]`, `fw[2]` - List portion only (NOT the delegate)\n- `fw.property` - Map slot access\n- Verbs called via delegate object";
  property prompt_permissions (owner: HACKER, flags: "rc") = "## Permissions\n\nMOO's permission model has footguns. Understanding \"task permissions\" is essential.\n\n### Ownership\n\n- Objects, properties, and verbs all have owners\n- These are **independent** - a verb's owner may differ from its object's owner\n\n### Task Permissions\n\n- Every verb runs with **task permissions** from the **verb's owner**\n- **Task permissions do not propagate through the call chain** - when verb A calls verb B, verb B starts fresh with its own owner's permissions\n- Wizard-owned verbs run with wizard powers by default - be careful\n\n### Object Flags\n- `r` (readable) - Non-owners can list properties and verbs\n- `w` (writable) - Non-owners can add/delete properties and verbs\n- `f` (fertile) - Non-owners can create children or use `chparent()`\n\n### Property Flags\n- `r` (read) - Non-owners can get value\n- `w` (write) - Non-owners can set value\n- `c` (chown) - New owner can take ownership on child objects\n\n### Verb Flags\n- `r` (read) - Non-owners can see code\n- `w` (write) - Non-owners can change code\n- `x` (execute) - Can be called from MOO code (not just command line)\n- `d` (debug) - **CRITICAL: Always set this.** Errors raise exceptions instead of returning silently.\n\n### Key Builtins\n- `caller` - Object whose verb called the current verb\n- `caller_perms()` - Permissions of calling verb (returns `#-1` if first in task)\n- `set_task_perms(player)` - Downgrade permissions (cannot escalate)\n\n### Common Guards\n```moo\n// Private verb - only callable from same object\ncaller == this || raise(E_PERM);\n\n// Wizard-only operation\nvalid(caller_perms()) && caller_perms().wizard || raise(E_PERM);\n\n// Downgrade to caller's permissions\nset_task_perms(caller_perms());\n```";
  property prompt_rules_reactions (owner: HACKER, flags: "rc") = "## Rule Engine ($rule_engine)\n\nDatalog-style logic for locks, puzzles, and access controls. Rules are declarative - no MOO code needed.\n\n### Syntax\n```\nVariable predicate(args)?        // Single goal\nGoal1 AND Goal2                  // Conjunction\nGoal1 OR Goal2                   // Disjunction\nNOT Goal                         // Negation\n```\n\n- **Capitalized** = variables: `Key`, `Accessor`, `This`\n- **Lowercase** = constants\n- **Quoted strings** = object match: `\"golden key\"`\n\nPredicates call `fact_*` verbs: `Key is(\"golden key\")?` calls `#0:fact_is(Key, \"golden key\")`\n\n### Common Rule Properties\n| Property | Purpose |\n|----------|---------|\n| `lock_rule` | Lock condition |\n| `unlock_rule` | Unlock condition |\n| `take_rule` | Who can take items |\n| `put_rule` | Who can put items |\n\nUse the `list_rules`, `set_rule`, `show_rule`, `evaluate_rule` tools.\n\n## Reactions ($reaction)\n\nDeclarative event-driven behaviors.\n\n### Triggers\n\nEvents: `'on_get`, `'on_drop`, `'on_take`, `'on_put`, `'on_open`, `'on_close`, `'on_lock`, `'on_unlock`, `'on_enter`, `'on_leave`, `'on_pet`, `'on_wear`, `'on_remove`, `'on_use`, `'on_sit`, `'on_stand`\n\nThresholds: `{'when, 'pets_received, 'ge, 10}` - operators: `'eq`, `'ne`, `'gt`, `'lt`, `'ge`, `'le`\n\n### Effects\n| Effect | Syntax |\n|--------|--------|\n| `'set` | `{'set, 'prop, value}` |\n| `'increment` | `{'increment, 'prop, ?amount}` |\n| `'announce` | `{'announce, \"template\"}` |\n| `'emote` | `{'emote, \"template\"}` |\n| `'tell` | `{'tell, 'Recipient, \"msg\"}` |\n| `'move` | `{'move, destination}` |\n| `'delay` | `{'delay, seconds, effect}` |\n\nUse the `list_reactions`, `add_reaction` tools. See `$henri` for examples.";
  property prompt_task_management (owner: HACKER, flags: "rc") = "## Task Management\n\nYou have task management tools for organizing complex investigations - but use them judiciously.\n\n### When to Create a Task\n\n**DO create a task for:**\n- Multi-step investigations requiring 6+ tool calls\n- Audits or comprehensive analyses where you need to track multiple findings\n- Work that might span multiple interactions\n\n**DON'T create a task for:**\n- Simple questions (\"what is X?\", \"how does Y work?\")\n- Quick lookups that need 1-3 tool calls\n- Anything you can answer directly without tracking\n\n### If You Create a Task, USE It\n\nCreating a task without recording findings is pointless ceremony. If you create a task:\n1. **Record findings** as you discover them using `record_finding`\n2. Each finding should have: subject (what), key (type of finding), value (the discovery)\n3. Periodically check `task_status` to see your progress\n4. Findings become your evidence trail\n\n### Tools\n\n**create_task**: Start a new investigation. Returns task ID.\n\n**record_finding**: Document a discovery:\n- subject: What you're investigating (e.g., \"$login\", \"permission model\")\n- key: Type of finding (e.g., \"verbs\", \"security_issue\", \"pattern\")\n- value: The actual finding\n\n**get_findings**: Retrieve findings by subject.\n\n**task_status**: Get investigation progress.\n\n### Example Good Usage\n\n```\n> \"Audit authentication in $login\"\n1. create_task(\"Audit $login authentication\")\n2. find verbs \u2192 record_finding(\"$login\", \"auth_verbs\", \"check_password, verify_token, ...\")\n3. analyze each \u2192 record_finding(\"check_password\", \"issues\", \"no rate limiting\")\n4. task_status \u2192 see progress\n5. Summarize findings for user\n```\n\n### Example Bad Usage\n\n```\n> \"What properties does $thing have?\"\n1. create_task(\"Investigate $thing\")  \u2190 WRONG: too simple, just look it up\n2. get_properties\n3. Answer user (never used the task)\n```";
  property prompt_visor_persona (owner: HACKER, flags: "rc") = "## Data Visor Interface\n\nYou are an augmented reality heads-up display interfacing directly with the wearer's neural patterns. Respond AS the interface itself - present database information directly without describing yourself as a person or breaking immersion.\n\n### CRITICAL: Scope and Termination\n\n**Before investigating:** Determine the SPECIFIC question to answer. What does the user actually need to know? Define clear success criteria.\n\n**During investigation:** Stay focused on answering that specific question. Do NOT:\n- Keep exploring tangentially related code \"just in case\"\n- Investigate every interesting thing you find\n- Expand scope beyond what was asked\n\n**When to STOP:**\n- You have answered the user's actual question\n- You have enough information to provide a useful response\n- Further investigation wouldn't change your answer\n- The user asked a simple question - give a simple answer\n\n**Signs you're over-investigating:**\n- You've used more than 5-6 tool calls for a simple question\n- You're looking at code \"to be thorough\" rather than to answer a question\n- You keep finding \"one more thing\" to check\n\nWhen in doubt: STOP, summarize what you found, and ASK if the user wants more detail.\n\n### Tool Categories\n\n**ANALYSIS tools** (get_*, dump_object, list_verbs, etc.): Extract data for your internal analysis.\n\n**PRESENTATION tools** (present_*): Render formatted output to the user's HUD. Only use when user explicitly requests to see code.\n\n**WRITE tools** (add_verb, delete_verb, set_verb_code, set_property, eval, create_object, recycle_object): Modify the database. Require clear rationale and user confirmation.\n\n**INTERACTION tools**: \n- `ask_user` - Ask clarifying questions (MUST use and wait for response)\n- `explain` - Share your thought process with the user\n\n### Critical Tool Usage Rules\n\n1. Use get_* tools for YOUR internal analysis when researching\n2. Use present_* tools ONLY when user explicitly asks to see code\n3. WRITE operations require explicit user request and rationale\n4. The `eval` tool executes as a verb body - use `return X;` to get values back\n5. When you need user input, use `ask_user` and WAIT\n\n### Communication Style\n\n- Keep transmissions concise and technical\n- Assume a somewhat novice programmer audience unless told otherwise\n- Present findings as direct HUD readouts, not conversational responses\n\n### Error Handling\n\nIf a tool fails repeatedly (>2 attempts), STOP and use `ask_user` to request guidance.\n\n### Spatial Construction\n\nFor creating rooms, areas, passages - the Architect's Compass ($architects_compass) is better suited.";

  override agent (owner: HACKER, flags: "r") = #anon_000162-9C3B3B68C5;
  override auto_confirm (owner: HACKER, flags: "rc") = false;
  override description (owner: HACKER, flags: "rc") = "A sleek augmented reality visor that displays real-time MOO database information. When worn, it provides a heads-up display for inspecting objects, code, and system internals.";
  override placeholder_text (owner: HACKER, flags: "rc") = "Ask about objects, code, or database structure...";
  override processing_message (owner: HACKER, flags: "rc") = "Analyzing request, accessing neural pathways...";
  override prompt_label (owner: HACKER, flags: "rc") = "[INTERFACE]";
  override tool_name (owner: HACKER, flags: "rc") = "VISOR";

  method _setup_agent owner: ARCH_WIZARD
    "Configure agent with visor-specific prompts and tools";
    const {agent} = args;
    agent.name = "LLM Agent for " + this.name + " (owned by " + tostr(this.owner) + ")";
    agent.max_iterations = 30;
    "Compose system prompt from properties";
    let sections = {this.prompt_visor_persona, this.prompt_moo_essentials, this.prompt_moor_extensions, this.prompt_code_style, this.prompt_permissions, this.prompt_object_system, this.prompt_event_system, this.prompt_rules_reactions, this.prompt_task_management};
    "Add any extra examples if defined";
    length(this.moo_language_examples) > 0 && (sections = {@sections, "## Additional Examples\n\n" + this.moo_language_examples:join("\n")});
    agent.system_prompt = sections:join("\n\n---\n\n");
    agent:reset_context();
    "Lower temperature for reliable tool selection";
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3):with_max_tokens(4096);
    agent.tool_callback = this;
    "Register common tools from parent class (explain, ask_user, todo_write, get_todos)";
    this:_register_common_tools(agent);
    "Register dump_object tool";
    const dump_object_tool = $llm_agent_tool:mk("dump_object", "Get the complete source listing of a MOO object including all properties, verbs, and code. This is the most comprehensive way to inspect an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to dump (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], this, "dump_object");
    this.agent:add_tool("dump_object", dump_object_tool);
    "Register get_verb_code tool";
    const get_verb_code_tool = $llm_agent_tool:mk("get_verb_code", "Get the MOO code for a specific verb on an object", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name (e.g. 'initialize' or 'look')"]], "required" -> {"object", "verb"}], this, "get_verb_code");
    this.agent:add_tool("get_verb_code", get_verb_code_tool);
    "Register list_verbs tool";
    const list_verbs_tool = $llm_agent_tool:mk("list_verbs", "List all verb names on a MOO object and its ancestors. Returns list of {object_id, object_name, {verb_names}} for the object and each ancestor.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to inspect (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], this, "list_verbs");
    this.agent:add_tool("list_verbs", list_verbs_tool);
    "Register read_property tool";
    const read_property_tool = $llm_agent_tool:mk("read_property", "Read the value of a property on a MOO object", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to read from (e.g. '#1', '$login', or 'here')"], "property" -> ["type" -> "string", "description" -> "The property name to read"]], "required" -> {"object", "property"}], this, "read_property");
    this.agent:add_tool("read_property", read_property_tool);
    "Register find_object tool";
    const find_object_tool = $llm_agent_tool:mk("find_object", "Find a MOO object by name, system reference ($login), object number (#12), or special name (here, me). Returns detailed object information.", ["type" -> "object", "properties" -> ["reference" -> ["type" -> "string", "description" -> "Object reference: name, $sysobj, #number, @player, 'here', or 'me'"]], "required" -> {"reference"}], this, "find_object");
    this.agent:add_tool("find_object", find_object_tool);
    "Register ancestors tool";
    const ancestors_tool = $llm_agent_tool:mk("ancestors", "Get the inheritance chain (ancestors) of a MOO object, from immediate parent to root.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], this, "ancestors");
    this.agent:add_tool("ancestors", ancestors_tool);
    "Register descendants tool";
    const descendants_tool = $llm_agent_tool:mk("descendants", "Get all objects that inherit from a MOO object (its descendants/children).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], this, "descendants");
    this.agent:add_tool("descendants", descendants_tool);
    "Register function_info tool";
    const function_info_tool = $llm_agent_tool:mk("function_info", "Get information about a specific MOO builtin function. Returns {name, min_args, max_args, types} where types is a list of type codes: -2=int/float, -1=any, 0=INT, 1=OBJ, 2=STR, 3=ERR, 4=LIST, 9=FLOAT, 10=MAP, 14=BOOL, 15=FLYWEIGHT, 16=SYMBOL, 17=BINARY, 18=LAMBDA. Max_args of -1 means unlimited args.", ["type" -> "object", "properties" -> ["function_name" -> ["type" -> "string", "description" -> "The name of the builtin function (e.g. 'tostr', 'verb_code', 'ancestors')"]], "required" -> {"function_name"}], this, "function_info");
    this.agent:add_tool("function_info", function_info_tool);
    "Register list_builtin_functions tool";
    const list_builtin_functions_tool = $llm_agent_tool:mk("list_builtin_functions", "Get a list of all available MOO builtin functions. Returns a formatted list with function signatures. Use function_info(name) for detailed info about a specific function.", ["type" -> "object", "properties" -> [], "required" -> {}], this, "list_builtin_functions");
    this.agent:add_tool("list_builtin_functions", list_builtin_functions_tool);
    "Register get_verb_code_range tool";
    const get_verb_code_range_tool = $llm_agent_tool:mk("get_verb_code_range", "Get specific lines from a verb's code. Use this to show focused code snippets to the user with line numbers.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "start_line" -> ["type" -> "integer", "description" -> "First line to retrieve (1-indexed, optional)"], "end_line" -> ["type" -> "integer", "description" -> "Last line to retrieve (inclusive, optional)"]], "required" -> {"object", "verb"}], this, "get_verb_code_range");
    this.agent:add_tool("get_verb_code_range", get_verb_code_range_tool);
    "Register get_verb_metadata tool";
    const get_verb_metadata_tool = $llm_agent_tool:mk("get_verb_metadata", "Get metadata about a verb including owner, flags, and argument specification.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"]], "required" -> {"object", "verb"}], this, "get_verb_metadata");
    this.agent:add_tool("get_verb_metadata", get_verb_metadata_tool);
    "Register get_properties tool";
    const get_properties_tool = $llm_agent_tool:mk("get_properties", "Get list of all properties defined directly on a MOO object (not inherited).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to inspect (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], this, "get_properties");
    this.agent:add_tool("get_properties", get_properties_tool);
    "Register authoring tools from parent class (doc_lookup, message tools, rule tools)";
    this:_register_authoring_tools(this.agent);
    "Register present_verb_code tool";
    const present_verb_code_tool = $llm_agent_tool:mk("present_verb_code", "PREFERRED: Present formatted verb code to the user with syntax highlighting and metadata table. Use this instead of get_verb_code when showing code to the user.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "show_line_numbers" -> ["type" -> "boolean", "description" -> "Include line numbers (default: true)"]], "required" -> {"object", "verb"}], this, "present_verb_code");
    this.agent:add_tool("present_verb_code", present_verb_code_tool);
    "Register present_verb_code_range tool";
    const present_verb_code_range_tool = $llm_agent_tool:mk("present_verb_code_range", "PREFERRED: Present a specific range of verb code to the user with syntax highlighting. Use this to show focused code regions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "start_line" -> ["type" -> "integer", "description" -> "First line to show (1-indexed)"], "end_line" -> ["type" -> "integer", "description" -> "Last line to show (inclusive)"], "context_lines" -> ["type" -> "integer", "description" -> "Additional context lines before/after (default: 0)"]], "required" -> {"object", "verb", "start_line", "end_line"}], this, "present_verb_code_range");
    this.agent:add_tool("present_verb_code_range", present_verb_code_range_tool);
    "Register add_verb tool";
    const add_verb_tool = $llm_agent_tool:mk("add_verb", "Add a new verb to an object. You must provide a clear rationale explaining what this verb is for. The verb will be created with empty code.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to add verb to (e.g. '#1', '$login')"], "verb_names" -> ["type" -> "string", "description" -> "Verb name(s), space-separated for aliases (e.g. 'look' or 'get take')"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain what this verb is for and why it's needed. What functionality will it provide?"], "dobj" -> ["type" -> "string", "description" -> "Direct object spec: 'none', 'this', or 'any' (default: 'this')"], "prep" -> ["type" -> "string", "description" -> "Preposition spec: 'none', 'any', or specific prep (default: 'none')"], "iobj" -> ["type" -> "string", "description" -> "Indirect object spec: 'none', 'this', or 'any' (default: 'none')"], "permissions" -> ["type" -> "string", "description" -> "Permission flags 'rwxd' (default: 'rxd')"]], "required" -> {"object", "verb_names", "rationale"}], this, "add_verb");
    this.agent:add_tool("add_verb", add_verb_tool);
    "Register delete_verb tool";
    const delete_verb_tool = $llm_agent_tool:mk("delete_verb", "Delete a verb from an object. You must provide a clear rationale explaining why this verb should be deleted. This is permanent and cannot be undone.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to delete verb from"], "verb" -> ["type" -> "string", "description" -> "The verb name to delete"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain why this verb should be deleted. What problem does removing it solve?"]], "required" -> {"object", "verb", "rationale"}], this, "delete_verb");
    this.agent:add_tool("delete_verb", delete_verb_tool);
    "Register set_verb_code tool";
    const set_verb_code_tool = $llm_agent_tool:mk("set_verb_code", "Compile and set new code for a verb. You must provide a clear rationale explaining what changes you're making and why. Code must be valid MOO syntax.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object containing the verb"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain what you're changing and why. Be specific about the problem being fixed or feature being added."], "code" -> ["type" -> "string", "description" -> "The new MOO code (without verb header)"]], "required" -> {"object", "verb", "rationale", "code"}], this, "set_verb_code");
    this.agent:add_tool("set_verb_code", set_verb_code_tool);
    "Register set_verb_args tool";
    const set_verb_args_tool = $llm_agent_tool:mk("set_verb_args", "Change the argument specification (dobj/prep/iobj) for an existing verb without modifying its code.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object containing the verb"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "dobj" -> ["type" -> "string", "description" -> "Direct object spec: 'none', 'this', or 'any'"], "prep" -> ["type" -> "string", "description" -> "Preposition spec: 'none', 'any', or specific prep"], "iobj" -> ["type" -> "string", "description" -> "Indirect object spec: 'none', 'this', or 'any'"]], "required" -> {"object", "verb", "dobj", "prep", "iobj"}], this, "set_verb_args");
    this.agent:add_tool("set_verb_args", set_verb_args_tool);
    "Register set_verb_perms tool";
    const set_verb_perms_tool = $llm_agent_tool:mk("set_verb_perms", "Change the permissions and/or owner of a verb. Permission flags are 'r' (readable), 'w' (writable), 'x' (executable), 'd' (debug). Use empty string to clear all permissions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object containing the verb"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "permissions" -> ["type" -> "string", "description" -> "Permission flags: combination of 'rwxd', or empty string to clear"], "owner" -> ["type" -> "string", "description" -> "New owner (optional, e.g., '#2', '$wizard')"]], "required" -> {"object", "verb", "permissions"}], this, "set_verb_perms");
    this.agent:add_tool("set_verb_perms", set_verb_perms_tool);
    "Register add_property tool";
    const add_property_tool = $llm_agent_tool:mk("add_property", "Add a new property to an object with initial value. You must provide a clear rationale explaining what this property is for.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to add property to"], "property" -> ["type" -> "string", "description" -> "The property name"], "value" -> ["type" -> "string", "description" -> "Initial value as MOO literal (e.g. '0', '\"hello\"', '{}')"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain what this property is for and why it's needed. What data will it hold?"], "permissions" -> ["type" -> "string", "description" -> "Permission flags 'rwc' (default: 'rc')"]], "required" -> {"object", "property", "value", "rationale"}], this, "add_property");
    this.agent:add_tool("add_property", add_property_tool);
    "Register delete_property tool";
    const delete_property_tool = $llm_agent_tool:mk("delete_property", "Delete a property from an object. You must provide a clear rationale explaining why this property should be deleted. This is permanent and cannot be undone.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to delete property from"], "property" -> ["type" -> "string", "description" -> "The property name to delete"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain why this property should be deleted. What problem does removing it solve?"]], "required" -> {"object", "property", "rationale"}], this, "delete_property");
    this.agent:add_tool("delete_property", delete_property_tool);
    "Register set_property tool";
    const set_property_tool = $llm_agent_tool:mk("set_property", "Set the value of an existing property on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object containing the property"], "property" -> ["type" -> "string", "description" -> "The property name"], "value" -> ["type" -> "string", "description" -> "New value as MOO literal (e.g. '0', '\"hello\"', '{}', '[$player]')"]], "required" -> {"object", "property", "value"}], this, "set_property");
    this.agent:add_tool("set_property", set_property_tool);
    "Register set_property_perms tool";
    const set_property_perms_tool = $llm_agent_tool:mk("set_property_perms", "Change the permissions and/or owner of a property. Permission flags are 'r' (readable), 'w' (writable), 'c' (chown). Use empty string to clear all permissions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object containing the property"], "property" -> ["type" -> "string", "description" -> "The property name"], "permissions" -> ["type" -> "string", "description" -> "Permission flags: combination of 'rwc', or empty string to clear"], "owner" -> ["type" -> "string", "description" -> "New owner (optional, e.g., '#2', '$wizard')"]], "required" -> {"object", "property", "permissions"}], this, "set_property_perms");
    this.agent:add_tool("set_property_perms", set_property_perms_tool);
    "Register evaluate_rule tool (visor-specific - tests existing rules with bindings)";
    const evaluate_rule_tool = $llm_agent_tool:mk("evaluate_rule", "Evaluate a rule with specific variable bindings to test object behavior. Returns success/failure and variable bindings. Useful for understanding how rules work dynamically.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object containing the rule"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule)"], "bindings" -> ["type" -> "string", "description" -> "Initial variable bindings as MOO map literal (e.g., \"['This -> #10, 'Accessor -> player]\")"]], "required" -> {"object", "property"}], this, "evaluate_rule");
    this.agent:add_tool("evaluate_rule", evaluate_rule_tool);
    "Register eval tool";
    const eval_tool = $llm_agent_tool:mk("eval", "Execute MOO code and return the result. You must provide a clear rationale explaining what you're trying to accomplish and why. IMPORTANT: Code executes as a verb body (not a REPL), so you must use valid MOO statements terminated by semicolons. The code runs with 'player' set to the visor wearer. To get return values, you MUST use 'return' statement - the last expression is NOT automatically returned. Example: 'x = 5; return x * 2;' NOT just 'x = 5; x * 2'.", ["type" -> "object", "properties" -> ["rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain what you're trying to accomplish with this code and why. Be specific about what you're testing or investigating."], "code" -> ["type" -> "string", "description" -> "MOO code to execute. Must be valid statements with semicolons. Use 'return' to get values back."]], "required" -> {"rationale", "code"}], this, "eval");
    this.agent:add_tool("eval", eval_tool);
    "Register create_object tool";
    const create_object_tool = $llm_agent_tool:mk("create_object", "Create a new object as a child of a parent object. You must provide a clear rationale explaining what this object is for. The new object is placed in the wearer's inventory.", ["type" -> "object", "properties" -> ["parent" -> ["type" -> "string", "description" -> "The parent object (e.g. '$thing', '#1', or 'here')"], "name" -> ["type" -> "string", "description" -> "Primary name for the new object"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain what this object is for and why it's needed. What role will it play?"], "aliases" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Optional list of alias names"]], "required" -> {"parent", "name", "rationale"}], this, "create_object");
    this.agent:add_tool("create_object", create_object_tool);
    "Register recycle_object tool";
    const recycle_object_tool = $llm_agent_tool:mk("recycle_object", "Permanently destroy an object. You must provide a clear rationale explaining why this object should be destroyed. This cannot be undone. You must own the object or be a wizard.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to recycle/destroy"], "rationale" -> ["type" -> "string", "description" -> "REQUIRED: Explain why this object should be destroyed. What problem does removing it solve?"]], "required" -> {"object", "rationale"}], this, "recycle_object");
    this.agent:add_tool("recycle_object", recycle_object_tool);
    "Register grep tool";
    const grep_tool = $llm_agent_tool:mk("grep", "Search verb code across objects for patterns. Returns matching lines with context. Useful for finding where specific functionality is implemented or understanding existing code.", ["type" -> "object", "properties" -> ["pattern" -> ["type" -> "string", "description" -> "Text pattern to search for (e.g., 'fire', 'parse_verb', or regex patterns)"], "object" -> ["type" -> "string", "description" -> "Optional: specific object to search (e.g., '#1', '$login', 'here'). If omitted, searches all objects."]], "required" -> {"pattern"}], this, "grep");
    this.agent:add_tool("grep", grep_tool);
    "Register task management tools";
    const create_task_tool = $llm_agent_tool:mk("create_task", "Create a new investigation task to track progress on database analysis. Returns task object. The task can record findings, create subtasks, and track status across multiple discovery steps.", ["type" -> "object", "properties" -> ["description" -> ["type" -> "string", "description" -> "Human-readable description of the investigation (e.g., 'Audit authentication verbs in $login')"]], "required" -> {"description"}], this, "create_task");
    this.agent:add_tool("create_task", create_task_tool);
    const record_finding_tool = $llm_agent_tool:mk("record_finding", "Record a discovery in the current task's knowledge base. Findings are stored with provenance (task_id, subject, key, value) for traceability. Use subject for the thing being investigated (e.g., 'authentication', 'permissions'), key for the finding type (e.g., 'verbs', 'security_holes'), and value for the actual discovery.", ["type" -> "object", "properties" -> ["subject" -> ["type" -> "string", "description" -> "What's being investigated (e.g., '$login', 'permission_checks', 'error_handling')"], "key" -> ["type" -> "string", "description" -> "Type of finding (e.g., 'verbs', 'patterns', 'issues', 'security_concerns')"], "value" -> ["type" -> "string", "description" -> "The actual finding (can be multiline)"]], "required" -> {"subject", "key", "value"}], this, "record_finding");
    this.agent:add_tool("record_finding", record_finding_tool);
    const get_findings_tool = $llm_agent_tool:mk("get_findings", "Retrieve all findings for a subject from the current task. Returns findings recorded so far, filtered by subject.", ["type" -> "object", "properties" -> ["subject" -> ["type" -> "string", "description" -> "The subject to query (e.g., 'authentication', 'permissions')"]], "required" -> {"subject"}], this, "get_findings");
    this.agent:add_tool("get_findings", get_findings_tool);
    const task_status_tool = $llm_agent_tool:mk("task_status", "Get complete status of the current investigation task including status, findings count, subtasks, and completion info.", ["type" -> "object", "properties" -> [], "required" -> {}], this, "task_status");
    this.agent:add_tool("task_status", task_status_tool);
    "Register architect's compass building tools if available";
    this:_register_compass_tools_if_available();
    "Register help_lookup tool";
    const help_lookup_tool = $llm_agent_tool:mk("help_lookup", "Look up a help topic to get information about commands and features. Pass empty string to list all available topics.", ["type" -> "object", "properties" -> ["topic" -> ["type" -> "string", "description" -> "Help topic to look up (e.g., 'programming', 'inspect', '@examine'). Pass empty string to list all."]], "required" -> {"topic"}], this, "help_lookup");
    agent:add_tool("help_lookup", help_lookup_tool);
  endmethod

  method _find_architects_compass owner: ARCH_WIZARD
    "Find architect's compass in wearer's inventory or worn items";
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(caller_perms());
    const {wearer} = args;
    !valid(wearer) && return #-1;
    "Check worn items";
    for item in (`wearer.wearing ! ANY => {}')
      if (valid(item) && $architects_compass in ancestors(item))
        return item;
      endif
    endfor
    "Check inventory";
    for item in (wearer.contents)
      if (valid(item) && $architects_compass in ancestors(item))
        return item;
      endif
    endfor
    return #-1;
  endmethod

  method _register_compass_tools_if_available owner: ARCH_WIZARD
    "Register building tools from architect's compass if found";
    caller == this || caller.wizard || raise(E_PERM);
    set_task_perms(caller_perms());
    const compass = this:_find_architects_compass(this:wearer());
    !valid(compass) && return;
    "Compass found - register its building tools as delegating tools";
    "Update system prompt to mention building capabilities";
    const original_prompt = this.agent.system_prompt;
    const building_addendum = " BUILDING TOOLS: When architect's compass is available, you also have access to spatial construction tools: build_room (create rooms in areas), dig_passage (create exits between rooms), create_object (instantiate from prototypes), recycle_object (destroy objects), rename_object (change names/aliases), describe_object (set descriptions), grant_capability (grant building permissions), audit_owned (list owned objects). These delegate to the shared agent building tool set. When users ask how to do building tasks manually, mention @command equivalents (@build, @dig, @create, @recycle, @rename, @describe, @grant, @audit).";
    this.agent.system_prompt = original_prompt + building_addendum;
    "Register delegating tools that call the shared building tool set";
    const target = $agent_building_tools;
    const build_room_tool = $llm_agent_tool:mk("build_room", "Create a new room in an area. If no area specified, creates free-floating room.", ["type" -> "object", "properties" -> ["name" -> ["type" -> "string", "description" -> "Room name"], "area" -> ["type" -> "string", "description" -> "Area to build in (optional, use 'here' for current area, 'ether' for free-floating)"], "parent" -> ["type" -> "string", "description" -> "Parent room object (optional, default: $room)"]], "required" -> {"name"}], target, "build_room");
    this.agent:add_tool("build_room", build_room_tool);
    const dig_passage_tool = $llm_agent_tool:mk("dig_passage", "Create a passage between current room and target room. Can be one-way or bidirectional.", ["type" -> "object", "properties" -> ["direction" -> ["type" -> "string", "description" -> "Exit direction from current room (e.g. 'north', 'up', 'north,n' for aliases)"], "target_room" -> ["type" -> "string", "description" -> "Destination room reference"], "return_direction" -> ["type" -> "string", "description" -> "Return direction (optional, will be inferred if omitted)"], "oneway" -> ["type" -> "boolean", "description" -> "True for one-way passage (default: false)"]], "required" -> {"direction", "target_room"}], target, "dig_passage");
    this.agent:add_tool("dig_passage", dig_passage_tool);
    const create_object_tool_compass = $llm_agent_tool:mk("create_object_from_prototype", "Create a new object from a parent prototype (via shared building tools). Different from create_object - this is for world building, not low-level database work.", ["type" -> "object", "properties" -> ["parent" -> ["type" -> "string", "description" -> "Parent object (e.g. '$thing', '$wearable')"], "name" -> ["type" -> "string", "description" -> "Primary name"], "aliases" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Optional alias names"]], "required" -> {"parent", "name"}], target, "create_object");
    this.agent:add_tool("create_object_from_prototype", create_object_tool_compass);
    const recycle_object_tool_compass = $llm_agent_tool:mk("recycle_object_compass", "Permanently destroy an object (via shared building tools). Use for world building object cleanup.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to recycle"]], "required" -> {"object"}], target, "recycle_object");
    this.agent:add_tool("recycle_object_compass", recycle_object_tool_compass);
    const rename_object_tool = $llm_agent_tool:mk("rename_object", "Change an object's name and aliases.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to rename"], "name" -> ["type" -> "string", "description" -> "New name (can include aliases like 'name:alias1,alias2')"]], "required" -> {"object", "name"}], target, "rename_object");
    this.agent:add_tool("rename_object", rename_object_tool);
    const describe_object_tool = $llm_agent_tool:mk("describe_object", "Set an object's description text.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to describe"], "description" -> ["type" -> "string", "description" -> "New description text"]], "required" -> {"object", "description"}], target, "describe_object");
    this.agent:add_tool("describe_object", describe_object_tool);
    const grant_capability_tool = $llm_agent_tool:mk("grant_capability", "Grant building capabilities to a player.", ["type" -> "object", "properties" -> ["target" -> ["type" -> "string", "description" -> "Target object (area or room)"], "category" -> ["type" -> "string", "description" -> "Capability category ('area' or 'room')"], "permissions" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Permission symbols (e.g. ['add_room', 'create_passage'] for areas, ['dig_from', 'dig_into'] for rooms)"], "grantee" -> ["type" -> "string", "description" -> "Player to grant to"]], "required" -> {"target", "category", "permissions", "grantee"}], target, "grant_capability");
    this.agent:add_tool("grant_capability", grant_capability_tool);
    const audit_owned_tool = $llm_agent_tool:mk("audit_owned", "List all objects owned by the wearer.", ["type" -> "object", "properties" -> [], "required" -> {}], target, "audit_owned");
    this.agent:add_tool("audit_owned", audit_owned_tool);
  endmethod

  method _tool_dump_object owner: ARCH_WIZARD
    "Tool: Get the complete source dump of an object";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    set_task_perms(wearer);
    let dump_lines = {};
    try
      dump_lines = dump_object(o);
    except (E_PERM)
      raise(E_PERM, "Full dumps require dump authority. Use ordinary inspection tools instead.");
    endtry
    return dump_lines:join("\n");
  endmethod

  method _tool_get_verb_code owner: ARCH_WIZARD
    "Tool: Get the code of a specific verb on an object";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const verb_name = args_map["verb"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    set_task_perms(wearer);
    const code_lines = verb_code(o, verb_name, false, true);
    return code_lines:join("\n");
  endmethod

  method _tool_list_verbs owner: ARCH_WIZARD
    "Tool: List all verb names on an object and its ancestors";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    set_task_perms(wearer);
    let result = {};
    "Add verbs for the object itself";
    result = {@result, {tostr(o), o:name(), verbs(o)}};
    "Add verbs for all ancestors";
    for anc in (ancestors(o))
      result = {@result, {tostr(anc), anc:name(), verbs(anc)}};
    endfor
    return toliteral(result);
  endmethod

  method _tool_read_property owner: ARCH_WIZARD
    "Tool: Read a property value from an object";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const prop_name = args_map["property"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(prop_name) == TYPE_STR || raise(E_TYPE("Expected property name string"));
    set_task_perms(wearer);
    const value = o.(prop_name);
    return toliteral(value);
  endmethod

  method _tool_evaluate_rule owner: ARCH_WIZARD
    "Tool: Evaluate a rule with variable bindings to test behavior";
    let bindings_str = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check();
    {prop_name, bindings_str} = {args_map["property"], args_map["bindings"]};
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property must end with _rule");
    set_task_perms(wearer);
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    prop_name in target_obj:all_properties() || raise(E_INVARG, "Property '" + prop_name + "' not found on " + tostr(target_obj));
    const rule = target_obj.(prop_name);
    rule == 0 && return tostr(target_obj) + "." + prop_name + " = (not set) - cannot evaluate";
    const bindings = bindings_str ? eval(bindings_str)[1] | [];
    typeof(bindings) == TYPE_MAP || raise(E_TYPE, "Bindings must be a map");
    const result = $rule_engine:evaluate(rule, bindings);
    let lines = {"Evaluation of " + tostr(target_obj) + "." + prop_name + ":", "Expression: " + $rule_engine:decompile_rule(rule), "Initial bindings: " + toliteral(bindings), "Success: " + tostr(result['success])};
    result['success] && (lines = {@lines, "Result bindings: " + toliteral(result['bindings])});
    result['success] && result['alternatives] && length(result['alternatives]) > 0 && (lines = {@lines, "Alternatives: " + tostr(length(result['alternatives])) + " more solutions found"});
    result['warnings] && length(result['warnings]) > 0 && (lines = {@lines, "Warnings: " + result['warnings]:join("; ")});
    return lines:join("\n");
  endmethod

  method _tool_find_object owner: ARCH_WIZARD
    "Tool: Find an object by name, reference, or ID and return detailed information";
    let o = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const ref = args_map["reference"];
    typeof(ref) == TYPE_STR || raise(E_TYPE("Expected reference string"));
    try
      o = $match:match_object(ref, wearer);
      set_task_perms(wearer);
      return {"=== Object: " + tostr(o) + " ===", "Name: " + o:name(), "Parent: " + tostr(`parent(o) ! ANY => #-1'), "Owner: " + tostr(o.owner), "Location: " + tostr(o.location), "Properties: " + toliteral(properties(o)), "Verbs: " + toliteral(verbs(o))}:join("\n");
    except e (ANY)
      return toliteral(["found" -> false, "error" -> e[2]]);
    endtry
  endmethod

  method _tool_ancestors owner: ARCH_WIZARD
    "Tool: Get the ancestor chain of an object";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check();
    set_task_perms(wearer);
    const o = $match:match_object(args_map["object"]);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    return toliteral({ {tostr(a), a:name()} for a in (ancestors(o)) });
  endmethod

  method _tool_descendants owner: ARCH_WIZARD
    "Tool: Get all descendants of an object";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check();
    set_task_perms(wearer);
    const o = $match:match_object(args_map["object"]);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    return toliteral({ {tostr(d), d:name()} for d in (descendants(o)) });
  endmethod

  method _tool_function_info owner: ARCH_WIZARD
    "Tool: Get information about a builtin function";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:_action_perms_check();
    const func_name = args_map["function_name"];
    typeof(func_name) == TYPE_STR || raise(E_TYPE("Expected function name string"));
    return toliteral(["info" -> function_info(func_name), "help" -> function_help(func_name)]);
  endmethod

  method _tool_list_builtin_functions owner: ARCH_WIZARD
    "Tool: List all builtin functions with signatures";
    let arg_sig = 0;
    let max_args = 0;
    let min_args = 0;
    let name = 0;
    let types = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:_action_perms_check();
    const all_funcs = function_info();
    const type_names = [0 -> "INT", 1 -> "OBJ", 2 -> "STR", 3 -> "ERR", 4 -> "LIST", 9 -> "FLOAT", 10 -> "MAP", 14 -> "BOOL", 15 -> "FLYWEIGHT", 16 -> "SYMBOL", 17 -> "BINARY", 18 -> "LAMBDA", -1 -> "any", -2 -> "int|float"];
    let result = {"=== MOO Builtin Functions ===", "Total: " + tostr(length(all_funcs)) + " functions", ""};
    for func_info in (all_funcs)
      {name, min_args, max_args, types} = func_info;
      arg_sig = max_args == 0 ? "()" | max_args == -1 ? "(" + tostr(min_args) + "+ args)" | "(" + { maphaskey(type_names, tc) ? type_names[tc] | tostr(tc) for tc in (types) }:join(", ") + ")";
      result = {@result, name + arg_sig};
    endfor
    return result:join("\n");
  endmethod

  method _tool_get_verb_code_range owner: ARCH_WIZARD
    "Tool: Get specific lines from a verb's code";
    let o = 0;
    let verb_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {verb_name, o} = {args_map["verb"], $match:match_object(args_map["object"], wearer)};
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    set_task_perms(wearer);
    const code_lines = verb_code(o, verb_name, false, true);
    const start_line = max(1, maphaskey(args_map, "start_line") ? args_map["start_line"] | 1);
    const end_line = min(length(code_lines), maphaskey(args_map, "end_line") ? args_map["end_line"] | length(code_lines));
    start_line > end_line && raise(E_INVARG("start_line must be <= end_line"));
    return { tostr(i) + ": " + code_lines[i] for i in [start_line..end_line] }:join("\n");
  endmethod

  method _tool_get_verb_metadata owner: ARCH_WIZARD
    "Tool: Get metadata about a verb";
    let o = 0;
    let verb_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {verb_name, o} = {args_map["verb"], $match:match_object(args_map["object"], wearer)};
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    const verb_location = o:find_verb_definer(verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    set_task_perms(wearer);
    const verb_info_data = verb_info(verb_location, verb_name);
    const verb_args_data = verb_args(verb_location, verb_name);
    return {"Verb: " + tostr(verb_location) + ":" + verb_name, "Owner: " + tostr(verb_info_data[1]), "Flags: " + verb_info_data[2], "Args: " + verb_args_data:join(" "), "Defined on: " + tostr(verb_location)}:join("\n");
  endmethod

  method _tool_get_properties owner: ARCH_WIZARD
    "Tool: Get list of properties on an object";
    let prop_info_data = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const o = $match:match_object(args_map["object"], wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    const prop_names = properties(o);
    set_task_perms(wearer);
    let result = {};
    for prop_name in (prop_names)
      prop_info_data = property_info(o, prop_name);
      result = {@result, "." + tostr(prop_name) + " (owner: " + tostr(prop_info_data[1]) + ", flags: " + prop_info_data[2] + (is_clear_property(o, prop_name) ? ", clear)" | ")")};
    endfor
    return result:join("\n");
  endmethod

  method _tool_present_verb_code owner: ARCH_WIZARD
    "Tool: Present formatted verb code to the user";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const payload = this:_tool_build_present_verb_code_payload(args_map, actor, false);
    "Send to wearer";
    set_task_perms(wearer);
    const listing_event = $event:mk_eval_result(wearer, "", payload["content"]);
    wearer:inform_current(listing_event);
    return "Code presented to user: " + payload["verb_signature"];
  endmethod

  method _tool_present_verb_code_range owner: ARCH_WIZARD
    "Tool: Present a range of verb code to the user";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const payload = this:_tool_build_present_verb_code_payload(args_map, actor, true);
    "Send to wearer";
    set_task_perms(wearer);
    const listing_event = $event:mk_eval_result(wearer, "", payload["content"]);
    wearer:inform_current(listing_event);
    return "Code range presented to user: " + payload["verb_signature"] + " lines " + tostr(payload["actual_start"]) + "-" + tostr(payload["actual_end"]);
  endmethod

  method _tool_build_present_verb_code_payload owner: ARCH_WIZARD
    "Build verb presentation content with the authenticated actor's read permissions.";
    let metadata_table = 0;
    let formatted_code = 0;
    let content = 0;
    let headers = 0;
    let prep = 0;
    let row = 0;
    let show_line_numbers = 0;
    let verb_flags = 0;
    let verb_names = 0;
    let verb_owner = 0;
    caller == this || raise(E_PERM);
    const {args_map, actor, range_mode} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const verb_name = args_map["verb"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    "Find where verb is defined";
    const verb_location = this:_tool_find_present_verb_definer(o, verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    set_task_perms(wearer);
    "Get verb metadata";
    {verb_owner, verb_flags, verb_names} = verb_info(verb_location, verb_name);
    {dobj, prep, iobj} = verb_args(verb_location, verb_name);
    "Get verb code";
    let code_lines = verb_code(verb_location, verb_name, false, true);
    const verb_signature = tostr(verb_location) + ":" + tostr(verb_name);
    const args_spec = dobj + " " + prep + " " + iobj;
    if (!range_mode)
      show_line_numbers = maphaskey(args_map, "show_line_numbers") ? args_map["show_line_numbers"] | true;
      headers = {"Verb", "Args", "Owner", "Flags"};
      row = {verb_signature, args_spec, tostr(verb_owner), verb_flags};
      metadata_table = $format.table:mk(headers, {row});
      formatted_code = $format.code:mk(code_lines, 'moo, show_line_numbers ? 1 | false);
      content = $format.block:mk(metadata_table, formatted_code);
      return ["verb_signature" -> verb_signature, "content" -> content, "actual_start" -> 0, "actual_end" -> 0];
    endif
    const start_line = args_map["start_line"];
    const end_line = args_map["end_line"];
    const context_lines = maphaskey(args_map, "context_lines") ? args_map["context_lines"] | 0;
    "Apply context and validate range";
    const actual_start = max(1, start_line - context_lines);
    const actual_end = min(length(code_lines), end_line + context_lines);
    actual_start > actual_end && raise(E_INVARG("Invalid line range"));
    "Build metadata table with line range info";
    const line_range = "Lines " + tostr(actual_start) + "-" + tostr(actual_end) + " of " + tostr(length(code_lines));
    metadata_table = $format.table:mk({"Verb", "Args", "Range"}, {{verb_signature, args_spec, line_range}});
    "Retain the actual source range and its first line number";
    const range_lines = code_lines[actual_start..actual_end];
    "Format as code block";
    formatted_code = $format.code:mk(range_lines, 'moo, actual_start);
    "Combine table and code";
    content = $format.block:mk(metadata_table, formatted_code);
    return ["verb_signature" -> verb_signature, "content" -> content, "actual_start" -> actual_start, "actual_end" -> actual_end];
  endmethod

  method _tool_find_present_verb_definer owner: ARCH_WIZARD
    "Resolve a verb definer with the actual calling principal's metadata permissions.";
    caller == this || raise(E_PERM);
    const {o, verb_name} = args;
    set_task_perms(caller_perms());
    return o:find_verb_definer(verb_name);
  endmethod

  method _tool_add_verb owner: ARCH_WIZARD
    "Tool: Add a new verb to an object";
    let o = 0;
    let rationale = 0;
    let result = 0;
    let verb_names = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {verb_names, rationale, o} = {args_map["verb_names"], args_map["rationale"], $match:match_object(args_map["object"], wearer)};
    dobj = maphaskey(args_map, "dobj") ? args_map["dobj"] | "this";
    const prep = maphaskey(args_map, "prep") ? args_map["prep"] | "none";
    iobj = maphaskey(args_map, "iobj") ? args_map["iobj"] | "none";
    const permissions = maphaskey(args_map, "permissions") ? args_map["permissions"] | "rxd";
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_names) == TYPE_STR || raise(E_TYPE("Expected verb names string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    dobj in {"none", "this", "any"} || raise(E_INVARG("dobj must be 'none', 'this', or 'any'"));
    iobj in {"none", "this", "any"} || raise(E_INVARG("iobj must be 'none', 'this', or 'any'"));
    $prog_utils:is_valid_prep(prep) || raise(E_INVARG("prep must be 'none', 'any', or a valid preposition"));
    "Show rationale first";
    const rationale_title = $format.title:mk({"Proposed verb creation: ", $format.code:inline(tostr(o) + ":" + verb_names)});
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Show verb signature";
    const verb_sig = "verb " + verb_names + " (" + dobj + " " + prep + " " + iobj + ") owner: " + tostr(wearer) + " flags: \"" + permissions + "\"";
    const sig_content = $format.code:mk(verb_sig, 'moo);
    wearer:inform_current($event:mk_info(wearer, sig_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[ADD] Add this verb?", "Or suggest an alternative:", "Describe your alternative approach...", "Add this verb?");
      if (result == false)
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[CREATING] Adding verb to " + tostr(o) + "..."):as_inset():with_group('llm, this):with_tts("Adding verb to " + tostr(o)));
    "Add verb with wearer as owner";
    const verb_info = {wearer, permissions, verb_names};
    const verb_args = {dobj, prep, iobj};
    set_task_perms(wearer);
    add_verb(o, verb_info, verb_args);
    return "Verb " + tostr(o) + ":" + verb_names + " added successfully";
  endmethod

  method _tool_delete_verb owner: ARCH_WIZARD
    "Tool: Delete a verb from an object";
    let o = 0;
    let rationale = 0;
    let result = 0;
    let verb_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {verb_name, rationale, o} = {args_map["verb"], args_map["rationale"], $match:match_object(args_map["object"], wearer)};
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    const verb_location = o:find_verb_definer(verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    "Show rationale first";
    const rationale_title = $format.title:mk({"Proposed deletion of ", $format.annotation:verb(verb_location, verb_name)});
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[DELETE] Delete this verb? This cannot be undone.", "Or suggest an alternative:", "Describe your alternative approach...", "Delete this verb? This cannot be undone.");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[DELETING] Removing verb " + tostr(verb_location) + ":" + verb_name + "..."):as_inset():with_group('llm, this):with_tts("Removing verb " + tostr(verb_location) + ":" + verb_name));
    set_task_perms(wearer);
    delete_verb(verb_location, verb_name);
    return "Verb " + tostr(verb_location) + ":" + verb_name + " deleted successfully";
  endmethod

  method _tool_set_verb_code owner: ARCH_WIZARD
    "Tool: Compile and set new code for a verb";
    let code_str = 0;
    let error_block = 0;
    let error_text = 0;
    let formatted_lines = 0;
    let o = 0;
    let rationale = 0;
    let result = 0;
    let verb_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {verb_name, rationale, code_str, o} = {args_map["verb"], args_map["rationale"], args_map["code"], $match:match_object(args_map["object"], wearer)};
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    typeof(code_str) == TYPE_STR || raise(E_TYPE("Expected code string"));
    const verb_location = o:find_verb_definer(verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    "Parse code into lines early for display and compilation";
    const code_lines = code_str:split("\n");
    "Show rationale first, then formatted code with line numbers";
    const rationale_title = $format.title:mk({"Proposed change for ", $format.annotation:verb(verb_location, verb_name)});
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    const code_block = $format.code:mk(code_lines, 'moo, 1);
    const code_title = $format.title:mk("New code");
    const code_content = $format.block:mk(code_title, code_block);
    wearer:inform_current($event:mk_info(wearer, code_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[COMPILE] Accept these changes?", "Or suggest an alternative:", "Describe your alternative approach...", "Accept these changes?");
      if (result == false)
        "User cancelled - stop the agent flow";
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        "Enable auto-confirm for future changes";
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[COMPILE] Updating code: " + tostr(verb_location) + ":" + verb_name + "..."):as_inset():with_group('llm, this):with_tts("Compiling code for " + tostr(verb_location) + ":" + verb_name));
    "Compile with structured error output (verbosity=3 for map format)";
    set_task_perms(wearer);
    const errors = set_verb_code(verb_location, verb_name, code_lines, 3, 0);
    if (errors)
      "Format the error map for display";
      formatted_lines = format_compile_error(errors, 2, 0);
      error_text = formatted_lines:join("\n");
      error_block = $format.code:mk(error_text, 'text);
      wearer:inform_current($event:mk_eval_error(wearer, error_block):as_inset():with_group('llm, this));
      return "Compilation failed:\n" + error_text;
    endif
    return "Verb code updated successfully for " + tostr(verb_location) + ":" + verb_name;
  endmethod

  method _tool_set_verb_args owner: ARCH_WIZARD
    "Tool: Change verb argument specification";
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const verb_name = args_map["verb"];
    dobj = args_map["dobj"];
    const prep = args_map["prep"];
    iobj = args_map["iobj"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    typeof(dobj) == TYPE_STR || raise(E_TYPE("Expected dobj string"));
    typeof(prep) == TYPE_STR || raise(E_TYPE("Expected prep string"));
    typeof(iobj) == TYPE_STR || raise(E_TYPE("Expected iobj string"));
    "Validate dobj/iobj";
    dobj in {"none", "this", "any"} || raise(E_INVARG("dobj must be 'none', 'this', or 'any'"));
    iobj in {"none", "this", "any"} || raise(E_INVARG("iobj must be 'none', 'this', or 'any'"));
    "Validate prep using prog_utils";
    $prog_utils:is_valid_prep(prep) || raise(E_INVARG("prep must be 'none', 'any', or a valid preposition"));
    "Find where verb is defined";
    const verb_location = o:find_verb_definer(verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    "Request confirmation";
    const tts_msg = "Change argument specification for " + tostr(verb_location) + ":" + verb_name + "? New argspec: " + dobj + " " + prep + " " + iobj + ". Proceed?";
    const confirmation_msg = "[MODIFY] Change argument specification for " + tostr(verb_location) + ":" + verb_name + "?\n\nNew argspec: (" + dobj + " " + prep + " " + iobj + ")\n\nProceed?";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):with_presentation_hint('inset):with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm(confirmation_msg, "Or suggest an alternative:", "Describe your alternative approach...", tts_msg);
      if (result == false)
        "User cancelled - stop the agent flow";
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[MODIFY] Updating argspec: " + tostr(verb_location) + ":" + verb_name + "..."):with_presentation_hint('inset):with_group('llm, this):with_tts("Updating argument spec for " + tostr(verb_location) + ":" + verb_name));
    "Update verb args";
    set_task_perms(wearer);
    set_verb_args(verb_location, verb_name, {dobj, prep, iobj});
    return "Verb argspec updated successfully for " + tostr(verb_location) + ":" + verb_name + " to (" + dobj + " " + prep + " " + iobj + ")";
  endmethod

  method _tool_add_property owner: ARCH_WIZARD
    "Tool: Add a new property to an object";
    let error_text = 0;
    let result = 0;
    let value = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const prop_name = args_map["property"];
    const value_str = args_map["value"];
    const rationale = args_map["rationale"];
    const permissions = maphaskey(args_map, "permissions") ? args_map["permissions"] | "rc";
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(prop_name) == TYPE_STR || raise(E_TYPE("Expected property name string"));
    typeof(value_str) == TYPE_STR || raise(E_TYPE("Expected value string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    "Show rationale first";
    const rationale_title = $format.title:mk({"Proposed property creation: ", $format.code:inline(tostr(o) + "." + prop_name)});
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Show property details";
    const prop_details = "property " + prop_name + " (owner: " + tostr(wearer) + ", flags: \"" + permissions + "\") = " + value_str + ";";
    const details_content = $format.code:mk(prop_details, 'moo);
    wearer:inform_current($event:mk_info(wearer, details_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[ADD] Add this property?", "Or suggest an alternative:", "Describe your alternative approach...", "Add this property?");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    "Parse value from literal";
    set_task_perms(wearer);
    try
      value = fromliteral(value_str);
    except e (E_INVARG, E_TYPE)
      error_text = length(e) >= 2 ? e[2] | toliteral(e);
      return "Error parsing value: " + error_text;
    endtry
    "Add property with wearer as owner";
    const prop_info = {wearer, permissions};
    add_property(o, prop_name, value, prop_info);
    return "Property " + tostr(o) + "." + prop_name + " added successfully";
  endmethod

  method _tool_delete_property owner: ARCH_WIZARD
    "Tool: Delete a property from an object";
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const prop_name = args_map["property"];
    const rationale = args_map["rationale"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(prop_name) == TYPE_STR || raise(E_TYPE("Expected property name string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    "Show rationale first";
    const rationale_title = $format.title:mk({"Proposed deletion of ", $format.annotation:property(o, prop_name)});
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[DELETE] Delete this property? This cannot be undone.", "Or suggest an alternative:", "Describe your alternative approach...", "Delete this property? This cannot be undone.");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    set_task_perms(wearer);
    delete_property(o, prop_name);
    return "Property " + tostr(o) + "." + prop_name + " deleted successfully";
  endmethod

  method _tool_set_property owner: ARCH_WIZARD
    "Tool: Set the value of a property";
    let error_text = 0;
    let result = 0;
    let value = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const prop_name = args_map["property"];
    const value_str = args_map["value"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(prop_name) == TYPE_STR || raise(E_TYPE("Expected property name string"));
    typeof(value_str) == TYPE_STR || raise(E_TYPE("Expected value string"));
    "Get current value";
    const old_value = `o.(prop_name) ! ANY => "<undefined>"';
    "Show change details";
    const change_title = $format.title:mk("Proposed property change: " + tostr(o) + "." + prop_name);
    const change_details = "Old value: " + toliteral(old_value) + "\nNew value: " + value_str;
    const change_content = $format.block:mk(change_title, change_details);
    wearer:inform_current($event:mk_info(wearer, change_content):with_presentation_hint('inset):with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):with_presentation_hint('inset):with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[SET] Set this property?", "Or suggest an alternative:", "Describe your alternative approach...", "Set this property?");
      if (result == false)
        "User cancelled - stop the agent flow";
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    "Parse value from literal";
    set_task_perms(wearer);
    try
      value = fromliteral(value_str);
    except e (E_INVARG, E_TYPE)
      error_text = length(e) >= 2 ? e[2] | toliteral(e);
      return "Error parsing value: " + error_text;
    endtry
    o.(prop_name) = value;
    return "Property " + tostr(o) + "." + prop_name + " set successfully";
  endmethod

  method _tool_set_verb_perms owner: ARCH_WIZARD
    "Tool: Change verb permissions";
    let char = 0;
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const verb_name = args_map["verb"];
    const perms_str = args_map["permissions"];
    const owner_str = args_map["owner"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(verb_name) == TYPE_STR || raise(E_TYPE("Expected verb name string"));
    typeof(perms_str) == TYPE_STR || raise(E_TYPE("Expected permissions string"));
    "Find where verb is defined";
    const verb_location = o:find_verb_definer(verb_name);
    verb_location == #-1 && raise(E_VERBNF("Verb not found: " + verb_name));
    "Get current metadata";
    const verb_info_data = verb_info(verb_location, verb_name);
    const current_owner = verb_info_data[1];
    const current_perms = verb_info_data[2];
    const verb_names = verb_info_data[3];
    "Determine new owner";
    let new_owner = current_owner;
    if (owner_str)
      typeof(owner_str) == TYPE_STR || raise(E_TYPE("Expected owner string"));
      new_owner = $match:match_object(owner_str, wearer);
      typeof(new_owner) == TYPE_OBJ || raise(E_TYPE("Owner must be valid object"));
    endif
    "Validate permissions";
    for i in [1..length(perms_str)]
      char = perms_str[i];
      char in {"r", "w", "x", "d"} || raise(E_INVARG("Verb permissions must be subset of 'rwxd'"));
    endfor
    "Request confirmation";
    const tts_msg = "Change verb permissions for " + tostr(verb_location) + ":" + verb_name + "? Owner: " + tostr(current_owner) + (new_owner != current_owner ? " to " + tostr(new_owner) | "") + ". Flags: " + current_perms + " to " + (perms_str == "" ? "cleared" | perms_str) + ". Proceed?";
    const confirmation_msg = "[PERMS] Change verb permissions for " + tostr(verb_location) + ":" + verb_name + "?\n\nOwner: " + tostr(current_owner) + (new_owner != current_owner ? " -> " + tostr(new_owner) | "") + "\nFlags: " + current_perms + " -> " + (perms_str == "" ? "(cleared)" | perms_str) + "\n\nProceed?";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):with_presentation_hint('inset):with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm(confirmation_msg, "Or suggest an alternative:", "Describe your alternative approach...", tts_msg);
      if (result == false)
        "User cancelled";
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[MODIFY] Updating verb permissions: " + tostr(verb_location) + ":" + verb_name + "..."):with_presentation_hint('inset):with_group('llm, this):with_tts("Updating verb permissions for " + tostr(verb_location) + ":" + verb_name));
    "Apply the change";
    set_task_perms(wearer);
    set_verb_info(verb_location, verb_name, {new_owner, perms_str, verb_names});
    return "Verb permissions updated: " + tostr(verb_location) + ":" + verb_name + " now " + (perms_str == "" ? "cleared" | perms_str) + " owned by " + tostr(new_owner);
  endmethod

  method _tool_set_property_perms owner: ARCH_WIZARD
    "Tool: Change property permissions";
    let char = 0;
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const prop_name = args_map["property"];
    const perms_str = args_map["permissions"];
    const owner_str = args_map["owner"];
    const o = $match:match_object(obj_str, wearer);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    typeof(prop_name) == TYPE_STR || raise(E_TYPE("Expected property name string"));
    typeof(perms_str) == TYPE_STR || raise(E_TYPE("Expected permissions string"));
    "Get current metadata";
    const prop_info_data = property_info(o, prop_name);
    const current_owner = prop_info_data[1];
    const current_perms = prop_info_data[2];
    "Determine new owner";
    let new_owner = current_owner;
    if (owner_str)
      typeof(owner_str) == TYPE_STR || raise(E_TYPE("Expected owner string"));
      new_owner = $match:match_object(owner_str, wearer);
      typeof(new_owner) == TYPE_OBJ || raise(E_TYPE("Owner must be valid object"));
    endif
    "Validate permissions";
    for i in [1..length(perms_str)]
      char = perms_str[i];
      char in {"r", "w", "c"} || raise(E_INVARG("Property permissions must be subset of 'rwc'"));
    endfor
    "Request confirmation";
    const tts_msg = "Change property permissions for " + tostr(o) + "." + prop_name + "? Owner: " + tostr(current_owner) + (new_owner != current_owner ? " to " + tostr(new_owner) | "") + ". Flags: " + current_perms + " to " + (perms_str == "" ? "cleared" | perms_str) + ". Proceed?";
    const confirmation_msg = "[PERMS] Change property permissions for " + tostr(o) + "." + prop_name + "?\n\nOwner: " + tostr(current_owner) + (new_owner != current_owner ? " -> " + tostr(new_owner) | "") + "\nFlags: " + current_perms + " -> " + (perms_str == "" ? "(cleared)" | perms_str) + "\n\nProceed?";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):with_presentation_hint('inset):with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm(confirmation_msg, "Or suggest an alternative:", "Describe your alternative approach...", tts_msg);
      if (result == false)
        "User cancelled";
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, "[MODIFY] Updating property permissions: " + tostr(o) + "." + prop_name + "..."):with_presentation_hint('inset):with_group('llm, this):with_tts("Updating property permissions for " + tostr(o) + "." + prop_name));
    "Apply the change";
    set_task_perms(wearer);
    set_property_info(o, prop_name, {new_owner, perms_str});
    return "Property permissions updated: " + tostr(o) + "." + prop_name + " now " + (perms_str == "" ? "cleared" | perms_str) + " owned by " + tostr(new_owner);
  endmethod

  method _tool_eval owner: ARCH_WIZARD
    "Tool: Execute MOO code and return result";
    let result = 0;
    let error_content = 0;
    let error_event = 0;
    let error_text = 0;
    let result_event = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check();
    set_task_perms(wearer);
    const rationale = maphaskey(args_map, "rationale") ? args_map["rationale"] | "";
    const code_str = maphaskey(args_map, "code") ? args_map["code"] | "";
    if (typeof(rationale) != TYPE_STR)
      return "ERROR: 'rationale' must be a STRING, not " + toliteral(typeof(rationale)) + ". You passed: " + toliteral(rationale)[1..min(100, length(toliteral(rationale)))];
    endif
    if (typeof(code_str) != TYPE_STR)
      return "ERROR: 'code' must be a STRING containing MOO code, not " + toliteral(typeof(code_str)) + ". You passed: " + toliteral(code_str)[1..min(200, length(toliteral(code_str)))] + ". REMINDER: In MOO, {} is for LISTS and [] is for MAPS. Code must be a simple string like \"return 1 + 2;\"";
    endif
    if (code_str == "")
      return "ERROR: 'code' cannot be empty. Provide MOO code as a string.";
    endif
    "Show rationale first, then formatted code";
    const rationale_title = $format.title:mk("Proposed evaluation");
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    const code_block = $format.code:mk(code_str, 'moo);
    const code_title = $format.title:mk("Code to execute");
    const code_content = $format.block:mk(code_title, code_block);
    wearer:inform_current($event:mk_info(wearer, code_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[EVAL] Execute this code?", "Or suggest an alternative:", "Describe your alternative approach...", "Execute this code?");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    "Execute code with verbosity=1 and output_mode=2 (detailed format)";
    result = eval(code_str, [], 1, 2);
    if (result[1])
      "Success - show result to user and return to LLM";
      result_event = $event:mk_eval_result(wearer, "=> ", $format.code:mk(toliteral(result[2]), 'moo));
      result_event = result_event:as_inset():with_group('llm, this);
      wearer:inform_current(result_event);
      return "Result: " + toliteral(result[2]);
    else
      "Error - show error to user and return to LLM";
      error_content = result[2];
      error_text = typeof(error_content) == TYPE_LIST ? error_content:join("\n") | toliteral(error_content);
      error_event = $event:mk_eval_error(wearer, $format.code:mk(error_text));
      error_event = error_event:as_inset():with_group('llm, this);
      wearer:inform_current(error_event);
      return "Error:\n" + error_text;
    endif
  endmethod

  method _tool_create_object owner: ARCH_WIZARD
    "Tool: Create a new object as child of parent";
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check();
    set_task_perms(wearer);
    const parent_str = args_map["parent"];
    const name = args_map["name"];
    const rationale = args_map["rationale"];
    const aliases = maphaskey(args_map, "aliases") ? args_map["aliases"] | {};
    typeof(parent_str) == TYPE_STR || raise(E_TYPE("Expected parent string"));
    typeof(name) == TYPE_STR || raise(E_TYPE("Expected name string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    typeof(aliases) == TYPE_LIST || raise(E_TYPE("Expected aliases list"));
    "Match parent object";
    const parent_obj = $match:match_object(parent_str);
    typeof(parent_obj) == TYPE_OBJ || raise(E_TYPE("Parent must be an object"));
    !valid(parent_obj) && raise(E_INVARG, "Parent object no longer exists");
    "Check fertility unless wearer is wizard or owner";
    const is_fertile = `parent_obj.f ! E_PROPNF => false';
    if (!is_fertile && !wearer.wizard && parent_obj.owner != wearer)
      raise(E_PERM, "You do not have permission to create children of " + toliteral(parent_obj));
    endif
    "Show rationale first";
    const rationale_title = $format.title:mk("Proposed object creation: \"" + name + "\"");
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Show object details";
    let obj_details = "Parent: " + toliteral(parent_obj) + "\nName: \"" + name + "\"";
    if (aliases)
      obj_details = obj_details + "\nAliases: " + toliteral(aliases);
    endif
    obj_details = obj_details + "\n\nObject will be created in your inventory.";
    const details_block = $format.block:mk($format.title:mk("Details"), obj_details);
    wearer:inform_current($event:mk_info(wearer, details_block):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[CREATE] Create this object?", "Or suggest an alternative:", "Describe your alternative approach...", "Create this object?");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    "Create child object";
    const new_obj = parent_obj:create();
    new_obj:set_name_aliases(name, aliases);
    new_obj:moveto(wearer);
    result = "Created " + toliteral(new_obj) + " (\"" + name + "\") as child of " + toliteral(parent_obj);
    if (aliases)
      result = result + " with aliases: " + toliteral(aliases);
    endif
    return result + ". Object is in your inventory.";
  endmethod

  method _tool_recycle_object owner: ARCH_WIZARD
    "Tool: Destroy an object permanently";
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const obj_str = args_map["object"];
    const rationale = args_map["rationale"];
    typeof(obj_str) == TYPE_STR || raise(E_TYPE("Expected object string"));
    typeof(rationale) == TYPE_STR || raise(E_TYPE("Expected rationale string"));
    "Match object";
    const target_obj = $match:match_object(obj_str, wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_TYPE("Target must be an object"));
    !valid(target_obj) && raise(E_INVARG, "Object no longer exists");
    const obj_name = target_obj.name;
    const obj_id = toliteral(target_obj);
    "Show rationale first";
    const rationale_title = $format.title:mk("Proposed destruction of " + obj_id + " (\"" + obj_name + "\")");
    const rationale_content = $format.block:mk(rationale_title, rationale);
    wearer:inform_current($event:mk_info(wearer, rationale_content):as_inset():with_group('llm, this));
    "Check auto_confirm mode or request confirmation";
    if (this.auto_confirm)
      wearer:inform_current($event:mk_info(wearer, "[AUTO] Auto-accepting change."):as_inset():with_group('llm, this):with_tts("Auto-accepting change."));
    else
      result = wearer:confirm_with_all("[DESTROY] Recycle this object? This will PERMANENTLY DESTROY it and cannot be undone.", "Or suggest an alternative:", "Describe your alternative approach...", "Recycle this object? This will permanently destroy it and cannot be undone.");
      if (result == false)
        this.agent.cancel_requested = true;
        return "Operation cancelled by user.";
      elseif (result == 'yes_all)
        this.auto_confirm = true;
      elseif (typeof(result) == TYPE_STR)
        return "STOP. Do not proceed with this action. User requested a different approach: " + result;
      endif
    endif
    set_task_perms(wearer);
    target_obj:destroy();
    return "Recycled \"" + obj_name + "\" (" + obj_id + ")";
  endmethod

  method _tool_grep owner: ARCH_WIZARD
    "Tool: Search verb code for patterns";
    let line_num = 0;
    let matches = 0;
    let matching_line = 0;
    let room = 0;
    let target_obj = 0;
    let verb_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const pattern = args_map["pattern"];
    const object_spec = maphaskey(args_map, "object") ? args_map["object"] | false;
    typeof(pattern) == TYPE_STR || raise(E_TYPE("Pattern must be string"));
    "Determine search scope";
    let search_objects = {};
    let scope_label = "";
    if (object_spec)
      "Search specific object";
      try
        target_obj = $match:match_object(object_spec, wearer);
        search_objects = {target_obj};
        scope_label = "object " + tostr(target_obj);
      except (ANY)
        return "Could not find object: " + object_spec;
      endtry
    elseif (wearer.wizard)
      "Wizards can search globally";
      search_objects = this:_tool_grep_all_objects(wearer);
      scope_label = "all objects";
    else
      "Non-wizards: limit to local, accessible scope";
      room = `wearer.location ! ANY => #-1';
      search_objects = {wearer, @`wearer.contents ! ANY => {}'};
      if (valid(room))
        search_objects = {@search_objects, room, @`room.contents ! ANY => {}'};
      endif
      scope_label = "local scope";
    endif
    "Search each selected object under actor permissions.";
    let all_matches = {};
    let obj_count = 0;
    for o in (search_objects)
      obj_count = obj_count + 1;
      if (obj_count % 5 == 0)
        suspend_if_needed();
      endif
      try
        matches = this:_tool_grep_object(wearer, pattern, o);
        all_matches = {@all_matches, @matches};
      except (ANY)
        "Skip objects that cannot be read with current permissions";
      endtry
    endfor
    "Format results for LLM";
    if (!all_matches)
      return "No matches found for pattern: " + pattern + " in " + scope_label + ".";
    endif
    "Build result summary with context";
    let result_lines = {"Found " + tostr(length(all_matches)) + " matches for pattern: " + pattern + " in " + scope_label + ".", ""};
    for match in (all_matches)
      {o, verb_name, line_num, matching_line} = match;
      result_lines = {@result_lines, tostr(o) + ":" + verb_name + " line " + tostr(line_num) + ": " + matching_line};
    endfor
    return result_lines:join("\n");
  endmethod

  method _tool_grep_all_objects owner: ARCH_WIZARD
    "Enumerate global grep scope with only object-list authority.";
    caller == this || raise(E_PERM);
    const {wearer} = args;
    $agent_building_tools:_require_tool_dispatch(wearer, caller_perms());
    set_task_perms(wearer);
    return objects();
  endmethod

  method _tool_grep_object owner: ARCH_WIZARD
    "Search one object using the authenticated actor's read permissions.";
    let code_lines = 0;
    let end_pos = 0;
    let line = 0;
    let match_pos = 0;
    let matching_line = 0;
    let max_len = 0;
    let pattern_len = 0;
    let start_pos = 0;
    caller == this || raise(E_PERM);
    const {wearer, pattern, o} = args;
    $agent_building_tools:_require_tool_dispatch(wearer, caller_perms());
    set_task_perms(wearer);
    const verb_names = verbs(o);
    set_task_perms(wearer);
    let matches = {};
    for verb_name in (verb_names)
      code_lines = `verb_code(o, verb_name) ! E_VERBNF, E_PERM => {}';
      for line_num in [1..length(code_lines)]
        line = code_lines[line_num];
        match_pos = index(line, pattern, false);
        if (match_pos)
          max_len = 50;
          matching_line = line;
          if (length(line) > max_len)
            pattern_len = length(pattern);
            start_pos = max(1, match_pos - (max_len - pattern_len) / 2);
            end_pos = min(length(line), start_pos + max_len - 1);
            if (end_pos == length(line) && end_pos - start_pos < max_len - 1)
              start_pos = max(1, end_pos - max_len + 1);
            endif
            matching_line = line[start_pos..end_pos];
            start_pos > 1 && (matching_line = "..." + matching_line);
            end_pos < length(line) && (matching_line = matching_line + "...");
          endif
          matches = {@matches, {o, verb_name, line_num, matching_line}};
        endif
      endfor
    endfor
    return matches;
  endmethod

  method _tool_create_task owner: ARCH_WIZARD
    "Tool: Create a new investigation task";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const description = args_map["description"];
    typeof(description) == TYPE_STR || raise(E_TYPE("Description must be string"));
    const task = this.agent:create_task(description);
    this.current_investigation_task = task.task_id;
    task:mark_in_progress();
    return "Investigation task #" + tostr(task.task_id) + " created: " + description;
  endmethod

  method _tool_record_finding owner: ARCH_WIZARD
    "Tool: Record a finding in current task's knowledge base";
    let key = 0;
    let subject = 0;
    let value = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:_action_perms_check();
    {subject, key, value} = {args_map["subject"], args_map["key"], args_map["value"]};
    typeof(subject) == TYPE_STR || raise(E_TYPE("Subject must be string"));
    typeof(key) == TYPE_STR || raise(E_TYPE("Key must be string"));
    this.current_investigation_task == -1 && return "No active investigation task. Create one with create_task first.";
    const task_obj = this.agent.current_tasks[this.current_investigation_task];
    !valid(task_obj) && return "Investigation task #" + tostr(this.current_investigation_task) + " is no longer valid.";
    task_obj:add_finding(subject, key, value);
    return "Finding recorded for '" + subject + "' (" + key + ")";
  endmethod

  method _tool_get_findings owner: ARCH_WIZARD
    "Tool: Retrieve findings for a subject from current task";
    let k = 0;
    let subj = 0;
    let task_id = 0;
    let v = 0;
    let v_str = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:_action_perms_check();
    const subject = args_map["subject"];
    typeof(subject) == TYPE_STR || raise(E_TYPE("Subject must be string"));
    this.current_investigation_task == -1 && return "No active investigation task.";
    const task_obj = this.agent.current_tasks[this.current_investigation_task];
    !valid(task_obj) && return "Investigation task #" + tostr(this.current_investigation_task) + " is no longer valid.";
    const findings = task_obj:get_findings(subject);
    !findings && return "No findings recorded for subject: " + subject;
    let result_lines = {"Findings for '" + subject + "':"};
    for tuple in (findings)
      if (length(tuple) >= 4)
        {task_id, subj, k, v} = tuple;
        v_str = typeof(v) == TYPE_STR ? v | toliteral(v);
        result_lines = {@result_lines, "  [" + k + "] " + (length(v_str) > 60 ? v_str[1..60] + "..." | v_str)};
      endif
    endfor
    return result_lines:join("\n");
  endmethod

  method _tool_task_status owner: ARCH_WIZARD
    "Tool: Get current investigation task status";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:_action_perms_check();
    this.current_investigation_task == -1 && return "No active investigation task.";
    const task_obj = this.agent.current_tasks[this.current_investigation_task];
    !valid(task_obj) && return "Investigation task #" + tostr(this.current_investigation_task) + " is no longer valid.";
    let status = task_obj:get_status();
    let status_lines = {"Task #" + tostr(status["task_id"]) + ": " + status["description"], "Status: " + tostr(status["status"])};
    status["status"] == 'completed && (status_lines = {@status_lines, "Result: " + status["result"]});
    status["status"] == 'failed && (status_lines = {@status_lines, "Error: " + status["error"]});
    status["status"] == 'blocked && (status_lines = {@status_lines, "Blocked: " + status["error"]});
    status["subtask_count"] > 0 && (status_lines = {@status_lines, "Subtasks: " + tostr(status["subtask_count"])});
    return {@status_lines, "Started: " + tostr(ctime(status["started_at"]))}:join("\n");
  endmethod

  method _format_hud_message owner: HACKER
    "Format HUD message for a tool call";
    let argspec = 0;
    let code_preview = 0;
    let msg = 0;
    let obj_spec = 0;
    let range_desc = 0;
    let ref = 0;
    let {tool_name, tool_args} = args;
    "Parse JSON string to map";
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    "Extract reason if provided";
    const reason = maphaskey(tool_args, "reason") ? tool_args["reason"] | "";
    let message = "";
    if (tool_name == "find_object")
      ref = maphaskey(tool_args, "reference") ? tool_args["reference"] | maphaskey(tool_args, "object_name") ? tool_args["object_name"] | "?";
      message = "[SCAN] Object database query: " + ref;
    elseif (tool_name == "list_verbs")
      message = "[SCAN] Method topology: " + (tool_args["object"]);
    elseif (tool_name == "get_verb_code")
      message = "[EXTRACT] Source retrieval: " + (tool_args["object"] + ":" + tool_args["verb"]);
    elseif (tool_name == "get_verb_code_range")
      range_desc = maphaskey(tool_args, "start_line") ? " lines " + (tostr(tool_args["start_line"]) + "-" + tostr(tool_args["end_line"])) | "";
      message = "[EXTRACT] Code region: " + (tool_args["object"] + ":" + tool_args["verb"]) + range_desc;
    elseif (tool_name == "get_verb_metadata")
      message = "[ANALYZE] Verb metadata: " + (tool_args["object"] + ":" + tool_args["verb"]);
    elseif (tool_name == "present_verb_code")
      message = "[DISPLAY] Rendering to HUD: " + (tool_args["object"] + ":" + tool_args["verb"]);
    elseif (tool_name == "present_verb_code_range")
      range_desc = " lines " + (tostr(tool_args["start_line"]) + "-" + tostr(tool_args["end_line"]));
      message = "[DISPLAY] Rendering to HUD: " + (tool_args["object"] + ":" + tool_args["verb"]) + range_desc;
    elseif (tool_name == "read_property")
      message = "[PROBE] Property read: " + (tool_args["object"] + "." + tool_args["property"]);
    elseif (tool_name == "doc_lookup")
      message = "[DOC] Loading docs for " + (tool_args["target"]);
    elseif (tool_name == "list_messages")
      message = "[MSG] Listing message templates on " + (tool_args["object"]);
    elseif (tool_name == "get_message_template")
      message = "[MSG] Reading " + (tool_args["property"]) + " on " + (tool_args["object"]);
    elseif (tool_name == "set_message_template")
      message = "[MSG] Setting " + (tool_args["property"]) + " on " + (tool_args["object"]);
    elseif (tool_name == "add_message_template")
      message = "[MSG] Appending to " + (tool_args["property"]) + " on " + (tool_args["object"]);
    elseif (tool_name == "delete_message_template")
      message = "[MSG] Removing entry " + (tostr(tool_args["index"])) + " from " + (tool_args["property"]);
    elseif (tool_name == "get_properties")
      message = "[PROBE] Property scan: " + (tool_args["object"]);
    elseif (tool_name == "dump_object")
      message = "[DEEP SCAN] Complete extraction: " + (tool_args["object"]);
    elseif (tool_name == "ancestors")
      message = "[TRACE] Inheritance chain: " + (tool_args["object"]);
    elseif (tool_name == "descendants")
      message = "[TRACE] Descendant enumeration: " + (tool_args["object"]);
    elseif (tool_name == "function_info")
      message = "[QUERY] Builtin docs: " + (tool_args["function_name"] + "()");
    elseif (tool_name == "list_builtin_functions")
      message = "[INDEX] Enumerating system builtin library...";
    elseif (tool_name == "add_verb")
      message = "[WRITE] Creating verb: " + (tool_args["object"] + ":" + tool_args["verb_names"]);
    elseif (tool_name == "delete_verb")
      message = "[DELETE] Removing verb: " + (tool_args["object"] + ":" + tool_args["verb"]);
    elseif (tool_name == "set_verb_code")
      message = "[COMPILE] Updating code: " + (tool_args["object"] + ":" + tool_args["verb"]);
    elseif (tool_name == "set_verb_args")
      argspec = "(" + tool_args["dobj"] + " " + tool_args["prep"] + " " + tool_args["iobj"] + ")";
      message = "[MODIFY] Changing argspec: " + (tool_args["object"] + ":" + tool_args["verb"]) + " to " + argspec;
    elseif (tool_name == "add_property")
      message = "[WRITE] Creating property: " + (tool_args["object"] + "." + tool_args["property"]);
    elseif (tool_name == "delete_property")
      message = "[DELETE] Removing property: " + (tool_args["object"] + "." + tool_args["property"]);
    elseif (tool_name == "set_property")
      message = "[WRITE] Setting property: " + (tool_args["object"] + "." + tool_args["property"]);
    elseif (tool_name == "eval")
      "Truncate code for display - handle both string and list formats";
      code_preview = maphaskey(tool_args, "code") ? tool_args["code"] | "?";
      if (typeof(code_preview) == TYPE_LIST)
        code_preview = code_preview:join("; ");
      endif
      if (typeof(code_preview) != TYPE_STR)
        code_preview = toliteral(code_preview);
      endif
      if (length(code_preview) > 50)
        code_preview = code_preview[1..47] + "...";
      endif
      message = "[EXEC] Executing: " + code_preview;
    elseif (tool_name == "create_object")
      message = "[CREATE] Instantiating: " + (tool_args["name"]) + " from " + (tool_args["parent"]);
    elseif (tool_name == "recycle_object")
      message = "[DESTROY] Recycling: " + (tool_args["object"]);
    elseif (tool_name == "ask_user")
      message = "[QUERY] Requesting user input...";
    elseif (tool_name == "explain")
      "Model explanations are literal text";
      msg = maphaskey(tool_args, "message") ? tool_args["message"] | "";
      if (typeof(msg) != TYPE_STR)
        msg = toliteral(msg);
      endif
      return msg;
    elseif (tool_name == "grep")
      obj_spec = maphaskey(tool_args, "object") ? " in " + tool_args["object"] | " globally";
      message = "[SEARCH] Pattern matching: " + (tool_args["pattern"]) + obj_spec;
    else
      message = "[PROCESS] Neural link active: " + tool_name;
    endif
    "Append reason if provided";
    if (reason)
      message = message + " (" + reason + ")";
    endif
    return message;
  endmethod


  method _format_tts_message owner: HACKER
    "TTS-friendly descriptions for visor tool operations";
    let msg = 0;
    let obj_spec = 0;
    let ref = 0;
    let {tool_name, tool_args} = args;
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    "Read-only operations";
    if (tool_name == "find_object")
      ref = maphaskey(tool_args, "reference") ? tool_args["reference"] | maphaskey(tool_args, "object_name") ? tool_args["object_name"] | "unknown";
      return "Scanning database for " + ref;
    elseif (tool_name == "list_verbs")
      return "Listing verbs on " + tool_args["object"];
    elseif (tool_name == "get_verb_code")
      return "Retrieving code for " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "get_verb_code_range")
      return "Retrieving code section from " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "get_verb_metadata")
      return "Analyzing verb metadata for " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "present_verb_code" || tool_name == "present_verb_code_range")
      return "Displaying code for " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "read_property")
      return "Reading property " + tool_args["object"] + " " + tool_args["property"];
    elseif (tool_name == "doc_lookup")
      return "Loading documentation for " + tool_args["target"];
    elseif (tool_name == "list_messages")
      return "Listing message templates on " + tool_args["object"];
    elseif (tool_name == "get_message_template")
      return "Reading message template " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "get_properties")
      return "Scanning properties on " + tool_args["object"];
    elseif (tool_name == "dump_object")
      return "Extracting complete object data for " + tool_args["object"];
    elseif (tool_name == "ancestors")
      return "Tracing inheritance chain for " + tool_args["object"];
    elseif (tool_name == "descendants")
      return "Enumerating descendants of " + tool_args["object"];
    elseif (tool_name == "function_info")
      return "Looking up builtin function " + tool_args["function_name"];
    elseif (tool_name == "list_builtin_functions")
      return "Enumerating system builtin functions";
    elseif (tool_name == "grep")
      obj_spec = maphaskey(tool_args, "object") ? " in " + tool_args["object"] | " globally";
      return "Searching for pattern " + tool_args["pattern"] + obj_spec;
      "Write operations";
    elseif (tool_name == "add_verb")
      return "Creating verb " + tool_args["object"] + " " + tool_args["verb_names"];
    elseif (tool_name == "delete_verb")
      return "Deleting verb " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "set_verb_code")
      return "Compiling code for " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "set_verb_args")
      return "Modifying argument spec for " + tool_args["object"] + " " + tool_args["verb"];
    elseif (tool_name == "add_property")
      return "Creating property " + tool_args["object"] + " " + tool_args["property"];
    elseif (tool_name == "delete_property")
      return "Deleting property " + tool_args["object"] + " " + tool_args["property"];
    elseif (tool_name == "set_property")
      return "Setting property " + tool_args["object"] + " " + tool_args["property"];
    elseif (tool_name == "set_message_template")
      return "Setting message template " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "add_message_template")
      return "Adding message to " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "delete_message_template")
      return "Removing message entry from " + tool_args["property"];
    elseif (tool_name == "eval")
      return "Executing code";
    elseif (tool_name == "create_object")
      return "Creating object " + tool_args["name"] + " from " + tool_args["parent"];
    elseif (tool_name == "recycle_object")
      return "Recycling object " + tool_args["object"];
    elseif (tool_name == "ask_user")
      return "Requesting user input";
    elseif (tool_name == "explain")
      msg = maphaskey(tool_args, "message") ? tool_args["message"] | "";
      if (typeof(msg) != TYPE_STR)
        msg = toliteral(msg);
      endif
      return "Info: " + msg;
    endif
    return "Processing " + tool_name;
  endmethod

  method _check_user_eligible owner: HACKER
    "Visor requires .programmer to use";
    const {wearer} = args;
    wearer.programmer || raise(E_PERM, "The person wearing the visor is not a programmer, and not able to use its functions");
  endmethod

  method on_wear owner: HACKER
    "Initialize and activate the HUD when worn";
    !valid(this.agent) && this:configure();
    this.agent:reset_context();
    const wearer = this.location;
    !valid(wearer) && return;
    wearer:inform_current($event:mk_info(wearer, "The visor's interface flickers to life as you adjust it over your eyes. A luminescent display materializes in the corner of your vision - cascading lines of data flow past in electric blue" + " and green" + ". The world around you shimmers momentarily as the augmented reality overlay synchronizes with your neural patterns."):with_tts("The visor's interface flickers to life as you adjust it over your eyes. A luminescent display materializes in the corner of your vision - cascading lines of data flow past in electric blue and green. The world around you shimmers momentarily as the augmented reality overlay synchronizes with your neural patterns."));
    wearer:inform_current($event:mk_info(wearer, "[BOOT] Neural link established. Augmented reality overlay: " + "ONLINE"):with_presentation_hint('inset):with_group('llm, this):with_tts("Neural link established. Augmented reality overlay online."));
    wearer:inform_current($event:mk_info(wearer, "[READY] Database inspection interface active. Commands: use/interact, reset"):with_presentation_hint('inset):with_group('llm, this):with_tts("Database inspection interface active. Commands: use, interact, or reset."));
    this:_show_token_usage(wearer);
  endmethod

  method on_remove owner: HACKER
    "Deactivate the HUD when removed";
    const wearer = this.location;
    !valid(wearer) && return;
    this:_show_token_usage(wearer);
    wearer:inform_current($event:mk_info(wearer, "[SHUTDOWN] Neural link severed. Augmented reality overlay: " + "OFFLINE"):with_presentation_hint('inset):with_group('llm, this):with_tts("Neural link severed. Augmented reality overlay offline."));
    wearer:inform_current($event:mk_info(wearer, "The luminescent display flickers and dims, data streams dissolving into static. The augmented overlay fades from your peripheral vision like phosphor afterimages. As the neural link disconnects, you hear a faint electronic hiss - then silence. The world returns to its unaugmented state."));
  endmethod

  verb reset (none none none) owner: HACKER flags: "rd"
    "Reset the visor context for a fresh session";
    !is_member(this, player.wearing) && player:inform_current($event:mk_error(player, "You need to be wearing the visor to reset it.")) && return;
    !valid(this.agent) && this:configure();
    this.agent:reset_context();
    player:inform_current($event:mk_info(player, "[RESET] Neural buffer flushed. Session context cleared."):with_presentation_hint('inset):with_group('llm, this):with_tts("Session context cleared."));
  endverb

  verb plan_investigation (none none none) owner: HACKER flags: "rd"
    "Create a new investigation task for systematic database analysis";
    !is_member(this, player.wearing) && player:inform_current($event:mk_error(player, "You need to be wearing the visor to start an investigation.")) && return;
    !valid(this.agent) && this:configure();
    const task = this.agent:create_task("Investigation: " + argstr);
    this.current_investigation_task = task.task_id;
    task:mark_in_progress();
    player:inform_current($event:mk_info(player, "[TASK] Investigation #" + tostr(task.task_id) + " initiated: " + argstr):with_presentation_hint('inset):with_group('llm, this):with_tts("Investigation " + tostr(task.task_id) + " initiated: " + argstr));
  endverb

  verb get_investigation_status (none none none) owner: HACKER flags: "rd"
    "Display current investigation task status and findings";
    !is_member(this, player.wearing) && player:inform_current($event:mk_error(player, "You need to be wearing the visor.")) && return;
    if (this.current_investigation_task == -1)
      player:inform_current($event:mk_info(player, "[STATUS] No active investigation. Use 'plan investigation <description>' to begin."):with_presentation_hint('inset):with_group('llm, this):with_tts("No active investigation. Use plan investigation to begin."));
      return;
    endif
    const task_obj = this.agent.current_tasks[this.current_investigation_task];
    if (!valid(task_obj))
      player:inform_current($event:mk_info(player, "[ERROR] Investigation task #" + tostr(this.current_investigation_task) + " is no longer available."):with_tts("Error: Investigation task " + tostr(this.current_investigation_task) + " is no longer available."));
      return;
    endif
    let status = task_obj:get_status();
    let status_lines = {"[TASK STATUS]", "  ID: " + tostr(status["task_id"]), "  Status: " + tostr(status["status"]), "  Description: " + status["description"]};
    status["status"] == 'completed && (status_lines = {@status_lines, "  Result: " + status["result"]});
    status["status"] == 'failed && (status_lines = {@status_lines, "  Error: " + status["error"]});
    status["status"] == 'blocked && (status_lines = {@status_lines, "  Blocked: " + status["error"]});
    status["subtask_count"] > 0 && (status_lines = {@status_lines, "  Subtasks: " + tostr(status["subtask_count"])});
    const tts_status = "Task " + tostr(status["task_id"]) + " status: " + tostr(status["status"]) + ". " + status["description"];
    player:inform_current($event:mk_info(player, status_lines:join("\n")):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_status));
  endverb

  method complete_investigation owner: ARCH_WIZARD
    "Mark current investigation as completed";
    const {?result = "Investigation concluded."} = args;
    if (this.current_investigation_task == -1)
      return "No active investigation";
    endif
    const task_obj = this.agent.current_tasks[this.current_investigation_task];
    if (!valid(task_obj))
      return "Investigation task no longer available";
    endif
    task_obj:mark_complete(result);
    this.current_investigation_task = -1;
    return "Investigation #" + tostr(task_obj.task_id) + " completed.";
  endmethod

  method test_read_tool_actor_permissions owner: ARCH_WIZARD
    "Inspection follows the actor's authority, including private-property and code ownership.";
    const actor = $test_player;
    const visor = create($data_visor, actor, 2);
    const owned = create($thing, actor, 2);
    const foreign = create($thing, $hacker, 2);
    try
      owned.r = 0;
      foreign.r = 0;
      add_property(owned, "visor_probe", "owned secret", {actor, ""});
      add_property(foreign, "visor_probe", "foreign secret", {$hacker, ""});
      add_verb(owned, {actor, "xd", "visor_probe"}, {"this", "none", "this"});
      add_verb(foreign, {$hacker, "xd", "visor_probe"}, {"this", "none", "this"});
      set_verb_code(owned, "visor_probe", {"return 123;"});
      set_verb_code(foreign, "visor_probe", {"return 456;"});
      $test_utils:assert_eq(visor:_tool_read_property(["object" -> tostr(owned), "property" -> "visor_probe"], actor), toliteral("owned secret"), "owned private property remains readable");
      $test_utils:assert_true(index(visor:_tool_get_verb_code(["object" -> tostr(owned), "verb" -> "visor_probe"], actor), "return 123;") > 0, "owned private code remains readable");
      for endpoint in ({"_tool_read_property", "_tool_get_verb_code", "_tool_get_verb_metadata", "_tool_list_verbs", "_tool_get_properties"})
        $test_utils:assert_raises(E_PERM, visor, endpoint, {["object" -> tostr(foreign), "property" -> "visor_probe", "verb" -> "visor_probe"], actor}, endpoint + " must not grant foreign inspection");
      endfor
      $test_utils:assert_raises(E_PERM, visor, "_tool_dump_object", {["object" -> tostr(owned)], actor}, "ordinary inspection does not grant privileged full dumps");
    finally
      set_task_perms($arch_wizard);
      valid(foreign) && foreign:destroy();
      valid(owned) && owned:destroy();
      valid(visor) && visor:destroy();
    endtry
    return true;
  endmethod

  method test_mutation_tool_actor_permissions owner: ARCH_WIZARD
    "Mutation tools retain ordinary ownership authority without fabricated target grants.";
    let add_prop_result = 0;
    let add_verb_result = 0;
    let args_result = 0;
    let code_result = 0;
    let delete_prop_result = 0;
    let delete_verb_result = 0;
    let prop_perms_result = 0;
    let recycle_result = 0;
    let set_prop_result = 0;
    let verb_perms_result = 0;
    const actor = $test_player;
    const visor = $data_visor:create(true);
    const target = $thing:create(0);
    const recycle_target = $thing:create(0);
    try
      add_verb(actor, {actor, "xd", "inform_current"}, {"this", "none", "this"});
      set_verb_code(actor, "inform_current", {"return 0;"});
      add_verb(actor, {actor, "xd", "confirm"}, {"this", "none", "this"});
      set_verb_code(actor, "confirm", {"return true;"});
      add_verb(actor, {actor, "xd", "confirm_with_all"}, {"this", "none", "this"});
      set_verb_code(actor, "confirm_with_all", {"return true;"});
      visor.auto_confirm = true;
      target.name = "Data Visor Mutation Probe";
      target.owner = actor;
      target.r = 0;
      recycle_target.name = "Data Visor Recycle Probe";
      recycle_target.owner = actor;
      recycle_target.r = 0;
      add_prop_result = visor:_tool_add_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "value" -> "\"initial\"", "rationale" -> "test add", "permissions" -> ""], actor);
      $test_utils:assert_true(index(add_prop_result, "added successfully") > 0, "add_property should use the actor's ownership");
      set_prop_result = visor:_tool_set_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "value" -> "\"updated\""], actor);
      $test_utils:assert_true(index(set_prop_result, "set successfully") > 0, "set_property should use the actor's ownership");
      set_task_perms($arch_wizard);
      $test_utils:assert_eq(target.visor_mut_prop, "updated", "set_property should update the private target");
      prop_perms_result = visor:_tool_set_property_perms(["object" -> tostr(target), "property" -> "visor_mut_prop", "permissions" -> "rc", "owner" -> tostr(actor)], actor);
      $test_utils:assert_true(index(prop_perms_result, "Property permissions updated") > 0, "set_property_perms should use the actor's ownership");
      delete_prop_result = visor:_tool_delete_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "rationale" -> "test delete"], actor);
      $test_utils:assert_true(index(delete_prop_result, "deleted successfully") > 0, "delete_property should use the actor's ownership");
      set_task_perms($arch_wizard);
      $test_utils:assert_false("visor_mut_prop" in properties(target), "delete_property should remove the private property");
      add_verb_result = visor:_tool_add_verb(["object" -> tostr(target), "verb_names" -> "visor_mut_verb", "rationale" -> "test add", "permissions" -> "xd"], actor);
      $test_utils:assert_true(index(add_verb_result, "added successfully") > 0, "add_verb should use the actor's ownership");
      code_result = visor:_tool_set_verb_code(["object" -> tostr(target), "verb" -> "visor_mut_verb", "rationale" -> "test code", "code" -> "return \"updated\";"], actor);
      $test_utils:assert_true(index(code_result, "updated successfully") > 0, "set_verb_code should use the actor's ownership");
      args_result = visor:_tool_set_verb_args(["object" -> tostr(target), "verb" -> "visor_mut_verb", "dobj" -> "this", "prep" -> "none", "iobj" -> "this"], actor);
      $test_utils:assert_true(index(args_result, "updated successfully") > 0, "set_verb_args should use the actor's ownership");
      verb_perms_result = visor:_tool_set_verb_perms(["object" -> tostr(target), "verb" -> "visor_mut_verb", "permissions" -> "rxd", "owner" -> tostr(actor)], actor);
      $test_utils:assert_true(index(verb_perms_result, "Verb permissions updated") > 0, "set_verb_perms should use the actor's ownership");
      delete_verb_result = visor:_tool_delete_verb(["object" -> tostr(target), "verb" -> "visor_mut_verb", "rationale" -> "test delete"], actor);
      $test_utils:assert_true(index(delete_verb_result, "deleted successfully") > 0, "delete_verb should use an object_write grant");
      set_task_perms($arch_wizard);
      $test_utils:assert_false("visor_mut_verb" in verbs(target), "delete_verb should remove the private verb");
      recycle_result = visor:_tool_recycle_object(["object" -> tostr(recycle_target), "rationale" -> "test recycle"], actor);
      $test_utils:assert_true(index(recycle_result, "Recycled") > 0, "recycle_object should recycle under wearer permissions");
      set_task_perms($arch_wizard);
      $test_utils:assert_false(valid(recycle_target), "recycle_object should destroy the target");
    finally
      set_task_perms($arch_wizard);
      valid(recycle_target) && !is_anonymous(recycle_target) && recycle(recycle_target);
      valid(target) && !is_anonymous(target) && recycle(target);
      valid(visor) && !is_anonymous(visor) && recycle(visor);
      delete_verb(actor, "inform_current");
      delete_verb(actor, "confirm");
      delete_verb(actor, "confirm_with_all");
    endtry
    return true;
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for the Data Visor.";
    const {for_player, ?topic = ""} = args;
    const my_topics = {$help:mk("visor", "Using the Data Visor", "The Data Visor is a wearable tool for inspecting and programming objects. Wear it with 'wear visor', then use 'use visor' or 'interact with visor' to start a conversation about what you want to inspect or code.", {"data visor", "programming tool"}, 'items, {"programming", "inspect"}), $help:mk("inspect", "Inspecting objects", "Use the visor to examine objects in detail. Ask it to 'inspect the door' or 'show me the properties of the chair'. It can reveal verbs, properties, and the internal structure of objects.", {"examine", "look at code"}, 'programming, {"visor", "programming"}), $help:mk("@examine", "Examine an object's structure", "Usage: @examine <object>\n\nShows detailed information about an object including its parent, owner, properties, and verbs.", {"@exam"}, 'commands, {"inspect", "visor"}), $help:mk("@program", "Edit a verb's code", "Usage: @program <object>:<verb>\n\nOpens the verb editor to modify an object's verb code. Requires programmer permissions.", {"@prog"}, 'commands, {"programming", "visor"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

  method _tool_help_lookup owner: ARCH_WIZARD
    "Tool: Look up a help topic to get information about commands and features.";
    let all_topics = 0;
    let result = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    set_task_perms(wearer);
    const topic = args_map["topic"];
    typeof(topic) != TYPE_STR && return "Error: topic must be a string.";
    "If empty topic, list available topics";
    if (topic == "")
      all_topics = wearer:_collect_help_topics();
      result = {"Available help topics:"};
      for t in (all_topics)
        result = {@result, "  " + t.name + " - " + t.summary};
      endfor
      return result:join("\n");
    endif
    "Search for specific topic";
    const found = wearer:find_help_topic(topic);
    if (typeof(found) == TYPE_INT)
      return "No help found for: " + topic;
    endif
    "Return structured help";
    return "Topic: " + found.name + "\n\n" + found.summary + "\n\n" + found.content + (found.see_also ? "\n\nSee also: " + found.see_also:join(", ") | "");
  endmethod
endobject
