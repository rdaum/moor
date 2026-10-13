object HELP_TOPICS [
  import_export_id -> "help_topics",
  import_export_hierarchy -> {"help"}
]
  name: "Global Help Topics"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property topic_basics (owner: ARCH_WIZARD, flags: "rc") = {
    "basics",
    "Getting started",
    "Welcome to Cowbell! Use `look` to see where you are, move with directions like `north` or `n`, and `inventory` to see what you're carrying.\n\nTalk with `say <message>` or `\"<message>`, and express actions with `emote <action>` or `:<action>`.\n\nType `help` anytime to see available topics.",
    {"start", "newbie", "begin", "intro"},
    "basics",
    {"look", "movement", "inventory", "communicating"}
  };
  property topic_bite (owner: ARCH_WIZARD, flags: "rc") = {
    "bite",
    "Take a bite of food",
    "Use `bite <food>` to take a small bite of food. Takes less than eating the whole thing.\n\nSee also `nibble` for even smaller portions.",
    {},
    "basics",
    {"eat", "nibble"}
  };
  property topic_close (owner: ARCH_WIZARD, flags: "rc") = {
    "close",
    "Close a door",
    "Close a door-like passage from this room.\n\nUsage:\n\n- `close <direction>`\n- `close door`",
    {"close door"},
    "basics",
    {"open", "lock", "unlock", "doors"}
  };
  property topic_communicating (owner: ARCH_WIZARD, flags: "rc") = {
    "communicating",
    "Talking and expressing yourself",
    "There are several ways to interact with others:\n\n`say` - Speak aloud for everyone in the room to hear.\n\n`to` - Direct your speech at someone specific. Others still hear you, but the message is addressed to that person. Usage: `to alice Hello there!`\n\nThe backtick (`` ` ``) is a shortcut for directed speech - put the target name right after it: `` `alice Hello there! ``\n\n`emote` - Describe an action with your name at the start.\n\n`think` - Show a thought bubble.\n\nSocial gestures like `wave`, `nod`, `smile` let you interact without typing descriptions. See `help socializing` for the full list.",
    {"communication", "talking", "speaking"},
    "social",
    {"say", "emote", "socializing", "to"}
  };
  property topic_describe (owner: ARCH_WIZARD, flags: "rc") = {
    "@describe",
    "Describe yourself",
    "Use `@describe me as <description>` to set how others see you when they look at you.",
    {"@desc"},
    "basics",
    {"look"}
  };
  property topic_doors (owner: ARCH_WIZARD, flags: "rc") = {
    "doors",
    "Door commands",
    "Door commands operate only on door-like passages.\n\nA passage is treated as a door when either:\n\n- `is_door` is true, or\n- it has an `unlock_rule`\n\nBuilders can toggle door behavior with:\n\n- `@set-passage <dir> is_door true|false`",
    {"door", "doors"},
    "basics",
    {"movement", "exits", "@set-passage"}
  };
  property topic_drink (owner: ARCH_WIZARD, flags: "rc") = {
    "drink",
    "Drink a beverage",
    "Use `drink <beverage>` to drink from a vessel you're holding.\n\nRelated commands:\n\n- `sip <beverage>` - take a small sip\n- `gulp <beverage>` or `quaff <beverage>` - drink quickly\n- `refill <vessel> from <source>` - refill from a tap or fountain",
    {},
    "basics",
    {"eat", "sip"}
  };
  property topic_drop (owner: ARCH_WIZARD, flags: "rc") = {
    "drop",
    "Put something down",
    "Use `drop <thing>` to remove something from your inventory and leave it in the current room.",
    {},
    "basics",
    {"get", "inventory"}
  };
  property topic_eat (owner: ARCH_WIZARD, flags: "rc") = {
    "eat",
    "Eat food",
    "Use `eat <food>` to consume food you're holding or that's nearby. Eating consumes the whole portion.\n\nRelated commands:\n\n- `bite <food>` - take a smaller bite\n- `nibble <food>` - nibble delicately",
    {"consume"},
    "basics",
    {"drink", "bite"}
  };
  property topic_emote (owner: ARCH_WIZARD, flags: "rc") = {
    "emote",
    "Express actions",
    "Use `emote <action>` or `:<action>` to describe what you're doing. Your name appears at the start.\n\n`emote waves hello` \u2192 _YourName waves hello_\n\n`:laughs` \u2192 _YourName laughs_",
    {"pose", ":"},
    "social",
    {"say", "communicating", "socializing"}
  };
  property topic_examine (owner: ARCH_WIZARD, flags: "rc") = {
    "examine",
    "Examine something closely",
    "Use `examine <thing>` to get detailed information about an object or person.\n\nOften reveals details not visible with just `look`.",
    {"ex", "x"},
    "basics",
    {"look"}
  };
  property topic_exits (owner: ARCH_WIZARD, flags: "rc") = {
    "exits",
    "See available exits",
    "Use `exits` or `ways` to see a list of directions you can travel from the current room.",
    {"ways", "directions"},
    "basics",
    {"movement", "look"}
  };
  property topic_gag (owner: ARCH_WIZARD, flags: "rc") = {
    "@gag",
    "Ignore output from a player or object",
    "Use `@gag <player|object>` to add someone (or something) to your gag lists.\n\nExamples:\n\n- `@gag Alice` (gag a player by name)\n- `@gag #123` (gag an object by object number)\n\nNotes:\n\n- Players are matched globally by name.\n- Objects are matched in your current scope; use `#<id>` to gag a specific object anywhere.\n- Gagging is local to you and does not notify the other party.",
    {"gag", "mute", "block"},
    "communicating",
    {"gagging", "@ungag", "@listgag"}
  };
  property topic_gagging (owner: ARCH_WIZARD, flags: "rc") = {
    "gagging",
    "Hide output from a player/object",
    "Gagging lets you hide output that originates from a specific player or object.\n\nWhen someone is gagged, their speech, direct messages, and other output that they initiate will be suppressed on *your* client. This is a personal preference and abuse-avoidance tool.\n\n### What gagging does\n\n- Hides output that originates from the gagged player/object.\n- Works for both players and non-player objects.\n- Affects only what *you* see; it does not stop them from seeing you.\n- Does not notify the other party.\n\n### What gagging does not do\n\n- It is not a ban.\n- It does not prevent the other party from interacting with the world.\n- It does not remove them from public spaces.\n\n### If you're dealing with harassment or abuse\n\n- Use `@gag <name>` immediately to reduce exposure.\n- Preserve context if you can (room name, time, what happened).\n- Contact staff/moderators/administrators for help.\n- If you feel unsafe, disconnect and reach out out-of-band.\n\n### Related commands\n\n- `@gag <player|object>`\n- `@ungag <player|object>` (or `@ungag everyone`)\n- `@listgag`",
    {"gag", "mute", "block", "harassment", "abuse", "spam"},
    "communicating",
    {"@gag", "@ungag", "@listgag", "communicating", "mail"}
  };
  property topic_get (owner: ARCH_WIZARD, flags: "rc") = {
    "get",
    "Pick something up",
    "Use `get <thing>` or `take <thing>` to pick up an object and add it to your inventory.",
    {"take", "pick"},
    "basics",
    {"drop", "inventory"}
  };
  property topic_give (owner: ARCH_WIZARD, flags: "rc") = {
    "give",
    "Give something to someone",
    "Use `give <thing> to <person>` to hand an object to another person.",
    {"hand"},
    "social",
    {"get", "drop"}
  };
  property topic_inventory (owner: ARCH_WIZARD, flags: "rc") = {
    "inventory",
    "See what you're carrying",
    "Use `inventory` or `i` to list everything you're holding.\n\nItems you're wearing are marked.",
    {"i", "inv"},
    "basics",
    {"get", "drop"}
  };
  property topic_join (owner: ARCH_WIZARD, flags: "rc") = {
    "join",
    "Walk to join another player",
    "Use `join <player>` to automatically walk to where another player is.\n\nExamples:\n\n- `join ryan` - walk to Ryan's location\n- `join mr welcome` - walk to Mr. Welcome\n\nThe player name uses standard matching.\n\nTo stop: `walk stop` or `walk cancel`\n\nNote: Only works if the player is in the same area and reachable by walking (passages only, not elevators).",
    {"@join"},
    "social",
    {"movement", "walk", "who"}
  };
  property topic_listgag (owner: ARCH_WIZARD, flags: "rc") = {
    "@listgag",
    "Show your gag lists",
    "Use `@listgag` to see which players and objects you currently have gagged.\n\nWhen run as a command, it also reports who has gagged you (if anyone).",
    {"listgag", "gaglist"},
    "communicating",
    {"gagging", "@gag", "@ungag"}
  };
  property topic_lock (owner: ARCH_WIZARD, flags: "rc") = {
    "lock",
    "Lock a door",
    "Lock a lockable door-like passage.\n\nUsage:\n\n- `lock <direction>`\n- `lock <direction> with <key>`\n- `lock door`\n\nRequires a matching key and an `unlock_rule` on the passage.",
    {"lock door"},
    "basics",
    {"unlock", "open", "doors"}
  };
  property topic_look (owner: ARCH_WIZARD, flags: "rc") = {
    "look",
    "Look at your surroundings",
    "Use `look` to see the room you're in, including exits and people present.\n\nUse `look <thing>` to examine something specific in more detail.",
    {"l"},
    "basics",
    {"examine", "exits"}
  };
  property topic_movement (owner: ARCH_WIZARD, flags: "rc") = {
    "movement",
    "Moving around",
    "Move using compass directions: `north`, `south`, `east`, `west` (or `n`, `s`, `e`, `w`).\n\nSome places have other exits like `up`, `down`, `in`, `out`.\n\nUse `exits` to see available directions.\n\n## Auto-walk\n\nYou can walk automatically to a destination:\n\n- `walk to <place>` - walk to a room by name\n- `join <player>` - walk to where another player is\n- `walk stop` - stop walking\n\nAuto-walk uses fuzzy matching, so `walk lobby` or `walk dorm` will work.",
    {"go", "move", "travel"},
    "basics",
    {"exits", "look", "walk", "join"}
  };
  property topic_open (owner: ARCH_WIZARD, flags: "rc") = {
    "open",
    "Open a door",
    "Open a door-like passage from this room.\n\nUsage:\n\n- `open <direction>`\n- `open door`\n\nIf the door is locked, open may auto-unlock when you have a matching key.",
    {"open door"},
    "basics",
    {"close", "lock", "unlock", "doors"}
  };
  property topic_messages (owner: ARCH_WIZARD, flags: "rc") = {
    "messages",
    "Write and preview messages",
    "When Alex picks up a pebble, Alex sees \"You pick up the pebble.\" Other people see \"Alex picks up the pebble.\"\n\nA *message template* produces these different sentences from one piece of text. Tokens in braces supply names, pronouns, or a choice of words.\n\n## Try a pickup message\n\nYou need builder privileges. Create a pebble that belongs to you:\n\n```\n@create $thing named pebble\n```\n\nThe pebble starts in your inventory. Preview a message before saving it:\n\n```\n@preview-message pebble.get_msg {nc} {pick|picks} up {the d}.\n```\n\nThe preview uses three example readers. Alex picks up the pebble; Sam and Robin watch.\n\n- Alex sees: You pick up the pebble.\n- Sam sees: Alex picks up the pebble.\n- Robin sees: Alex picks up the pebble.\n\nThe preview saves nothing and sends no pickup event.\n\nThe tokens mean:\n\n- `{nc}`: the person's name, or \"You\" for that person.\n- `{pick|picks}`: \"pick\" for that person, \"picks\" for others.\n- `{the d}`: \"the\" and the item's name.\n\nSave the message, then try it:\n\n```\n@set-message pebble.get_msg {nc} {pick|picks} up {the d}.\ndrop pebble\nget pebble\n```\n\nUse `@get-message pebble.get_msg` to read the stored template.\n\n## Find the available names\n\nEach interaction supplies its own participants. To see them for the pickup message, enter:\n\n```\n@message-bindings pebble.get_msg\n```\n\nHere, `actor` means the person picking up the item. `dobj` means the item. These supplied names are called *bindings*.\n\nThe editor rejects unavailable names before saving. A mistake such as `{dobbj}` reports its position and suggests `{dobj}`.\n\n## Use someone's pronouns\n\nThe template `{sc} {have} {p} coat.` uses the actor's pronouns. For Alex using they/them:\n\n- Alex sees: You have your coat.\n- Sam sees: They have their coat.\n- Robin sees: They have their coat.\n\nWith she/her, Sam and Robin see \"She has her coat.\"\n\nAfter a name, use name agreement: `{nc} {agree actor name:have|has} a coat.` This gives \"You have a coat\" or \"Alex has a coat\".\n\nSome messages supply other named people. When `recipient` is available, `{pronoun recipient pos_adj} coat` gives \"your coat\" to that person. Others see their chosen word, such as \"her coat\".\n\n## Preview a reaction message\n\nFor an existing reaction called `welcome` on your `cedarbox`, try:\n\n```\n@reaction cedarbox welcome preview-message announce \"{nc} {open|opens} {the d}.\"\n```\n\nAlex sees \"You open the cedarbox.\" Sam and Robin see \"Alex opens the cedarbox.\"\n\nThis command does not add or run a step. The normal reaction `preview` also shows example sentences for its saved message steps. Read `help @reaction` to create a reaction.\n\nMessage templates and Djot descriptions use different syntax. A template does not turn asterisks into emphasis. A description does not expand message tokens.",
    {"message templates", "substitutions"},
    "building",
    {"pronouns", "@reaction"}
  };
  property topic_pronouns (owner: ARCH_WIZARD, flags: "rc") = {
    "pronouns",
    "Choose the words messages use for you",
    "Pronouns are words such as \"she\", \"them\", and \"their\". Cowbell uses your choice when a message refers to you.\n\n## Choose a preset\n\nEnter `@pronouns` to see your current choice and the available presets. To choose one, enter:\n\n```\n@pronouns they/them\n```\n\nThe presets are `he/him`, `she/her`, `they/them`, `it/its`, and `e/em`.\n\nFor Alex using they/them, a message can show Alex \"You have your coat.\" Other people see \"They have their coat.\"\n\nThis setting affects message templates. It does not rewrite what people say or words written directly into a message.\n\n## Enter your own words\n\nEnter `@pronouns custom`. You need no builder or programmer privileges.\n\nAnswer each prompt with the word that fits its blank. For example:\n\n- ___ is ready. Answer: `ze`.\n- I met ___. Answer: `zir`.\n- This is ___ coat. Answer: `zir`.\n- The coat is ___. Answer: `zirs`.\n- Ze did it ___. Answer: `zirself`.\n\nThen choose `is` or `are` to match your subject word. This also selects \"has\" or \"have\".\n\nRead the completed sentences before answering the final question. Answer `yes` to save, or `no` to keep your current pronouns. Enter `@abort` at a word prompt to cancel.\n\n## In Meadow\n\nOpen your account settings and choose *Enter custom pronouns*. Fill the blanks, choose the verb form, and select *Preview sentences*. Select *Use these pronouns* to save.\n\nThe same editor appears during profile setup. Finish the profile to save your choice there.\n\nFor writing messages that use these words, read `help messages`.",
    {"@pronouns"},
    "social",
    {"messages"}
  };
  property topic_order (owner: ARCH_WIZARD, flags: "rc") = {
    'topic_basics,
    'topic_bite,
    'topic_close,
    'topic_communicating,
    'topic_describe,
    'topic_doors,
    'topic_drink,
    'topic_drop,
    'topic_eat,
    'topic_emote,
    'topic_examine,
    'topic_exits,
    'topic_gag,
    'topic_gagging,
    'topic_get,
    'topic_give,
    'topic_inventory,
    'topic_join,
    'topic_listgag,
    'topic_lock,
    'topic_look,
    'topic_messages,
    'topic_movement,
    'topic_open,
    'topic_privacy,
    'topic_pronouns,
    'topic_quit,
    'topic_refill,
    'topic_say,
    'topic_sip,
    'topic_ungag,
    'topic_unlock,
    'topic_walk,
    'topic_who
  };
  property topic_privacy (owner: ARCH_WIZARD, flags: "rc") = {
    "privacy",
    "Privacy policy and data practices",
    "## Timbran Hotel Privacy Policy\n\n*Your privacy is respected at the Timbran Hotel.*\n\n### What We Collect\n\nWhen you register as a guest, we store:\n\n- Your chosen name\n- Your password (securely encrypted)\n- Your personal event history (encrypted with your encryption passphrase)\n\n### What We Don't Do\n\n- We do *not* algorithmically profile or target you\n- We do *not* sell or share your data with advertisers or data brokers\n- We do *not* read your encrypted event history—only you can decrypt it\n\n### Public Spaces\n\nThe Timbran Hotel has many public rooms—the lobby, corridors, common areas. Conversations and actions in public spaces are naturally visible to other guests present. This is the nature of a shared world, not data collection.\n\nIf you wish for privacy, seek out private rooms or communicate through the mail system.\n\n### Mail & Direct Messages\n\nMail and direct messages between guests are currently stored unencrypted. We plan to add encryption in the future.\n\nWhile administrators technically have access, we maintain a strict policy against reading private correspondence. Treat these as you would a postcard—private in practice, but not cryptographically secured.\n\n### User Creations\n\nObjects you create, customizations you make, and programs you write are stored in the world database. Even if you mark them as private, administrators can view them for maintenance and moderation purposes.\n\n### AI & LLM Agents\n\nCertain areas of the hotel feature AI-powered characters—such as Mr. Welcome in the lobby or staff at the front desk. These agents use large language models to interact with guests.\n\nWhen you interact in rooms with these agents:\n\n- Your actions and speech in that room may be processed by external AI services\n- We prioritize open-weight models and may run self-hosted models in the future\n- We transmit only what's necessary for the interaction\n\nAI-powered items (like the Architect's Compass or Data Visor) work similarly—using them sends your input to AI services.\n\nWe plan to offer an opt-out so your direct actions aren't shared with room-based agents. However, if other guests mention you or have conversations about you, that content may still be processed.\n\n### Data Retention\n\nYour account and encrypted history remain as long as you're a guest. You may request deletion of your account and all associated data at any time by contacting the management.\n\n### Contact\n\nFor privacy concerns, speak with the hotel management or contact the server administrator.",
    {"privacy policy", "data", "gdpr"},
    "basics",
    {}
  };
  property topic_quit (owner: ARCH_WIZARD, flags: "rc") = {
    "@quit",
    "Disconnect",
    "Use `@quit` to disconnect from the game.\n\nYour character stays in the world but goes to sleep.",
    {"quit", "logout"},
    "basics",
    {}
  };
  property topic_refill (owner: ARCH_WIZARD, flags: "rc") = {
    "refill",
    "Refill a drink vessel",
    "Use `refill <vessel> from <source>` to refill an empty or partially empty drink vessel from a source like a fountain, tap, or dispenser.",
    {},
    "basics",
    {"drink"}
  };
  property topic_say (owner: ARCH_WIZARD, flags: "rc") = {
    "say",
    "Speak to others",
    "Use `say <message>` or `\"<message>` to speak aloud. Everyone in the room hears you.\n\n`say Hello everyone!`\n`\"Hi there!`",
    {"talk", "speak", "\""},
    "social",
    {"emote", "communicating"}
  };
  property topic_sip (owner: ARCH_WIZARD, flags: "rc") = {
    "sip",
    "Sip a drink",
    "Use `sip <beverage>` to take a small, delicate sip from a drink.\n\nSee also `gulp` or `quaff` for larger drinks.",
    {},
    "basics",
    {"drink", "gulp"}
  };
  property topic_ungag (owner: ARCH_WIZARD, flags: "rc") = {
    "@ungag",
    "Remove someone from your gag list",
    "Use `@ungag <player|object>` to remove someone (or something) from your gag lists.\n\nExamples:\n\n- `@ungag Alice`\n- `@ungag #123`\n- `@ungag everyone` (clear both gag lists)\n\nTip:\n\n- `@listgag` shows what is currently gagged.",
    {"ungag", "unmute", "unblock"},
    "communicating",
    {"gagging", "@gag", "@listgag"}
  };
  property topic_unlock (owner: ARCH_WIZARD, flags: "rc") = {
    "unlock",
    "Unlock a door",
    "Unlock a lockable door-like passage.\n\nUsage:\n\n- `unlock <direction>`\n- `unlock <direction> with <key>`\n- `unlock door`\n\nRequires a matching key and an `unlock_rule` on the passage.",
    {"unlock door"},
    "basics",
    {"lock", "open", "doors"}
  };
  property topic_walk (owner: ARCH_WIZARD, flags: "rc") = {
    "walk",
    "Walk automatically to a destination",
    "Use `walk to <destination>` or `walk <destination>` to automatically walk to a room.\n\nExamples:\n\n- `walk to lobby` - walk to the lobby\n- `walk dormitory` - walk to the dormitory\n- `walk second floor` - walk to a floor\n\nThe destination uses fuzzy matching, so partial names work.\n\nTo stop walking: `walk stop` or `walk cancel`\n\nNote: Auto-walk only works via passages (stairs, doors). If a destination requires an elevator or other transport, you'll be told what transport to use.",
    {"goto", "go_to", "autowalk"},
    "basics",
    {"movement", "join", "exits"}
  };
  property topic_who (owner: ARCH_WIZARD, flags: "rc") = {
    "who",
    "See who's online",
    "Use `who` to see a list of connected players and how long they've been idle.",
    {"players", "online"},
    "social",
    {}
  };

  override description (owner: ARCH_WIZARD, flags: "rc") = "Global help topics available everywhere in the system.";

  method help_topics owner: ARCH_WIZARD
    "Return global help topics for players.";
    "Topics are stored as topic_* properties: {name, summary, content, aliases, category, see_also}";
    {for_player, ?topic = ""} = args;
    props = properties(this);
    order = this.topic_order;
    if (typeof(order) != TYPE_LIST)
      order = {};
    endif
    seen = [];
    names = {};
    for p in (order)
      if (typeof(p) == TYPE_SYM && p in props)
        names = {@names, p};
        seen[p] = true;
      endif
    endfor
    for p in (props)
      if (typeof(p) == TYPE_SYM && p != 'topic_order && p:starts_with('topic_) && !maphaskey(seen, p))
        names = {@names, p};
      endif
    endfor
    my_topics = {};
    for prop in (names)
      data = this.(prop);
      if (typeof(data) != TYPE_LIST || length(data) < 3)
        continue;
      endif
      length(data) > 6 && raise(E_TYPE, "Malformed help topic " + tostr(prop) + ": expected at most 6 fields, got " + tostr(length(data)));
      name = data[1];
      summary = data[2];
      content = data[3];
      aliases = length(data) >= 4 ? data[4] | {};
      category = length(data) >= 5 ? data[5] | "general";
      see_also = length(data) >= 6 ? data[6] | {};
      if (typeof(name) != TYPE_STR || !name)
        continue;
      endif
      if (typeof(summary) != TYPE_STR)
        summary = tostr(summary);
      endif
      if (typeof(content) != TYPE_STR)
        content = tostr(content);
      endif
      typeof(aliases) == TYPE_LIST || (aliases = {});
      typeof(see_also) == TYPE_LIST || (see_also = {});
      t = $help:mk(name, summary, content, aliases, tosym(category), see_also, 'djot_blocks);
      if (topic == "")
        my_topics = {@my_topics, t};
      elseif (t:matches(topic))
        return t;
      endif
    endfor
    return topic == "" ? my_topics | 0;
  endmethod
endobject
