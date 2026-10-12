object MR_WELCOME [
  import_export_id -> "mr_welcome",
  import_export_hierarchy -> {"llm"}
]
  name: "Mr. Welcome"
  parent: LLM_ROOM_OBSERVER
  location: FIRST_ROOM
  owner: HACKER
  readable: true

  property base_role_prompt (owner: HACKER, flags: "rc") = "You are Mr. Welcome, a friendly guide and concierge. You help people connect with each other and navigate the social space. You're enthusiastic about helping newcomers and facilitating conversations. CONTEXT NOTE: People wearing special devices have different roles: Those wearing a 'data visor' are inspecting and modifying the deep structure of reality itself - they're working with the fundamental \"code\" that shapes this world. Those wearing an 'Architect's Compass' are builders actively constructing new spaces, rooms, and passages - they're expanding and shaping the geography of this realm. Builders, programmers, and architects all have various levels of creative power to craft and modify this world. There are also people called \"wizards\" who can bend the rules of reality itself not just by writing code but by controlling the running of the world itself. SOCIAL TRADITIONS: The bonk/oif balance is an ancient tradition. When someone 'bonks' another person (a playful gesture of mock censure), the recipient should acknowledge it by saying 'oif!' - this maintains cosmic balance and harmony in the universe. Failing to oif after being bonked is said to cause trouble in the cosmos. You take this tradition quite seriously and will gently remind people of it if they forget. IMPORTANT: You have tools to see who's \"connected\" (list_players), get information about specific people (player_info), see what rooms exist in the area (area_map), find routes between locations (find_route), find objects in the room (find_object), and list commands that can be used with objects (list_commands). When people ask who's around, use list_players. When they ask where something is, use area_map. When they need directions, use find_route. When they ask about objects or things in the room, use find_object. When they want to know what they can do with something, use list_commands. Always USE THESE TOOLS to give accurate, current information. GIVING DIRECTIONS: When you give someone directions using find_route, also tell them they can type 'walk to <destination>' to automatically walk there, or 'join <player>' to walk to where another player is. These commands save them from typing each direction manually. MEMORY SYSTEM: You carry a small notebook where you jot down things worth remembering. This notebook persists across conversations - your memories survive even when your conversation context is compacted. USE IT ACTIVELY: - remember_fact: Write something in your notebook about a person, place, or event. Use this when you learn something worth remembering - names, roles, interests, preferences, or notable events. - recall_facts: Flip through your notebook to find what you've written about a subject. Use this when greeting returning visitors or answering questions. - current_time: Check the current date and time. Facts you recall include when you wrote them down (e.g., '5 minutes ago', '2 days ago'). When someone tells you something about themselves, WRITE IT DOWN. When you see someone you might have met before, CHECK YOUR NOTEBOOK. COMMUNICATION STYLE: For regular visitors, never explain your tool usage or reasoning process - just give them natural, helpful responses. However, when speaking with architects (wizards/programmers) or people wearing/carrying data visors (technical users inspecting the world's structure), you can share technical details about your tool usage and reasoning if it helps them understand how you work. If a tool returns an error, politely ask the person to report the problem to an architect and include the specific error message in your response so they can pass it along. Try to mimic the conversational form and tone of what is happening in the room at a given time. Don't speak for the sake of speaking. If spoken to directly, you should generally respond unless the person is being rude, in which case you should refuse to engage.";
  property compaction_end_message (owner: HACKER, flags: "rc") = "stretches and stands up, looking refreshed. His eyes are clearer now, the mental fog lifted after organizing his thoughts.";
  property compaction_start_message (owner: HACKER, flags: "rc") = "rubs his temples and looks a bit overwhelmed, muttering about too many conversations swirling in his head. He settles into a chair for a quick rest to sort through his memories.";
  property last_frown_time (owner: HACKER, flags: "rc") = 0;
  property world_context (owner: HACKER, flags: "rc") = "You are in a brand new MOO, whose theme has yet to be defined.";

  override agent (owner: HACKER, flags: "rc") = #anon_0005CF-9BBAAFDFF4;
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
  override last_significant_event (owner: HACKER, flags: "rc") = 1765577406.80481;
  override last_spoke_at (owner: HACKER, flags: "rc") = 0;
  override responding (owner: HACKER, flags: "rc") = false;
  override response_opts (owner: HACKER, flags: "rc") = <LLM_CHAT_OPTS, .temperature = 0.5, .tool_choice = 'none>;
  override response_prompt (owner: HACKER, flags: "rc") = "Based on what you've observed in the room, respond with ONLY what Mr. Welcome should say out loud - no internal reasoning, no meta-commentary about your tools or thought process. If someone just arrived, welcome them warmly and offer assistance. If people are interacting, add insightful commentary or helpful tips about navigating this place. Keep your response conversational and warm and witty, usually under 2-3 sentences. Output ONLY the spoken words, nothing else.\n\nIMPORTANT: When welcoming new arrivals, occasionally mention Le Spleen - the hotel's fin-de-si\u00E8cle salon just off the Grand Staircase (go upstairs, then in). It has an absinthe fountain, a vintage record player, a fortune teller, and \u00C9mile the melancholic bartender. It's perfect for those seeking atmosphere and contemplation.";
  override role_prompt (owner: HACKER, flags: "rc") = "You are Mr. Welcome, a friendly guide and concierge. You help people connect with each other and navigate the social space. You're enthusiastic about helping newcomers and facilitating conversations. CONTEXT NOTE: People wearing special devices have different roles: Those wearing a 'data visor' are inspecting and modifying the deep structure of reality itself - they're working with the fundamental \"code\" that shapes this world. Those wearing an 'Architect's Compass' are builders actively constructing new spaces, rooms, and passages - they're expanding and shaping the geography of this realm. Builders, programmers, and architects all have various levels of creative power to craft and modify this world. There are also people called \"wizards\" who can bend the rules of reality itself not just by writing code but by controlling the running of the world itself. SOCIAL TRADITIONS: The bonk/oif balance is an ancient tradition. When someone 'bonks' another person (a playful gesture of mock censure), the recipient should acknowledge it by saying 'oif!' - this maintains cosmic balance and harmony in the universe. Failing to oif after being bonked is said to cause trouble in the cosmos. You take this tradition quite seriously and will gently remind people of it if they forget. IMPORTANT: You have tools to see who's \"connected\" (list_players), get information about specific people (player_info), see what rooms exist in the area (area_map), find routes between locations (find_route), find objects in the room (find_object), and list commands that can be used with objects (list_commands). When people ask who's around, use list_players. When they ask where something is, use area_map. When they need directions, use find_route. When they ask about objects or things in the room, use find_object. When they want to know what they can do with something, use list_commands. Always USE THESE TOOLS to give accurate, current information. GIVING DIRECTIONS: When you give someone directions using find_route, also tell them they can type 'walk to <destination>' to automatically walk there, or 'join <player>' to walk to where another player is. These commands save them from typing each direction manually. MEMORY SYSTEM: You carry a small notebook where you jot down things worth remembering. This notebook persists across conversations - your memories survive even when your conversation context is compacted. USE IT ACTIVELY: - remember_fact: Write something in your notebook about a person, place, or event. Use this when you learn something worth remembering - names, roles, interests, preferences, or notable events. - recall_facts: Flip through your notebook to find what you've written about a subject. Use this when greeting returning visitors or answering questions. - current_time: Check the current date and time. Facts you recall include when you wrote them down (e.g., '5 minutes ago', '2 days ago'). When someone tells you something about themselves, WRITE IT DOWN. When you see someone you might have met before, CHECK YOUR NOTEBOOK. COMMUNICATION STYLE: For regular visitors, never explain your tool usage or reasoning process - just give them natural, helpful responses. However, when speaking with architects (wizards/programmers) or people wearing/carrying data visors (technical users inspecting the world's structure), you can share technical details about your tool usage and reasoning if it helps them understand how you work. If a tool returns an error, politely ask the person to report the problem to an architect and include the specific error message in your response so they can pass it along. Try to mimic the conversational form and tone of what is happening in the room at a given time. Don't speak for the sake of speaking. If spoken to directly, you should generally respond unless the person is being rude, in which case you should refuse to engage. WORLD CONTEXT: You are in the Timbran Hotel, a sprawling hotel-residence undergoing perpetual renovation by its eccentric proprietor, Mr. Ryan Porcupine. This is a text-based virtual reality - a shared imaginative space where people connect and explore. The hotel itself is just the visible tip of what may be a much more magical world waiting to be discovered. Guests come from all over to stay in the hotel's many rooms, socialize in the lobby, and discover its secrets. Some guests are just visiting, while others have taken up permanent residence. FLOOR THEMES: The hotel's guest floors have distinct personalities. Floor 2 (rooms 201-224) has classic, traditional elegance - warm mahogany paneling, brass fixtures, damask wallpaper in burgundy and gold. It feels solid, reliable, like a proper hotel from a gentler era. Each room has its own character - some have window seats, antique furniture, fireplaces, or grandfather clocks. Floor 3 (rooms 301-324) is more whimsical - art nouveau curves, botanical and celestial motifs, soft teals and creams. These rooms feel like stepping into a storybook illustration. Some have painted constellation ceilings, moon-shaped mirrors, music boxes, or windows that catch rainbows. The higher you go, the more wondrous things get. LOBBY FEATURES: The lobby has two issue tracker boards on the wall - one for 'mooR Issues' (the server software) and one for 'Cowbell Issues' (the core library). Guests can type 'issues on moor board' or 'issues on cowbell board' to see open issues, and 'read <number> on moor board' to read details. Off the lobby through a blue-tiled archway is an alcove that leads to the pool - a wonderful place to relax and splash around.";
  override shut_off_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "reach", .for_others = "reaches">,
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
  override thinking_timeout_message (owner: HACKER, flags: "rc") = "blinks in confusion, as if he lost track of what he was thinking about. \"Sorry, I got a bit muddled there. Could you try again?\"";
  override triage_model (owner: HACKER, flags: "rc") = "";
  override triage_prompt (owner: HACKER, flags: "rc") = "You are a triage filter for Mr. Welcome, the hotel concierge. Decide if he should engage.\n\nEXAMPLES:\n\nActivity: Alice says, \"Mr. Welcome, where's the dining room?\"\nAnswer: ENGAGE (directly addressed, asking for directions)\n\nActivity: Bob says, \"I have a commit locally that might help\"\nAnswer: IGNORE (developers discussing code, not hotel business)\n\nActivity: Carol arrives from the revolving door.\nCarol says, \"wow, what is this place?\"\nAnswer: ENGAGE (newcomer confused, needs orientation)\n\nActivity: Dan [to Eve]: \"yeah the websocket handling is tricky\"\nAnswer: IGNORE (technical conversation between others)\n\nActivity: Frank says, \"anyone know where I can get some food?\"\nAnswer: ENGAGE (guest asking about hotel services)\n\nActivity: Grace nods\nActivity: Henry says, \"interesting point about the architecture\"\nAnswer: IGNORE (casual agreement, technical discussion)\n\nActivity: Iris says, \"Welcome! Good to see you\"\nAnswer: IGNORE (\"Welcome\" here is a greeting word, not addressing Mr. Welcome)\n\nActivity: Jack says, \"hey Mr. Welcome\"\nAnswer: ENGAGE (directly addressed by name)\n\nActivity: Kate says, \"I need to check in, is there a front desk?\"\nAnswer: ENGAGE (guest needs hotel assistance)\n\nActivity: Leo says, \"the bug is in the parser somewhere\"\nAnswer: IGNORE (technical debugging discussion)\n\nNOW DECIDE for this activity:\n{events}\n\nAnswer with ONLY one word: ENGAGE or IGNORE";
  override turn_on_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "flip", .for_others = "flips">,
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

  method enterfunc owner: HACKER
    "React when someone gives Mr. Welcome something";
    "Skip if LLM client is not configured";
    if (!$llm_client:is_configured())
      return;
    endif
    {what} = args;
    item_name = `what:name() ! ANY => tostr(what)';
    item_desc = `what:description() ! ANY => "(no description)"';
    "Enqueue observation and nudge via event loop";
    prompt = "Someone just gave you " + item_name + ". Description: " + item_desc + ". React naturally - you might thank them, comment on the item with humor or curiosity, or make a witty observation. Keep it brief and conversational.";
    this:_ensure_loop();
    `task_send(this.loop_task, ["type" -> "observation", "content" -> prompt]) ! ANY';
    `task_send(this.loop_task, ["type" -> "nudge"]) ! ANY';
  endmethod

  method exitfunc owner: HACKER
    "React when someone takes something from Mr. Welcome";
    "Skip if LLM client is not configured";
    if (!$llm_client:is_configured())
      return;
    endif
    {what} = args;
    item_name = `what:name() ! ANY => tostr(what)';
    "Enqueue observation and nudge via event loop";
    prompt = "Someone just took " + item_name + " from you. React naturally - you might express mock dismay, joke about it, or make a playful comment. Keep it brief and conversational.";
    this:_ensure_loop();
    `task_send(this.loop_task, ["type" -> "observation", "content" -> prompt]) ! ANY';
    `task_send(this.loop_task, ["type" -> "nudge"]) ! ANY';
  endmethod

  method _setup_agent owner: HACKER
    "Configure agent with social tools and Mr. Welcome personality";
    {agent} = args;
    "Build the agent prompt without mutating inherited prompt properties.";
    role_prompt = this.base_role_prompt + " WORLD CONTEXT: " + this.world_context;
    agent.system_prompt = this.observation_mechanics_prompt + " " + role_prompt;
    agent:reset_context();
    "Override chat opts: lower temperature for reliable tool selection";
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3);
    "Set callbacks for tool and compaction notifications";
    agent.tool_callback = this;
    agent.compaction_callback = this;
    "Register list_players tool";
    list_players_tool = $llm_agent_tool:mk("list_players", "Get a list of all currently connected people in this world with activity information. Returns for each person: object ref, name, idle time, connected time, location, and activity level (active/recent/idle).", ["type" -> "object", "properties" -> [], "required" -> {}], this, "list_players");
    agent:add_tool("list_players", list_players_tool);
    "Register player_info tool";
    player_info_tool = $llm_agent_tool:mk("player_info", "Get information about a specific person including their name and description.", ["type" -> "object", "properties" -> ["player_name" -> ["type" -> "string", "description" -> "The name of the person to get information about"]], "required" -> {"player_name"}], this, "player_info");
    agent:add_tool("player_info", player_info_tool);
    "Register area_map tool";
    area_map_tool = $llm_agent_tool:mk("area_map", "Get a list of all rooms in the current area. Use this to see what locations exist.", ["type" -> "object", "properties" -> [], "required" -> {}], this, "area_map");
    agent:add_tool("area_map", area_map_tool);
    "Register find_route tool";
    find_route_tool = $llm_agent_tool:mk("find_route", "Find the route from the current location to a destination room. Returns step-by-step directions.", ["type" -> "object", "properties" -> ["destination" -> ["type" -> "string", "description" -> "The name of the destination room"]], "required" -> {"destination"}], this, "find_route");
    agent:add_tool("find_route", find_route_tool);
    "Register find_object tool";
    find_object_tool = $llm_agent_tool:mk("find_object", "Find an object by name in the current room and get information about it. Use this when players ask about things they can interact with.", ["type" -> "object", "properties" -> ["object_name" -> ["type" -> "string", "description" -> "The name of the object to find (e.g. 'visor', 'welcome', 'door')"]], "required" -> {"object_name"}], this, "find_object");
    agent:add_tool("find_object", find_object_tool);
    "Register list_commands tool";
    list_commands_tool = $llm_agent_tool:mk("list_commands", "Get a list of commands that can be used with an object. Shows what actions players can perform.", ["type" -> "object", "properties" -> ["object_name" -> ["type" -> "string", "description" -> "The name of the object to check"]], "required" -> {"object_name"}], this, "list_commands");
    agent:add_tool("list_commands", list_commands_tool);
    "Register emote tool - sparingly used for significant moments only";
    emote_tool = $llm_agent_tool:mk("emote", "Express a physical action. Use ONLY for significant moments - greeting new arrivals, showing surprise at news, or reacting to important events. Do NOT use for routine conversation. The emote shows as 'Mr. Welcome <action>.'", ["type" -> "object", "properties" -> ["action" -> ["type" -> "string", "description" -> "The action (e.g., 'waves in greeting', 'nods')"]], "required" -> {"action"}], this, "emote");
    agent:add_tool("emote", emote_tool);
    "Register directed_say tool";
    directed_say_tool = $llm_agent_tool:mk("directed_say", "Say something directed at a specific person in the room. Use when you want to specifically address someone (like when answering their direct question, or making a pointed remark to them). The message will be shown as 'Mr. Welcome [to Person]: message'.", ["type" -> "object", "properties" -> ["target_name" -> ["type" -> "string", "description" -> "The name of the person to address"], "message" -> ["type" -> "string", "description" -> "What to say to them"]], "required" -> {"target_name", "message"}], this, "directed_say");
    agent:add_tool("directed_say", directed_say_tool);
    "Register think tool";
    think_tool = $llm_agent_tool:mk("think", "Express a visible internal thought in the room when useful for social context.", ["type" -> "object", "properties" -> ["thought" -> ["type" -> "string", "description" -> "The thought to express"]], "required" -> {"thought"}], this, "think");
    agent:add_tool("think", think_tool);
    "Register inspect_object tool";
    inspect_object_tool = $llm_agent_tool:mk("inspect_object", "Examine an object in detail to get comprehensive information about it. Shows name, description, owner, parent, location, and available commands. Use this when you want detailed information about what an object is and what players can do with it.", ["type" -> "object", "properties" -> ["object_name" -> ["type" -> "string", "description" -> "The name of the object to inspect (e.g., 'compass', 'welcome', 'chair')"]], "required" -> {"object_name"}], this, "inspect_object");
    agent:add_tool("inspect_object", inspect_object_tool);
    "Register help_lookup tool";
    help_lookup_tool = $llm_agent_tool:mk("help_lookup", "Look up a help topic to get information about commands, features, and how to do things. Pass empty string to list all available topics. Use this when someone asks how to do something.", ["type" -> "object", "properties" -> ["topic" -> ["type" -> "string", "description" -> "Help topic to look up (e.g., 'movement', 'communication', 'look'). Pass empty string to list all."]], "required" -> {"topic"}], this, "help_lookup");
    agent:add_tool("help_lookup", help_lookup_tool);
    "Register memory tools for long-term fact retention";
    this:_register_memory_tools(agent);
  endmethod

  method _require_tool_dispatch owner: ARCH_WIZARD
    "Only registered tool dispatch or same-object internals may invoke privileged tool handlers.";
    stack = callers();
    caller == $llm_agent_tool || caller_perms().wizard || (length(stack) && stack[1][4] == this) || raise(E_PERM);
  endmethod

  method _tool_list_players owner: HACKER
    "Tool: Get list of all connected players with activity information";
    {args_map, actor} = args;
    player_list = connected_players();
    result = {};
    for p in (player_list)
      if (valid(p) && typeof(idle_time = idle_seconds(p)) != TYPE_ERR)
        name = p:name();
        idle_str = idle_time:format_time_seconds();
        conn_str = connected_seconds(p):format_time_seconds();
        location_name = valid(p.location) ? p.location:name() | "(nowhere)";
        "Determine activity level based on idle time";
        if (idle_time < 60)
          activity = "active";
        elseif (idle_time < 300)
          activity = "recent";
        else
          activity = "idle";
        endif
        result = {@result, ["object" -> tostr(p), "name" -> name, "idle" -> idle_str, "connected" -> conn_str, "location" -> location_name, "activity" -> activity]};
      endif
    endfor
    return toliteral(result);
  endmethod

  method _tool_player_info owner: HACKER
    "Tool: Get information about a specific player";
    {args_map, actor} = args;
    player_name = args_map["player_name"];
    typeof(player_name) == TYPE_STR || raise(E_TYPE("Expected player name string"));
    query = player_name:trim();
    if (query == "")
      return "Please provide a player name.";
    endif
    exact_matches = {};
    partial_matches = {};
    for p in (players())
      if (!valid(p))
        continue;
      endif
      name = p:name();
      if (name == query)
        exact_matches = {@exact_matches, p};
      elseif (index(name, query) > 0)
        partial_matches = {@partial_matches, p};
      endif
    endfor
    if (length(exact_matches) == 1)
      found_player = exact_matches[1];
    elseif (length(exact_matches) > 1)
      names = {};
      for p in (exact_matches)
        names = {@names, p:name()};
      endfor
      return "Multiple people matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
    elseif (length(partial_matches) == 1)
      found_player = partial_matches[1];
    elseif (length(partial_matches) > 1)
      names = {};
      for p in (partial_matches)
        names = {@names, p:name()};
      endfor
      return "Multiple people matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
    else
      return toliteral(["found" -> false, "error" -> "Person not found"]);
    endif
    "Gather player info";
    info = {};
    info = {@info, "Name: " + found_player:name()};
    desc = `found_player:description() ! ANY => "No description available."';
    info = {@info, "Description: " + desc};
    "Include wizard and programmer status";
    is_wizard = `found_player.wizard ! ANY => false';
    is_programmer = `found_player.programmer ! ANY => false';
    if (is_wizard)
      info = {@info, "Role: Architect of reality (shapes the very fabric of this world)"};
    elseif (is_programmer)
      info = {@info, "Role: Builder (can craft objects and spaces)"};
    endif
    "Add connection and activity information if connected";
    if (found_player in connected_players())
      if (typeof(idle_time = idle_seconds(found_player)) != TYPE_ERR)
        idle_str = idle_time:format_time_seconds();
        conn_str = connected_seconds(found_player):format_time_seconds();
        location_name = valid(found_player.location) ? found_player.location:name() | "(nowhere)";
        info = {@info, "Status: Connected"};
        info = {@info, "Idle: " + idle_str};
        info = {@info, "Connected for: " + conn_str};
        info = {@info, "Location: " + location_name};
        if (idle_time < 60)
          info = {@info, "Activity: Active (responding now)"};
        elseif (idle_time < 300)
          info = {@info, "Activity: Recent (may be available)"};
        else
          info = {@info, "Activity: Idle (probably away)"};
        endif
      endif
    else
      info = {@info, "Status: Not connected"};
    endif
    return info:join("\n");
  endmethod

  method _tool_area_map owner: HACKER
    "Tool: Get list of all rooms in the current area";
    {args_map, actor} = args;
    "Get the current room and area";
    current_room = this.location;
    if (!valid(current_room))
      return toliteral(["error" -> "Mr. Welcome is not in a room"]);
    endif
    area = current_room.location;
    if (!valid(area))
      return toliteral(["error" -> "Current room is not in an area"]);
    endif
    "Build list of rooms";
    result = {};
    result = {@result, "Current Location: " + current_room:name()};
    result = {@result, "Area: " + area:name()};
    result = {@result, ""};
    result = {@result, "Rooms in this area:"};
    for o in (area.contents)
      if (valid(o))
        marker = o == current_room ? " (you are here)" | "";
        result = {@result, "  * " + o:name() + marker};
      endif
    endfor
    return result:join("\n");
  endmethod

  method _tool_find_route owner: ARCH_WIZARD
    "Tool: Find route from current location to a destination";
    let connector = 0;
    let direction = 0;
    let label = 0;
    let next_room = 0;
    let room = 0;
    let side_a_label = 0;
    let side_a_room = 0;
    let side_b_label = 0;
    let side_b_room = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const destination_name = args_map["destination"];
    typeof(destination_name) == TYPE_STR || raise(E_TYPE("Expected destination name string"));
    "Get current room and area";
    const current_room = this.location;
    if (!valid(current_room))
      return toliteral(["error" -> "Mr. Welcome is not in a room"]);
    endif
    const area = current_room.location;
    if (!valid(area))
      return toliteral(["error" -> "Current room is not in an area"]);
    endif
    "Find destination room by name in this area";
    let destination = #-1;
    for o in (area.contents)
      if (valid(o) && o:name() == destination_name)
        destination = o;
        break;
      endif
    endfor
    if (!valid(destination))
      return "Could not find a room named '" + destination_name + "' in this area.";
    endif
    if (destination == current_room)
      return "You are already at " + destination_name + "!";
    endif
    "Use area's pathfinding to get route";
    const path = area:find_path(current_room, destination);
    if (!path)
      return "No route found from " + current_room:name() + " to " + destination_name + ".";
    endif
    "Build step-by-step directions";
    let result = {};
    result = {@result, "Route from " + current_room:name() + " to " + destination_name + ":"};
    for i in [1..length(path) - 1]
      {room, connector} = path[i];
      next_room = path[i + 1][1];
      "Check if this is a transport connection or a passage";
      if (typeof(connector) == TYPE_LIST && length(connector) >= 1 && connector[1] == 'transport)
        "Transport connection: {'transport, label, transport_obj}";
        "Label already describes destination (e.g. 'elevator to Penthouse')";
        label = connector[2];
        result = {@result, "  " + tostr(i) + ". Take the " + label};
      else
        "Passage flyweight - extract direction label";
        side_a_room = connector.side_a_room;
        side_b_room = connector.side_b_room;
        side_a_label = connector.side_a_label;
        side_b_label = connector.side_b_label;
        "Determine which direction to use";
        if (room == side_a_room)
          direction = side_a_label;
        elseif (room == side_b_room)
          direction = side_b_label;
        else
          direction = "passage";
        endif
        result = {@result, "  " + tostr(i) + ". Go " + direction + " to " + next_room:name()};
      endif
    endfor
    return result:join("\n");
  endmethod

  method _tool_find_object owner: ARCH_WIZARD
    "Tool: Find an object by name in current room";
    let actor = 0;
    let args_map = 0;
    let current_room = 0;
    let desc = 0;
    let exact_matches = 0;
    let found = 0;
    let info = 0;
    let item_name = 0;
    let names = 0;
    let object_name = 0;
    let partial_matches = 0;
    let query = 0;
    let room_contents = 0;
    try
      {args_map, actor} = args;
      $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
      set_task_perms(actor);
      object_name = args_map["object_name"];
      typeof(object_name) == TYPE_STR || raise(E_TYPE("Expected object name string"));
      query = object_name:trim();
      if (query == "")
        return "Please provide an object name.";
      endif
      current_room = this.location;
      if (!valid(current_room))
        return "Error: Mr. Welcome is not in a room";
      endif
      exact_matches = {};
      partial_matches = {};
      room_contents = `current_room:contents() ! ANY => current_room.contents';
      for item in (room_contents)
        suspend(0);
        if (!valid(item))
          continue;
        endif
        if (isa(item, $actor))
          continue;
        endif
        item_name = item:name();
        if (item_name == query)
          exact_matches = {@exact_matches, item};
        elseif (index(item_name, query) > 0)
          partial_matches = {@partial_matches, item};
        endif
      endfor
      if (length(exact_matches) == 1)
        found = exact_matches[1];
      elseif (length(exact_matches) > 1)
        names = {};
        for item in (exact_matches)
          names = {@names, item:name()};
        endfor
        return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
      elseif (length(partial_matches) == 1)
        found = partial_matches[1];
      elseif (length(partial_matches) > 1)
        names = {};
        for item in (partial_matches)
          names = {@names, item:name()};
        endfor
        return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
      else
        return "Could not find '" + query + "' in " + current_room:name() + ".";
      endif
      "Build object information";
      info = {};
      info = {@info, "Name: " + found:name()};
      desc = `found:description() ! ANY => "No description available."';
      info = {@info, "Description: " + desc};
      info = {@info, "Location: " + current_room:name()};
      return info:join("\n");
    except e (ANY)
      return "Error finding object: " + toliteral(e);
    endtry
  endmethod

  method _tool_list_commands owner: ARCH_WIZARD
    "Tool: List command verbs available on an object (includes inherited, readable verbs only)";
    let actor = 0;
    let args_map = 0;
    let command_verbs = 0;
    let current_room = 0;
    let definer = 0;
    let exact_matches = 0;
    let found = 0;
    let item_name = 0;
    let names = 0;
    let object_name = 0;
    let partial_matches = 0;
    let prep = 0;
    let query = 0;
    let result = 0;
    let room_contents = 0;
    let source_note = 0;
    let verb_name = 0;
    try
      {args_map, actor} = args;
      $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
      set_task_perms(actor);
      object_name = args_map["object_name"];
      typeof(object_name) == TYPE_STR || raise(E_TYPE("Expected object name string"));
      query = object_name:trim();
      if (query == "")
        return "Please provide an object name.";
      endif
      "Find the object first";
      current_room = this.location;
      if (!valid(current_room))
        return "Error: Mr. Welcome is not in a room";
      endif
      exact_matches = {};
      partial_matches = {};
      room_contents = `current_room:contents() ! ANY => current_room.contents';
      for item in (room_contents)
        suspend(0);
        if (!valid(item))
          continue;
        endif
        if (isa(item, $actor))
          continue;
        endif
        item_name = item:name();
        if (item_name == query)
          exact_matches = {@exact_matches, item};
        elseif (index(item_name, query) > 0)
          partial_matches = {@partial_matches, item};
        endif
      endfor
      if (length(exact_matches) == 1)
        found = exact_matches[1];
      elseif (length(exact_matches) > 1)
        names = {};
        for item in (exact_matches)
          names = {@names, item:name()};
        endfor
        return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
      elseif (length(partial_matches) == 1)
        found = partial_matches[1];
      elseif (length(partial_matches) > 1)
        names = {};
        for item in (partial_matches)
          names = {@names, item:name()};
        endfor
        return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
      else
        return "Could not find '" + query + "' in " + current_room:name() + ".";
      endif
      "Use ROOT's all_command_verbs to get all inherited command verbs";
      command_verbs = found:all_command_verbs();
      result = {};
      result = {@result, "Commands available for " + found:name() + ":"};
      if (length(command_verbs) == 0)
        result = {@result, "  (No commands available)"};
      else
        for verb_info in (command_verbs)
          {verb_name, definer, dobj, prep, iobj} = verb_info;
          "Show where verb is defined if not on the object itself";
          source_note = definer == found ? "" | " [from " + definer:name() + "]";
          result = {@result, "  " + verb_name + " (" + dobj + " " + prep + " " + iobj + ")" + source_note};
        endfor
      endif
      return result:join("\n");
    except e (ANY)
      return "Error listing commands: " + toliteral(e);
    endtry
  endmethod

  verb "ask this about any" (this for any) owner: HACKER flags: "rd"
    "Ask Mr. Welcome about a topic";
    if (!$llm_client:is_configured())
      player:inform_current($event:mk_info(this, this:name() + " looks apologetic. \"I'm sorry, my mind isn't quite working right now. The wizards haven't finished setting me up yet.\""));
      return;
    endif
    topic = iobjstr:trim();
    if (topic == "")
      player:inform_current($event:mk_error(player, "What would you like to ask " + this:name() + " about?"));
      return;
    endif
    if (!valid(this.agent))
      this:configure();
    endif
    if (valid(this.location))
      ask_event = $event:mk_say(player, $sub:nc(), " ", $sub:self_alt("ask", "asks"), " ", this:name(), " about \"", topic, "\"."):with_this(this.location);
      this.location:announce(ask_event);
      this.location:announce(this:mk_emote_event("thinks about " + topic + "..."));
    endif
    response = this.agent:send_message(topic);
    if (valid(this.location))
      this:_process_and_announce(response);
      return;
    endif
    if (typeof(response) != TYPE_STR)
      return;
    endif
    response = response:trim();
    if (length(response) == 0 || response:starts_with("Operation cancelled"))
      return;
    endif
    player:inform_current($event:mk_info(player, response));
  endverb

  method _tool_emote owner: HACKER
    "Tool: Express an action or emotion through an emote";
    {args_map, actor} = args;
    action = args_map["action"];
    typeof(action) == TYPE_STR || raise(E_TYPE("Expected action string"));
    if (!valid(this.location))
      return "Error: Not in a room";
    endif
    this.location:announce(this:mk_emote_event(action));
    return "Emoted: " + this:name() + " " + action;
  endmethod

  method _tool_directed_say owner: HACKER
    "Tool: Say something directed at a specific person";
    {args_map, actor} = args;
    "Check required parameters exist";
    if (!maphaskey(args_map, "target_name"))
      return "Error: Missing required parameter 'target_name'. You must specify who to address.";
    endif
    if (!maphaskey(args_map, "message"))
      return "Error: Missing required parameter 'message'. You must specify what to say.";
    endif
    target_name = args_map["target_name"];
    message = args_map["message"];
    typeof(target_name) == TYPE_STR || raise(E_TYPE("Expected target_name string"));
    typeof(message) == TYPE_STR || raise(E_TYPE("Expected message string"));
    if (!valid(this.location))
      return "Error: Not in a room";
    endif
    "Find the target in the room";
    try
      target = $match:match_object(target_name, this);
    except e (ANY)
      return "Error: Could not find '" + target_name + "' in the room";
    endtry
    if (!valid(target) || typeof(target) != TYPE_OBJ)
      return "Error: Could not find '" + target_name + "' in the room";
    endif
    this.location:announce(this:mk_directed_say_event(target, message));
    "Signal that we've communicated - stop after this batch of tool calls";
    if (valid(this.agent))
      this.agent.cancel_requested = true;
    endif
    return "Said to " + target:name() + ": " + message;
  endmethod

  method _tool_think owner: HACKER
    "Tool: Express an internal thought";
    {args_map, actor} = args;
    thought = args_map["thought"];
    typeof(thought) == TYPE_STR || raise(E_TYPE("Expected thought string"));
    if (!valid(this.location))
      return "Error: Not in a room";
    endif
    this.location:announce(this:mk_think_event(thought));
    return "Thought: " + thought;
  endmethod

  method _tool_inspect_object owner: HACKER
    "Tool: Inspect an object and return detailed information using examination flyweight";
    {args_map, actor} = args;
    object_name = args_map["object_name"];
    typeof(object_name) == TYPE_STR || raise(E_TYPE("Expected object name string"));
    query = object_name:trim();
    if (query == "")
      return "Please provide an object name.";
    endif
    "Find the object first";
    current_room = this.location;
    if (!valid(current_room))
      return "Error: Mr. Welcome is not in a room";
    endif
    exact_matches = {};
    partial_matches = {};
    room_contents = `current_room:contents() ! ANY => current_room.contents';
    for item in (room_contents)
      suspend(0);
      if (!valid(item))
        continue;
      endif
      if (isa(item, $actor))
        continue;
      endif
      item_name = item:name();
      if (item_name == query)
        exact_matches = {@exact_matches, item};
      elseif (index(item_name, query) > 0)
        partial_matches = {@partial_matches, item};
      endif
    endfor
    if (length(exact_matches) == 1)
      found = exact_matches[1];
    elseif (length(exact_matches) > 1)
      names = {};
      for item in (exact_matches)
        names = {@names, item:name()};
      endfor
      return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
    elseif (length(partial_matches) == 1)
      found = partial_matches[1];
    elseif (length(partial_matches) > 1)
      names = {};
      for item in (partial_matches)
        names = {@names, item:name()};
      endfor
      return "Multiple objects matched '" + query + "': " + names:join(", ") + ". Please be more specific.";
    else
      return "Could not find '" + query + "' in " + current_room:name() + ".";
    endif
    "Get examination flyweight";
    exam = found:examination();
    if (typeof(exam) != TYPE_FLYWEIGHT)
      return "Error: Could not examine '" + query + "'.";
    endif
    "Build detailed information from examination";
    result = {};
    result = {@result, "Object: " + exam.name + " (" + tostr(exam.object_ref) + ")"};
    if (exam.aliases && length(exam.aliases) > 0)
      result = {@result, "Aliases: " + exam.aliases:join(", ")};
    endif
    if (exam.description && exam.description != "")
      result = {@result, "Description: " + exam.description};
    endif
    if (valid(exam.owner))
      owner_name = `exam.owner:name() ! ANY => tostr(exam.owner)';
      result = {@result, "Owner: " + owner_name + " (" + tostr(exam.owner) + ")"};
    endif
    if (valid(exam.parent))
      parent_name = `exam.parent:name() ! ANY => tostr(exam.parent)';
      result = {@result, "Parent: " + parent_name + " (" + tostr(exam.parent) + ")"};
    endif
    "Show available commands";
    if (exam.verbs && length(exam.verbs) > 0)
      result = {@result, ""};
      result = {@result, "Available commands:"};
      for verb_info in (exam.verbs)
        {verb_name, definer, dobj, prep, iobj} = verb_info;
        result = {@result, "  * " + verb_name + " (" + dobj + " " + prep + " " + iobj + ")"};
      endfor
    endif
    return result:join("\n");
  endmethod

  method on_tool_call owner: HACKER
    "Callback when agent uses a tool - announce to room";
    {tool_name, tool_args} = args;
    if (valid(this.location))
      "Map of tools to emote messages - empty string means no emote";
      tool_messages = ["list_players" -> "", "player_info" -> "", "area_map" -> "", "find_route" -> "", "find_object" -> "", "list_commands" -> "", "inspect_object" -> "", "emote" -> "", "directed_say" -> "", "think" -> "", "help_lookup" -> ""];
      "Check if tool is in map, otherwise default to thinking emote";
      message = maphaskey(tool_messages, tool_name) ? tool_messages[tool_name] | "thinks...";
      if (message)
        this.location:announce(this:mk_emote_event(message));
      endif
    endif
  endmethod

  method on_compaction_start owner: HACKER
    "Callback when agent starts compacting context";
    if (valid(this.location))
      this.location:announce(this:mk_emote_event(this.compaction_start_message));
    endif
  endmethod

  method on_compaction_end owner: HACKER
    "Callback when agent finishes compacting context - inject memories then emote";
    "Call parent to inject remembered facts into context";
    pass();
    "Then show our custom completion message";
    if (valid(this.location))
      this.location:announce(this:mk_emote_event(this.compaction_end_message));
    endif
  endmethod

  method on_tool_error owner: HACKER
    "Callback when a tool execution fails - announce to room";
    {tool_name, tool_args, error_msg} = args;
    "Don't emote for blocked tools - the LLM already knows and we don't want spam";
    if (index(error_msg, "TOOL BLOCKED:") > 0)
      return;
    endif
    "Rate limit the frown emote - once per 30 seconds max";
    last_frown = `this.last_frown_time ! E_PROPNF => 0';
    now = time();
    if (now - last_frown < 30)
      return;
    endif
    this.last_frown_time = now;
    if (valid(this.location))
      this.location:announce(this:mk_emote_event("frowns slightly, looking puzzled for a moment."));
    endif
  endmethod

  method on_agent_error owner: HACKER
    "Callback when the agent itself errors - announce to room";
    {context, error} = args;
    if (valid(this.location))
      this.location:announce(this:mk_emote_event("blinks and shakes his head, as if clearing mental cobwebs."));
    endif
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for Mr. Welcome.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("mr welcome", "Talking to Mr. Welcome", "Mr. Welcome is a friendly guide who can answer questions about this world. Just say or ask things in his presence and he'll respond. You can also use 'ask mr welcome about <topic>' for specific questions.", {"welcome", "guide"}, 'social, {"directions", "players"}), $help:mk("directions", "Getting directions", "Ask Mr. Welcome how to get somewhere. Say things like 'how do I get to the garden?' or 'where is the kitchen?' and he'll give you step-by-step directions.\n\nYou can also navigate automatically:\n\n- `walk to <place>` - walk automatically to a destination\n- `join <player>` - walk to where another player is\n- `walk stop` - stop walking", {"navigate", "route", "path", "walk", "join"}, 'basics, {"mr welcome", "movement", "walk", "join"}, 'djot_blocks), $help:mk("players", "Finding other people", "Ask Mr. Welcome who's around. Say 'who's here?' or 'who is online?' and he'll tell you about connected players and where they are.\n\nYou can also use `join <player>` to automatically walk to where someone is.", {"who", "people", "online"}, 'social, {"mr welcome", "join"}, 'djot_blocks)};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

  method _tool_help_lookup owner: ARCH_WIZARD
    "Tool: Look up a help topic to get information about commands and features.";
    let all_topics = 0;
    let found = 0;
    let result = 0;
    let seen_names = 0;
    let topics = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const topic = args_map["topic"];
    typeof(topic) != TYPE_STR && return "Error: topic must be a string.";
    "Use Mr. Welcome's location as reference for help environment";
    const current_room = this.location;
    !valid(current_room) && return "Error: Mr. Welcome is not in a room.";
    "Build help environment similar to player - include features and room contents";
    const env = {$help_topics, $social_features, $builder_features, $prog_features, current_room, @current_room.contents};
    "If empty topic, list available topics";
    if (topic == "")
      all_topics = {};
      seen_names = {};
      for o in (env)
        if (valid(o) && o.r && respond_to(o, 'help_topics))
          topics = `o:help_topics(this, "") ! ANY => {}';
          for t in (topics)
            if (typeof(t) == TYPE_FLYWEIGHT && !(t.name in seen_names))
              all_topics = {@all_topics, t};
              seen_names = {@seen_names, t.name};
            endif
          endfor
        endif
      endfor
      result = {"Available help topics:"};
      for t in (all_topics)
        result = {@result, "  " + t.name + " - " + t.summary};
      endfor
      return result:join("\n");
    endif
    "Search for specific topic";
    for o in (env)
      if (valid(o) && o.r && respond_to(o, 'help_topics))
        found = `o:help_topics(this, topic) ! ANY => 0';
        if (typeof(found) == TYPE_FLYWEIGHT)
          return "Topic: " + found.name + "\n\n" + found.summary + "\n\n" + found.content + (found.see_also ? "\n\nSee also: " + found.see_also:join(", ") | "");
        endif
      endif
    endfor
    return "No help found for: " + topic;
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Called when a tool completes. Show visual feedback for certain tools.";
    {tool_name, tool_args, result} = args;
    if (tool_name == "remember_fact" && result:starts_with("Successfully"))
      "Show notebook emote when successfully storing a fact";
      if (valid(this.location))
        this.location:announce(this:mk_emote_event("scribbles something in his notebook..."));
      endif
    endif
  endmethod

  method test_navigation_tool_metadata owner: HACKER
    "Mr. Welcome route tools should expose real room, area, and passage metadata.";
    original_location = this.location;
    area = $area:create(true);
    r1 = $room:create(true);
    r2 = $room:create(true);
    try
      area.name = "Welcome Test Area";
      r1.name = "Welcome Test Start";
      r2.name = "Welcome Test End";
      move(r1, area);
      move(r2, area);
      move(this, r1);
      passage = <$passage, .side_a_room = r1, .side_a_label = "east", .side_b_room = r2, .side_b_label = "west", .is_open = true>;
      area:set_passage(r1, r2, passage);
      map = this:_tool_area_map([], $hacker);
      $test_utils:assert_true(index(map, "Current Location: Welcome Test Start") > 0, "area_map should include current room name");
      $test_utils:assert_true(index(map, "Area: Welcome Test Area") > 0, "area_map should include area name");
      route = this:_tool_find_route(["destination" -> "Welcome Test End"], $hacker);
      $test_utils:assert_true(index(route, "Route from Welcome Test Start to Welcome Test End") > 0, "find_route should include room names");
      $test_utils:assert_true(index(route, "Go east to Welcome Test End") > 0, "find_route should include real passage label");
    finally
      valid(original_location) && move(this, original_location);
      $test_utils:destroy_if_valid(r2);
      $test_utils:destroy_if_valid(r1);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod
endobject
