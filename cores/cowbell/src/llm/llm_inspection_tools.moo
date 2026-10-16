object LLM_INSPECTION_TOOLS [
  import_export_id -> "llm_inspection_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Inspection Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Reusable inspection operations for in-world and external agents.";

  method get_tools owner: ARCH_WIZARD
    "Return the domain's immutable tool descriptors with explicit effect classifications.";
    const target_obj = this;
    let tools = {};
    tools = {@tools, $llm_agent_tool:mk("list_verbs", "List verbs on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to list verbs from"]], "required" -> {"object"}], target_obj, "list_verbs", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("get_verb_code", "Get source code of a verb.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object containing verb"], "verb" -> ["type" -> "string", "description" -> "Verb name"]], "required" -> {"object", "verb"}], target_obj, "get_verb_code", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("list_properties", "List properties on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"]], "required" -> {"object"}], target_obj, "list_properties", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("get_property", "Get a property value.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object"], "property" -> ["type" -> "string", "description" -> "Property name"]], "required" -> {"object", "property"}], target_obj, "get_property", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("audit_owned", "List all objects owned by the actor.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "audit_owned", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("area_map", "Get a list of all rooms in the current area. Use this to see what locations already exist and understand the spatial layout.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "area_map", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("find_route", "Find the route between connected rooms, including rooms in different areas. Shows step-by-step directions. Useful for understanding how rooms are connected.", ["type" -> "object", "properties" -> ["from_room" -> ["type" -> "string", "description" -> "Starting room (optional, defaults to actor's location)"], "to_room" -> ["type" -> "string", "description" -> "Destination room name or object number"]], "required" -> {"to_room"}], target_obj, "find_route", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("list_prototypes", "List available prototype objects that can be used as parents when creating objects. Shows $thing, $wearable, $room, etc. with descriptions of what each is for.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "list_prototypes", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("inspect_object", "Examine an object to see detailed information including name, description, parent, owner, location, and properties.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (name or object number)"]], "required" -> {"object"}], target_obj, "inspect_object", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("dump_object", "Get the complete source listing of a MOO object including all properties, verbs, and code. This is the most comprehensive way to inspect an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to dump (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], target_obj, "dump_object", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("ancestors", "Get the inheritance chain (ancestors) of a MOO object, from immediate parent to root.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], target_obj, "ancestors", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("descendants", "Get all objects that inherit from a MOO object (its descendants/children).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"]], "required" -> {"object"}], target_obj, "descendants", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("function_info", "Get information about a specific MOO builtin function. Returns {name, min_args, max_args, types} where types is a list of type codes: -2=int/float, -1=any, 0=INT, 1=OBJ, 2=STR, 3=ERR, 4=LIST, 9=FLOAT, 10=MAP, 14=BOOL, 15=FLYWEIGHT, 16=SYMBOL, 17=BINARY, 18=LAMBDA. Max_args of -1 means unlimited args.", ["type" -> "object", "properties" -> ["function_name" -> ["type" -> "string", "description" -> "The name of the builtin function (e.g. 'tostr', 'verb_code', 'ancestors')"]], "required" -> {"function_name"}], target_obj, "function_info", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("list_builtin_functions", "Get a list of all available MOO builtin functions. Returns a formatted list with function signatures. Use function_info(name) for detailed info about a specific function.", ["type" -> "object", "properties" -> [], "required" -> {}], target_obj, "list_builtin_functions", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("get_verb_code_range", "Get specific lines from a verb's code. Use this to show focused code snippets to the user with line numbers.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "start_line" -> ["type" -> "integer", "description" -> "First line to retrieve (1-indexed, optional)", "minimum" -> 1], "end_line" -> ["type" -> "integer", "description" -> "Last line to retrieve (inclusive, optional)", "minimum" -> 1]], "required" -> {"object", "verb"}], target_obj, "get_verb_code_range", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("get_verb_metadata", "Get metadata about a verb including owner, flags, and argument specification.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"]], "required" -> {"object", "verb"}], target_obj, "get_verb_metadata", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("list_commands", "List readable command verbs on an object.",
      ["type" -> "object", "properties" -> ["object" -> ["type" -> "string"]],
        "required" -> {"object"}], target_obj, "list_commands", 'inspection)};
    return {@tools, @$llm_code_search:get_tools()};
  endmethod

  method list_commands owner: ARCH_WIZARD
    "Resolve an object once and read command metadata under the authenticated actor's permissions.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target = $llm_tool_provider:_resolve_object(parameters["object"], actor);
    valid(target) || raise(E_INVARG, "Object not found.");
    const commands = $llm_command_context:command_specs(actor, target);
    let result = {"Commands available for " + tostr(target) + ":"};
    for entry in (commands)
      const {name, definer, direct, prep, indirect} = entry;
      result = {@result, name + " (" + direct + " " + prep + " " + indirect
        + ") [from " + tostr(definer) + "]"};
      value_bytes(result) <= 65536 || raise(E_QUOTA, "Command list is too large.");
    endfor
    return result:join("\n");
  endmethod

  method audit_owned owner: ARCH_WIZARD
    "Tool: List all owned objects";
    let p = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const current_room = actor.location;
    !valid(current_room) && raise(E_INVARG, "Error: You are not in a room.");
    const area = current_room.location;
    !valid(area) && raise(E_INVARG, "Error: Current room is not in an area.");
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
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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
    typeof(from_room) == TYPE_OBJ || raise(E_INVARG, "Error: Could not find starting room '" + tostr(from_spec) + "'.");
    !valid(from_room) && raise(E_INVARG, "Error: You are not in a room.");
    const area = from_room.location;
    !valid(area) && raise(E_INVARG, "Error: Starting room is not in an area.");
    "Resolve to_room - may be object or string";
    if (typeof(to_spec) == TYPE_OBJ)
      to_room = to_spec;
    else
      to_room = $match:match_object(to_spec, actor);
    endif
    typeof(to_room) == TYPE_OBJ || raise(E_INVARG, "Error: Could not find destination room '" + tostr(to_spec) + "'.");
    !valid(to_room) && raise(E_INVARG, "Error: Destination room does not exist.");
    {from_name, to_name} = {from_room:name(), to_room:name()};
    to_room == from_room && return "You are already at " + to_name + "!";
    const path = area:find_path(from_room, to_room, true, true, actor);
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
        result = {@result, "  " + tostr(i) + ". At " + room:name() + ", use " + connector[4] + " (" + label + ")"};
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
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(args_map["object"], actor);
    typeof(target) == TYPE_OBJ || raise(E_INVARG, "Error: Could not find object '" + tostr(args_map["object"]) + "'.");
    !valid(target) && raise(E_INVARG, "Error: Object does not exist.");
    const obj_name = target:name();
    const desc = $llm_tool_provider:description_text(target, actor);
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

  method get_verb_code owner: ARCH_WIZARD
    "Read bounded source under actor authority; an omitted verb lists local definitions.";
    const {request, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const name = `request["verb"] ! E_RANGE => ""';
    name == "" && return $llm_inspection_tools:list_verbs(request, actor);
    return $llm_verb_source:read(actor, request["object"], name):text();
  endmethod

  method list_verbs owner: ARCH_WIZARD
    "Tool: List verbs on an object.";
    let info = 0;
    let vargs = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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

  method list_properties owner: ARCH_WIZARD
    "Tool: List properties on an object.";
    let val = 0;
    let val_str = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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

  method test_read_tool_actor_permissions owner: ARCH_WIZARD
    "Inspection follows the actor's authority, including private-property and code ownership.";
    const actor = $test_player;
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
      $test_utils:assert_eq(this:get_property(["object" -> tostr(owned), "property" -> "visor_probe"], actor), tostr(owned) + ".visor_probe = " + toliteral("owned secret"), "owned private property remains readable");
      $test_utils:assert_true(index(this:get_verb_code(["object" -> tostr(owned), "verb" -> "visor_probe"], actor), "return 123;") > 0, "owned private code remains readable");
      for endpoint in ({"get_property", "get_verb_code", "get_verb_metadata", "list_verbs", "list_properties"})
        $test_utils:assert_raises(E_PERM, this, endpoint, {["object" -> tostr(foreign), "property" -> "visor_probe", "verb" -> "visor_probe"], actor}, endpoint + " must not grant foreign inspection");
      endfor
      $test_utils:assert_raises(E_PERM, this, "dump_object", {["object" -> tostr(owned)], actor}, "ordinary inspection does not grant privileged full dumps");
    finally
      set_task_perms($arch_wizard);
      valid(foreign) && foreign:destroy();
      valid(owned) && owned:destroy();
    endtry
    return true;
  endmethod

  method dump_object owner: ARCH_WIZARD
    "Tool: Get the complete source dump of an object";
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
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

  method ancestors owner: ARCH_WIZARD
    "Tool: Get the ancestor chain of an object";
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    set_task_perms(wearer);
    const o = $match:match_object(args_map["object"], actor);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    return toliteral({ {tostr(a), a:name()} for a in (ancestors(o)) });
  endmethod

  method descendants owner: ARCH_WIZARD
    "Tool: Get all descendants of an object";
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    set_task_perms(wearer);
    const o = $match:match_object(args_map["object"], actor);
    typeof(o) == TYPE_OBJ || raise(E_TYPE("Expected valid object"));
    return toliteral({ {tostr(d), d:name()} for d in (descendants(o)) });
  endmethod

  method function_info owner: ARCH_WIZARD
    "Tool: Get information about a builtin function";
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const func_name = args_map["function_name"];
    typeof(func_name) == TYPE_STR || raise(E_TYPE("Expected function name string"));
    return toliteral(["info" -> function_info(func_name), "help" -> function_help(func_name)]);
  endmethod

  method list_builtin_functions owner: ARCH_WIZARD
    "Tool: List all builtin functions with signatures";
    let arg_sig = 0;
    let max_args = 0;
    let min_args = 0;
    let name = 0;
    let types = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
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

  method get_verb_code_range owner: ARCH_WIZARD
    "Read a bounded source selection with its original line numbers.";
    const {request, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    return $llm_verb_source:read(actor, request["object"], request["verb"],
      `request["start_line"] ! E_RANGE => 1', `request["end_line"] ! E_RANGE => 0'):numbered_text();
  endmethod

  method get_verb_metadata owner: ARCH_WIZARD
    "Read native verb metadata without loading source.";
    const {request, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    return $llm_verb_source:metadata(actor, request["object"], request["verb"]):metadata_text();
  endmethod

endobject
