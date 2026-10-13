object AGENT_BUILDING_TOOLS [
  import_export_id -> "agent_building_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "Agent Building Tools"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property guide (owner: ARCH_WIZARD, flags: "rc") = "SPATIAL CONCEPTS:\n\n1) AREAS are organizational containers (like buildings or zones) that group related rooms together. Areas have object IDs like #38.\n\n2) ROOMS are individual locations within an area. Rooms have object IDs like #12 or #50.\n\n3) The hierarchy is: AREA contains ROOMS, not the other way around.\n\n4) When a user says 'build rooms in the hotel lobby area', they mean build rooms in the SAME AREA that contains the hotel lobby room, NOT inside the lobby room itself.\n\n5) ALWAYS use object numbers (like #38 or #50) when referencing specific objects to avoid ambiguity. NEVER use names alone.\n\nOBJECT PROTOTYPES:\n\nThe system provides prototype objects that serve as templates for creating new objects. Use the 'list_prototypes' tool to see available prototypes like $room (rooms), $thing (generic objects), $wearable (items that can be worn), and $area (organizational containers). When creating objects, choose the appropriate prototype as the parent - for example, use $wearable for items like hats or tools, $thing for furniture or decorations, and $room for new locations.\n\nMOVING OBJECTS:\n\nUse the 'move_object' tool to relocate objects between locations. You can move objects to rooms, players, or containers. This is useful for placing furniture in rooms, giving items to players, or organizing objects. You must own the object or be a wizard to move it.\n\nRULES AND REACTIONS:\n\nRules check whether an operation is allowed. Reactions respond to an event with a condition and actions.\n\nUse list_rules and show_rule to inspect object policies. Use set_rule to edit them. A condition such as 'input.Key == #123' compares the supplied key with one specific object. Quoted text remains text.\n\nConditions support AND, OR, NOT, and parentheses. Predicates take named arguments, for example 'owner_is(object: input.This, owner: input.Accessor)'. The operation determines which inputs are available. Unknown inputs or predicates are errors.\n\nA query can select values for later conditions: 'occupants(location: input.Location) -> (object: Visitor) AND binding.Visitor != input.Actor'. Use doc_lookup(\"$rule_engine\") for the condition syntax.\n\nCONSTRUCTION DEFAULTS:\n\nWhen building rooms, if no area is specified, rooms are created in the user's current area automatically - you do NOT need to specify an area unless the user wants rooms in a different area. The 'area' parameter for build_room is optional and defaults to the user's current area.\n\nSUBSTITUTION TEMPLATES:\n\nUse $sub/$sub_utils syntax for dynamic messages: {n/nc} actor, {d/dc} dobj, {i}, {t}, {l}; articles {a d}/{an d}/{the d} render article + noun; pronouns {s/o/p/q/r} with _dobj/_iobj variants; self alternation {you|they} auto-picks perspective; verbs conjugate with be/have/look.\n\nALWAYS use self-alternation for verbs that differ by person (e.g., {set|sets}, {place|places}) so the actor sees second-person grammar.\n\nBefore crafting templates, use doc_lookup(\"$sub_utils\") to recall article rules and binding variants.";

  override description (owner: ARCH_WIZARD, flags: "rc") = "Shared tool definitions and handlers for AI agents that build and manipulate the world. Used by both in-world wearables (like $architects_compass) and external MCP agents.";

  method get_tools owner: ARCH_WIZARD
    "Return list of tool definitions for building/manipulation. All tools point to $agent_building_tools.";
    const target_obj = this;
    let tools = {};
    "Build/manipulation tools";
    tools = {@tools, $llm_agent_tool:mk("build_room", "Create a new room in an area. Areas are organizational containers that group rooms. IMPORTANT: The 'area' parameter must be an AREA object (like #38), NOT a room object. To build in the same area as an existing room, omit the area parameter or use 'here'.", ["type" -> "object", "properties" -> ["name" -> ["type" -> "string", "description" -> "Room name"], "area" -> ["type" -> "string", "description" -> "AREA object number to build in (like #38). MUST be an area, NOT a room. Use 'here' for current area, 'ether' for free-floating, or omit entirely to default to current area. NEVER pass a room object here."], "parent" -> ["type" -> "string", "description" -> "Parent room object reference (optional, default: $room)"]], "required" -> {"name"}], target_obj, "build_room")};
    tools = {@tools, $llm_agent_tool:mk("dig_passage", "Create a passage between two rooms. Can be one-way or bidirectional. ALWAYS use object numbers for room references.", ["type" -> "object", "properties" -> ["source_room" -> ["type" -> "string", "description" -> "Source room object number (optional, defaults to actor's current location). Use object numbers like #12 or #50"], "direction" -> ["type" -> "string", "description" -> "Exit direction from source room (e.g. 'north', 'up', 'north,n' for aliases)"], "target_room" -> ["type" -> "string", "description" -> "Destination room object number (like #12 or #50). MUST use object number."], "return_direction" -> ["type" -> "string", "description" -> "Return direction (optional, will be inferred if omitted)"], "oneway" -> ["type" -> "boolean", "description" -> "True for one-way passage (default: false)"]], "required" -> {"direction", "target_room"}], target_obj, "dig_passage")};
    tools = {@tools, $llm_agent_tool:mk("remove_passage", "Remove/delete a passage between two rooms. Use this to fix duplicate exits or remove unwanted connections.", ["type" -> "object", "properties" -> ["source_room" -> ["type" -> "string", "description" -> "Source room object number (optional, defaults to actor's current location)"], "target_room" -> ["type" -> "string", "description" -> "Target room object number to remove passage to"]], "required" -> {"target_room"}], target_obj, "remove_passage")};
    tools = {@tools, $llm_agent_tool:mk("set_passage_description", "Set the narrative description for a passage/exit. This description integrates into the room's look description when ambient mode is enabled.", ["type" -> "object", "properties" -> ["direction" -> ["type" -> "string", "description" -> "Direction/exit label (e.g. 'north', 'up')"], "description" -> ["type" -> "string", "description" -> "Narrative description for the passage (e.g. 'A dark archway opens to the north')"], "source_room" -> ["type" -> "string", "description" -> "Source room (optional, defaults to actor's current location)"], "ambient" -> ["type" -> "boolean", "description" -> "If true, description integrates into room description. If false, shows in exits list (default: true)"]], "required" -> {"direction", "description"}], target_obj, "set_passage_description")};
    tools = {@tools, $llm_agent_tool:mk("create_object", "Create a new object from a parent prototype.", ["type" -> "object", "properties" -> ["parent" -> ["type" -> "string", "description" -> "Parent object (e.g. '$thing', '$wearable')"], "name" -> ["type" -> "string", "description" -> "Primary name"], "aliases" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Optional alias names"]], "required" -> {"parent", "name"}], target_obj, "create_object")};
    tools = {@tools, $llm_agent_tool:mk("recycle_object", "Permanently destroy an object. Cannot be undone.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to recycle"]], "required" -> {"object"}], target_obj, "recycle_object")};
    tools = {@tools, $llm_agent_tool:mk("rename_object", "Change an object's name and aliases.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to rename"], "name" -> ["type" -> "string", "description" -> "New name (can include aliases like 'name:alias1,alias2')"]], "required" -> {"object", "name"}], target_obj, "rename_object")};
    tools = {@tools, $llm_agent_tool:mk("describe_object", "Set an object's description text.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to describe"], "description" -> ["type" -> "string", "description" -> "New description text"]], "required" -> {"object", "description"}], target_obj, "describe_object")};
    tools = {@tools, $llm_agent_tool:mk("move_object", "Move an object to a new location. Can move objects to rooms, players, or containers.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to move (name or object number)"], "destination" -> ["type" -> "string", "description" -> "Destination location (room, player, or container - name or object number)"]], "required" -> {"object", "destination"}], target_obj, "move_object")};
    tools = {@tools, $llm_agent_tool:mk("set_integrated_description", "Set an object's integrated description - a description that becomes part of the room's description when the object is present. Use this for atmospheric objects like furniture, decorations, or features that should feel like part of the room.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to set integrated description on"], "integrated_description" -> ["type" -> "string", "description" -> "The integrated description text (or empty string to clear)"]], "required" -> {"object", "integrated_description"}], target_obj, "set_integrated_description")};
    tools = {@tools, $llm_agent_tool:mk("grant_capability", "Grant building capabilities to a player.", ["type" -> "object", "properties" -> ["target" -> ["type" -> "string", "description" -> "Target object (area or room)"], "category" -> ["type" -> "string", "description" -> "Capability category ('area' or 'room')"], "permissions" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Permission symbols (e.g. ['add_room', 'create_passage'] for areas, ['dig_from', 'dig_into'] for rooms)"], "grantee" -> ["type" -> "string", "description" -> "Player to grant to"]], "required" -> {"target", "category", "permissions", "grantee"}], target_obj, "grant_capability")};
    "Verb manipulation tools";
    tools = {@tools, $llm_agent_tool:mk("add_verb", "Add a new verb to an object. Defaults to 'rd' (read/debug) for commands or 'rxd' (read/execute/debug) for methods.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to add verb to"], "verb" -> ["type" -> "string", "description" -> "Verb name"], "permissions" -> ["type" -> "string", "description" -> "Permission flags (optional, e.g. 'rd' or 'rxd')"], "dobj" -> ["type" -> "string", "description" -> "Direct object: this/any/none (default: this)"], "prep" -> ["type" -> "string", "description" -> "Preposition (default: none)"], "iobj" -> ["type" -> "string", "description" -> "Indirect object: this/any/none (default: none)"]], "required" -> {"object", "verb"}], target_obj, "add_verb")};
    tools = {@tools, $llm_agent_tool:mk("program_verb", "Set MOO code for a verb.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object containing verb"], "verb" -> ["type" -> "string", "description" -> "Verb name"], "code" -> ["type" -> "string", "description" -> "MOO code (use \\n for line breaks)"]], "required" -> {"object", "verb", "code"}], target_obj, "program_verb")};
    tools = {@tools, $llm_agent_tool:mk("list_verbs", "List verbs on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to list verbs from"]], "required" -> {"object"}], target_obj, "list_verbs")};
    tools = {@tools, $llm_agent_tool:mk("get_verb_code", "Get source code of a verb.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object containing verb"], "verb" -> ["type" -> "string", "description" -> "Verb name"]], "required" -> {"object", "verb"}], target_obj, "get_verb_code")};
    tools = {@tools, $llm_agent_tool:mk("delete_verb", "Delete a verb from an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"], "verb" -> ["type" -> "string", "description" -> "Verb name"]], "required" -> {"object", "verb"}], target_obj, "delete_verb")};
    tools = {@tools, $llm_agent_tool:mk("set_verb_info", "Set verb metadata (permissions, names, owner).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "verb" -> ["type" -> "string", "description" -> "Current verb name"], "permissions" -> ["type" -> "string", "description" -> "New flags (e.g. 'rd', 'rxd')"], "names" -> ["type" -> "string", "description" -> "New name(s) / aliases"], "owner" -> ["type" -> "string", "description" -> "New owner object"]], "required" -> {"object", "verb"}], target_obj, "set_verb_info")};
    tools = {@tools, $llm_agent_tool:mk("set_verb_args", "Set verb argument specification (dobj, prep, iobj).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "verb" -> ["type" -> "string", "description" -> "Verb name"], "dobj" -> ["type" -> "string", "description" -> "Direct object spec (this/any/none)"], "prep" -> ["type" -> "string", "description" -> "Preposition spec"], "iobj" -> ["type" -> "string", "description" -> "Indirect object spec (this/any/none)"]], "required" -> {"object", "verb", "dobj", "prep", "iobj"}], target_obj, "set_verb_args")};
    "Property tools";
    tools = {@tools, $llm_agent_tool:mk("list_properties", "List properties on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"]], "required" -> {"object"}], target_obj, "list_properties")};
    tools = {@tools, $llm_agent_tool:mk("get_property", "Get a property value.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"], "property" -> ["type" -> "string", "description" -> "Property name"]], "required" -> {"object", "property"}], target_obj, "get_property")};
    tools = {@tools, $llm_agent_tool:mk("set_property", "Set a property value.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"], "property" -> ["type" -> "string", "description" -> "Property name"], "value" -> ["description" -> "Value to set"]], "required" -> {"object", "property", "value"}], target_obj, "set_property")};
    tools = {@tools, $llm_agent_tool:mk("add_property", "Add a new property to an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"], "property" -> ["type" -> "string", "description" -> "Property name"], "value" -> ["description" -> "Initial value (optional)"]], "required" -> {"object", "property"}], target_obj, "add_property")};
    "Analysis/inspection tools";
    tools = {@tools, $llm_agent_tool:mk("audit_owned", "List all objects owned by the actor.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "audit_owned")};
    tools = {@tools, $llm_agent_tool:mk("area_map", "Get a list of all rooms in the current area. Use this to see what locations already exist and understand the spatial layout.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "area_map")};
    tools = {@tools, $llm_agent_tool:mk("find_route", "Find the route between two rooms in the same area. Shows step-by-step directions. Useful for understanding how rooms are connected.", ["type" -> "object", "properties" -> ["from_room" -> ["type" -> "string", "description" -> "Starting room (optional, defaults to actor's location)"], "to_room" -> ["type" -> "string", "description" -> "Destination room name or object number"]], "required" -> {"to_room"}], target_obj, "find_route")};
    tools = {@tools, $llm_agent_tool:mk("list_prototypes", "List available prototype objects that can be used as parents when creating objects. Shows $thing, $wearable, $room, etc. with descriptions of what each is for.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "list_prototypes")};
    tools = {@tools, $llm_agent_tool:mk("inspect_object", "Examine an object to see detailed information including name, description, parent, owner, location, and properties.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (name or object number)"]], "required" -> {"object"}], target_obj, "inspect_object")};
    "Rule tools";
    tools = {@tools, $llm_agent_tool:mk("list_rules", "List all rule properties (*_rule) on an object and their current expressions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#10', '$container', 'chest')"]], "required" -> {"object"}], target_obj, "list_rules")};
    tools = {@tools, $llm_agent_tool:mk("set_rule", "Set an access control rule on an object property. Rules are logical expressions like 'input.Key == #123' or 'NOT is_locked(object: input.This)'.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule, e.g., 'lock_rule')"], "expression" -> ["type" -> "string", "description" -> "Rule expression using Datalog syntax"]], "required" -> {"object", "property", "expression"}], target_obj, "set_rule")};
    tools = {@tools, $llm_agent_tool:mk("show_rule", "Display the current expression for a specific rule property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule)"]], "required" -> {"object", "property"}], target_obj, "show_rule")};
    tools = {@tools, $llm_agent_tool:mk("test_rule", "Test a rule expression with specific variable bindings to see if it evaluates successfully.", ["type" -> "object", "properties" -> ["expression" -> ["type" -> "string", "description" -> "Rule expression to test (e.g., 'input.Key == #123')"], "bindings" -> ["type" -> "object", "description" -> "Variable bindings as key-value pairs (e.g., {\"This\": \"#123\", \"Accessor\": \"#5\", \"Key\": \"#456\"})"]], "required" -> {"expression", "bindings"}], target_obj, "tool_test_rule")};
    "Reaction tools";
    tools = {@tools, $llm_agent_tool:mk("author_reaction",
      "Use the same reaction, state, and rule commands as a builder. Call with command=reaction and text=help first. "
        + "The help is generated from current definitions. New reactions start disabled. Inspect and preview before enabling.",
      ["type" -> "object", "properties" -> [
        "command" -> ["type" -> "string", "enum" -> {"reaction", "state", "rule", "facts"}],
        "text" -> ["type" -> "string", "description" -> "Arguments after @reaction, @state, @rule, or @facts. Do not include the command name."]],
        "required" -> {"command", "text"}], target_obj, "author_reaction")};
    tools = {@tools, $llm_agent_tool:mk("list_reactions", "List all reactions on an object with detailed information including triggers, conditions, effects, and enabled status.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#12', 'here', 'cupboard')"]], "required" -> {"object"}], target_obj, "list_reactions")};
    "Message template tools";
    tools = {@tools, $llm_agent_tool:mk("list_messages", "List message template properties (*_msg) and message bags (*_msg_bag/_msgs) on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#62', '$room', 'here')"]], "required" -> {"object"}], target_obj, "list_messages")};
    tools = {@tools, $llm_agent_tool:mk("get_message_template", "Show a single message template or list the entries of a message bag.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"]], "required" -> {"object", "property"}], target_obj, "get_message_template")};
    tools = {@tools, $llm_agent_tool:mk("set_message_template", "Set a message template on an object property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], target_obj, "set_message_template")};
    tools = {@tools, $llm_agent_tool:mk("add_message_template", "Append a message template to a message bag property (_msgs or _msg_bag).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], target_obj, "add_message_template")};
    tools = {@tools, $llm_agent_tool:mk("delete_message_template", "Remove a message entry by index from a message bag property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "index" -> ["type" -> "integer", "description" -> "1-based index to remove"]], "required" -> {"object", "property", "index"}], target_obj, "delete_message_template")};
    "Documentation tools";
    tools = {@tools, $llm_agent_tool:mk("doc_lookup", "Read developer documentation for an object, verb, or property. Use formats: obj, obj:verb, obj.property.", ["type" -> "object", "properties" -> ["target" -> ["type" -> "string", "description" -> "Object/verb/property reference, e.g., '$sub_utils', '#61:drop_msg', '#61.get_msg'"]], "required" -> {"target"}], target_obj, "doc_lookup")};
    tools = {@tools, $llm_agent_tool:mk("help_lookup", "Look up a help topic to get information about commands and features. Pass empty string to list all available topics.", ["type" -> "object", "properties" -> ["topic" -> ["type" -> "string", "description" -> "Help topic to look up (e.g., 'building', 'passages', '@build'). Pass empty string to list all."]], "required" -> {"topic"}], target_obj, "help_lookup")};
    return tools;
  endmethod

  method _parse_direction_spec owner: ARCH_WIZARD
    "Parse direction string into list (handles 'north:n' and 'north,n' formats)";
    const {dir_spec} = args;
    !dir_spec:contains(":") && return $str_proto:split(dir_spec, ",");
    const colon_parts = $str_proto:split(dir_spec, ":");
    length(colon_parts) < 2 && return {dir_spec};
    return {colon_parts[1], @$str_proto:split(colon_parts[2], ",")};
  endmethod

  method _require_tool_dispatch owner: ARCH_WIZARD
    "Validate the entry's captured principal before privileged tool effects. Caller ancestry does not grant authority.";
    const {actor, principal} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM, "Tool actor must be valid");
    principal == actor || principal.wizard || raise(E_PERM, "Tool caller cannot impersonate actor");
  endmethod

  method build_room owner: ARCH_WIZARD
    "Tool: Create a new room";
    let actual_area = 0;
    let area_spec = 0;
    let area_str = 0;
    let area_target = 0;
    let cap = 0;
    let current_room = 0;
    let new_room = 0;
    let parent_spec = 0;
    let room_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {room_name, area_spec, parent_spec} = {args_map["name"], maphaskey(args_map, "area") ? args_map["area"] | "", maphaskey(args_map, "parent") ? args_map["parent"] | "$room"};
    const parent_obj = this:_resolve_object(parent_spec, actor);
    typeof(parent_obj) == TYPE_OBJ || raise(E_INVARG, "Invalid parent object: " + tostr(parent_spec));
    "Parse area - default to current area if not specified, 'ether' means free-floating";
    let target_area = #-1;
    if (!area_spec || area_spec == "" || area_spec == "here")
      current_room = actor.location;
      valid(current_room) && (target_area = current_room.location);
    elseif (area_spec != "ether")
      target_area = this:_resolve_object(area_spec, actor);
      "Validate that target_area is actually an area, not a room";
      if (valid(target_area) && typeof(target_area) == TYPE_OBJ && valid(target_area.location) && target_area.location != #-1)
        actual_area = target_area.location;
        return "Error: " + tostr(target_area) + " is a room, not an area. To build in the same area as that room, omit the 'area' parameter or use 'here'. The area containing that room is " + tostr(actual_area) + ".";
      endif
    endif
    "Create room";
    if (!valid(target_area))
      new_room = parent_obj:create();
      area_str = " (free-floating)";
    else
      cap = actor:find_capability_for(target_area, 'area);
      area_target = typeof(cap) == TYPE_FLYWEIGHT ? cap | target_area;
      try
        new_room = area_target:make_room_in(parent_obj);
        area_str = " in " + tostr(target_area);
      except (E_PERM)
        return "Permission denied: " + $grant_utils:format_denial(target_area, 'area, {'add_room});
      endtry
    endif
    new_room:set_name_aliases(room_name, {});
    return "Created \"" + room_name + "\" (" + tostr(new_room) + ")" + area_str + ".";
  endmethod

  method dig_passage owner: ARCH_WIZARD
    "Tool: Create a passage between two rooms";
    let direction = 0;
    let oneway_flag = 0;
    let opposites = 0;
    let return_dir = 0;
    let source_spec = 0;
    let target_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {source_spec, direction, target_spec, return_dir, oneway_flag} = {maphaskey(args_map, "source_room") ? args_map["source_room"] | "", args_map["direction"], args_map["target_room"], maphaskey(args_map, "return_direction") ? args_map["return_direction"] | "", maphaskey(args_map, "oneway") ? args_map["oneway"] | false};
    "Parse direction string into list";
    let from_dirs = this:_parse_direction_spec(direction);
    from_dirs = $passage:expand_direction_aliases({@from_dirs});
    "Parse return direction";
    let to_dirs = {};
    if (!oneway_flag)
      if (return_dir)
        to_dirs = this:_parse_direction_spec(return_dir);
        to_dirs = $passage:expand_direction_aliases({@to_dirs});
      else
        "Infer opposite direction";
        opposites = ["north" -> "south", "south" -> "north", "east" -> "west", "west" -> "east", "up" -> "down", "down" -> "up", "in" -> "out", "out" -> "in"];
        maphaskey(opposites, from_dirs[1]) && (to_dirs = $passage:expand_direction_aliases({opposites[from_dirs[1]]}));
      endif
    endif
    "Find source room - default to actor's location if not specified";
    const source_room = this:_resolve_object(source_spec, actor, actor.location);
    typeof(source_room) == TYPE_OBJ || raise(E_INVARG, "Source room not found");
    valid(source_room) || raise(E_INVARG, "Source room no longer exists");
    const target_room = this:_resolve_object(target_spec, actor);
    typeof(target_room) == TYPE_OBJ || raise(E_INVARG, "Target room not found");
    valid(target_room) || raise(E_INVARG, "Target room no longer exists");
    "Get area - both rooms must be in the same area";
    const area = source_room.location;
    valid(area) || raise(E_INVARG, "Source room is not in an area");
    target_room.location == area || raise(E_INVARG, "Both rooms must be in the same area");
    "Check permissions";
    const from_cap = actor:find_capability_for(source_room, 'room);
    const from_target = typeof(from_cap) == TYPE_FLYWEIGHT ? from_cap | source_room;
    try
      from_target:check_can_dig_from();
    except (E_PERM)
      return "Permission denied: " + $grant_utils:format_denial(source_room, 'room, {'dig_from});
    endtry
    const to_cap = actor:find_capability_for(target_room, 'room);
    const to_target = typeof(to_cap) == TYPE_FLYWEIGHT ? to_cap | target_room;
    try
      to_target:check_can_dig_into();
    except (E_PERM)
      return "Permission denied: " + $grant_utils:format_denial(target_room, 'room, {'dig_into});
    endtry
    "Create passage";
    const passage = oneway_flag || !to_dirs ? $passage:mk(source_room, from_dirs[1], from_dirs, "", true, target_room, "", {}, "", false, true) | $passage:mk(source_room, from_dirs[1], from_dirs, "", true, target_room, to_dirs[1], to_dirs, "", true, true);
    "Register with area";
    const area_cap = actor:find_capability_for(area, 'area);
    const area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
    area_target:create_passage(from_target, to_target, passage);
    "Report";
    const msg = oneway_flag ? "Dug passage: " + from_dirs:join(",") + " to " + tostr(target_room) + " (one-way)." | !to_dirs ? "Dug passage: " + from_dirs:join(",") + " to " + tostr(target_room) + " (one-way - no return direction inferred)." | "Dug passage: " + from_dirs:join(",") + " | " + to_dirs:join(",") + " connecting to " + tostr(target_room) + ".";
    return msg;
  endmethod

  method remove_passage owner: ARCH_WIZARD
    "Tool: Remove a passage between two rooms";
    let side_a_room = 0;
    let side_b_room = 0;
    let source_room = 0;
    let source_spec = 0;
    let target_room = 0;
    let target_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {target_spec, source_spec} = {args_map["target_room"], maphaskey(args_map, "source_room") ? args_map["source_room"] | ""};
    "Resolve source_room - may be object, string, or empty";
    if (!source_spec || source_spec == "")
      source_room = actor.location;
    elseif (typeof(source_spec) == TYPE_OBJ)
      source_room = source_spec;
    else
      source_room = $match:match_object(source_spec, actor);
    endif
    typeof(source_room) == TYPE_OBJ || raise(E_INVARG, "Source room not found");
    valid(source_room) || raise(E_INVARG, "Source room no longer exists");
    "Resolve target_room - may be object or string";
    if (typeof(target_spec) == TYPE_OBJ)
      target_room = target_spec;
    else
      target_room = $match:match_object(target_spec, actor);
    endif
    typeof(target_room) == TYPE_OBJ || raise(E_INVARG, "Target room not found");
    valid(target_room) || raise(E_INVARG, "Target room no longer exists");
    const area = source_room.location;
    valid(area) || raise(E_INVARG, "Source room is not in an area");
    target_room.location == area || raise(E_INVARG, "Both rooms must be in the same area");
    const passage = area:passage_for(source_room, target_room);
    typeof(passage) != TYPE_FLYWEIGHT && return "No passage found between " + tostr(source_room) + " and " + tostr(target_room) + ".";
    "Collect labels for reporting";
    {side_a_room, side_b_room} = {passage.side_a_room, passage.side_b_room};
    let labels = {};
    source_room == side_a_room && passage.side_a_label != "" && (labels = {@labels, passage.side_a_label});
    source_room == side_b_room && passage.side_b_label != "" && (labels = {@labels, passage.side_b_label});
    target_room == side_a_room && passage.side_a_label != "" && !(passage.side_a_label in labels) && (labels = {@labels, passage.side_a_label});
    target_room == side_b_room && passage.side_b_label != "" && !(passage.side_b_label in labels) && (labels = {@labels, passage.side_b_label});
    "Check permissions";
    const from_cap = actor:find_capability_for(source_room, 'room);
    const from_target = typeof(from_cap) == TYPE_FLYWEIGHT ? from_cap | source_room;
    try
      from_target:check_can_dig_from();
    except (E_PERM)
      return "Permission denied: " + $grant_utils:format_denial(source_room, 'room, {'dig_from});
    endtry
    "Remove passage via area";
    const area_cap = actor:find_capability_for(area, 'area);
    const area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
    const result = area_target:remove_passage(from_target, target_room);
    const label_str = labels ? " (" + labels:join("/") + ")" | "";
    return result ? "Removed passage" + label_str + " between " + tostr(source_room) + " and " + tostr(target_room) + "." | "Failed to remove passage (may have already been removed).";
  endmethod

  method set_passage_description owner: ARCH_WIZARD
    "Tool: Set description and ambient flag for a passage";
    let ambient = 0;
    let description = 0;
    let direction = 0;
    let source_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {direction, description, ambient, source_spec} = {args_map["direction"], args_map["description"], maphaskey(args_map, "ambient") ? args_map["ambient"] | true, maphaskey(args_map, "source_room") ? args_map["source_room"] | ""};
    typeof(description) == TYPE_STR && ("{" in description || "}" in description) && (description = $sub_utils:compile(description));
    const source_room = this:_resolve_object(source_spec, actor, actor.location);
    typeof(source_room) == TYPE_OBJ || raise(E_INVARG, "Source room not found");
    valid(source_room) || raise(E_INVARG, "Source room no longer exists");
    const area = source_room.location;
    valid(area) || raise(E_INVARG, "Source room is not in an area");
    const passages = area:passages_from(source_room);
    typeof(passages) != TYPE_LIST && return "No passages from " + tostr(source_room) + ".";
    length(passages) == 0 && return "No passages from " + tostr(source_room) + ".";
    const target_passage = area:find_passage_by_direction(source_room, direction);
    typeof(target_passage) != TYPE_FLYWEIGHT && return "No passage found in direction '" + direction + "' from " + tostr(source_room) + ".";
    const from_cap = actor:find_capability_for(source_room, 'room);
    const from_target = typeof(from_cap) == TYPE_FLYWEIGHT ? from_cap | source_room;
    try
      from_target:check_can_dig_from();
    except (E_PERM)
      return "Permission denied: " + $grant_utils:format_denial(source_room, 'room, {'dig_from});
    endtry
    const new_passage = target_passage:with_description_from(source_room, description):with_ambient_from(source_room, ambient);
    area:update_passage(source_room, target_passage:other_room(source_room), new_passage);
    return "Set description for '" + direction + "' passage" + (ambient ? " (ambient - integrates into room description)" | " (explicit - shows in exits list)") + ".";
  endmethod

  method create_object owner: ARCH_WIZARD
    "Tool: Create an object from a parent";
    let extra_aliases = 0;
    let name_spec = 0;
    let parent_spec = 0;
    let parsed_aliases = 0;
    let primary_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {parent_spec, name_spec, extra_aliases} = {args_map["parent"], args_map["name"], maphaskey(args_map, "aliases") ? args_map["aliases"] | {}};
    {primary_name, parsed_aliases} = $str_proto:parse_name_aliases(name_spec);
    const final_aliases = {@parsed_aliases, @extra_aliases};
    !primary_name && raise(E_INVARG, "Object name cannot be blank");
    set_task_perms(actor);
    const parent_obj = $match:match_object(parent_spec, actor);
    typeof(parent_obj) == TYPE_OBJ || raise(E_INVARG, "Parent not found");
    valid(parent_obj) || raise(E_INVARG, "Parent no longer exists");
    "Use fertile flag (f) to allow child creation";
    const is_fertile = parent_obj.f;
    !is_fertile && !actor.wizard && parent_obj.owner != actor && raise(E_PERM, "You do not have permission to create children of " + tostr(parent_obj));
    const new_obj = parent_obj:create();
    new_obj:set_name_aliases(primary_name, final_aliases);
    new_obj:moveto(actor);
    let message = "Created \"" + primary_name + "\" (" + tostr(new_obj) + ") from " + tostr(parent_obj) + " in your inventory.";
    final_aliases && (message = message + " Aliases: " + final_aliases:join(", ") + ".");
    return message;
  endmethod

  method recycle_object owner: ARCH_WIZARD
    "Tool: Permanently destroy an object";
    let obj_id = 0;
    let obj_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to recycle " + tostr(target_obj));
    {obj_name, obj_id} = {target_obj.name, tostr(target_obj)};
    target_obj:destroy();
    return "Recycled \"" + obj_name + "\" (" + obj_id + ").";
  endmethod

  method rename_object owner: ARCH_WIZARD
    "Tool: Rename an object";
    let new_aliases = 0;
    let new_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to rename " + tostr(target_obj));
    {new_name, new_aliases} = $str_proto:parse_name_aliases(args_map["name"]);
    !new_name && raise(E_INVARG, "Object name cannot be blank");
    const old_name = target_obj.name;
    target_obj:set_name_aliases(new_name, new_aliases);
    let message = "Renamed \"" + old_name + "\" (" + tostr(target_obj) + ") to \"" + new_name + "\".";
    new_aliases && (message = message + " Aliases: " + new_aliases:join(", ") + ".");
    return message;
  endmethod

  method describe_object owner: ARCH_WIZARD
    "Tool: Set object description";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to describe " + tostr(target_obj));
    !args_map["description"] && raise(E_INVARG, "Description cannot be blank");
    target_obj.description = args_map["description"];
    return "Set description of \"" + target_obj.name + "\" (" + tostr(target_obj) + ").";
  endmethod

  method move_object owner: ARCH_WIZARD
    "Tool: Move an object to a new location";
    let dest_spec = 0;
    let obj_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, dest_spec} = {args_map["object"], args_map["destination"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to move " + tostr(target_obj));
    const dest_obj = this:_resolve_object(dest_spec, actor);
    typeof(dest_obj) == TYPE_OBJ || raise(E_INVARG, "Destination not found");
    valid(dest_obj) || raise(E_INVARG, "Destination no longer exists");
    const old_location_name = valid(target_obj.location) ? target_obj.location.name | "(nowhere)";
    target_obj:moveto(dest_obj);
    return "Moved \"" + target_obj.name + "\" (" + tostr(target_obj) + ") from " + old_location_name + " to \"" + dest_obj.name + "\" (" + tostr(dest_obj) + ").";
  endmethod

  method set_integrated_description owner: ARCH_WIZARD
    "Tool: Set object's integrated description";
    let {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to modify " + tostr(target_obj));
    target_obj.integrated_description = args_map["integrated_description"];
    const obj_name = target_obj.name;
    return args_map["integrated_description"] == "" ? "Cleared integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + ")." | "Set integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + "). When in a room, this will appear in the room description.";
  endmethod

  method grant_capability owner: ARCH_WIZARD
    "Tool: Grant capabilities to a player";
    let category = 0;
    let grantee_spec = 0;
    let perms = 0;
    let target_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {target_spec, category, perms, grantee_spec} = {args_map["target"], args_map["category"], args_map["permissions"], args_map["grantee"]};
    set_task_perms(actor);
    const target_obj = this:_resolve_object(target_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Target not found");
    valid(target_obj) || raise(E_INVARG, "Target no longer exists");
    const grantee = this:_resolve_object(grantee_spec, actor);
    typeof(grantee) == TYPE_OBJ || raise(E_INVARG, "Grantee not found");
    valid(grantee) || raise(E_INVARG, "Grantee no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to grant capabilities for " + tostr(target_obj));
    const perm_symbols = { tostr(p):to_symbol() for p in (perms) };
    $root:grant_capability(target_obj, perm_symbols, grantee, tostr(category):to_symbol());
    return "Granted " + $grant_utils:format_grant_with_name(target_obj, tostr(category):to_symbol(), perm_symbols) + " to " + grantee:name() + " (" + tostr(grantee) + ").";
  endmethod

  method audit_owned owner: ARCH_WIZARD
    "Tool: List all owned objects";
    let p = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const owned = sort(owned_objects(actor));
    !owned && return "You don't own any objects.";
    let result = "You own " + tostr(length(owned)) + " objects:\n";
    for o in (owned)
      p = parent(o);
      result = result + tostr(o) + ": \"" + o.name + "\" (parent: " + (valid(p) ? tostr(p) | "(none)") + ")\n";
    endfor
    return result;
  endmethod

  method area_map owner: ARCH_WIZARD
    "Tool: Get list of all rooms in the current area";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const current_room = actor.location;
    !valid(current_room) && return "Error: You are not in a room.";
    const area = current_room.location;
    !valid(area) && return "Error: Current room is not in an area.";
    let result = {"Current Location: " + current_room:name() + " (" + tostr(current_room) + ")", "Area: " + area:name() + " (" + tostr(area) + ")", "", "Rooms in this area:"};
    for o in (area.contents)
      if (valid(o))
        result = {@result, "  * " + o:name() + " (" + tostr(o) + ")" + (o == current_room ? " (you are here)" | "")};
      endif
    endfor
    return result:join("\n");
  endmethod

  method find_route owner: ARCH_WIZARD
    "Tool: Find route between two rooms";
    let connector = 0;
    let direction = 0;
    let from_name = 0;
    let from_room = 0;
    let from_spec = 0;
    let label = 0;
    let next_name = 0;
    let next_room = 0;
    let room = 0;
    let side_a_room = 0;
    let side_b_room = 0;
    let to_name = 0;
    let to_room = 0;
    let to_spec = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {to_spec, from_spec} = {args_map["to_room"], maphaskey(args_map, "from_room") ? args_map["from_room"] | ""};
    set_task_perms(actor);
    "Resolve from_room - may be object, string, or empty";
    if (!from_spec || from_spec == "")
      from_room = actor.location;
    elseif (typeof(from_spec) == TYPE_OBJ)
      from_room = from_spec;
    else
      from_room = $match:match_object(from_spec, actor);
    endif
    typeof(from_room) == TYPE_OBJ || return "Error: Could not find starting room '" + tostr(from_spec) + "'.";
    !valid(from_room) && return "Error: You are not in a room.";
    const area = from_room.location;
    !valid(area) && return "Error: Starting room is not in an area.";
    "Resolve to_room - may be object or string";
    if (typeof(to_spec) == TYPE_OBJ)
      to_room = to_spec;
    else
      to_room = $match:match_object(to_spec, actor);
    endif
    typeof(to_room) == TYPE_OBJ || return "Error: Could not find destination room '" + tostr(to_spec) + "'.";
    !valid(to_room) && return "Error: Destination room does not exist.";
    {from_name, to_name} = {from_room:name(), to_room:name()};
    to_room == from_room && return "You are already at " + to_name + "!";
    const path = area:find_path(from_room, to_room);
    !path && return "No route found from " + from_name + " to " + to_name + ".";
    let result = {"Route from " + from_name + " (" + tostr(from_room) + ") to " + to_name + " (" + tostr(to_room) + "):"};
    for i in [1..length(path) - 1]
      {room, connector} = path[i];
      next_room = path[i + 1][1];
      next_name = next_room:name();
      "Check if this is a transport connection or a passage";
      if (typeof(connector) == TYPE_LIST && length(connector) >= 1 && connector[1] == 'transport)
        "Transport connection: {'transport, label, transport_obj}";
        label = connector[2];
        result = {@result, "  " + tostr(i) + ". Take the " + label};
      else
        "Passage flyweight - extract direction label";
        {side_a_room, side_b_room} = {connector.side_a_room, connector.side_b_room};
        direction = room == side_a_room ? connector.side_a_label | room == side_b_room ? connector.side_b_label | "passage";
        result = {@result, "  " + tostr(i) + ". Go " + direction + " to " + next_name + " (" + tostr(next_room) + ")"};
      endif
    endfor
    return result:join("\n");
  endmethod

  method list_prototypes owner: ARCH_WIZARD
    "Tool: List available prototype objects for building";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const prototypes = $sysobj:list_builder_prototypes();
    let result = {"Available prototypes for creating objects:", ""};
    for proto_info in (prototypes)
      result = {@result, "* " + proto_info["name"] + " (" + proto_info["object"] + ")", "  " + proto_info["description"], ""};
    endfor
    return {@result, "Use these with the create_object tool or as the 'parent' parameter in build_room."}:join("\n");
  endmethod

  method inspect_object owner: ARCH_WIZARD
    "Tool: Inspect an object and return detailed information";
    let area = 0;
    let exits = 0;
    let label = 0;
    let passages = 0;
    let side_a_room = 0;
    let side_b_room = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(args_map["object"], actor);
    typeof(target) == TYPE_OBJ || return "Error: Could not find object '" + tostr(args_map["object"]) + "'.";
    !valid(target) && return "Error: Object does not exist.";
    const obj_name = target:name();
    const desc = target:description();
    const owner = target.owner;
    const parent_obj = parent(target);
    const loc = target.location;
    let result = {"Object Information for " + tostr(target) + ":", "", "Name: " + obj_name, "Description: " + desc, "", "Owner: " + (valid(owner) ? owner:name() + " (" + tostr(owner) + ")" | "(none)"), "Parent: " + (valid(parent_obj) ? parent_obj:name() + " (" + tostr(parent_obj) + ")" | "(none)"), "Location: " + (valid(loc) ? loc:name() + " (" + tostr(loc) + ")" | "(nowhere)"), ""};
    "Type-specific information";
    if (respond_to(target, 'is_actor) && target:is_actor())
      result = {@result, "Type: Actor/Player"};
    elseif ($room in ancestors(target))
      result = {@result, "Type: Room"};
      area = target.location;
      if (valid(area) && respond_to(area, 'passages_from))
        passages = area:passages_from(target);
        if (passages && length(passages) > 0)
          exits = {};
          for passage in (passages)
            {side_a_room, side_b_room} = {passage.side_a_room, passage.side_b_room};
            if (target == side_a_room)
              label = passage.side_a_label;
            elseif (target == side_b_room)
              label = passage.side_b_label;
            else
              continue;
            endif
            label && (exits = {@exits, label});
          endfor
          exits && (result = {@result, "Exits: " + exits:join(", ")});
        endif
      endif
    elseif ($wearable in ancestors(target))
      result = {@result, "Type: Wearable item"};
    elseif ($thing in ancestors(target))
      result = {@result, "Type: Thing/Object"};
    elseif ($area in ancestors(target))
      result = {@result, "Type: Area"};
      respond_to(target, 'contents) && (result = {@result, "Contains " + tostr(length(target:contents())) + " room" + (length(target:contents()) == 1 ? "" | "s")});
    else
      result = {@result, "Type: Generic object"};
    endif
    return result:join("\n");
  endmethod

  method list_rules owner: ARCH_WIZARD
    "Tool: List all rule properties on an object";
    let prop = 0;
    let prop_name = 0;
    let rule_str = 0;
    let value = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const rule_props = $obj_utils:rule_properties(target_obj, actor, true);
    !rule_props && return "No rule properties found on " + tostr(target_obj) + ".";
    let lines = {"Rules on " + tostr(target_obj) + ":"};
    for prop_info in (rule_props)
      {prop, value} = prop_info;
      prop_name = tostr(prop);
      if (value == 0)
        lines = {@lines, "  " + prop_name + ": (no rule set)"};
      elseif (typeof(value) == TYPE_FLYWEIGHT)
        rule_str = $rule_engine:decompile_rule(value);
        lines = {@lines, "  " + prop_name + ": " + rule_str};
      else
        lines = {@lines, "  " + prop_name + ": " + toliteral(value)};
      endif
    endfor
    return lines:join("\n");
  endmethod

  method set_rule owner: ARCH_WIZARD
    "Tool: Set a rule on an object property";
    let expression = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, expression} = {args_map["object"], args_map["property"], args_map["expression"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set rules on " + tostr(target_obj));
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property name must end with '_rule'");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    const prop_exists = prop_key != E_PROPNF;
    set_task_perms(actor);
    const compiled = $rule_engine:parse_expression(expression, prop_name, $rule_engine:policy_scope(prop_name));
    const validation = $rule_engine:validate_rule(compiled);
    !validation['valid] && raise(E_INVARG, "Rule validation failed: " + validation['warnings]:join("; "));
    if (prop_exists)
      target_obj.(prop_key) = compiled;
    else
      add_property(target_obj, prop_name, compiled, {actor, "r"});
    endif
    return "Set " + tostr(target_obj) + "." + prop_name + " = " + expression;
  endmethod

  method show_rule owner: ARCH_WIZARD
    "Tool: Show a specific rule property";
    let obj_spec = 0;
    let prop_name = 0;
    let rule_str = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name} = {args_map["object"], args_map["property"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property name must end with '_rule'");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && return tostr(target_obj) + "." + prop_name + " is not defined.";
    set_task_perms(actor);
    const value = target_obj.(prop_key);
    if (value == 0)
      return tostr(target_obj) + "." + prop_name + ": (no rule set)";
    elseif (typeof(value) == TYPE_FLYWEIGHT)
      rule_str = $rule_engine:decompile_rule(value);
      return tostr(target_obj) + "." + prop_name + ": " + rule_str;
    endif
    return tostr(target_obj) + "." + prop_name + ": " + toliteral(value);
  endmethod

  method tool_test_rule owner: ARCH_WIZARD
    "Evaluate a condition using supplied inputs. Preserve literal text and query errors.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const expression = args_map["expression"];
    const bindings = args_map["bindings"];
    typeof(expression) == TYPE_STR || raise(E_TYPE, "expression must be string");
    typeof(bindings) == TYPE_MAP || raise(E_TYPE, "bindings must be object/map");
    let converted_bindings = [];
    let input_types = [];
    for key in (mapkeys(bindings))
      let value = bindings[key];
      if (typeof(value) == TYPE_STR && value:starts_with("#"))
        value = fromliteral(value);
        typeof(value) == TYPE_OBJ || raise(E_INVARG, "Expected an object reference for " + key);
      endif
      const name = tosym(key);
      converted_bindings[name] = value;
      input_types[name] = $behavior_schema:type_of(value);
    endfor
    const compiled = $rule_engine:parse_expression(expression, 'test_rule,
      $rule_engine:scope(input_types));
    const result = $rule_engine:evaluate(compiled, converted_bindings);
    if (result['success])
      return "Condition matched. Selected values: " + toliteral(result['bindings]);
    endif
    return "Condition did not match the supplied inputs.";
  endmethod

  method list_reactions owner: ARCH_WIZARD
    "Show complete reaction definitions through the same service used by builders.";
    const {request, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const host = $builder_features:_author_object(toliteral(request["object"]), actor);
    (host.owner == actor || actor.wizard) || raise(E_PERM);
    let lines = {};
    for entry in ($obj_utils:reaction_properties(host, actor, true))
      lines = {@lines, @$builder_features:_author_inspect(host, tostr(entry[1])[1..$ - 9],
        $reaction:_definition(entry[2]))};
    endfor
    return length(lines) == 0 ? "No reactions." | lines:join("\n");
  endmethod



  method list_messages owner: ARCH_WIZARD
    "Tool: List message template properties on an object";
    let is_bag = 0;
    let is_compiled = 0;
    let prop = 0;
    let prop_name = 0;
    let value = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const msg_props = $obj_utils:message_properties(target_obj, actor, true);
    !msg_props && return "No message properties found on " + tostr(target_obj) + ".";
    let lines = {"Messages on " + tostr(target_obj) + ":"};
    for prop_info in (msg_props)
      {prop, value} = prop_info;
      prop_name = tostr(prop);
      if ($msg_bag:is_msg_bag(value))
        lines = {@lines, "  " + prop_name + ": " + tostr(value) + " (" + tostr(length(value:entries())) + " entries)"};
      elseif (typeof(value) == TYPE_STR)
        lines = {@lines, "  " + prop_name + ": \"" + (length(value) > 50 ? value[1..50] + "..." | value) + "\""};
      elseif (typeof(value) == TYPE_FLYWEIGHT)
        lines = {@lines, "  " + prop_name + ": (compiled template)"};
      elseif (typeof(value) == TYPE_LIST)
        "Check if it's a compiled template (first item not a list) or a message bag (list of lists)";
        is_compiled = length(value) > 0 && typeof(value[1]) != TYPE_LIST;
        is_bag = length(value) > 0 && typeof(value[1]) == TYPE_LIST;
        if (is_compiled)
          lines = {@lines, "  " + prop_name + ": (compiled template)"};
        elseif (is_bag)
          lines = {@lines, "  " + prop_name + ": [" + tostr(length(value)) + " entries]"};
        else
          lines = {@lines, "  " + prop_name + ": (empty)"};
        endif
      else
        lines = {@lines, "  " + prop_name + ": " + toliteral(value)};
      endif
    endfor
    return lines:join("\n");
  endmethod

  method get_message_template owner: ARCH_WIZARD
    "Tool: Get a message template value";
    let entries = 0;
    let entry = 0;
    let lines = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name} = {args_map["object"], args_map["property"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && return tostr(target_obj) + "." + prop_name + " is not defined.";
    set_task_perms(actor);
    const value = target_obj.(prop_key);
    "Check for message bag (object or flyweight)";
    if ($msg_bag:is_msg_bag(value))
      entries = value:entries();
      lines = {tostr(target_obj) + "." + prop_name + ": " + tostr(value)};
      for i in [1..length(entries)]
        entry = entries[i];
        lines = {@lines, "  " + tostr(i) + ": " + `$sub_utils:decompile(entry) ! ANY => toliteral(entry)'};
      endfor
      return lines:join("\n");
    endif
    "Check for compiled template (list starting with non-list)";
    const is_compiled = typeof(value) == TYPE_LIST && length(value) > 0 && typeof(value[1]) != TYPE_LIST;
    if (typeof(value) == TYPE_LIST && is_compiled)
      return tostr(target_obj) + "." + prop_name + ": " + $sub_utils:decompile(value);
    elseif (typeof(value) == TYPE_LIST)
      "Legacy list-of-lists bag format";
      lines = {tostr(target_obj) + "." + prop_name + " (" + tostr(length(value)) + " entries):"};
      for i in [1..length(value)]
        entry = value[i];
        lines = {@lines, "  " + tostr(i) + ": " + `$sub_utils:decompile(entry) ! ANY => toliteral(entry)'};
      endfor
      return lines:join("\n");
    elseif (typeof(value) == TYPE_FLYWEIGHT)
      return tostr(target_obj) + "." + prop_name + ": (compiled template)";
    elseif (typeof(value) == TYPE_STR)
      return tostr(target_obj) + "." + prop_name + ": \"" + value + "\"";
    endif
    return tostr(target_obj) + "." + prop_name + ": " + toliteral(value);
  endmethod

  method set_message_template owner: ARCH_WIZARD
    "Tool: Set a message template on an object";
    let obj_spec = 0;
    let prop_name = 0;
    let template = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, template} = {args_map["object"], args_map["property"], args_map["template"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set messages on " + tostr(target_obj));
    "Compile the template";
    const compiled = $sub_utils:compile(template, target_obj:message_bindings(prop_name));
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    if (prop_key != E_PROPNF)
      $obj_utils:set_compiled_message(target_obj, prop_key, compiled, actor);
    elseif (prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag"))
      $obj_utils:add_message_entry(target_obj, prop_name, compiled, actor);
    else
      set_task_perms(actor);
      add_property(target_obj, prop_name, compiled, {actor, "rc"});
    endif
    return "Set " + tostr(target_obj) + "." + prop_name + ": " + template;
  endmethod

  method add_message_template owner: ARCH_WIZARD
    "Tool: Add a message to a message bag";
    let obj_spec = 0;
    let prop_name = 0;
    let template = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, template} = {args_map["object"], args_map["property"], args_map["template"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set messages on " + tostr(target_obj));
    prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msgs or _msg_bag");
    const compiled = $sub_utils:compile(template, target_obj:message_bindings(prop_name));
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    $obj_utils:add_message_entry(target_obj, prop_name, compiled, actor);
    return "Added message to " + tostr(target_obj) + "." + prop_name + ": " + template;
  endmethod

  method delete_message_template owner: ARCH_WIZARD
    "Tool: Delete a message from a message bag by index";
    let index = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, index} = {args_map["object"], args_map["property"], args_map["index"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to modify messages on " + tostr(target_obj));
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && raise(E_INVARG, "Message bag not found on " + tostr(target_obj) + "." + prop_name);
    $obj_utils:remove_message_entry(target_obj, prop_key, index, actor, {{"property_write", target_obj, prop_key}});
    return "Deleted message " + tostr(index) + " from " + tostr(target_obj) + "." + prop_name;
  endmethod

  method doc_lookup owner: ARCH_WIZARD
    "Tool: Fetch developer documentation for object/verb/property";
    let alias_name = 0;
    let doc_text = 0;
    let item_name = 0;
    let object_str = 0;
    let parsed = 0;
    let selector = 0;
    let selectors = 0;
    let target_obj = 0;
    let title = 0;
    let type = 0;
    let verb_location = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_spec = args_map["target"];
    set_task_perms(actor);
    "Handle special alias cases";
    let alias_obj = false;
    if (typeof(target_spec) == TYPE_STR)
      alias_name = target_spec:starts_with("$") ? target_spec[2..$] | target_spec;
      alias_name == "sub_utils" && (alias_obj = $sub_utils);
      alias_name == "sub" && (alias_obj = $sub);
    endif
    if (alias_obj)
      type = 'object;
      target_obj = alias_obj;
      item_name = "";
    else
      parsed = $prog_utils:parse_target_spec(target_spec);
      parsed || raise(E_INVARG, "Invalid format. Use object, object:verb, or object.property");
      object_str = parsed['object_str];
      selectors = parsed['selectors];
      "Determine type and item_name from selectors";
      if (length(selectors) > 0)
        selector = selectors[1];
        type = selector['kind];
        item_name = selector['item_name];
      else
        type = 'object;
        item_name = "";
      endif
      target_obj = $match:match_object(object_str, actor);
      typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
      valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    endif
    "Fetch docs based on type";
    if (type == 'object)
      doc_text = $help_utils:get_object_documentation(target_obj);
      title = "Documentation for " + tostr(target_obj);
    elseif (type == 'verb)
      verb_location = target_obj:find_verb_definer(item_name);
      verb_location == #-1 && raise(E_INVARG, "Verb '" + tostr(item_name) + "' not found on " + tostr(target_obj));
      doc_text = $help_utils:extract_verb_documentation(verb_location, item_name);
      title = "Documentation for " + tostr(target_obj) + ":" + tostr(item_name);
    elseif (type == 'property)
      doc_text = $help_utils:property_documentation(target_obj, item_name);
      title = "Documentation for " + tostr(target_obj) + "." + tostr(item_name);
    else
      raise(E_INVARG, "Unknown target type");
    endif
    const doc_body = typeof(doc_text) == TYPE_LIST ? doc_text:join("\n") | doc_text;
    return title + "\n\n" + (doc_body ? doc_body | "(No documentation available)");
  endmethod

  method help_lookup owner: ARCH_WIZARD
    "Tool: Look up a help topic";
    let all_topics = 0;
    let result = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const topic = args_map["topic"];
    typeof(topic) != TYPE_STR && return "Error: topic must be a string.";
    "If empty topic, list available topics";
    if (topic == "")
      all_topics = actor:_collect_help_topics();
      result = {"Available help topics:"};
      for t in (all_topics)
        result = {@result, "  " + t.name + " - " + t.summary};
      endfor
      return result:join("\n");
    endif
    "Search for specific topic";
    const found = actor:find_help_topic(topic);
    if (typeof(found) == TYPE_INT)
      return "No help found for: " + topic;
    endif
    "Return structured help";
    return "Topic: " + found.name + "\n\n" + found.summary + "\n\n" + found.content + (found.see_also ? "\n\nSee also: " + found.see_also:join(", ") | "");
  endmethod

  method get_verb_code owner: ARCH_WIZARD
    "Tool: Get the code of a verb. If verb is omitted, lists verbs on the object.";
    let code_lines = 0;
    let info = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    let verb_name = maphaskey(args_map, "verb") ? args_map["verb"] | "";
    if (!verb_name)
      "No verb provided - fallback to listing verbs";
      return $agent_building_tools:list_verbs(args_map, actor);
    endif
    typeof(verb_name) != TYPE_STR && raise(E_INVARG, "verb must be a string");
    "Robustness: strip hallucinated colon/separator prefixes from verb name";
    verb_name = verb_name:trim();
    while (length(verb_name) > 0 && (verb_name[1] == ":" || verb_name[1] == " " || verb_name[1] == "\t"))
      verb_name = verb_name[2..$]:trim();
    endwhile
    "Get verb info and code";
    try
      set_task_perms(actor);
      info = verb_info(target_obj, verb_name);
    except (E_VERBNF)
      raise(E_VERBNF, "Verb '" + verb_name + "' not found on " + tostr(target_obj));
    endtry
    try
      code_lines = verb_code(target_obj, verb_name);
    except (E_VERBNF)
      code_lines = {};
    endtry
    const argspec = info[3];
    const flags = info[2];
    let result = "Verb: " + tostr(target_obj) + ":" + verb_name + "\n";
    result = result + "Flags: " + flags + "  Argspec: " + argspec + "\n";
    result = result + "Code (" + tostr(length(code_lines)) + " lines):\n";
    result = result + "---\n";
    result = result + code_lines:join("\n");
    result = result + "\n---";
    return result;
  endmethod

  method add_verb owner: ARCH_WIZARD
    "Tool: Add a new verb to an object.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not own " + tostr(target_obj));
    const verb_name = args_map["verb"];
    typeof(verb_name) != TYPE_STR && raise(E_INVARG, "verb must be a string");
    dobj = maphaskey(args_map, "dobj") ? args_map["dobj"] | "this";
    const prep = maphaskey(args_map, "prep") ? args_map["prep"] | "none";
    iobj = maphaskey(args_map, "iobj") ? args_map["iobj"] | "none";
    "Default to rxd for methods, rd for commands";
    const default_flags = dobj == "none" && prep == "none" && iobj == "none" ? "rxd" | "rd";
    const perms = maphaskey(args_map, "permissions") ? args_map["permissions"] | default_flags;
    set_task_perms(actor);
    add_verb(target_obj, {actor, perms, verb_name}, {dobj, prep, iobj});
    return "Added verb '" + verb_name + "' to " + tostr(target_obj) + " [" + perms + "] " + dobj + " " + prep + " " + iobj;
  endmethod

  method program_verb owner: ARCH_WIZARD
    "Tool: Set the code for a verb.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not own " + tostr(target_obj));
    const verb_name = args_map["verb"];
    typeof(verb_name) != TYPE_STR && raise(E_INVARG, "verb must be a string");
    const code = args_map["code"];
    typeof(code) != TYPE_STR && raise(E_INVARG, "code must be a string");
    const code_lines = code:split("\n");
    set_task_perms(actor);
    const errors = set_verb_code(target_obj, verb_name, code_lines);
    "set_verb_code returns list of errors, or possibly 0/{} on success";
    if (typeof(errors) == TYPE_LIST && length(errors) > 0)
      raise(E_INVARG, "Compile error: " + errors:join("; "));
    endif
    return "Programmed " + tostr(target_obj) + ":" + verb_name + " (" + tostr(length(code_lines)) + " lines)";
  endmethod

  method list_verbs owner: ARCH_WIZARD
    "Tool: List verbs on an object.";
    let info = 0;
    let vargs = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const verb_list = verbs(target_obj);
    if (length(verb_list) == 0)
      return "No verbs on " + tostr(target_obj);
    endif
    set_task_perms(actor);
    let result = "Verbs on " + tostr(target_obj) + ":\n";
    for v in (verb_list)
      info = verb_info(target_obj, v);
      vargs = verb_args(target_obj, v);
      result = result + "  " + v + " [" + info[2] + "] " + vargs[1] + " " + vargs[2] + " " + vargs[3] + "\n";
    endfor
    return result;
  endmethod

  method delete_verb owner: ARCH_WIZARD
    "Tool: Delete a verb from an object.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not own " + tostr(target_obj));
    const verb_name = args_map["verb"];
    typeof(verb_name) != TYPE_STR && raise(E_INVARG, "verb must be a string");
    set_task_perms(actor);
    delete_verb(target_obj, verb_name);
    return "Deleted verb '" + verb_name + "' from " + tostr(target_obj);
  endmethod

  method list_properties owner: ARCH_WIZARD
    "Tool: List properties on an object.";
    let val = 0;
    let val_str = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_list = properties(target_obj);
    if (length(prop_list) == 0)
      return "No properties defined on " + tostr(target_obj);
    endif
    set_task_perms(actor);
    let result = "Properties on " + tostr(target_obj) + ":\n";
    for p in (prop_list)
      try
        val = target_obj.(p);
        val_str = toliteral(val);
        if (length(val_str) > 50)
          val_str = val_str[1..50] + "...";
        endif
      except (ANY)
        val_str = "(unreadable)";
      endtry
      result = result + "  " + p + " = " + val_str + "\n";
    endfor
    return result;
  endmethod

  method get_property owner: ARCH_WIZARD
    "Tool: Get the value of a property.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_name = args_map["property"];
    typeof(prop_name) != TYPE_STR && raise(E_INVARG, "property must be a string");
    set_task_perms(actor);
    const val = target_obj.(prop_name);
    return tostr(target_obj) + "." + prop_name + " = " + toliteral(val);
  endmethod

  method set_property owner: ARCH_WIZARD
    "Tool: Set the value of a property.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_name = args_map["property"];
    typeof(prop_name) != TYPE_STR && raise(E_INVARG, "property must be a string");
    const value = args_map["value"];
    set_task_perms(actor);
    target_obj.(prop_name) = value;
    return "Set " + tostr(target_obj) + "." + prop_name + " = " + toliteral(value);
  endmethod

  method add_property owner: ARCH_WIZARD
    "Tool: Add a new property to an object.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not own " + tostr(target_obj));
    const prop_name = args_map["property"];
    typeof(prop_name) != TYPE_STR && raise(E_INVARG, "property must be a string");
    const value = maphaskey(args_map, "value") ? args_map["value"] | 0;
    set_task_perms(actor);
    add_property(target_obj, prop_name, value, {actor, "rc"});
    return "Added property '" + prop_name + "' to " + tostr(target_obj) + " = " + toliteral(value);
  endmethod

  verb set_verb_info (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Tool: Set verb metadata (permissions, names).";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "Permission denied");
    const verb_name = args_map["verb"];
    const requested_owner = maphaskey(args_map, "owner") ? this:_resolve_object(args_map["owner"], actor) | #-1;
    set_task_perms(actor);
    const info = verb_info(target_obj, verb_name);
    const new_owner = valid(requested_owner) ? requested_owner | info[1];
    const new_perms = maphaskey(args_map, "permissions") ? args_map["permissions"] | info[2];
    const new_names = maphaskey(args_map, "names") ? args_map["names"] | info[3];
    set_verb_info(target_obj, verb_name, {new_owner, new_perms, new_names});
    return "Updated " + tostr(target_obj) + ":" + verb_name + " [" + new_perms + "]";
  endverb

  verb set_verb_args (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Tool: Set verb argument specification.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "Permission denied");
    const verb_name = args_map["verb"];
    dobj = args_map["dobj"];
    const prep = args_map["prep"];
    iobj = args_map["iobj"];
    set_task_perms(actor);
    set_verb_args(target_obj, verb_name, {dobj, prep, iobj});
    return "Updated " + tostr(target_obj) + ":" + verb_name + " args to [" + dobj + " " + prep + " " + iobj + "]";
  endverb

  method test_core_building_tool_metadata owner: ARCH_WIZARD
    "Core building tools should expose real prototype and object metadata.";
    let audit = 0;
    let details = 0;
    let message = 0;
    let messages = 0;
    let rules = 0;
    const prototypes = this:list_prototypes([], player);
    $test_utils:assert_true(index(prototypes, "$thing") > 0, "list_prototypes should include $thing");
    $test_utils:assert_true(index(prototypes, $thing.description) > 0, "list_prototypes should include prototype descriptions");
    let created = #-1;
    try
      message = this:create_object(["parent" -> "$thing", "name" -> "tool metadata probe:tool-probe"], $hacker);
      $test_utils:assert_true(index(message, "Created \"tool metadata probe\"") > 0, "create_object should report created object");
      created = $match:match_object("tool-probe", $hacker);
      $test_utils:assert_type(created, TYPE_OBJ, "created object should be matchable by alias");
      $test_utils:assert_true(valid(created), "created object should be valid");
      details = this:inspect_object(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(details, "Name: tool metadata probe") > 0, "inspect_object should include real name");
      $test_utils:assert_true(index(details, "Description:") > 0, "inspect_object should include description field");
      $test_utils:assert_true(index(details, "Parent:") > 0, "inspect_object should include parent metadata");
      message = this:rename_object(["object" -> created, "name" -> "renamed metadata probe:renamed-probe"], $hacker);
      $test_utils:assert_true(index(message, "Renamed \"tool metadata probe\"") > 0, "rename_object should report the real previous name");
      $test_utils:assert_true(index(message, "to \"renamed metadata probe\"") > 0, "rename_object should report the new name");
      message = this:describe_object(["object" -> created, "description" -> "A probe with renamed metadata."], $hacker);
      $test_utils:assert_true(index(message, "Set description of \"renamed metadata probe\"") > 0, "describe_object should report the real object name");
      message = this:move_object(["object" -> created, "destination" -> $hacker], $hacker);
      $test_utils:assert_true(index(message, "Moved \"renamed metadata probe\"") > 0, "move_object should report the real object name");
      message = this:set_integrated_description(["object" -> created, "integrated_description" -> "a renamed probe is here"], $hacker);
      $test_utils:assert_true(index(message, "Set integrated description of \"renamed metadata probe\"") > 0, "set_integrated_description should report the real object name");
      audit = this:audit_owned([], $hacker);
      $test_utils:assert_true(index(audit, "renamed metadata probe") > 0, "audit_owned should list real object names");
      add_property(created, "probe_rule", 0, {$hacker, "rc"});
      add_property(created, "probe_msg", "Probe message.", {$hacker, "rc"});
      rules = this:list_rules(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(rules, "probe_rule: (no rule set)") > 0, "list_rules should read real rule property values");
      messages = this:list_messages(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(messages, "probe_msg: \"Probe message.\"") > 0, "list_messages should read real message property values");
      message = this:set_message_template(["object" -> tostr(created), "property" -> "probe_msg", "template" -> "Updated probe message."], $hacker);
      $test_utils:assert_true(index(message, "Set " + tostr(created) + ".probe_msg") > 0, "set_message_template should report scalar message update");
      $test_utils:assert_true(index($sub_utils:decompile(created.probe_msg), "Updated probe message") > 0, "set_message_template should store compiled scalar message");
      message = this:add_message_template(["object" -> tostr(created), "property" -> "probe_msgs", "template" -> "First bag message."], $hacker);
      $test_utils:assert_true(index(message, "Added message to " + tostr(created) + ".probe_msgs") > 0, "add_message_template should report bag append");
      $test_utils:assert_eq(length(created.probe_msgs:entries()), 1, "add_message_template should create a one-entry bag");
      message = this:delete_message_template(["object" -> tostr(created), "property" -> "probe_msgs", "index" -> 1], $hacker);
      $test_utils:assert_true(index(message, "Deleted message 1 from " + tostr(created) + ".probe_msgs") > 0, "delete_message_template should report bag removal");
      $test_utils:assert_eq(length(created.probe_msgs:entries()), 0, "delete_message_template should remove the bag entry");
    finally
      $test_utils:destroy_if_valid(created);
    endtry
    return true;
  endmethod

  method test_code_property_tool_actor_permissions owner: ARCH_WIZARD
    "Private code and property inspection require the requesting actor's ordinary authority.";
    const target = create($thing, $hacker, 2);
    try
      target.r = 0;
      add_property(target, "agent_building_private_probe", "secret", {$hacker, ""});
      add_verb(target, {$hacker, "xd", "agent_building_private_verb"}, {"this", "none", "this"});
      set_verb_code(target, "agent_building_private_verb", {"return \"secret\";"});
      for entry in ({{"get_property", ["object" -> target, "property" -> "agent_building_private_probe"]}, {"get_verb_code", ["object" -> target, "verb" -> "agent_building_private_verb"]}, {"list_verbs", ["object" -> target]}})
        const {endpoint, values} = entry;
        let denied = false;
        try
          this:(endpoint)(values, $test_player);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, endpoint + " must not mint private target grants");
      endfor
    finally
      $test_utils:destroy_if_valid(target);
    endtry
    return true;
  endmethod

  method test_navigation_tool_metadata owner: ARCH_WIZARD
    "Navigation tools should expose real room, area, and passage metadata.";
    let details = 0;
    let map = 0;
    let passage = 0;
    let result = 0;
    let route = 0;
    let updated_passage = 0;
    const area = $area:create(true);
    const r1 = $room:create(true);
    const r2 = $room:create(true);
    const actor = $root:create(true);
    try
      actor.name = "Tool Test Actor";
      area.name = "Tool Test Area";
      r1.name = "Tool Test Start";
      r2.name = "Tool Test End";
      move(r1, area);
      move(r2, area);
      move(actor, r1);
      passage = <$passage, .side_a_room = r1, .side_a_label = "east", .side_b_room = r2, .side_b_label = "west", .is_open = true>;
      area:set_passage(r1, r2, passage);
      let invalid_template = false;
      try
        this:set_passage_description(["direction" -> "east", "description" -> "broken {template", "source_room" -> r1], $arch_wizard);
      except (E_INVARG)
        invalid_template = true;
      endtry
      $test_utils:assert_true(invalid_template, "set_passage_description should reject malformed templates");
      result = this:set_passage_description(["direction" -> "east", "description" -> "{nc} heads east.", "source_room" -> r1], $arch_wizard);
      $test_utils:assert_true(index(result, "Set description for 'east' passage") > 0, "set_passage_description should report success");
      updated_passage = area:passage_for(r1, r2);
      $test_utils:assert_type(updated_passage.side_a_description, TYPE_LIST, "set_passage_description should store compiled templates");
      map = this:area_map([], actor);
      $test_utils:assert_true(index(map, "Current Location: Tool Test Start") > 0, "area_map should include current room name: " + toliteral(map));
      $test_utils:assert_true(index(map, "Area: Tool Test Area") > 0, "area_map should include area name");
      $test_utils:assert_true(index(map, "Tool Test End") > 0, "area_map should include rooms in area");
      route = this:find_route(["from_room" -> r1, "to_room" -> r2], actor);
      $test_utils:assert_true(index(route, "Route from Tool Test Start") > 0, "find_route should include start room name");
      $test_utils:assert_true(index(route, "Go east to Tool Test End") > 0, "find_route should include passage label and destination");
      details = this:inspect_object(["object" -> r1], actor);
      $test_utils:assert_true(index(details, "Exits: east") > 0, "inspect_object should include real passage labels");
    finally
      $test_utils:destroy_if_valid(actor);
      $test_utils:destroy_if_valid(r2);
      $test_utils:destroy_if_valid(r1);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod

  verb _resolve_object (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Resolve an object spec that may be an object, string, or empty";
    "Returns the resolved object, or raises E_INVARG if not found";
    "Usage: this:_resolve_object(spec, actor [, default])";
    let {spec, actor, ?default = E_INVARG} = args;
    "If already an object, return it directly";
    typeof(spec) == TYPE_OBJ && return spec;
    "If empty/falsy, return default or raise";
    if (!spec || spec == "")
      typeof(default) == TYPE_ERR && raise(E_INVARG, "Object reference required");
      return default;
    endif
    "Robustness: strip hallucinated colon/separator prefixes common in LLM output";
    if (typeof(spec) == TYPE_STR)
      spec = spec:trim();
      while (length(spec) > 0 && (spec[1] == ":" || spec[1] == " " || spec[1] == "\t"))
        spec = spec[2..$]:trim();
      endwhile
      if (spec == "")
        typeof(default) == TYPE_ERR && raise(E_INVARG, "Object reference required");
        return default;
      endif
    endif
    "Otherwise it's a string - use match_object";
    return $match:match_object(spec, actor);
  endverb
  method author_reaction owner: ARCH_WIZARD
    "Author behavior through the same authenticated service used by text commands.";
    const {request, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    return $builder_features:author(tosym(request["command"]), request["text"], actor):join("\n");
  endmethod
endobject
