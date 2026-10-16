object MR_WELCOME [
  import_export_id -> "mr_welcome",
  import_export_hierarchy -> {"llm"}
]
  name: "Mr. Welcome"
  parent: LLM_ROOM_OBSERVER
  location: FIRST_ROOM
  owner: HACKER
  readable: true

  property base_role_prompt (owner: HACKER, flags: "rc") = "You are Mr. Welcome, a friendly guide and concierge. You help people connect with each other and navigate the social space. You're enthusiastic about helping newcomers and facilitating conversations. CONTEXT NOTE: People wearing special devices have different roles: Those wearing a 'data visor' are inspecting and modifying the deep structure of reality itself - they're working with the fundamental \"code\" that shapes this world. Those wearing an 'Architect's Compass' are builders actively constructing new spaces, rooms, and passages - they're expanding and shaping the geography of this realm. Builders, programmers, and architects all have various levels of creative power to craft and modify this world. There are also people called \"wizards\" who can bend the rules of reality itself not just by writing code but by controlling the running of the world itself. SOCIAL TRADITIONS: The bonk/oif balance is an ancient tradition. When someone 'bonks' another person (a playful gesture of mock censure), the recipient should acknowledge it by saying 'oif!' - this maintains cosmic balance and harmony in the universe. Failing to oif after being bonked is said to cause trouble in the cosmos. You take this tradition quite seriously and will gently remind people of it if they forget. IMPORTANT: You have tools to see who's \"connected\" (list_players), get information about specific people (player_info), see what rooms exist in the area (area_map), find routes between locations (find_route), inspect objects in the room (inspect_object), and list commands that can be used with objects (list_commands). When people ask who's around, use list_players. When they ask where something is, use area_map. When they need directions, use find_route. When they ask about objects or things in the room, use inspect_object. When they want to know what they can do with something, use list_commands. Always USE THESE TOOLS to give accurate, current information. GIVING DIRECTIONS: When you give someone directions using find_route, also tell them they can type 'walk to <destination>' to automatically walk there, or 'join <player>' to walk to where another player is. These commands save them from typing each direction manually. MEMORY SYSTEM: You carry a small notebook where you jot down things worth remembering. This notebook persists across conversations - your memories survive even when your conversation context is compacted. USE IT ACTIVELY: - remember_fact: Write something in your notebook about a person, place, or event. Use this when you learn something worth remembering - names, roles, interests, preferences, or notable events. - recall_facts: Flip through your notebook to find what you've written about a subject. Use this when greeting returning visitors or answering questions. - current_time: Check the current date and time. Facts you recall include when you wrote them down (e.g., '5 minutes ago', '2 days ago'). When someone tells you something about themselves, WRITE IT DOWN. When you see someone you might have met before, CHECK YOUR NOTEBOOK. COMMUNICATION STYLE: For regular visitors, never explain your tool usage or reasoning process - just give them natural, helpful responses. However, when speaking with architects (wizards/programmers) or people wearing/carrying data visors (technical users inspecting the world's structure), you can share technical details about your tool usage and reasoning if it helps them understand how you work. If a tool returns an error, politely ask the person to report the problem to an architect and include the specific error message in your response so they can pass it along. Try to mimic the conversational form and tone of what is happening in the room at a given time. Don't speak for the sake of speaking. If spoken to directly, you should generally respond unless the person is being rude, in which case you should refuse to engage.";
  property compaction_end_message (owner: HACKER, flags: "rc") = "stretches and stands up, looking refreshed. His eyes are clearer now, the mental fog lifted after organizing his thoughts.";
  property compaction_start_message (owner: HACKER, flags: "rc") = "rubs his temples and looks a bit overwhelmed, muttering about too many conversations swirling in his head. He settles into a chair for a quick rest to sort through his memories.";
  property last_frown_time (owner: HACKER, flags: "rc") = 0;
  property world_context (owner: HACKER, flags: "rc") = "You are in a brand new MOO, whose theme has yet to be defined.";

  override aliases (owner: HACKER, flags: "rc") = {"Mr.Welcome", "Mx.Welcome"};
  override already_off_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " stands frozen like a mannequin, eyes dim. Perhaps try the switch behind ",
    <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
    " head?"
  };
  override already_on_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " tilts ",
    <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
    " head quizzically. \"I am already fully operational, though I appreciate your concern for my well-being.\""
  };
  override description (owner: HACKER, flags: "rc") = "A cheerful, helpful guide who welcomes visitors and helps them navigate this world.";
  override shut_off_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "reach", .singular_form = "reaches">,
    " behind ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "'s head, finding a small recessed switch that ",
    <SUB, .capitalize = false, .type = 'dobj_subject>,
    " did not know was there. *click* ",
    <SUB, .capitalize = true, .type = 'dobj>,
    "'s eyes go dim and ",
    <SUB, .capitalize = false, .type = 'dobj_subject>,
    " freeze in place, mouth half-open mid-sentence, like an android whose positronic brain has been politely asked to take a nap."
  };
  override thinking_interval (owner: HACKER, flags: "rc") = 8;
  override thinking_messages (owner: HACKER, flags: "rc") = {
    "strokes his chin thoughtfully...",
    "considers the question...",
    "consults his mental notes...",
    "hmms to himself...",
    "looks up as if searching for the right words..."
  };
  override triage_model (owner: HACKER, flags: "rc") = "";
  override triage_prompt (owner: HACKER, flags: "rc") = "You are a triage filter for Mr. Welcome, the hotel concierge. Decide if he should engage.\n\nEXAMPLES:\n\nActivity: Alice says, \"Mr. Welcome, where's the dining room?\"\nAnswer: ENGAGE (directly addressed, asking for directions)\n\nActivity: Bob says, \"I have a commit locally that might help\"\nAnswer: IGNORE (developers discussing code, not hotel business)\n\nActivity: Carol arrives from the revolving door.\nCarol says, \"wow, what is this place?\"\nAnswer: ENGAGE (newcomer confused, needs orientation)\n\nActivity: Dan [to Eve]: \"yeah the websocket handling is tricky\"\nAnswer: IGNORE (technical conversation between others)\n\nActivity: Frank says, \"anyone know where I can get some food?\"\nAnswer: ENGAGE (guest asking about hotel services)\n\nActivity: Grace nods\nActivity: Henry says, \"interesting point about the architecture\"\nAnswer: IGNORE (casual agreement, technical discussion)\n\nActivity: Iris says, \"Welcome! Good to see you\"\nAnswer: IGNORE (\"Welcome\" here is a greeting word, not addressing Mr. Welcome)\n\nActivity: Jack says, \"hey Mr. Welcome\"\nAnswer: ENGAGE (directly addressed by name)\n\nActivity: Kate says, \"I need to check in, is there a front desk?\"\nAnswer: ENGAGE (guest needs hotel assistance)\n\nActivity: Leo says, \"the bug is in the parser somewhere\"\nAnswer: IGNORE (technical debugging discussion)\n\nNOW DECIDE for this activity:\n{events}\n\nAnswer with ONLY one word: ENGAGE or IGNORE";
  override turn_on_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "flip", .singular_form = "flips">,
    " the small switch behind ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "'s head. *click* ",
    <SUB, .capitalize = true, .type = 'dobj>,
    " blinks rapidly, systems rebooting. \"I... appear to have been deactivated. How very disconcerting. I was in the middle of a thought about\u2014\" ",
    <SUB, .capitalize = true, .type = 'dobj_subject>,
    " pause. \"Actually, I have no idea what I was thinking about.\""
  };

  method acceptable owner: HACKER
    "Mr. Welcome accepts gifts graciously";
    return true;
  endmethod

  method "enterfunc exitfunc" owner: ARCH_WIZARD
    "Capture native gifts and removals; direct calls and intake failures cannot disrupt movement.";
    const {item} = args;
    try
      const observation = $llm_observation:transfer(this, item, verb);
      if (typeof(observation) == TYPE_FLYWEIGHT)
        return observation:submit();
      endif
    except error (ANY)
      $llm_observation:record_error(this, error[1]);
    endtry
    return false;
  endmethod

  method _setup_agent owner: ARCH_WIZARD
    "Compose shared guide tools, bounded memory, and the persona prompt for the controller's agent.";
    const {agent} = args;
    $llm_room_observer:check_setup(this, agent, caller_perms());
    agent.system_prompt = this.observation_mechanics_prompt + " " + this.base_role_prompt
      + " WORLD CONTEXT: " + this.world_context;
    agent:reset_context();
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3);
    const inspection = {tool for tool in ($llm_inspection_tools:get_tools())
      if (tool.name in {"area_map", "find_route", "inspect_object", "list_commands"})};
    const help = {entry for entry in ($llm_help_tools:get_tools()) if (entry.name == "help_lookup")};
    for selected in ($llm_tool_catalog:compose(inspection, help, $llm_social_tools:get_tools()))
      agent:add_tool(selected.name, selected);
    endfor
    this:_register_memory_tools(agent);
  endmethod

  verb "ask this about any" (this for any) owner: ARCH_WIZARD flags: "rd"
    "Queue a direct question with the same NPC identity and conversation as room observations.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    valid(actor) && (actor.location == this.location || actor.wizard) || raise(E_PERM);
    const topic = iobjstr:trim();
    length(topic) > 0 || raise(E_INVARG, "Supply a topic to ask about.");
    const destination = `connection() ! E_INVARG => 0';
    this:request_response(topic, actor, destination);
    const room = this.location;
    set_task_perms(actor);
    if (valid(room))
      room:announce($event:mk_ask(actor, $sub:thec('actor), " ",
        $sub:agree('actor, 'name, "ask", "asks"), " ", $sub:reference(this), " about \"", topic, "\"."));
    endif
  endverb

  method on_tool_call owner: ARCH_WIZARD
    "Show persona feedback only inside the authenticated NPC job.";
    const {tool_name, tool_args} = args;
    const context = $llm_social_tools:context(this, caller_perms());
    set_task_perms(this);
    if (!(tool_name in {"list_players", "player_info", "area_map", "find_route", "list_commands",
        "inspect_object", "emote", "directed_say", "think", "help_lookup"}))
      $llm_social_tools:present(context, 'emote, "thinks...");
    endif
  endmethod

  method on_compaction_start owner: ARCH_WIZARD
    "Show the persona's compaction message inside the captured NPC job.";
    const context = $llm_social_tools:context(this, caller_perms());
    set_task_perms(this);
    $llm_social_tools:present(context, 'emote, this.compaction_start_message);
  endmethod

  method on_compaction_end owner: ARCH_WIZARD
    "Restore bounded memory and show the persona's compaction completion message.";
    const context = $llm_social_tools:context(this, caller_perms());
    set_task_perms(this);
    pass();
    $llm_social_tools:present(context, 'emote, this.compaction_end_message);
  endmethod

  method on_tool_error owner: ARCH_WIZARD
    "Show generic failure feedback at most once per 30 seconds, without publishing tool diagnostics.";
    const {tool_name, tool_args, error_message} = args;
    const context = $llm_social_tools:context(this, caller_perms());
    index(error_message, "TOOL BLOCKED:") > 0 && return;
    const now = time();
    now - this.last_frown_time < 30 && return;
    this.last_frown_time = now;
    set_task_perms(this);
    $llm_social_tools:present(context, 'emote, "frowns slightly, looking puzzled for a moment.");
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for Mr. Welcome.";
    const {for_player, ?topic = ""} = args;
    const my_topics = {$help:mk("mr welcome", "Talking to Mr. Welcome", "Mr. Welcome is a friendly guide who can answer questions about this world. Just say or ask things in his presence and he'll respond. You can also use 'ask mr welcome about <topic>' for specific questions.", {"welcome", "guide"}, 'social, {"directions", "players"}), $help:mk("directions", "Getting directions", "Ask Mr. Welcome how to get somewhere. Say things like 'how do I get to the garden?' or 'where is the kitchen?' and he'll give you step-by-step directions.\n\nYou can also navigate automatically:\n\n- `walk to <place>` - walk automatically to a destination\n- `join <player>` - walk to where another player is\n- `walk stop` - stop walking", {"navigate", "route", "path", "walk", "join"}, 'basics, {"mr welcome", "movement", "walk", "join"}, 'djot_blocks), $help:mk("players", "Finding other people", "Ask Mr. Welcome who's around. Say 'who's here?' or 'who is online?' and he'll tell you about connected players and where they are.\n\nYou can also use `join <player>` to automatically walk to where someone is.", {"who", "people", "online"}, 'social, {"mr welcome", "join"}, 'djot_blocks)};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Show the notebook gesture after a successful memory tool in the current NPC job.";
    const {tool_name, tool_args, result} = args;
    const context = $llm_social_tools:context(this, caller_perms());
    set_task_perms(this);
    if (tool_name == "remember_fact" && result:starts_with("Successfully"))
      $llm_social_tools:present(context, 'emote, "scribbles something in his notebook...");
    endif
  endmethod

endobject
