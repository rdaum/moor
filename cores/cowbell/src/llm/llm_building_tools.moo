object LLM_BUILDING_TOOLS [
  import_export_id -> "llm_building_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Building Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Reusable building operations for in-world and external agents.";

  method get_tools owner: ARCH_WIZARD
    "Return the domain's immutable tool descriptors with explicit effect classifications.";
    const target_obj = this;
    let tools = {};
    tools = {@tools, $llm_agent_tool:mk("build_room", "Create a new room in an area. Areas are organizational containers that group rooms. IMPORTANT: The 'area' parameter must be an AREA object (like #38), NOT a room object. To build in the same area as an existing room, omit the area parameter or use 'here'.", ["type" -> "object", "properties" -> ["name" -> ["type" -> "string", "description" -> "Room name"], "area" -> ["type" -> "string", "description" -> "AREA object number to build in (like #38). MUST be an area, NOT a room. Use 'here' for current area, 'ether' for free-floating, or omit entirely to default to current area. NEVER pass a room object here."], "parent" -> ["type" -> "string", "description" -> "Parent room object reference (optional, default: $room)"]], "required" -> {"name"}], target_obj, "build_room", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("dig_passage", "Create a passage between two rooms. Can be one-way or bidirectional. ALWAYS use object numbers for room references.", ["type" -> "object", "properties" -> ["source_room" -> ["type" -> "string", "description" -> "Source room object number (optional, defaults to actor's current location). Use object numbers like #12 or #50"], "direction" -> ["type" -> "string", "description" -> "Exit direction from source room (e.g. 'north', 'up', 'north,n' for aliases)"], "target_room" -> ["type" -> "string", "description" -> "Destination room object number (like #12 or #50). MUST use object number."], "return_direction" -> ["type" -> "string", "description" -> "Return direction (optional, will be inferred if omitted)"], "oneway" -> ["type" -> "boolean", "description" -> "True for one-way passage (default: false)"], "allow_parallel" -> ["type" -> "boolean", "description" -> "Create a separate connection between rooms already connected (default: false)"]], "required" -> {"direction", "target_room"}], target_obj, "dig_passage", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("remove_passage", "Remove/delete a passage between two rooms. Use this to fix duplicate exits or remove unwanted connections.", ["type" -> "object", "properties" -> ["source_room" -> ["type" -> "string", "description" -> "Source room object number (optional, defaults to actor's current location)"], "target_room" -> ["type" -> "string", "description" -> "Target room object number to remove passage to"], "direction" -> ["type" -> "string", "description" -> "Source exit name; required when several passages connect the rooms"]], "required" -> {"target_room"}], target_obj, "remove_passage", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("set_passage_description", "Set the narrative description for a passage/exit. This description integrates into the room's look description when ambient mode is enabled.", ["type" -> "object", "properties" -> ["direction" -> ["type" -> "string", "description" -> "Direction/exit label (e.g. 'north', 'up')"], "description" -> ["type" -> "string", "description" -> "Narrative description for the passage (e.g. 'A dark archway opens to the north')"], "format" -> ["type" -> "string", "enum" -> {"literal", "djot", "template"}, "description" -> "Description syntax. Omit to preserve the current syntax."], "source_room" -> ["type" -> "string", "description" -> "Source room (optional, defaults to actor's current location)"], "ambient" -> ["type" -> "boolean", "description" -> "If true, description integrates into room description. If false, shows in exits list (default: true)"]], "required" -> {"direction", "description"}], target_obj, "set_passage_description", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("create_object", "Create a new object from a parent prototype.", ["type" -> "object", "properties" -> ["parent" -> ["type" -> "string", "description" -> "Parent object (e.g. '$thing', '$wearable')"], "name" -> ["type" -> "string", "description" -> "Primary name"], "aliases" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Optional alias names"]], "required" -> {"parent", "name"}], target_obj, "create_object", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("recycle_object", "Permanently destroy an object. Cannot be undone.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to recycle"]], "required" -> {"object"}], target_obj, "recycle_object", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("rename_object", "Change an object's name and aliases.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to rename"], "name" -> ["type" -> "string", "description" -> "New name (can include aliases like 'name:alias1,alias2')"]], "required" -> {"object", "name"}], target_obj, "rename_object", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("describe_object", "Set an object's description text.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to describe"], "description" -> ["type" -> "string", "description" -> "New description text"]], "required" -> {"object", "description"}], target_obj, "describe_object", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("move_object", "Move an object to a new location. Can move objects to rooms, players, or containers.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to move (name or object number)"], "destination" -> ["type" -> "string", "description" -> "Destination location (room, player, or container - name or object number)"]], "required" -> {"object", "destination"}], target_obj, "move_object", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("set_integrated_description", "Set an object's integrated description - a description that becomes part of the room's description when the object is present. Use this for atmospheric objects like furniture, decorations, or features that should feel like part of the room.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object to set integrated description on"], "integrated_description" -> ["type" -> "string", "description" -> "The integrated description text (or empty string to clear)"]], "required" -> {"object", "integrated_description"}], target_obj, "set_integrated_description", 'mutation)};
    let bounded = {};
    for tool in (tools)
      let schema = tool.parameters;
      let fields = schema["properties"];
      for field in (mapkeys(fields))
        let definition = fields[field];
        if (`definition["type"] ! E_RANGE => ""' == "string")
          definition["maxLength"] = field in {"description", "integrated_description"} ? 8192 | 256;
        elseif (`definition["type"] ! E_RANGE => ""' == "array")
          definition["maxItems"] = 32;
          definition["items"] = ["type" -> "string", "maxLength" -> 256];
        endif
        fields[field] = definition;
      endfor
      fields["rationale"] = ["type" -> "string", "maxLength" -> 4096];
      schema["properties"] = fields;
      schema["additionalProperties"] = false;
      bounded = {@bounded, $llm_agent_tool:mk(tool.name, tool.description, schema,
        this, tool.target_verb, tool.effect)};
    endfor
    return bounded;
  endmethod

  method _snapshot owner: ARCH_WIZARD
    "Capture the object, destination, and connection state relevant to a building proposal.";
    const {actor, target, kind, member} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    if (kind == 'construction)
      const {prototype, area} = member;
      const parent_state = $llm_tool_provider:_snapshot(actor, prototype, 'object, "");
      if (!valid(area))
        return {parent_state, area};
      endif
      let rooms = [];
      for room in (area.contents)
        rooms[room] = {room.name, room.owner};
      endfor
      return {parent_state, area, area.owner, rooms};
    elseif (kind == 'connections)
      const {destination, area} = member;
      let endpoints = {};
      for endpoint in ({target, destination})
        let passages = [];
        for passage in ($room:passages(endpoint))
          passages[passage.connection_id] = passage;
        endfor
        endpoints = {@endpoints, {endpoint, endpoint.owner, endpoint.location,
          endpoint.passage_areas, passages}};
      endfor
      return {@endpoints, area, area.owner};
    elseif (kind == 'move)
      return {$llm_tool_provider:_snapshot(actor, target, 'object, ""),
        $llm_tool_provider:_snapshot(actor, member, 'object, "")};
    elseif (kind == 'rename)
      return {$llm_tool_provider:_snapshot(actor, target, 'object, ""), target.aliases};
    endif
    return pass(@args);
  endmethod

  method build_room owner: ARCH_WIZARD
    "Tool: Create a new room";
    let area_spec = 0;
    let area_str = 0;
    let area_target = 0;
    let cap = 0;
    let current_room = 0;
    let new_room = 0;
    let parent_spec = 0;
    let room_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {room_name, area_spec, parent_spec} = {args_map["name"], maphaskey(args_map, "area") ? args_map["area"] | "", maphaskey(args_map, "parent") ? args_map["parent"] | "$room"};
    const parent_obj = this:_resolve_object(parent_spec, actor);
    (typeof(parent_obj) == TYPE_OBJ && valid(parent_obj) && isa(parent_obj, $room))
      || raise(E_INVARG, "The parent must be a room prototype.");
    typeof(room_name) == TYPE_STR && room_name:trim() != "" || raise(E_INVARG, "A room needs a name.");
    "Parse area - default to current area if not specified, 'ether' means free-floating";
    let target_area = #-1;
    if (typeof(area_spec) == TYPE_OBJ)
      valid(area_spec) || raise(E_INVARG, "Area no longer exists.");
      target_area = area_spec;
    elseif (!area_spec || area_spec == "" || area_spec == "here")
      current_room = actor.location;
      valid(current_room) && (target_area = current_room.location);
    elseif (area_spec != "ether")
      target_area = this:_resolve_object(area_spec, actor);
      valid(target_area) || raise(E_INVARG, "Area not found.");
    endif
    if (valid(target_area))
      isa(target_area, $area) || raise(E_INVARG, "Choose an area, or use here for the current area.");
      $area:check_room_name(target_area, room_name);
    endif
    const approval_target = valid(target_area) ? target_area | parent_obj;
    const preview = this:_preview(actor, approval_target, 'construction, {parent_obj, target_area});
    if (valid(target_area))
      cap = actor:find_capability_for(target_area, 'area);
      area_target = typeof(cap) == TYPE_FLYWEIGHT ? cap | target_area;
      try
        $root:_check_permissions_as(area_target, actor, 'add_room);
      except (E_PERM)
        return $llm_tool_result:mk('failure, "Permission denied: "
          + $grant_utils:format_denial(target_area, 'area, {'add_room}), "permission_denied");
      endtry
    endif
    const approval = this:_approve(actor, 'build_room, approval_target, 'construction,
      {parent_obj, target_area}, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    "Create room";
    if (!valid(target_area))
      new_room = parent_obj:create();
      area_str = " (free-floating)";
    else
      new_room = area_target:make_room_in(parent_obj, room_name);
      area_str = " in " + tostr(target_area);
    endif
    new_room:set_name_aliases(room_name, {});
    return "Created \"" + room_name + "\" (" + tostr(new_room) + ")" + area_str + ".";
  endmethod

  method dig_passage owner: ARCH_WIZARD
    "Tool: Create a passage between two rooms";
    let direction = 0;
    let oneway_flag = 0;
    let return_dir = 0;
    let source_spec = 0;
    let target_spec = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {source_spec, direction, target_spec, return_dir, oneway_flag} = {maphaskey(args_map, "source_room") ? args_map["source_room"] | "", args_map["direction"], args_map["target_room"], maphaskey(args_map, "return_direction") ? args_map["return_direction"] | "", maphaskey(args_map, "oneway") ? args_map["oneway"] | false};
    const directions = $passage:parse_directions(direction, return_dir, oneway_flag);
    const from_dirs = directions['from_dir];
    const to_dirs = directions['to_dir];
    const parallel = `args_map["allow_parallel"] ! E_RANGE => false';
    typeof(parallel) == TYPE_BOOL || raise(E_TYPE);
    "Find source room - default to actor's location if not specified";
    const source_room = this:_resolve_object(source_spec, actor, actor.location);
    typeof(source_room) == TYPE_OBJ || raise(E_INVARG, "Source room not found");
    valid(source_room) || raise(E_INVARG, "Source room no longer exists");
    const target_room = this:_resolve_object(target_spec, actor);
    typeof(target_room) == TYPE_OBJ || raise(E_INVARG, "Target room not found");
    valid(target_room) || raise(E_INVARG, "Target room no longer exists");
    "Use the source area as the connection store.";
    const area = source_room.location;
    valid(area) || raise(E_INVARG, "Source room is not in an area");
    const preview = this:_preview(actor, source_room, 'connections, {target_room, area});
    "Check permissions";
    const from_cap = actor:find_capability_for(source_room, 'room);
    const from_target = typeof(from_cap) == TYPE_FLYWEIGHT ? from_cap | source_room;
    try
      from_target:check_can_dig_from();
    except (E_PERM)
      return $llm_tool_result:mk('failure, "Permission denied: " + $grant_utils:format_denial(source_room, 'room, {'dig_from}), "permission_denied");
    endtry
    const to_cap = actor:find_capability_for(target_room, 'room);
    const to_target = typeof(to_cap) == TYPE_FLYWEIGHT ? to_cap | target_room;
    try
      to_target:check_can_dig_into();
    except (E_PERM)
      return $llm_tool_result:mk('failure, "Permission denied: " + $grant_utils:format_denial(target_room, 'room, {'dig_into}), "permission_denied");
    endtry
    const approval = this:_approve(actor, 'dig_passage, source_room, 'connections, {target_room, area}, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    "Create passage";
    let passage = oneway_flag || !to_dirs ? $passage:mk(source_room, from_dirs[1], from_dirs, "", true, target_room, "", {}, "", false, true) | $passage:mk(source_room, from_dirs[1], from_dirs, "", true, target_room, to_dirs[1], to_dirs, "", true, true);
    "Register with area";
    const area_cap = actor:find_capability_for(area, 'area);
    const area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
    passage = area_target:create_passage(from_target, to_target, passage, parallel);
    "Report";
    const msg = oneway_flag ? "Dug passage: " + from_dirs:join(",") + " to " + tostr(target_room) + " (one-way)." | !to_dirs ? "Dug passage: " + from_dirs:join(",") + " to " + tostr(target_room) + " (one-way - no return direction inferred)." | "Dug passage: " + from_dirs:join(",") + " | " + to_dirs:join(",") + " connecting to " + tostr(target_room) + ".";
    return msg;
  endmethod

  method remove_passage owner: ARCH_WIZARD
    "Remove the selected installed connection, including one whose endpoint moved to another area.";
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const source_room = this:_resolve_object(`args_map["source_room"] ! E_RANGE => ""', actor, actor.location);
    const target_room = this:_resolve_object(args_map["target_room"], actor);
    valid(source_room) && valid(target_room) || raise(E_INVARG, "Endpoint no longer exists.");
    const direction = `args_map["direction"] ! E_RANGE => ""';
    const passage = direction == "" ? $room:passage_to(source_room, target_room)
      | $room:find_passage(source_room, direction);
    typeof(passage) == TYPE_FLYWEIGHT || return $llm_tool_result:mk('failure,
      "No passage found between " + tostr(source_room) + " and " + tostr(target_room) + ".", "passage_missing");
    passage:other_room(source_room) == target_room || raise(E_INVARG, "That exit leads to a different room.");
    const area = $room:find_connection(source_room, passage.connection_id)[1];
    const preview = this:_preview(actor, source_room, 'connections, {target_room, area});
    const source = $area:_subject_for(actor, source_room, 'room);
    const store = $area:_subject_for(actor, area, 'area);
    try
      source:check_can_dig_from();
    except (E_PERM)
      return $llm_tool_result:mk('failure, "Permission denied: "
        + $grant_utils:format_denial(source_room, 'room, {'dig_from}), "permission_denied");
    endtry
    const approval = this:_approve(actor, 'remove_passage, source_room, 'connections,
      {target_room, area}, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    const removed = store:remove_passage(source, target_room, passage.connection_id);
    removed || return $llm_tool_result:mk('failure, "The passage was not removed.", "passage_missing");
    return "Removed passage between " + tostr(source_room) + " and " + tostr(target_room) + ".";
  endmethod

  method set_passage_description owner: ARCH_WIZARD
    "Describe a passage with the same explicit formats and endpoint checks as builder commands.";
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const direction = args_map["direction"];
    const description = args_map["description"];
    const room = this:_resolve_object(`args_map["source_room"] ! E_RANGE => ""', actor, actor.location);
    valid(room) && isa(room, $room) || raise(E_INVARG, "Source room not found.");
    const passage = $room:find_passage(room, direction);
    typeof(passage) == TYPE_FLYWEIGHT || raise(E_INVARG, "No passage called '" + direction + "' here.");
    const {area, current} = $room:find_connection(room, passage.connection_id);
    const destination = current.side_a_room == room ? current.side_b_room | current.side_a_room;
    const preview = this:_preview(actor, room, 'connections, {destination, area});
    let kind = maphaskey(args_map, "format") ? tosym(args_map["format"]) | current:description_kind_for(room);
    kind == 'djot && (kind = 'djot_inline);
    const ambient = `args_map["ambient"] ! E_RANGE => true';
    const updated = current:with_description_from(room, description, kind):with_ambient_from(room, ambient);
    const subject = $area:_subject_for(actor, room, 'room);
    const approval = this:_approve(actor, 'set_passage_description, room, 'connections,
      {destination, area}, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    area:update_passage(subject, destination, updated);
    return "Set description for '" + direction + "' passage.";
  endmethod

  method create_object owner: ARCH_WIZARD
    "Tool: Create an object from a parent";
    let extra_aliases = 0;
    let name_spec = 0;
    let parent_spec = 0;
    let parsed_aliases = 0;
    let primary_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {parent_spec, name_spec, extra_aliases} = {args_map["parent"], args_map["name"], maphaskey(args_map, "aliases") ? args_map["aliases"] | {}};
    {primary_name, parsed_aliases} = $str_proto:parse_name_aliases(name_spec);
    const final_aliases = {@parsed_aliases, @extra_aliases};
    !primary_name && raise(E_INVARG, "Object name cannot be blank");
    set_task_perms(actor);
    const parent_obj = this:_resolve_object(parent_spec, actor);
    typeof(parent_obj) == TYPE_OBJ || raise(E_INVARG, "Parent not found");
    valid(parent_obj) || raise(E_INVARG, "Parent no longer exists");
    "Use fertile flag (f) to allow child creation";
    const is_fertile = parent_obj.f;
    !is_fertile && !actor.wizard && parent_obj.owner != actor && raise(E_PERM, "You do not have permission to create children of " + tostr(parent_obj));
    const approval = this:_approve(actor, 'create_object, parent_obj, 'object, "", args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    const new_obj = create(parent_obj, actor, 2);
    try
      new_obj.r = 1;
      new_obj:set_name_aliases(primary_name, final_aliases);
      strcmp(new_obj.name, primary_name) == 0 && new_obj.aliases == final_aliases
        || raise(E_INVARG, "The new object did not retain its requested name and aliases.");
      $llm_tool_provider:check(this, actor, actor);
      const moved = new_obj:moveto(actor);
      typeof(moved) != TYPE_ERR || raise(error_code(moved), "Could not place the new object.");
      new_obj.location == actor || raise(E_INVARG, "Could not place the new object.");
    except error (ANY)
      "Only this newly allocated object belongs to cleanup; never recycle an arbitrary callback result.";
      if (valid(new_obj))
        try
          new_obj:destroy();
        except cleanup (ANY)
          return $llm_tool_result:mk('unknown, "Creation failed; cleanup also failed for "
            + tostr(new_obj) + ": " + tostr(cleanup[2]), "creation_cleanup_failed");
        endtry
        !valid(new_obj) || return $llm_tool_result:mk('unknown,
          "Creation failed; the object still exists: " + tostr(new_obj), "creation_cleanup_failed");
      endif
      raise(error[1], error[2]);
    endtry
    let message = "Created \"" + primary_name + "\" (" + tostr(new_obj) + ") from " + tostr(parent_obj) + " in your inventory.";
    final_aliases && (message = message + " Aliases: " + final_aliases:join(", ") + ".");
    return message;
  endmethod

  method recycle_object owner: ARCH_WIZARD
    "Tool: Permanently destroy an object";
    let obj_id = 0;
    let obj_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to recycle " + tostr(target_obj));
    {obj_name, obj_id} = {target_obj.name, tostr(target_obj)};
    const approval = this:_approve(actor, 'recycle_object, target_obj, 'object, "", args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    target_obj:destroy();
    !valid(target_obj) || return $llm_tool_result:mk('unknown,
      "The object still exists after its destruction callback.", "recycle_incomplete");
    return "Recycled \"" + obj_name + "\" (" + obj_id + ").";
  endmethod

  method rename_object owner: ARCH_WIZARD
    "Tool: Rename an object";
    let new_aliases = 0;
    let new_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to rename " + tostr(target_obj));
    {new_name, new_aliases} = $str_proto:parse_name_aliases(args_map["name"]);
    !new_name && raise(E_INVARG, "Object name cannot be blank");
    const old_name = target_obj.name;
    const approval = this:_approve(actor, 'rename_object, target_obj, 'rename, "", args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    target_obj:set_name_aliases(new_name, new_aliases);
    let message = "Renamed \"" + old_name + "\" (" + tostr(target_obj) + ") to \"" + new_name + "\".";
    new_aliases && (message = message + " Aliases: " + new_aliases:join(", ") + ".");
    return message;
  endmethod

  method describe_object owner: ARCH_WIZARD
    "Tool: Set object description";
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to describe " + tostr(target_obj));
    !args_map["description"] && raise(E_INVARG, "Description cannot be blank");
    const approval = this:_approve(actor, 'describe_object, target_obj, 'property, "description", args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    target_obj.description = args_map["description"];
    return "Set description of \"" + target_obj.name + "\" (" + tostr(target_obj) + ").";
  endmethod

  method move_object owner: ARCH_WIZARD
    "Tool: Move an object to a new location";
    let dest_spec = 0;
    let obj_spec = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
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
    const approval = this:_approve(actor, 'move_object, target_obj, 'move, dest_obj, args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    const result = target_obj:moveto(dest_obj);
    typeof(result) != TYPE_ERR || raise(error_code(result), "Could not move object.");
    target_obj.location == dest_obj || raise(E_INVARG, "Could not move object.");
    return "Moved \"" + target_obj.name + "\" (" + tostr(target_obj) + ") from " + old_location_name + " to \"" + dest_obj.name + "\" (" + tostr(dest_obj) + ").";
  endmethod

  method set_integrated_description owner: ARCH_WIZARD
    "Tool: Set object's integrated description";
    let {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You do not have permission to modify " + tostr(target_obj));
    const approval = this:_approve(actor, 'set_integrated_description, target_obj, 'property, "integrated_description", args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    target_obj.integrated_description = args_map["integrated_description"];
    const obj_name = target_obj.name;
    return args_map["integrated_description"] == "" ? "Cleared integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + ")." | "Set integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + "). When in a room, this will appear in the room description.";
  endmethod

endobject
