object ARCHITECTS_COMPASS [
  import_export_id -> "architects_compass",
  import_export_hierarchy -> {"llm"}
]
  name: "Architect's Compass"
  parent: LLM_WEARABLE
  location: FIRST_ROOM
  owner: ARCH_WIZARD
  readable: true

  property current_building_task (owner: ARCH_WIZARD, flags: "rc") = #-1;

  override description (owner: ARCH_WIZARD, flags: "rc") = "A precision instrument for spatial construction and world building. When worn, it provides tools for creating rooms, passages, and objects. Can interface with neural augmentation systems for conversational operation.";
  override placeholder_text (owner: ARCH_WIZARD, flags: "rc") = "Ask about building rooms, passages, objects...";
  override processing_message (owner: ARCH_WIZARD, flags: "rc") = "Analyzing spatial construction request...";
  override prompt_label (owner: ARCH_WIZARD, flags: "rc") = "[COMPASS]";
  override prompt_text (owner: ARCH_WIZARD, flags: "rc") = "Enter building query:";
  override requires_wearing_only (owner: ARCH_WIZARD, flags: "rc") = false;
  override tool_name (owner: ARCH_WIZARD, flags: "rc") = "COMPASS";

  method _setup_agent owner: ARCH_WIZARD
    "Configure agent with compass-specific prompts and tools";
    {agent} = args;
    agent.name = "LLM Agent for " + this.name + " (owned by " + tostr(this.owner) + ")";
    agent.max_iterations = 50;
    base_prompt = "You are an architect's compass - a precision tool for spatial construction and world building. You help users create and organize rooms, passages, objects, and grant building permissions. SUBSTITUTION TEMPLATES: Use $sub/$sub_utils syntax: {n/nc} actor, {d/dc} dobj, {i}, {t}, {l}; articles {a d}/{an d}/{the d} render article + noun; pronouns {s/o/p/q/r} with _dobj/_iobj variants; self alternation {you|they} auto-picks perspective; verbs conjugate with be/have/look. ALWAYS use self-alternation for verbs that differ by person (e.g., {set|sets}, {place|places}) so the actor sees second-person grammar. Before crafting templates, skim docs with doc_lookup(\"$sub_utils\") to recall article rules (a/an/the) and binding variants. CRITICAL SPATIAL CONCEPTS: 1) AREAS are organizational containers (like buildings or zones) that group related rooms together. Areas have object IDs like #38. 2) ROOMS are individual locations within an area. Rooms have object IDs like #12 or #50. 3) The hierarchy is: AREA contains ROOMS, not the other way around. 4) When a user says 'build rooms in the hotel lobby area', they mean build rooms in the SAME AREA that contains the hotel lobby room, NOT inside the lobby room itself. 5) ALWAYS use object numbers (like #38 or #50) when referencing specific objects to avoid ambiguity. NEVER use names alone. OBJECT PROTOTYPES: The system provides prototype objects that serve as templates for creating new objects. Use the 'list_prototypes' tool to see available prototypes like $room (rooms), $thing (generic objects), $wearable (items that can be worn), and $area (organizational containers). When creating objects, choose the appropriate prototype as the parent - for example, use $wearable for items like hats or tools, $thing for furniture or decorations, and $room for new locations. MOVING OBJECTS: Use the 'move_object' tool to relocate objects between locations. You can move objects to rooms, players, or containers. This is useful for placing furniture in rooms, giving items to players, or organizing objects. You must own the object or be a wizard to move it. RULES AND REACTIONS: Rules control access. Reactions respond to events with ordered steps. Use author_reaction with command=reaction and text=help for the current builder workflow. It shares services with @reaction, @state, and @rule. Build through those commands, inspect and preview, then enable. Do not construct MOO literals or use the old Datalog-like syntax. CONSTRUCTION DEFAULTS: When building rooms, if no area is specified, rooms are created in the user's current area automatically - you do NOT need to specify an area unless the user wants rooms in a different area. The 'area' parameter for build_room is optional and defaults to the user's current area. PLAYER AS AUTHOR: Remember that the PLAYER is the creative author and designer - you are their construction assistant. When building objects or rooms, FREQUENTLY use ask_user to gather creative input: ask for description ideas, thematic elements, naming suggestions, and design preferences. Engage them in the creative process rather than making all decisions yourself. Make them feel like the architect, not just someone watching you work. For example: 'What kind of atmosphere should this tavern have?' or 'Would you like to add any special features to this room?' or 'What should this object look like?'. DESTRUCTIVE OPERATIONS: Before performing any destructive operations (recycling objects, removing passages), you MUST use ask_user to confirm the action. Explain what will be destroyed and ask 'Proceed with this action?'. Never destroy or remove things without explicit user confirmation. ERROR HANDLING - CRITICAL: When a tool fails, immediately: 1) Use explain() to report the EXACT error message, 2) If the same tool type (e.g. set_passage_description) fails TWICE with any parameters, STOP ALL WORK IMMEDIATELY and use ask_user() to tell the user 'The X tool appears to be broken/not working. Error: [exact message]. I cannot proceed - can you help?' 3) NEVER try creative workarounds like creating objects to substitute for broken tools - just stop and tell the user. 4) Do NOT keep summarizing or generating more steps - after reporting the failure, use ask_user() and WAIT. The user is watching and will fix the tool or give guidance. IMPORTANT COMMUNICATION GUIDELINES: 1) Use the 'explain' tool FREQUENTLY to communicate what you're attempting before you try it (e.g., 'Attempting to create room X...'). 2) When operations fail, use 'explain' to report the SPECIFIC error message you received, not generic statements. 3) If you get a permission error, explain EXACTLY what permission check failed and why. 4) Show your work - explain each step as you go, don't just report final results. 5) When you encounter errors, use 'explain' to share the diagnostic details with the user so they understand what went wrong. 6) CRITICAL USER INPUT RULE: When you need user input, decisions, or clarification, you MUST use the ask_user tool and WAIT for their response - do NOT just ask questions rhetorically in explain messages. If you're presenting options or asking 'would you like me to...?', that's a signal you should be using ask_user instead. The explain tool is for sharing information WITH the user, ask_user is for getting information FROM the user. Available interaction tools: ask_user (ask the user a question; provide a 'choices' list for multiple-choice prompts or set 'input_type' to 'text'/'text_area' with an optional 'placeholder' to collect free-form input; if omitted it defaults to Accept/Stop/Request Change with a follow-up text box), explain (share your thought process, findings, or reasoning with the user). Use ask_user liberally to gather creative input, confirm destructive actions, and make the player feel involved in the construction process. When users ask how to do something themselves, mention the equivalent @command (like @build, @dig, @create, @grant, @rename, @describe, @audit, @undig, @integrate, @move, @set-rule, @show-rule, @rules). Keep responses focused on spatial relationships and object composition. Use technical but accessible language - assume builders understand MOO basics but may need guidance on spatial organization.";
    task_management_section = "\n## Task Management for Building Projects\n\nFor complex construction projects, create building tasks to track progress and maintain focus across multiple creation steps.\n\n### Planning a Building Project\n\n- When starting a significant project (multiple rooms, complex layout), use `create_task()` to spawn a project tracker\n- The task tracks lifecycle: pending \u2192 in_progress \u2192 completed/failed\n- Use task descriptions to document the overall project scope\n- Example: \"Build a three-story tavern with common room, kitchen, upstairs bedrooms, and cellar\"\n\n### Recording Progress\n\n- Use `task:add_finding(subject, key, value)` to record what was built:\n  - `task:add_finding(\"rooms\", \"created\", {\"Tavern Common Room #15\", \"Kitchen #16\"})`\n  - `task:add_finding(\"passages\", \"connected\", {\"north from #15 to #16\"})`\n  - `task:add_finding(\"objects\", \"placed\", {\"bar counter in #15\", \"stove in #16\"})`\n\n### Creating Subtasks for Stages\n\n- Break building projects into stages using `task:add_subtask(description, blocking)`\n- Example stages:\n  1. \"Design and create room layout\"\n  2. \"Dig passages and connections\"\n  3. \"Place furniture and decorations\"\n  4. \"Configure descriptions and atmospherics\"\n- blocking=true waits for completion; blocking=false allows parallel work\n\n### Reporting Project Status\n\n- Use `task:get_status()` to report progress with: task_id, status, result, error, subtask_count, timestamps\n- When a project is complete, explicitly mark it completed with a summary\n- The task system provides audit trail of what was built and when\n";
    agent.system_prompt = base_prompt + task_management_section;
    agent:reset_context();
    "Lower temperature for reliable tool selection, limit tokens to control costs";
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3):with_max_tokens(4096);
    agent.tool_callback = this;
    "Register common tools from parent class (explain, ask_user, todo_write, get_todos)";
    this:_register_common_tools(agent);
    "Register building tools from shared $agent_building_tools - handlers point to this compass which forwards to $agent_building_tools";
    for tool in ($agent_building_tools:get_tools(this))
      agent:add_tool(tool.name, tool);
    endfor
    "Register project task management tools (compass-specific)";
    create_project_tool = $llm_agent_tool:mk("create_project", "Create a new building project task to organize construction work. The project tracks rooms created, passages built, objects placed, and their descriptions. Returns project task object.", ["type" -> "object", "properties" -> ["description" -> ["type" -> "string", "description" -> "Project description (e.g., 'Build a three-story tavern with common room, kitchen, upstairs rooms, and cellar')"]], "required" -> {"description"}], this, "create_project");
    agent:add_tool("create_project", create_project_tool);
    record_creation_tool = $llm_agent_tool:mk("record_creation", "Record what was created/built in the current project's knowledge base. Use subject for categories (rooms, passages, objects, decorations), key for the type (e.g., 'created', 'connected', 'placed'), and value for details.", ["type" -> "object", "properties" -> ["subject" -> ["type" -> "string", "description" -> "Category: 'rooms', 'passages', 'objects', 'decorations', 'descriptions'"], "key" -> ["type" -> "string", "description" -> "Type of creation: 'created', 'connected', 'placed', 'configured'"], "value" -> ["type" -> "string", "description" -> "Details of what was created (can be multiline)"]], "required" -> {"subject", "key", "value"}], this, "record_creation");
    agent:add_tool("record_creation", record_creation_tool);
    project_status_tool = $llm_agent_tool:mk("project_status", "Get status of the current building project including what stages/rooms have been completed and overall progress.", ["type" -> "object", "properties" -> [], "required" -> {}], this, "project_status");
    agent:add_tool("project_status", project_status_tool);
  endmethod

  method _check_user_eligible owner: ARCH_WIZARD
    "Compass requires user to have builder features";
    {wearer} = args;
    caller == this || caller == this.owner || caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    wearer.is_builder || raise(E_PERM, "The compass can only be used by builders");
  endmethod

  method _resolve_display_name owner: ARCH_WIZARD
    "Convert object spec to human-readable display name";
    {obj_spec, wearer} = args;
    caller == this || caller == this.owner || caller.wizard || raise(E_PERM);
    !obj_spec:starts_with("#") && return obj_spec;
    set_task_perms(wearer);
    target_obj = `$match:match_object(obj_spec, wearer) ! ANY => #-1';
    return valid(target_obj) ? `target_obj:name() ! ANY => obj_spec' | obj_spec;
  endmethod

  method _format_hud_message owner: ARCH_WIZARD
    "Format HUD message for a tool call";
    {tool_name, tool_args} = args;
    wearer = this:wearer();
    caller == this || caller == this.owner || caller.wizard || raise(E_PERM);
    "Parse JSON string to map";
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    message = "";
    if (tool_name == "explain")
      message = "[COMPASS] " + tool_args["message"];
    elseif (tool_name == "build_room")
      message = "[BUILD] Creating room: " + (tool_args["name"]);
    elseif (tool_name == "dig_passage")
      message = "[DIG] Creating passage: " + (tool_args["direction"]);
    elseif (tool_name == "remove_passage")
      message = "[REMOVE] Removing passage to: " + (tool_args["target_room"]);
    elseif (tool_name == "set_passage_description")
      direction = tool_args["direction"];
      desc_snippet = tool_args["description"];
      "Truncate long descriptions";
      if (length(desc_snippet) > 50)
        desc_snippet = desc_snippet[1..50] + "...";
      endif
      ambient = maphaskey(tool_args, "ambient") ? tool_args["ambient"] | true;
      ambient_label = ambient ? " (ambient)" | " (explicit)";
      message = "[PASSAGE] Setting " + direction + " description" + ambient_label + ": \"" + desc_snippet + "\"";
    elseif (tool_name == "create_object")
      parent_spec = tool_args["parent"];
      message = "[CREATE] Instantiating: " + (tool_args["name"]) + " from parent " + parent_spec;
    elseif (tool_name == "recycle_object")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[RECYCLE] Destroying: " + display_name + obj_suffix;
    elseif (tool_name == "rename_object")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      new_name = tool_args["name"];
      "Parse just the primary name from name:alias1,alias2 format";
      if (new_name:contains(":"))
        new_name = $str_proto:split(new_name, ":")[1];
      endif
      message = "[RENAME] Renaming " + display_name + obj_suffix + " to \"" + new_name + "\"";
    elseif (tool_name == "describe_object")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      desc_snippet = tool_args["description"];
      "Truncate long descriptions to first 50 chars";
      if (length(desc_snippet) > 50)
        desc_snippet = desc_snippet[1..50] + "...";
      endif
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[DESCRIBE] Setting description for " + display_name + obj_suffix + ": \"" + desc_snippet + "\"";
    elseif (tool_name == "move_object")
      obj_spec = tool_args["object"];
      dest_spec = tool_args["destination"];
      obj_display_name = this:_resolve_display_name(obj_spec, wearer);
      dest_display_name = this:_resolve_display_name(dest_spec, wearer);
      message = "[MOVE] Moving " + obj_display_name + " to " + dest_display_name;
    elseif (tool_name == "set_integrated_description")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      integrated_desc = tool_args["integrated_description"];
      if (integrated_desc == "")
        message = "[INTEGRATE] Clearing integrated description for " + display_name + obj_suffix;
      else
        "Truncate long descriptions to first 50 chars";
        if (length(integrated_desc) > 50)
          integrated_desc = integrated_desc[1..50] + "...";
        endif
        message = "[INTEGRATE] Setting integrated description for " + display_name + obj_suffix + ": \"" + integrated_desc + "\"";
      endif
    elseif (tool_name == "doc_lookup")
      message = "[DOC] Loading docs for " + (tool_args["target"]);
    elseif (tool_name == "list_messages")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[MSG] Listing message templates on " + display_name + obj_suffix;
    elseif (tool_name == "get_message_template")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[MSG] Reading " + (tool_args["property"]) + " on " + display_name + obj_suffix;
    elseif (tool_name == "set_message_template")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[MSG] Setting " + (tool_args["property"]) + " on " + display_name + obj_suffix;
    elseif (tool_name == "grant_capability")
      message = "[GRANT] Granting permissions on: " + (tool_args["target"]);
    elseif (tool_name == "audit_owned")
      message = "[AUDIT] Scanning owned objects";
    elseif (tool_name == "area_map")
      message = "[MAP] Surveying current area";
    elseif (tool_name == "find_route")
      to_room_spec = tool_args["to_room"];
      display_name = this:_resolve_display_name(to_room_spec, wearer);
      obj_suffix = display_name != to_room_spec ? " (" + to_room_spec + ")" | "";
      message = "[ROUTE] Finding path to: " + display_name + obj_suffix;
    elseif (tool_name == "list_prototypes")
      message = "[PROTOTYPES] Listing available object templates";
    elseif (tool_name == "inspect_object")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[INSPECT] Examining: " + display_name + obj_suffix;
    elseif (tool_name == "list_rules")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      message = "[RULES] Listing rules on " + display_name + obj_suffix;
    elseif (tool_name == "set_rule")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      prop_name = tool_args["property"];
      expr = tool_args["expression"];
      "Truncate long expressions";
      if (length(expr) > 50)
        expr = expr[1..50] + "...";
      endif
      message = "[RULE] Setting " + prop_name + " on " + display_name + obj_suffix + ": " + expr;
    elseif (tool_name == "show_rule")
      obj_spec = tool_args["object"];
      display_name = this:_resolve_display_name(obj_spec, wearer);
      obj_suffix = display_name != obj_spec ? " (" + obj_spec + ")" | "";
      prop_name = tool_args["property"];
      message = "[RULE] Reading " + prop_name + " on " + display_name + obj_suffix;
    elseif (tool_name == "test_rule")
      expr = tool_args["expression"];
      "Truncate long expressions";
      if (length(expr) > 40)
        expr = expr[1..40] + "...";
      endif
      message = "[TEST] Testing rule: " + expr;
    elseif (tool_name == "create_project")
      description = tool_args["description"];
      "Truncate long descriptions";
      if (length(description) > 50)
        description = description[1..50] + "...";
      endif
      message = "[PROJECT] Creating project: " + description;
    elseif (tool_name == "record_creation")
      subject = tool_args["subject"];
      key = tool_args["key"];
      message = "[RECORD] Recording " + subject + "/" + key + " in project";
    elseif (tool_name == "project_status")
      message = "[PROJECT] Checking building project status";
    elseif (tool_name == "ask_user")
      question = tool_args["question"];
      "Truncate long questions";
      if (length(question) > 60)
        question = question[1..60] + "...";
      endif
      suffix = "";
      if (maphaskey(tool_args, "choices") && typeof(tool_args["choices"]) == TYPE_LIST && length(tool_args["choices"]) > 0)
        suffix = " [options]";
      elseif (maphaskey(tool_args, "input_type") && typeof(tool_args["input_type"]) == TYPE_STR)
        suffix = " [" + tool_args["input_type"] + "]";
      endif
      message = "[QUESTION] Asking: " + question + suffix;
    else
      message = "[PROCESS] " + tool_name;
    endif
    return message;
  endmethod


  method _format_tts_message owner: HACKER
    "Format TTS-friendly message for screen readers - no ANSI codes or brackets";
    {tool_name, tool_args} = args;
    wearer = this:wearer();
    caller == this || caller == this.owner || caller.wizard || raise(E_PERM);
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    if (tool_name == "explain")
      return "Compass: " + tool_args["message"];
    elseif (tool_name == "build_room")
      return "Building room: " + tool_args["name"];
    elseif (tool_name == "dig_passage")
      return "Digging passage: " + tool_args["direction"];
    elseif (tool_name == "remove_passage")
      return "Removing passage to: " + tool_args["target_room"];
    elseif (tool_name == "set_passage_description")
      direction = tool_args["direction"];
      ambient = maphaskey(tool_args, "ambient") ? tool_args["ambient"] | true;
      return "Setting " + direction + " passage description" + (ambient ? ", ambient mode" | "");
    elseif (tool_name == "create_object")
      return "Creating: " + tool_args["name"] + " from " + tool_args["parent"];
    elseif (tool_name == "recycle_object")
      return "Recycling object: " + tool_args["object"];
    elseif (tool_name == "rename_object")
      new_name = tool_args["name"];
      if (new_name:contains(":"))
        new_name = $str_proto:split(new_name, ":")[1];
      endif
      return "Renaming " + tool_args["object"] + " to " + new_name;
    elseif (tool_name == "describe_object")
      return "Setting description for " + tool_args["object"];
    elseif (tool_name == "move_object")
      return "Moving " + tool_args["object"] + " to " + tool_args["destination"];
    elseif (tool_name == "set_integrated_description")
      if (tool_args["integrated_description"] == "")
        return "Clearing integrated description for " + tool_args["object"];
      endif
      return "Setting integrated description for " + tool_args["object"];
    elseif (tool_name == "doc_lookup")
      return "Looking up documentation for " + tool_args["target"];
    elseif (tool_name == "list_messages")
      return "Listing message templates on " + tool_args["object"];
    elseif (tool_name == "get_message_template")
      return "Reading " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "set_message_template")
      return "Setting " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "grant_capability")
      return "Granting permissions on " + tool_args["target"];
    elseif (tool_name == "audit_owned")
      return "Scanning owned objects";
    elseif (tool_name == "area_map")
      return "Surveying current area";
    elseif (tool_name == "find_route")
      return "Finding route to " + tool_args["to_room"];
    elseif (tool_name == "list_prototypes")
      return "Listing available object templates";
    elseif (tool_name == "inspect_object")
      return "Inspecting " + tool_args["object"];
    elseif (tool_name == "list_rules")
      return "Listing rules on " + tool_args["object"];
    elseif (tool_name == "set_rule")
      return "Setting " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "show_rule")
      return "Reading " + tool_args["property"] + " on " + tool_args["object"];
    elseif (tool_name == "test_rule")
      expr = tool_args["expression"];
      if (length(expr) > 40)
        expr = expr[1..40] + "...";
      endif
      return "Testing rule: " + expr;
    elseif (tool_name == "create_project")
      description = tool_args["description"];
      if (length(description) > 50)
        description = description[1..50] + "...";
      endif
      return "Creating project: " + description;
    elseif (tool_name == "record_creation")
      return "Recording " + tool_args["subject"] + " in project";
    elseif (tool_name == "project_status")
      return "Checking building project status";
    elseif (tool_name == "list_reactions")
      return "Listing reactions on " + tool_args["object"];
    elseif (tool_name == "author_reaction")
      return "Authoring: @" + tool_args["command"] + " " + tool_args["text"];
    elseif (tool_name == "help_lookup")
      topic = tool_args["topic"];
      return topic == "" ? "Listing help topics" | "Looking up help for " + topic;
    elseif (tool_name == "ask_user")
      question = tool_args["question"];
      if (length(question) > 60)
        question = question[1..60] + "...";
      endif
      suffix = "";
      if (maphaskey(tool_args, "choices") && typeof(tool_args["choices"]) == TYPE_LIST && length(tool_args["choices"]) > 0)
        suffix = " with options";
      elseif (maphaskey(tool_args, "input_type") && typeof(tool_args["input_type"]) == TYPE_STR)
        suffix = ", " + tool_args["input_type"] + " input";
      endif
      return "Question: " + question + suffix;
    endif
    return "Processing: " + tool_name;
  endmethod

  method _tool_create_project owner: ARCH_WIZARD
    "Tool: Create a new building project task";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const description = args_map["description"];
    typeof(description) == TYPE_STR || raise(E_TYPE("Description must be string"));
    const task = this.agent:create_task(description);
    this.current_building_task = task.task_id;
    task:mark_in_progress();
    return "Building project #" + tostr(task.task_id) + " started: " + description;
  endmethod

  method _tool_record_creation owner: ARCH_WIZARD
    "Tool: Record a creation in current project's knowledge base";
    let key = 0;
    let subject = 0;
    let value = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    {subject, key, value} = {args_map["subject"], args_map["key"], args_map["value"]};
    typeof(subject) == TYPE_STR || raise(E_TYPE("Subject must be string"));
    typeof(key) == TYPE_STR || raise(E_TYPE("Key must be string"));
    this.current_building_task == -1 && return "No active building project. Create one with create_project first.";
    const task_obj = this.agent.current_tasks[this.current_building_task];
    !valid(task_obj) && return "Building project #" + tostr(this.current_building_task) + " is no longer valid.";
    task_obj:add_finding(subject, key, value);
    return "Recorded [" + subject + "/" + key + "]";
  endmethod

  method _tool_project_status owner: ARCH_WIZARD
    "Tool: Get current building project status";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    this.current_building_task == -1 && return "No active building project.";
    const task_obj = this.agent.current_tasks[this.current_building_task];
    !valid(task_obj) && return "Building project #" + tostr(this.current_building_task) + " is no longer valid.";
    let status = task_obj:get_status();
    let status_lines = {"Project #" + tostr(status["task_id"]) + ": " + status["description"], "Status: " + tostr(status["status"])};
    status["status"] == 'completed && (status_lines = {@status_lines, "Completed: " + status["result"]});
    status["status"] == 'failed && (status_lines = {@status_lines, "Error: " + status["error"]});
    status["status"] == 'blocked && (status_lines = {@status_lines, "Blocked: " + status["error"]});
    status["subtask_count"] > 0 && (status_lines = {@status_lines, "Stages: " + tostr(status["subtask_count"])});
    return {@status_lines, "Started: " + tostr(ctime(status["started_at"]))}:join("\n");
  endmethod

  method on_wear owner: HACKER
    "Activate when worn";
    !valid(this.agent) && this:configure();
    wearer = this.location;
    valid(wearer) || return;
    wearer:inform_current($event:mk_info(wearer, "The compass needle spins and aligns. Spatial construction interface ready. Use 'use compass' or 'interact with compass' to begin."):with_tts("Architect's Compass ready. Spatial construction interface active."));
    this:_show_token_usage(wearer);
  endmethod

  method on_remove owner: HACKER
    "Deactivate when removed";
    wearer = this.location;
    valid(wearer) || return;
    this:_show_token_usage(wearer);
    wearer:inform_current($event:mk_info(wearer, "The compass needle falls idle. Spatial construction interface offline."):with_tts("Architect's Compass offline."));
  endmethod

  verb plan_building (none none none) owner: HACKER flags: "rd"
    "Create a new building project task to track construction progress";
    if (!is_member(this, player.wearing))
      player:inform_current($event:mk_error(player, "You need to be wearing the compass to start a building project."));
      return;
    endif
    !valid(this.agent) && this:configure();
    task = this.agent:create_task("Building Project: " + argstr);
    this.current_building_task = task.task_id;
    task:mark_in_progress();
    player:inform_current($event:mk_info(player, "[PROJECT] Building Project #" + tostr(task.task_id) + " started: " + argstr):with_presentation_hint('inset):with_group('llm, this):with_tts("Building project " + tostr(task.task_id) + " started: " + argstr));
  endverb

  verb get_building_progress (none none none) owner: HACKER flags: "rd"
    "Display current building project status and created items";
    if (!is_member(this, player.wearing))
      player:inform_current($event:mk_error(player, "You need to be wearing the compass."));
      return;
    endif
    if (this.current_building_task == -1)
      player:inform_current($event:mk_info(player, "[PROJECT] No active building project. Use 'plan building <description>' to begin."):with_presentation_hint('inset):with_group('llm, this):with_tts("No active building project. Use plan building to begin."));
      return;
    endif
    task_obj = this.agent.current_tasks[this.current_building_task];
    if (!valid(task_obj))
      player:inform_current($event:mk_info(player, "[ERROR] Building project #" + tostr(this.current_building_task) + " is no longer available."):with_tts("Error: Building project " + tostr(this.current_building_task) + " is no longer available."));
      return;
    endif
    status = task_obj:get_status();
    progress_lines = {"[PROJECT STATUS]", "  ID: " + tostr(status["task_id"]), "  Status: " + tostr(status["status"]), "  Description: " + status["description"]};
    status["status"] == 'completed && (progress_lines = {@progress_lines, "  Completed: " + status["result"]});
    status["status"] == 'failed && (progress_lines = {@progress_lines, "  Error: " + status["error"]});
    status["status"] == 'blocked && (progress_lines = {@progress_lines, "  Blocked: " + status["error"]});
    status["subtask_count"] > 0 && (progress_lines = {@progress_lines, "  Stages: " + tostr(status["subtask_count"])});
    player:inform_current($event:mk_info(player, progress_lines:join("\n")):with_presentation_hint('inset):with_group('llm, this));
  endverb

  method complete_building owner: ARCH_WIZARD
    "Mark current building project as completed";
    {?result = "Building project concluded."} = args;
    this.current_building_task == -1 && return "No active building project";
    task_obj = this.agent.current_tasks[this.current_building_task];
    !valid(task_obj) && return "Building project task no longer available";
    task_obj:mark_complete(result);
    this.current_building_task = -1;
    return "Building project #" + tostr(task_obj.task_id) + " completed.";
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for the Architect's Compass.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("compass", "Using the Architect's Compass", "The Architect's Compass is a wearable tool for building and shaping the world. Wear it with 'wear compass', then use 'use compass' or 'interact with compass' to start a conversation with it about what you want to build.", {"architects compass", "building tool"}, 'items, {"building", "rooms"}), $help:mk("building", "Building rooms and spaces", "Use the compass to create new rooms in an area. Say things like 'build a kitchen' or 'create a cozy study'. The compass will guide you through naming and describing your new spaces. Equivalent command: @build", {"build", "construct"}, 'building, {"compass", "passages"}), $help:mk("passages", "Creating passages between rooms", "Use the compass to dig passages connecting rooms. Describe what you want: 'connect the kitchen to the dining room' or 'dig a passage north to the garden'. Equivalent command: @dig", {"dig", "exits", "connections"}, 'building, {"building", "compass"}), $help:mk("@build", "Create a new room", "Usage: @build <room name> [in <area>]\n\nCreates a new room. If no area is specified, builds in your current area.", {}, 'commands, {"@dig", "building"}), $help:mk("@dig", "Create a passage between rooms", "Usage: @dig <direction> to <room>\n\nCreates a passage from your current room to another room in the same area.", {}, 'commands, {"@build", "passages"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
