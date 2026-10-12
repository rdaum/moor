object BUILDER_FEATURES [
  import_export_id -> "builder_features",
  import_export_hierarchy -> {"features"}
]
  name: "Builder Features"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  property direction_opposites (owner: HACKER, flags: "rc") = [
    "d" -> "u",
    "down" -> "up",
    "downstairs" -> "upstairs",
    "e" -> "w",
    "east" -> "west",
    "i" -> "o",
    "in" -> "out",
    "n" -> "s",
    "ne" -> "sw",
    "north" -> "south",
    "northeast" -> "southwest",
    "northwest" -> "southeast",
    "nw" -> "se",
    "o" -> "i",
    "out" -> "in",
    "s" -> "n",
    "se" -> "nw",
    "south" -> "north",
    "southeast" -> "northwest",
    "southwest" -> "northeast",
    "sw" -> "ne",
    "u" -> "d",
    "up" -> "down",
    "upstairs" -> "downstairs",
    "w" -> "e",
    "west" -> "east"
  ];
  property help_source (owner: ARCH_WIZARD, flags: "rc") = BUILDER_HELP_TOPICS;

  override description (owner: HACKER, flags: "rc") = "Provides building commands (@create, @build, @dig, etc.) for builders.";

  method _require_builder_command owner: ARCH_WIZARD
    caller == this || raise(E_PERM);
    {who} = args;
    who.is_builder || raise(E_PERM, "Builder features required.");
  endmethod

  verb "@add-message" (any any any) owner: HACKER flags: "rd"
    "HINT: <object>.<property> <template> -- Add an entry to a message bag.";
    "Add a compiled message template entry to a message bag property. Usage: @add-message <object>.<prop> <template>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (length(args) < 2)
      player:inform_current($event:mk_error(player, "Usage: @add-message <object>.<prop> <template>"):with_audience('utility));
      return;
    endif
    prop_spec = args[1];
    parsed = $prog_utils:parse_target_spec(prop_spec);
    if (parsed && parsed['type] == 'property)
      target_name = parsed['object_str];
      prop_name = parsed['item_name];
    elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
      sel = parsed['selectors][1];
      sel['kind] == 'property || raise(E_INVARG, "Property must be in the form object.property");
      target_name = parsed['object_str];
      prop_name = sel['item_name];
    else
      raise(E_INVARG, "Property must be in the form object.property");
    endif
    target = $match:match_object(target_name, player);
    valid(target) || raise(E_INVARG, "Object not found");
    prop_name:ends_with("_msg_bag") || prop_name:ends_with("_msgs") || raise(E_INVARG, "Property must end with _msg_bag or _msgs");
    "Preserve full template text after target spec";
    offset = index(argstr, prop_spec) + length(prop_spec);
    text = argstr[offset..length(argstr)]:trim();
    !text && raise(E_INVARG, "Usage: @add-message <object>.<prop> <template>");
    {success, compiled} = $obj_utils:validate_and_compile_template(text);
    if (!success)
      player:inform_current($event:mk_error(player, "Template compilation failed: " + compiled):with_audience('utility));
      return;
    endif
    prop_key = $obj_utils:property_key(target, prop_name);
    grants = prop_key == E_PROPNF ? {{"property_define", target}} | {{"property_write", target, prop_key}};
    $obj_utils:add_message_entry(target, prop_name, compiled, player, grants);
    player:inform_current($event:mk_info(player, "Added message to " + tostr(target) + "." + prop_name):with_audience('utility));
  endverb

  verb "@del-message" (any any any) owner: HACKER flags: "rd"
    "HINT: <object>.<property> <index> -- Remove one message bag entry.";
    "Remove a message entry by index from a message bag property. Usage: @del-message <object>.<prop> <index>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (length(args) < 2)
      player:inform_current($event:mk_error(player, "Usage: @del-message <object>.<prop> <index>"):with_audience('utility));
      return;
    endif
    prop_spec = args[1];
    idx = toint(args[2]);
    parsed = $prog_utils:parse_target_spec(prop_spec);
    if (parsed && parsed['type] == 'property)
      target_name = parsed['object_str];
      prop_name = parsed['item_name];
    elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
      sel = parsed['selectors][1];
      sel['kind] == 'property || raise(E_INVARG, "Property must be in the form object.property");
      target_name = parsed['object_str];
      prop_name = sel['item_name];
    else
      raise(E_INVARG, "Property must be in the form object.property");
    endif
    target = $match:match_object(target_name, player);
    valid(target) || raise(E_INVARG, "Object not found");
    prop_name:ends_with("_msg_bag") || prop_name:ends_with("_msgs") || raise(E_INVARG, "Property must end with _msg_bag or _msgs");
    prop_key = $obj_utils:property_key(target, prop_name);
    prop_key == E_PROPNF && raise(E_INVARG, "Message bag not found on " + tostr(target) + "." + prop_name);
    $obj_utils:remove_message_entry(target, prop_key, idx, player, {{"property_write", target, prop_key}});
    player:inform_current($event:mk_info(player, "Removed message #" + tostr(idx) + " from " + tostr(target) + "." + prop_name):with_audience('utility));
  endverb

  verb "@create" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <parent> named <name> -- Create a new object.";
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "named") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@create PARENT named NAME[:ALIASES]")));
      return;
    endif
    try
      parent_obj = $match:match_object(dobjstr, player);
      typeof(parent_obj) != TYPE_OBJ && raise(E_INVARG, "That parent reference is not an object.");
      !valid(parent_obj) && raise(E_INVARG, "That parent object no longer exists.");
      if (!parent_obj.f && !player.wizard && parent_obj.owner != player)
        raise(E_PERM, "You do not have permission to create children of " + tostr(parent_obj) + ".");
      endif
      parsed = $str_proto:parse_name_aliases(iobjstr);
      primary_name = parsed[1];
      alias_list = parsed[2];
      !primary_name && raise(E_INVARG, "Primary object name cannot be blank.");
      new_obj = this:_create_child_object(player, parent_obj, primary_name, alias_list);
      object_id = tostr(new_obj);
      parent_name = `parent_obj.name ! ANY => tostr(parent_obj)';
      parent_id = tostr(parent_obj);
      message = "Created \"" + primary_name + "\" (" + object_id + ") as a child of \"" + parent_name + "\" (" + parent_id + ").";
      if (alias_list)
        alias_str = alias_list:join(", ");
        message = message + " Aliases: " + alias_str + ".";
      endif
      message = message + " It is now in your inventory.";
      player:inform_current($event:mk_info(player, message));
      return new_obj;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method _create_child_object owner: ARCH_WIZARD
    "Create the child object, apply naming, and move it into the builder's inventory.";
    caller == this || caller.wizard || raise(E_PERM);
    {builder_player, parent_obj, primary_name, alias_list} = args;
    set_task_perms(builder_player);
    new_obj = parent_obj:create();
    try
      new_obj:set_name_aliases(primary_name, alias_list);
      move_result = new_obj:moveto(builder_player);
      if (typeof(move_result) == TYPE_ERR)
        raise(error_code(move_result), "Could not move new object into builder inventory.");
      endif
      new_obj.location == builder_player || raise(E_INVARG, "Could not move new object into builder inventory.");
    except e (ANY)
      valid(new_obj) && new_obj:destroy();
      raise(e[1], length(e) >= 2 ? e[2] | "Could not create child object.");
    endtry
    return new_obj;
  endmethod

  verb "@recycle @destroy" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> -- Destroy an object permanently.";
    player.is_builder || raise(E_PERM);
    set_task_perms(player);
    if (!dobjstr)
      raise(E_INVARG, "Usage: @recycle OBJECT");
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to recycle " + tostr(target_obj) + ".");
      endif
      obj_name = target_obj.name;
      obj_id = tostr(target_obj);
      target_obj:destroy();
      player:inform_current($event:mk_info(player, "Recycled \"" + obj_name + "\" (" + obj_id + ")."));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@grant" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<category>(<caps>) to <player> -- Grant capabilities.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "to") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@grant TARGET.CATEGORY(CAP1,CAP2) to PLAYER")));
      return;
    endif
    try
      "Parse grant specification using $grant_utils";
      {target_obj, category, cap_list} = $grant_utils:parse_grant(dobjstr);
      "Permission check - must be owner or wizard";
      !player.wizard && target_obj.owner != player && raise(E_PERM, "You must be owner or wizard to grant capabilities for " + tostr(target_obj) + ".");
      "Match grantee - use iobj from parser, or fall back to match_object if parser failed";
      if (iobj == $failed_match)
        grantee = $match:match_object(iobjstr, player);
      else
        grantee = iobj;
      endif
      if (typeof(grantee) != TYPE_OBJ)
        raise(E_INVARG, "Grantee must be an object.");
      endif
      if (grantee == #-1 || grantee == #-2 || grantee == #-3)
        raise(E_INVARG, "Could not find player");
      endif
      if (!valid(grantee))
        raise(E_INVARG, "Grantee no longer exists.");
      endif
      "Prepare success display before mutating grant state.";
      grant_display = $grant_utils:format_grant_with_name(target_obj, category, cap_list);
      grantee_str = grantee:name() + " (" + tostr(grantee) + ")";
      "Grant the capability";
      cap = $root:grant_capability(target_obj, cap_list, grantee, category);
      "Report success using formatted grant spec";
      message = "Granted " + grant_display + " to " + grantee_str + ".";
      player:inform_current($event:mk_info(player, message));
      return cap;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@audit @owned" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: [<player>] -- Show objects owned by a player.";
    caller == player || raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    "Determine target player";
    target = dobjstr ? $match:match_player(dobjstr, player) | player;
    "Get owned objects";
    owned = sort(owned_objects(target));
    target_name = `target.name ! ANY => tostr(target)';
    if (!owned)
      message = target == player ? "You don't own any objects." | target_name + " doesn't own any objects.";
      player:inform_current($event:mk_info(player, message));
      return 0;
    endif
    "Build table, skipping non-readable objects when auditing others";
    headers = {"Name", "Parent", "Location", "Flags"};
    rows = {};
    skipped = 0;
    for item in (owned)
      "Skip non-readable objects when auditing another player";
      if (target != player && !item.r)
        skipped = skipped + 1;
        continue;
      endif
      obj_name = `item.name ! ANY => "(no name)"';
      name_with_id = obj_name + " (" + tostr(item) + ")";
      parent = parent(item);
      parent_str = valid(parent) ? `parent.name ! ANY => tostr(parent)' + " (" + tostr(parent) + ")" | "none";
      loc = `item.location ! ANY => #-1';
      loc_str = valid(loc) ? `loc.name ! ANY => tostr(loc)' + " (" + tostr(loc) + ")" | "Nowhere";
      "Build flags string: r=readable w=write f=fertile";
      flags = "";
      item.r && (flags = flags + "r");
      item.w && (flags = flags + "w");
      item.f && (flags = flags + "f");
      rows = {@rows, {name_with_id, parent_str, loc_str, flags}};
    endfor
    "Output results";
    if (!rows)
      message = target_name + " has no readable objects.";
      player:inform_current($event:mk_info(player, message));
      return 0;
    endif
    {min_id, max_id} = {owned[1], owned[$]};
    header = "Objects owned by " + target_name + " (from " + tostr(min_id) + " to " + tostr(max_id) + "):";
    player:inform_current($event:mk_info(player, header));
    player:inform_current($event:mk_info(player, $format.table:mk(headers, rows)));
    count = length(rows);
    footer = tostr(count) + " object" + (count == 1 ? "" | "s") + ".  Flags: r=readable w=write f=fertile";
    if (skipped > 0)
      footer = footer + "  (" + tostr(skipped) + " non-readable object" + (skipped == 1 ? "" | "s") + " hidden)";
    endif
    player:inform_current($event:mk_info(player, footer));
    return count;
  endverb

  verb "@build" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <name> [in <area>] -- Create a new room.";
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @build NAME [in AREA] [as PARENT]");
    endif
    try
      "Parse the command string";
      result = this:_parse_build_command(argstr);
      room_name = result['name];
      target_area = result['area];
      parent_obj = result['parent];
      "Reject duplicate room names in the target area";
      if (valid(target_area))
        this:_require_unique_room_name(target_area, room_name);
      endif
      "Create and place the room";
      if (valid(target_area))
        "Use capability if we have one, otherwise use area directly";
        cap = player:find_capability_for(target_area, 'area);
        "Use capability if found, otherwise use area directly";
        area_target = typeof(cap) == TYPE_FLYWEIGHT ? cap | target_area;
        try
          new_room = area_target:make_room_in(parent_obj);
          area_str = " in " + tostr(target_area);
        except (E_PERM)
          message = $grant_utils:format_denial(target_area, 'area, {'add_room});
          raise(E_PERM, message);
        endtry
      else
        "Free-floating room - create directly";
        new_room = parent_obj:create();
        area_str = " (free-floating)";
      endif
      new_room:set_name_aliases(room_name, {});
      "Report success";
      message = "Created \"" + room_name + "\" (" + tostr(new_room) + ")" + area_str + ".";
      player:inform_current($event:mk_info(player, message));
      return new_room;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method _require_unique_room_name owner: ARCH_WIZARD
    "Raise if target_area already contains a room with room_name.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {target_area, room_name} = args;
    for existing_room in (target_area.contents)
      if (!valid(existing_room))
        raise(E_INVARG, "Invalid object in area contents while checking duplicate room names: " + toliteral(existing_room));
      endif
      try
        existing_name = existing_room:name();
      except e (ANY)
        raise(E_INVARG, "Could not read room name for " + tostr(existing_room) + ": " + toliteral(e));
      endtry
      if (existing_name == room_name)
        raise(E_INVARG, "A room named \"" + room_name + "\" already exists in " + tostr(target_area) + ".");
      endif
    endfor
    return true;
  endmethod

  method _parse_build_command owner: ARCH_WIZARD
    "Parse @build command arguments. Returns map with 'name, 'area, 'parent.";
    caller == this || raise(E_PERM);
    {command_str} = args;
    command_str = command_str:trim();
    "Defaults";
    current_room = player.location;
    target_area = valid(current_room) ? current_room.location | #-1;
    parent_obj = $room;
    working = command_str;
    "Tokenize while preserving quoted segments for robust 'in/as' parsing";
    tokens = {};
    buf = "";
    in_quotes = false;
    for i in [1..length(working)]
      ch = working[i];
      if (ch == "\"")
        in_quotes = !in_quotes;
        continue;
      endif
      if (ch == " " && !in_quotes)
        if (buf != "")
          tokens = {@tokens, buf};
          buf = "";
        endif
      else
        buf = buf + ch;
      endif
    endfor
    buf != "" && (tokens = {@tokens, buf});
    "Parse trailing 'as <parent>'";
    if (length(tokens) >= 2 && tokens[$ - 1]:lowercase() == "as")
      parent_spec = tokens[$];
      parent_obj = $match:match_object(parent_spec, player);
      typeof(parent_obj) != TYPE_OBJ && raise(E_INVARG, "That parent reference is not an object.");
      !valid(parent_obj) && raise(E_INVARG, "That parent object no longer exists.");
      tokens = tokens[1..$ - 2];
    endif
    "Parse trailing 'in <area>'";
    if (length(tokens) >= 2 && tokens[$ - 1]:lowercase() == "in")
      area_spec = tokens[$];
      if (area_spec:lowercase() == "ether")
        target_area = #-1;
      else
        target_area = $match:match_object(area_spec, player);
        typeof(target_area) != TYPE_OBJ && raise(E_INVARG, "That area reference is not an object.");
        !valid(target_area) && raise(E_INVARG, "That area no longer exists.");
      endif
      tokens = tokens[1..$ - 2];
    endif
    name_part = tokens ? tokens:join(" ") | "";
    "Parse room name using same logic as @create";
    parsed = $str_proto:parse_name_aliases(name_part);
    room_name = parsed[1];
    !room_name && raise(E_INVARG, "Room name cannot be blank.");
    return ['name -> room_name, 'area -> target_area, 'parent -> parent_obj];
  endmethod

  verb "@dig @tunnel" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <dir> to <room> -- Create a passage to an existing room. Supports --dry-run and --allow-parallel.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "to") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@dig [--dry-run] [--allow-parallel] [oneway] DIR[|RETURNDIR] to ROOM")));
      return;
    endif
    try
      "Parse the direction spec and options";
      result = this:_parse_dig_command(dobjstr);
      is_oneway = result['oneway];
      from_dir = result['from_dir];
      to_dir = result['to_dir];
      dry_run = maphaskey(result, 'dry_run) ? result['dry_run] | false;
      allow_parallel = maphaskey(result, 'allow_parallel) ? result['allow_parallel] | false;
      "Get current room and area first for scoped matching";
      current_room = player.location;
      !valid(current_room) && raise(E_INVARG, "You must be in a room to dig passages.");
      area = current_room.location;
      !valid(area) && raise(E_INVARG, "Your current room is not in an area.");
      "Find target room - only accept iobj if it's actually in this area";
      if (typeof(iobj) == TYPE_OBJ && valid(iobj) && iobj in area.contents)
        target_room = iobj;
      else
        target_room = $match:resolve_in_scope(iobjstr, area.contents);
        if (typeof(target_room) != TYPE_OBJ)
          if (target_room == $ambiguous_match)
            candidates = this:_matching_room_candidates(area, iobjstr);
            if (candidates)
              max_show = length(candidates) > 5 ? 5 | length(candidates);
              shown = candidates[1..max_show];
              tail = length(candidates) > 5 ? ", ..." | "";
              raise(E_INVARG, "Ambiguous room match for '" + iobjstr + "' in this area. Candidates: " + shown:join(", ") + tail);
            else
              raise(E_INVARG, "Ambiguous room match for '" + iobjstr + "' in this area. Use a more specific name or object id.");
            endif
          else
            raise(E_INVARG, "No room found matching '" + iobjstr + "' in this area.");
          endif
        endif
        !valid(target_room) && raise(E_INVARG, "That room no longer exists.");
      endif
      "Check permissions on both rooms using capabilities if we have them";
      from_room_cap = player:find_capability_for(current_room, 'room);
      from_room_target = typeof(from_room_cap) == TYPE_FLYWEIGHT ? from_room_cap | current_room;
      try
        from_room_target:check_can_dig_from();
      except (E_PERM)
        message = $grant_utils:format_denial(current_room, 'room, {'dig_from});
        raise(E_PERM, message);
      endtry
      to_room_cap = player:find_capability_for(target_room, 'room);
      to_room_target = typeof(to_room_cap) == TYPE_FLYWEIGHT ? to_room_cap | target_room;
      try
        to_room_target:check_can_dig_into();
      except (E_PERM)
        message = $grant_utils:format_denial(target_room, 'room, {'dig_into});
        raise(E_PERM, message);
      endtry
      "Reject duplicate aliases in the requested direction set";
      seen_from = {};
      for alias in (from_dir)
        alias in seen_from && raise(E_INVARG, "Duplicate alias in from-direction: '" + alias + "'.");
        seen_from = {@seen_from, alias};
      endfor
      if (!is_oneway)
        seen_to = {};
        for alias in (to_dir)
          alias in seen_to && raise(E_INVARG, "Duplicate alias in return-direction: '" + alias + "'.");
          seen_to = {@seen_to, alias};
        endfor
      endif
      "Build alias->room map for current room exits";
      existing_from = [];
      for p in (area:passages_from(current_room))
        side_a_room = p.side_a_room;
        side_b_room = p.side_b_room;
        if (current_room == side_a_room)
          aliases = p.side_a_aliases;
          other_room = side_b_room;
        elseif (current_room == side_b_room)
          aliases = p.side_b_aliases;
          other_room = side_a_room;
        else
          aliases = {};
          other_room = #-1;
        endif
        for existing_alias in (aliases)
          !maphaskey(existing_from, existing_alias) && (existing_from[existing_alias] = other_room);
        endfor
      endfor
      "Reject alias conflicts with existing exits from current room";
      for alias in (from_dir)
        if (maphaskey(existing_from, alias))
          conflict_room = existing_from[alias];
          conflict_name = this:_room_name_for_error(conflict_room);
          raise(E_INVARG, "Direction alias '" + alias + "' already exists from this room (to " + conflict_name + " (" + tostr(conflict_room) + ")).");
        endif
      endfor
      "Build alias->room map for target room exits when bidirectional";
      if (!is_oneway)
        existing_to = [];
        for p in (area:passages_from(target_room))
          side_a_room = p.side_a_room;
          side_b_room = p.side_b_room;
          if (target_room == side_a_room)
            aliases = p.side_a_aliases;
            other_room = side_b_room;
          elseif (target_room == side_b_room)
            aliases = p.side_b_aliases;
            other_room = side_a_room;
          else
            aliases = {};
            other_room = #-1;
          endif
          for existing_alias in (aliases)
            !maphaskey(existing_to, existing_alias) && (existing_to[existing_alias] = other_room);
          endfor
        endfor
        for alias in (to_dir)
          if (maphaskey(existing_to, alias))
            conflict_room = existing_to[alias];
            conflict_name = this:_room_name_for_error(conflict_room);
            raise(E_INVARG, "Return direction alias '" + alias + "' already exists from target room (to " + conflict_name + " (" + tostr(conflict_room) + ")).");
          endif
        endfor
      endif
      "Block duplicate passage between same room pair unless explicitly allowed";
      if (!allow_parallel)
        existing_pair = area:passage_for(current_room, target_room);
        if (typeof(existing_pair) == TYPE_FLYWEIGHT || (typeof(existing_pair) == TYPE_OBJ && valid(existing_pair)))
          raise(E_INVARG, "A passage between here and " + tostr(target_room) + " already exists. Use --allow-parallel to create another.");
        endif
      endif
      "Dry-run mode: report planned operation and stop before mutation";
      if (dry_run)
        mode_str = is_oneway ? "one-way" | "bidirectional";
        flags_str = allow_parallel ? "--allow-parallel" | "(no extra flags)";
        preview = "Dry-run OK: " + mode_str + " passage from " + from_dir:join(",") + (is_oneway ? "" | " | " + to_dir:join(",")) + " to " + tostr(target_room) + " " + flags_str + ".";
        player:inform_current($event:mk_info(player, preview));
        return true;
      endif
      "Create the passage flyweight";
      if (is_oneway)
        passage = $passage:mk(current_room, from_dir[1], from_dir, "", true, target_room, "", {}, "", false, true);
      else
        passage = $passage:mk(current_room, from_dir[1], from_dir, "", true, target_room, to_dir[1], to_dir, "", true, true);
      endif
      "Register with area using capability if we have one";
      area_cap = player:find_capability_for(area, 'area);
      area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
      area_target:create_passage(from_room_target, to_room_target, passage);
      "Report success";
      if (is_oneway)
        message = "Dug passage: " + from_dir:join(",") + " to " + tostr(target_room) + " (one-way).";
      else
        message = "Dug passage: " + from_dir:join(",") + " | " + to_dir:join(",") + " connecting to " + tostr(target_room) + ".";
      endif
      player:inform_current($event:mk_info(player, message));
      return passage;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@undig @remove-exit @delete-passage" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <direction> -- Remove a passage from the current room.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!dobjstr)
      raise(E_INVARG, "Usage: @undig DIRECTION or @undig ROOM");
    endif
    try
      "Get current room and its area first";
      current_room = player.location;
      !valid(current_room) && raise(E_INVARG, "You must be in a room to remove passages.");
      area = current_room.location;
      !valid(area) && raise(E_INVARG, "Your current room is not in an area.");
      input = dobjstr:trim();
      target_room = #-1;
      "First, resolve as a direction/alias from this room (same matcher as movement).";
      passage = area:find_passage_by_direction(current_room, input);
      if (typeof(passage) == TYPE_FLYWEIGHT || typeof(passage) == TYPE_OBJ)
        target_room = passage:other_room(current_room);
      endif
      "If no direction match, try matching as room reference in-area.";
      if (!valid(target_room))
        if (typeof(dobj) == TYPE_OBJ && valid(dobj))
          target_room = dobj;
        else
          target_room = $match:resolve_in_scope(input, area.contents);
          if (typeof(target_room) != TYPE_OBJ)
            if (target_room == $ambiguous_match)
              raise(E_INVARG, "Ambiguous room match for '" + input + "' in this area.");
            else
              raise(E_INVARG, "No passage or room found matching '" + input + "'.");
            endif
          endif
        endif
        !valid(target_room) && raise(E_INVARG, "No passage or room found matching '" + input + "'.");
        target_room.location != area && raise(E_INVARG, "Target room must be in the same area.");
        passage = area:passage_for(current_room, target_room);
      endif
      "Look up passage (can be flyweight or $passage object)";
      if (typeof(passage) != TYPE_FLYWEIGHT && (typeof(passage) != TYPE_OBJ || !valid(passage)))
        raise(E_INVARG, "No passage found between here and " + tostr(target_room) + ".");
      endif
      labels = {};
      side_a_room = passage.side_a_room;
      side_b_room = passage.side_b_room;
      if (current_room == side_a_room)
        label = passage.side_a_label;
        if (label != "")
          labels = {@labels, label};
        endif
      elseif (current_room == side_b_room)
        label = passage.side_b_label;
        if (label != "")
          labels = {@labels, label};
        endif
      endif
      if (target_room == side_a_room)
        label = passage.side_a_label;
        if (label != "" && !(label in labels))
          labels = {@labels, label};
        endif
      elseif (target_room == side_b_room)
        label = passage.side_b_label;
        if (label != "" && !(label in labels))
          labels = {@labels, label};
        endif
      endif
      "Check permissions - must have dig_from on current room";
      from_room_cap = player:find_capability_for(current_room, 'room);
      from_room_target = typeof(from_room_cap) == TYPE_FLYWEIGHT ? from_room_cap | current_room;
      try
        from_room_target:check_can_dig_from();
      except (E_PERM)
        message = $grant_utils:format_denial(current_room, 'room, {'dig_from});
        raise(E_PERM, message);
      endtry
      "Remove passage via area using capability if we have one";
      area_cap = player:find_capability_for(area, 'area);
      area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
      result = area_target:remove_passage(from_room_target, target_room);
      "Report success";
      if (result)
        label_str = length(labels) > 0 ? " (" + labels:join("/") + ")" | "";
        message = "Removed passage" + label_str + " to " + tostr(target_room) + ".";
        player:inform_current($event:mk_info(player, message));
        return true;
      else
        raise(E_INVARG, "Failed to remove passage (may have already been removed).");
      endif
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method _parse_dig_command owner: ARCH_WIZARD
    "Parse @dig direction spec. Returns map with 'oneway, 'from_dir, 'to_dir, 'dry_run, 'allow_parallel.";
    caller == this || raise(E_PERM);
    {dir_spec} = args;
    dir_spec = dir_spec:trim();
    "Parse optional flags: --dry-run, --allow-parallel";
    dry_run = false;
    allow_parallel = false;
    while (true)
      if (dir_spec:starts_with("--dry-run "))
        dry_run = true;
        dir_spec = dir_spec[11..length(dir_spec)]:trim();
        continue;
      elseif (dir_spec == "--dry-run")
        dry_run = true;
        dir_spec = "";
        break;
      elseif (dir_spec:starts_with("--allow-parallel "))
        allow_parallel = true;
        dir_spec = dir_spec[18..length(dir_spec)]:trim();
        continue;
      elseif (dir_spec == "--allow-parallel")
        allow_parallel = true;
        dir_spec = "";
        break;
      endif
      break;
    endwhile
    !dir_spec && raise(E_INVARG, "Direction spec is required.");
    "Check for oneway flag";
    is_oneway = false;
    if (dir_spec:starts_with("oneway "))
      is_oneway = true;
      dir_spec = dir_spec[8..length(dir_spec)]:trim();
    endif
    !dir_spec && raise(E_INVARG, "Direction spec is required.");
    "Check for explicit bidirectional spec (|)";
    if ("|" in dir_spec)
      parts = dir_spec:split("|");
      length(parts) == 2 || raise(E_INVARG, "Direction spec must be 'dir' or 'dir|returndir'.");
      from_dirs = {};
      for d in (parts[1]:split(","))
        t = d:trim();
        t && (from_dirs = {@from_dirs, t});
      endfor
      to_dirs = {};
      for d in (parts[2]:split(","))
        t = d:trim();
        t && (to_dirs = {@to_dirs, t});
      endfor
      !from_dirs && raise(E_INVARG, "Missing from-direction aliases.");
      !to_dirs && raise(E_INVARG, "Missing return-direction aliases.");
      "Expand standard aliases";
      from_dirs = $passage:expand_direction_aliases(from_dirs);
      to_dirs = $passage:expand_direction_aliases(to_dirs);
      return ['oneway -> is_oneway, 'from_dir -> from_dirs, 'to_dir -> to_dirs, 'dry_run -> dry_run, 'allow_parallel -> allow_parallel];
    endif
    "Single direction - split on commas for aliases";
    from_dirs = {};
    for d in (dir_spec:split(","))
      t = d:trim();
      t && (from_dirs = {@from_dirs, t});
    endfor
    !from_dirs && raise(E_INVARG, "Missing direction aliases.");
    "Infer opposite direction";
    to_dirs = this:_infer_opposite_directions(from_dirs);
    !to_dirs && raise(E_INVARG, "Can't infer opposite direction for '" + dir_spec + "'. Use 'dir|returndir' syntax.");
    "Expand standard aliases";
    from_dirs = $passage:expand_direction_aliases(from_dirs);
    to_dirs = $passage:expand_direction_aliases(to_dirs);
    return ['oneway -> is_oneway, 'from_dir -> from_dirs, 'to_dir -> to_dirs, 'dry_run -> dry_run, 'allow_parallel -> allow_parallel];
  endmethod

  method _room_name_for_error owner: ARCH_WIZARD
    "Return a room's display name for error messages, raising on invalid room refs.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {room} = args;
    valid(room) || raise(E_INVARG, "Invalid room reference while reporting passage conflict: " + toliteral(room));
    try
      return room:name();
    except e (ANY)
      raise(E_INVARG, "Could not read room name for " + tostr(room) + ": " + toliteral(e));
    endtry
  endmethod

  method _matching_room_candidates owner: ARCH_WIZARD
    "Return formatted room candidates in area matching a room search string.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {area, search_string} = args;
    valid(area) || raise(E_INVARG, "Invalid area while resolving room candidates: " + toliteral(area));
    needle = search_string:trim():lowercase();
    candidates = {};
    for room in (area.contents)
      valid(room) || raise(E_INVARG, "Invalid object in area contents while resolving room candidates: " + toliteral(room));
      try
        room_name = room:name();
        aliases = room:aliases();
      except e (ANY)
        raise(E_INVARG, "Could not read room search metadata for " + tostr(room) + ": " + toliteral(e));
      endtry
      typeof(aliases) == TYPE_LIST || raise(E_TYPE, "Room aliases must be a list for " + tostr(room));
      matched = needle in room_name:lowercase() > 0;
      if (!matched)
        for a in (aliases)
          typeof(a) == TYPE_STR || raise(E_TYPE, "Room aliases must contain strings for " + tostr(room));
          if (needle in a:lowercase() > 0)
            matched = true;
            break;
          endif
        endfor
      endif
      if (matched)
        candidates = {@candidates, room_name + " (" + tostr(room) + ")"};
      endif
    endfor
    return candidates;
  endmethod

  method _infer_opposite_directions owner: ARCH_WIZARD
    "Infer opposite directions for common compass/spatial directions.";
    caller == this || raise(E_PERM);
    {directions} = args;
    result = {};
    for dir in (directions)
      if (!maphaskey(this.direction_opposites, dir))
        return false;
      endif
      opposite = this.direction_opposites[dir];
      result = {@result, opposite};
    endfor
    return result;
  endmethod

  verb "@rename" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> to <name> -- Rename an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "to") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@rename OBJECT to NAME[:ALIASES]")));
      return;
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to rename " + tostr(target_obj) + ".");
      endif
      parsed = $str_proto:parse_name_aliases(iobjstr);
      new_name = parsed[1];
      new_aliases = parsed[2];
      !new_name && raise(E_INVARG, "Object name cannot be blank.");
      old_name = `target_obj.name ! ANY => "(no name)"';
      this:_do_rename_object(target_obj, new_name, new_aliases);
      message = "Renamed \"" + old_name + "\" (" + tostr(target_obj) + ") to \"" + new_name + "\".";
      if (new_aliases)
        alias_str = new_aliases:join(", ");
        message = message + " Aliases: " + alias_str + ".";
      endif
      player:inform_current($event:mk_info(player, message));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method _do_rename_object owner: ARCH_WIZARD
    "Internal helper to rename object with elevated permissions";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {target_obj, new_name, new_aliases} = args;
    target_obj:set_name_aliases(new_name, new_aliases);
  endmethod

  verb "@describe" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> as <description> -- Set object description.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "as") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@describe OBJECT_OR_DIRECTION as DESCRIPTION")));
      return;
    endif
    try
      "Try to match as object first, then fall through to passage matching on ordinary not-found errors.";
      target_obj = false;
      try
        target_obj = $match:match_object(dobjstr, player);
      except e (E_INVARG, E_PROPNF)
        target_obj = false;
      except e (ANY)
        raise(e[1], "Object lookup failed: " + toliteral(e));
      endtry
      if (typeof(target_obj) == TYPE_OBJ && valid(target_obj))
        "It's an object - use existing object description logic";
        if (!player.wizard && target_obj.owner != player)
          raise(E_PERM, "You do not have permission to describe " + tostr(target_obj) + ".");
        endif
        new_description = iobjstr:trim();
        !new_description && raise(E_INVARG, "Description cannot be blank.");
        this:_do_describe_object(target_obj, new_description);
        obj_name = `target_obj.name ! ANY => tostr(target_obj)';
        message = "Set description of \"" + obj_name + "\" (" + tostr(target_obj) + ").";
        player:inform_current($event:mk_info(player, message));
        return 1;
      else
        "Not an object - try to match as a passage direction";
        result = this:_do_describe_passage(dobjstr, iobjstr);
        if (result)
          return 1;
        else
          raise(E_INVARG, "Could not find object or passage matching '" + dobjstr + "'.");
        endif
      endif
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@edit-description @edit-d" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> -- Open multi-line editor for object description.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!dobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@edit-description <object>")));
      return;
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      if (!valid(target_obj))
        raise(E_INVARG, "I don't see that here.");
      endif
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You don't own that.");
      endif
      current_desc = `target_obj.description ! E_PROPNF => ""';
      editor_title = "Edit Description: " + target_obj.name;
      conn = connection();
      session_id = player:start_edit_session(target_obj, "set_description", {conn});
      present(conn, session_id, "text/djot", "text-editor", current_desc, {{"object", $url_utils:to_curie_str($builder_features)}, {"verb", "receive_description_edit"}, {"title", editor_title}, {"text_mode", "string"}, {"session_id", session_id}});
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
    endtry
  endverb

  method receive_description_edit owner: ARCH_WIZARD
    "Callback for text-editor when editing descriptions.";
    {session_id, content} = args;
    if (content == 'close)
      player:end_edit_session(session_id);
      return;
    endif
    session = player:get_edit_session(session_id);
    target_obj = session['target];
    conn = session['args][1];
    target_obj:set_description(content);
    obj_name = `target_obj.name ! ANY => tostr(target_obj)';
    player:inform_connection(conn, $event:mk_info(player, "Description updated for " + obj_name + "."));
  endmethod

  method _do_describe_object owner: ARCH_WIZARD
    "Internal helper to set object description with elevated permissions";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {target_obj, new_description} = args;
    target_obj.description = new_description;
  endmethod

  method _do_describe_passage owner: ARCH_WIZARD
    "Internal helper to set passage description with elevated permissions. Returns false if passage not found.";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {direction, description_str} = args;
    direction = direction:trim();
    description = description_str:trim();
    !description && raise(E_INVARG, "Description cannot be blank.");
    "Get current room - this is the player";
    current_room = player.location;
    if (!valid(current_room))
      return false;
    endif
    "Get area";
    area = current_room.location;
    if (!valid(area))
      return false;
    endif
    "Find passage matching the direction";
    passages = area:passages_from(current_room);
    if (!passages || length(passages) == 0)
      return false;
    endif
    "Find passage matching the direction";
    target_passage = area:find_passage_by_direction(current_room, direction);
    if (!target_passage)
      return false;
    endif
    "Check permissions";
    cap = player:find_capability_for(current_room, 'room);
    room_target = typeof(cap) == TYPE_FLYWEIGHT ? cap | current_room;
    room_target:check_can_dig_from();
    "Update passage description and set ambient flag";
    new_passage = target_passage:with_description_from(current_room, description);
    new_passage = new_passage:with_ambient_from(current_room, true);
    other_room = target_passage:other_room(current_room);
    area:update_passage(current_room, other_room, new_passage);
    message = "Set ambient description for '" + direction + "' passage: \"" + description + "\"";
    player:inform_current($event:mk_info(player, message));
    return true;
  endmethod

  verb "@par*ent" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> [--brief|--full] -- Show the parent of an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@parent OBJECT [--brief|--full]")));
      return;
    endif
    input = argstr:trim();
    tokens = input:words();
    mode = "normal";
    while (length(tokens) > 1)
      tok = tokens[$];
      if (tok == "--brief")
        mode = "brief";
        tokens = tokens[1..$ - 1];
      elseif (tok == "--full")
        mode = "full";
        tokens = tokens[1..$ - 1];
      else
        break;
      endif
    endwhile
    target_spec = tokens:join(" "):trim();
    if (!target_spec)
      player:inform_current($event:mk_error(player, $format.code:mk("@parent OBJECT [--brief|--full]")));
      return;
    endif
    fn render_obj(o, display_mode)
      if (display_mode == "brief")
        return tostr(o);
      endif
      name_txt = `o.name ! ANY => tostr(o)';
      if (display_mode == "full")
        owner_obj = `o.owner ! ANY => $nothing';
        owner_txt = owner_obj == $nothing ? "(unknown)" | tostr(owner_obj);
        p = `parent(o) ! ANY => #-1';
        if (valid(p))
          p_name = `p.name ! ANY => tostr(p)';
          ptxt = tostr(p_name, " (", p, ")");
        else
          ptxt = "(none)";
        endif
        return tostr(name_txt, " (", o, ") owner ", owner_txt, " parent ", ptxt);
      endif
      return tostr(name_txt, " (", o, ")");
    endfn
    try
      target_obj = this:_resolve_object_ref(target_spec, player, "object");
      parent_obj = parent(target_obj);
      title = $format.title:mk(render_obj(target_obj, mode));
      if (!valid(parent_obj))
        summary = $format.code:mk("Parent: none");
      else
        summary = $format.code:mk("Parent: [1] " + render_obj(parent_obj, mode));
      endif
      player:inform_current($event:mk_info(player, $format.block:mk(title, summary)));
      return parent_obj;
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return false;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return false;
    endtry
  endverb

  verb "@chi*ldren @kids @desc*endants" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> [depth] [--brief|--full] -- Show children and descendants of an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@children OBJECT [DEPTH] [--brief|--full]")));
      return;
    endif
    input = argstr:trim();
    tokens = input:words();
    mode = "normal";
    depth_limit = 0;
    depth_set = 0;
    while (length(tokens) > 1)
      tok = tokens[$];
      if (tok == "--brief")
        mode = "brief";
        tokens = tokens[1..$ - 1];
      elseif (tok == "--full")
        mode = "full";
        tokens = tokens[1..$ - 1];
      elseif (!depth_set && match(tok, "^[0-9]+$"))
        depth_limit = toint(tok);
        depth_set = 1;
        tokens = tokens[1..$ - 1];
      else
        break;
      endif
    endwhile
    if (depth_set && depth_limit <= 0)
      player:inform_current($event:mk_error(player, "Depth must be a positive integer."));
      return;
    endif
    target_spec = tokens:join(" "):trim();
    if (!target_spec)
      player:inform_current($event:mk_error(player, $format.code:mk("@children OBJECT [DEPTH] [--brief|--full]")));
      return;
    endif
    fn render_obj(o, display_mode)
      if (display_mode == "brief")
        return tostr(o);
      endif
      name_txt = `o.name ! ANY => tostr(o)';
      if (display_mode == "full")
        owner_obj = `o.owner ! ANY => $nothing';
        owner_txt = owner_obj == $nothing ? "(unknown)" | tostr(owner_obj);
        p = `parent(o) ! ANY => #-1';
        if (valid(p))
          p_name = `p.name ! ANY => tostr(p)';
          ptxt = tostr(p_name, " (", p, ")");
        else
          ptxt = "(none)";
        endif
        return tostr(name_txt, " (", o, ") owner ", owner_txt, " parent ", ptxt);
      endif
      return tostr(name_txt, " (", o, ")");
    endfn
    try
      target_obj = this:_resolve_object_ref(target_spec, player, "object");
      direct_children = children(target_obj);
      children_rendered = {};
      for c in (direct_children)
        children_rendered = {@children_rendered, render_obj(c, mode)};
      endfor
      queue = {};
      seen = [];
      for c in (direct_children)
        if (!(c in mapkeys(seen)))
          seen[c] = 1;
          queue = {@queue, {c, 1}};
        endif
      endfor
      by_depth = [];
      shown_descendants = 0;
      while (queue)
        item = queue[1];
        queue = length(queue) > 1 ? queue[2..$] | {};
        node = item[1];
        depth = item[2];
        shown_descendants = shown_descendants + 1;
        if (!(depth in mapkeys(by_depth)))
          by_depth[depth] = {};
        endif
        by_depth[depth] = {@by_depth[depth], render_obj(node, mode)};
        if (depth_limit && depth >= depth_limit)
          continue;
        endif
        for gc in (children(node))
          if (!(gc in mapkeys(seen)))
            seen[gc] = 1;
            queue = {@queue, {gc, depth + 1}};
          endif
        endfor
      endwhile
      depth_lines = {};
      for depth in (mapkeys(by_depth))
        lst = by_depth[depth];
        depth_lines = {@depth_lines, tostr("[", depth, "] ", lst:english_list("none"))};
      endfor
      title = $format.title:mk(render_obj(target_obj, mode));
      summary = tostr("Children: ", length(direct_children), " | Descendants shown: ", shown_descendants);
      if (depth_limit)
        summary = summary + tostr(" | Depth limit: ", depth_limit);
      else
        summary = summary + " | Depth limit: none";
      endif
      children_code = $format.code:mk(children_rendered:english_list("none"));
      descendants_code = $format.code:mk(depth_lines ? depth_lines:join("\n") | "none");
      content = $format.block:mk(title, $format.code:mk(summary), $format.title:mk("Children:"), children_code, $format.title:mk("Descendants by depth:"), descendants_code);
      player:inform_current($event:mk_info(player, content));
      return direct_children;
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return false;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return false;
    endtry
  endverb

  verb "@integrate" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> as <description> -- Set object integrated description.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "as") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@integrate OBJECT as DESCRIPTION")));
      return;
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to set integrated description on " + tostr(target_obj) + ".");
      endif
      new_description = iobjstr:trim();
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      if (new_description == "")
        success_message = "Cleared integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + ").";
      else
        success_message = "Set integrated description of \"" + obj_name + "\" (" + tostr(target_obj) + "): \"" + new_description + "\"";
      endif
      this:_do_set_integrated_description(target_obj, new_description);
      player:inform_current($event:mk_info(player, success_message));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method _do_set_integrated_description owner: ARCH_WIZARD
    "Internal helper to set object integrated description with elevated permissions";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {target_obj, new_description} = args;
    target_obj.integrated_description = new_description;
  endmethod

  verb "@move" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> to <location> -- Move an object to a new location.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "to") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@move OBJECT to LOCATION")));
      return;
    endif
    try
      "Match the object to move";
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That object reference is not valid.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      "Check permissions - must own the object or be a wizard";
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to move " + tostr(target_obj) + ".");
      endif
      "Match the destination location";
      dest_loc = $match:match_object(iobjstr, player);
      typeof(dest_loc) != TYPE_OBJ && raise(E_INVARG, "That destination reference is not valid.");
      !valid(dest_loc) && raise(E_INVARG, "That destination no longer exists.");
      "Get current location for messaging";
      old_loc = target_obj.location;
      is_player = target_obj.player;
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      dest_name = `dest_loc.name ! ANY => tostr(dest_loc)';
      success_message = "Moved " + obj_name + " (" + tostr(target_obj) + ") to " + dest_name + " (" + tostr(dest_loc) + ").";
      "Perform the move";
      move_result = target_obj:moveto(dest_loc);
      if (typeof(move_result) == TYPE_ERR)
        raise(error_code(move_result), "Could not move object.");
      endif
      target_obj.location == dest_loc || raise(E_INVARG, "Could not move object.");
      "If it's a player, notify rooms after the move succeeds";
      if (is_player && valid(old_loc))
        old_loc:announce_all_but({target_obj}, obj_name + " disappears suddenly for parts unknown.");
      endif
      if (is_player && valid(dest_loc))
        dest_loc:announce_all_but({target_obj}, obj_name + " materializes out of thin air.");
      endif
      "Report success to the builder";
      player:inform_current($event:mk_info(player, success_message));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@edit" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<property> -- Edit a property on an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, "Usage: " + verb + " <object>.<property>"));
      return;
    endif
    target_spec = argstr:trim();
    parsed = $prog_utils:parse_target_spec(target_spec);
    if (parsed && parsed['type] == 'property)
      object_str = parsed['object_str];
      prop_name = parsed['item_name];
    elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
      sel = parsed['selectors][1];
      if (sel['kind] != 'property)
        player:inform_current($event:mk_error(player, "Invalid property reference format. Use 'object.property'"));
        return;
      endif
      object_str = parsed['object_str];
      prop_name = sel['item_name];
    else
      player:inform_current($event:mk_error(player, "Invalid property reference format. Use 'object.property'"));
      return;
    endif
    "Match the object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + e[2]));
      return;
    endtry
    "Check property exists and open editor";
    if (!target_obj:check_property_exists(prop_name))
      player:inform_current($event:mk_error(player, "Property '" + prop_name + "' not found on " + target_obj.name + "."));
      return;
    endif
    "Open the property editor";
    this:present_property_editor(target_obj, prop_name);
    player:inform_current($event:mk_info(player, "Opened property editor for " + tostr(target_obj) + "." + prop_name));
  endverb

  method present_property_editor owner: ARCH_WIZARD
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    editor_id = "edit-" + tostr(target_obj) + "-" + prop_name;
    editor_title = "Edit " + prop_name + " on " + tostr(target_obj);
    object_curie = $url_utils:to_curie_str(target_obj);
    present(connection(), editor_id, "text/plain", "property-value-editor", "", {{"object", object_curie}, {"property", prop_name}, {"title", editor_title}});
  endmethod

  verb "@set-m*essage @setm" (any any any) owner: HACKER flags: "rd"
    "Set a custom message template on an object property.";
    "Usage: @set-message OBJECT.PROPERTY template string...";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr || length(args) < 2)
      raise(E_INVARG, "Usage: @set-message OBJECT.PROPERTY template string...");
    endif
    try
      target_spec = args[1];
      "Parse property reference";
      parsed = $prog_utils:parse_target_spec(target_spec);
      if (!parsed)
        player:inform_current($event:mk_error(player, "Usage: @set-message <object>.<prop-name> template..."));
        return;
      endif
      "Handle both old 'property type and new 'compound type";
      if (parsed['type] == 'property)
        object_str = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        if (sel['kind] != 'property)
          player:inform_current($event:mk_error(player, "Usage: @set-message <object>.<prop-name> template..."));
          return;
        endif
        object_str = parsed['object_str];
        prop_name = sel['item_name];
      else
        player:inform_current($event:mk_error(player, "Usage: @set-message <object>.<prop-name> template..."));
        return;
      endif
      prop_name:ends_with("_msg") || prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with '_msg', '_msgs', or '_msg_bag'.");
      "Match the target object";
      try
        target_obj = $match:match_object(object_str, player);
      except e (E_INVARG, E_PROPNF)
        player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
        return;
      except e (ANY)
        raise(e[1], "Object lookup failed: " + toliteral(e));
      endtry
      "Get remainder after target spec";
      offset = index(argstr, target_spec) + length(target_spec);
      template_string = argstr[offset..length(argstr)]:trim();
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object does not exist.");
      "Check if property is writable";
      {writable, error_msg} = $obj_utils:check_message_property_writable(target_obj, prop_name, player);
      if (!writable)
        raise(E_PERM, error_msg);
      endif
      "Compile and validate the template";
      {success, result} = $obj_utils:validate_and_compile_template(template_string);
      if (!success)
        raise(E_INVARG, "Template compilation failed: " + result);
      endif
      compiled_list = result;
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      success_message = "Set message template on " + obj_name + " (" + tostr(target_obj) + ")." + prop_name + ".";
      "Set the compiled message";
      prop_key = $obj_utils:property_key(target_obj, prop_name);
      prop_key == E_PROPNF && raise(E_PROPNF, "Property not found: " + prop_name);
      $obj_utils:set_compiled_message(target_obj, prop_key, compiled_list, player, {{"property_write", target_obj, prop_key}});
      player:inform_current($event:mk_info(player, success_message));
      return true;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@get-m*essage @getm" (any none any) owner: HACKER flags: "rd"
    "Show a message template for a single property.";
    "Usage: @get-message OBJECT.PROPERTY";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    !argstr && raise(E_INVARG, "Usage: @get-message OBJECT.PROPERTY");
    try
      target_spec = args[1];
      parsed = $prog_utils:parse_target_spec(target_spec);
      "Handle both 'property and 'compound parse results";
      if (parsed && parsed['type] == 'property)
        object_str = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Usage: @get-message OBJECT.PROPERTY");
        object_str = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Usage: @get-message OBJECT.PROPERTY");
      endif
      prop_name:ends_with("_msg") || prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with '_msg', '_msgs', or '_msg_bag'.");
      target_obj = $match:match_object(object_str, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object does not exist.");
      value = E_PROPNF;
      for prop_info in ($obj_utils:message_properties(target_obj, player, true))
        if (tostr(prop_info[1]) == prop_name)
          value = prop_info[2];
          break;
        endif
      endfor
      value == E_PROPNF && raise(E_INVARG, "Property '" + prop_name + "' not found on " + tostr(target_obj) + ".");
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      if ($msg_bag:is_msg_bag(value))
        entries = value:entries();
        if (!entries)
          header = obj_name + " (" + tostr(target_obj) + ")." + prop_name + " = (empty message bag)";
          player:inform_current($event:mk_info(player, header));
        else
          header = obj_name + " (" + tostr(target_obj) + ")." + prop_name + " (" + tostr(length(entries)) + " entries):";
          rows = {};
          idx = 1;
          for entry in (entries)
            template_str = typeof(entry) == TYPE_LIST ? `$sub_utils:decompile(entry) ! ANY => toliteral(entry)' | toliteral(entry);
            rows = {@rows, {tostr(idx), template_str}};
            idx = idx + 1;
          endfor
          table = $format.table:mk({"#", "Template"}, rows);
          player:inform_current($event:mk_info(player, header));
          player:inform_current($event:mk_info(player, table));
        endif
      else
        "If compiled template list, decompile to readable string";
        display_value = typeof(value) == TYPE_LIST ? `$sub_utils:decompile(value) ! ANY => toliteral(value)' | toliteral(value);
        message = obj_name + " (" + tostr(target_obj) + ")." + prop_name + " = " + display_value;
        player:inform_current($event:mk_info(player, message));
      endif
      return value;
    except e (ANY)
      msg = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, msg));
      return 0;
    endtry
  endverb

  verb "@set-r*ule" (any any any) owner: HACKER flags: "rd"
    "Set a rule property on an object. Usage: @set-rule <object>.<rule-property> <expression>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
    endif
    if (!args[1])
      raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
    endif
    try
      "args[1] is the object.property part, rest of argstr is the rule expression";
      prop_spec = args[1];
      rule_expr = argstr[length(prop_spec) + 1..$]:trim();
      if (!rule_expr || rule_expr == "")
        raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
      endif
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property");
      endif
      "Match object";
      target = $match:match_object(target_name, player);
      valid(target) || raise(E_INVARG, "Object not found");
      "Property must end with _rule to prevent accidents";
      prop_name:ends_with("_rule") || raise(E_INVARG, "Rule properties must end with '_rule'");
      "Permission check";
      if (!player.wizard && target.owner != player)
        raise(E_PERM, "You don't own " + tostr(target) + ".");
      endif
      "Parse and validate rule";
      rule = $rule_engine:parse_expression(rule_expr, tosym(prop_name));
      "Validate for bounded negation violations without evaluating";
      validation = $rule_engine:validate_rule(rule);
      if (length(validation['warnings]) > 0)
        for warning in (validation['warnings])
          player:inform_current($event:mk_error(player, "Warning: " + warning));
        endfor
        if (!validation['valid])
          raise(E_INVARG, "Rule has errors - fix bounded negation issues.");
        endif
      endif
      target.(prop_name) = rule;
      message = "Set rule on " + tostr(target) + "." + prop_name + ": \"" + rule_expr + "\"";
      player:inform_current($event:mk_info(player, message));
      return rule;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@clear-rule" (any none none) owner: HACKER flags: "rd"
    "Clear a rule property. Usage: @clear-rule <object>.<rule-property>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @clear-rule OBJECT.RULE_PROPERTY");
    endif
    try
      prop_spec = argstr:trim();
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property");
      endif
      "Match object";
      target = $match:match_object(target_name, player);
      valid(target) || raise(E_INVARG, "Object not found");
      "Property must end with _rule";
      prop_name:ends_with("_rule") || raise(E_INVARG, "Rule properties must end with '_rule'");
      "Permission check";
      if (!player.wizard && target.owner != player)
        raise(E_PERM, "You don't own " + tostr(target) + ".");
      endif
      "Clear the rule";
      target.(prop_name) = 0;
      message = "Cleared rule on " + tostr(target) + "." + prop_name;
      player:inform_current($event:mk_info(player, message));
      return true;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@show-rule" (any none none) owner: HACKER flags: "rd"
    "Show a rule property. Usage: @show-rule <object>.<rule-property>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @show-rule OBJECT.RULE_PROPERTY");
    endif
    try
      prop_spec = argstr:trim();
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property");
      endif
      "Match object";
      target = $match:match_object(target_name, player);
      valid(target) || raise(E_INVARG, "Object not found");
      "Property must end with _rule";
      prop_name:ends_with("_rule") || raise(E_INVARG, "Rule properties must end with '_rule'");
      "Get the rule";
      rule = E_PROPNF;
      for prop_info in ($obj_utils:rule_properties(target, player, true))
        if (tostr(prop_info[1]) == prop_name)
          rule = prop_info[2];
          break;
        endif
      endfor
      rule == E_PROPNF && raise(E_INVARG, "Property not found: " + prop_name);
      if (rule == 0)
        message = tostr(target) + "." + prop_name + " = (no rule set)";
      else
        rule_expr = $rule_engine:decompile_rule(rule);
        message = tostr(target) + "." + prop_name + " = \"" + rule_expr + "\"";
      endif
      player:inform_current($event:mk_info(player, message));
      return rule;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@mes*sages @msg" (any none none) owner: HACKER flags: "rd"
    "HINT: <object> -- Show all customizable message properties.";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    !dobjstr && raise(E_INVARG, "Usage: @messages OBJECT");
    try
      target_obj = $match:match_object(dobjstr, player);
      "Get message properties";
      msg_props = $obj_utils:message_properties(target_obj, player, true);
      if (!msg_props || length(msg_props) == 0)
        obj_name = `target_obj.name ! ANY => tostr(target_obj)';
        message = obj_name + " (" + tostr(target_obj) + ") has no message properties.";
        player:inform_current($event:mk_info(player, message));
        return 0;
      endif
      "Build table";
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      headers = {"Property Name", "Current Value"};
      rows = {};
      for prop_info in (msg_props)
        {prop_name, prop_value} = prop_info;
        "Summarize the value - check for both object and flyweight bags";
        if (typeof(prop_value) == TYPE_OBJ && isa(prop_value, $msg_bag))
          value_summary = "message bag (" + tostr(length(prop_value:entries())) + " entries)";
        elseif (typeof(prop_value) == TYPE_FLYWEIGHT && prop_value.delegate == $msg_bag)
          value_summary = "message bag (" + tostr(length(prop_value:entries())) + " entries)";
        elseif (typeof(prop_value) == TYPE_LIST)
          value_summary = `$sub_utils:decompile(prop_value) ! ANY => toliteral(prop_value)';
        else
          value_summary = toliteral(prop_value);
        endif
        rows = {@rows, {tostr(prop_name), value_summary}};
      endfor
      "Output results";
      header = "Message properties for " + obj_name + " (" + tostr(target_obj) + "):";
      player:inform_current($event:mk_info(player, header));
      table_result = $format.table:mk(headers, rows);
      player:inform_current($event:mk_info(player, table_result));
      return length(msg_props);
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@rules" (any none none) owner: HACKER flags: "rd"
    "HINT: <object> -- Show all rule properties on an object.";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!dobjstr)
      raise(E_INVARG, "Usage: @rules OBJECT");
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
    "Get rule properties";
    rule_props = $obj_utils:rule_properties(target_obj, player, true);
    if (!rule_props || length(rule_props) == 0)
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      message = obj_name + " (" + tostr(target_obj) + ") has no rule properties.";
      player:inform_current($event:mk_info(player, message));
      return 0;
    endif
    "Build table";
    obj_name = `target_obj.name ! ANY => tostr(target_obj)';
    headers = {"Property Name", "Rule Expression"};
    rows = {};
    for prop_info in (rule_props)
      {prop_name, prop_value} = prop_info;
      "Decompile the rule if it's set";
      if (prop_value == 0)
        rule_expr = "(not set)";
      else
        rule_expr = $rule_engine:decompile_rule(prop_value);
      endif
      rows = {@rows, {tostr(prop_name), rule_expr}};
    endfor
    "Output results";
    header = "Rule properties for " + obj_name + " (" + tostr(target_obj) + "):";
    player:inform_current($event:mk_info(player, header));
    table_result = $format.table:mk(headers, rows);
    player:inform_current($event:mk_info(player, table_result));
    return length(rule_props);
  endverb

  verb "@reactions" (any none none) owner: HACKER flags: "rd"
    "HINT: <object> -- Show all reactions on an object.";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!dobjstr)
      raise(E_INVARG, "Usage: @reactions OBJECT");
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
    reaction_props = $obj_utils:reaction_properties(target_obj, player, true);
    if (!reaction_props || length(reaction_props) == 0)
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      message = obj_name + " (" + tostr(target_obj) + ") has no reactions.";
      player:inform_current($event:mk_info(player, message));
      return 0;
    endif
    obj_name = `target_obj.name ! ANY => tostr(target_obj)';
    header = "Reactions on " + obj_name + " (" + tostr(target_obj) + "):";
    player:inform_current($event:mk_info(player, header));
    headers = {"Property", "Trigger", "When", "Effects", "Enabled"};
    rows = {};
    for prop_info in (reaction_props)
      {prop_name, reaction} = prop_info;
      trigger_str = "??";
      if (typeof(reaction.trigger) == TYPE_SYM)
        trigger_str = tostr(reaction.trigger);
      elseif (typeof(reaction.trigger) == TYPE_LIST && length(reaction.trigger) >= 4 && reaction.trigger[1] == 'when)
        {_, prop, op, val} = reaction.trigger;
        trigger_str = tostr(prop) + " " + tostr(op) + " " + tostr(val);
      elseif (typeof(reaction.trigger) == TYPE_LIST)
        trigger_str = toliteral(reaction.trigger);
      endif
      if (reaction.when == 0)
        when_str = "-";
      else
        when_str = $rule_engine:decompile_rule(reaction.when);
      endif
      effects_parts = {};
      for effect in (reaction.effects)
        if (typeof(effect) == TYPE_FLYWEIGHT && effect.type)
          effects_parts = {@effects_parts, tostr(effect.type)};
        elseif (typeof(effect) == TYPE_LIST && length(effect) > 0)
          effects_parts = {@effects_parts, tostr(effect[1])};
        endif
      endfor
      effects_str = effects_parts:join(", ");
      if (effects_str == "")
        effects_str = "(none)";
      endif
      enabled_str = reaction.enabled ? "yes" | "no";
      rows = {@rows, {prop_name, trigger_str, when_str, effects_str, enabled_str}};
    endfor
    table_result = $format.table:mk(headers, rows);
    player:inform_current($event:mk_info(player, table_result));
    return length(reaction_props);
  endverb

  verb "@add-reaction @set-reaction" (any any any) owner: HACKER flags: "rd"
    "Add a reaction to an object. Usage: @add-reaction <object>.<name>_reaction <trigger> <when> <effects>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr || length(args) < 4)
      raise(E_INVARG, "Usage: @add-reaction OBJECT.NAME_reaction TRIGGER WHEN EFFECTS");
    endif
    try
      prop_spec = args[1];
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property_reaction");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property_reaction");
      endif
      "Match object";
      target_obj = $match:match_object(target_name, player);
      valid(target_obj) || raise(E_INVARG, "Object not found");
      "Property must end with _reaction";
      prop_name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
      "Permission check";
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You don't own " + tostr(target_obj) + ".");
      endif
      "Parse trigger literal";
      trigger_str = args[2];
      try
        trigger = fromliteral(trigger_str);
      except e (E_INVARG, E_TYPE)
        detail = length(e) >= 2 ? e[2] | toliteral(e);
        raise(E_INVARG, "Invalid trigger expression: " + detail);
      endtry
      "Parse when clause";
      when_str = args[3];
      when_clause = when_str == "0" ? 0 | when_str;
      "Parse effects literal";
      effects_parts = args[4..length(args)];
      effects_str = $list_proto:join(effects_parts, " ");
      try
        effects = fromliteral(effects_str);
      except e (E_INVARG, E_TYPE)
        detail = length(e) >= 2 ? e[2] | toliteral(e);
        raise(E_INVARG, "Invalid effects expression: " + detail);
      endtry
      typeof(effects) != TYPE_LIST && raise(E_INVARG, "Effects must evaluate to a list.");
      reaction = $reaction:mk(trigger, when_clause, effects);
      "Add or update property";
      prop_key = $obj_utils:property_key(target_obj, prop_name);
      if (prop_key != E_PROPNF)
        target_obj.(prop_key) = reaction;
      else
        add_property(target_obj, prop_name, reaction, {player, "r"});
      endif
      trigger_display = typeof(trigger) == TYPE_SYM ? tostr(trigger) | "threshold";
      message = "Set " + tostr(target_obj) + "." + prop_name + ": trigger=" + trigger_display + ", effects=" + tostr(length(effects));
      player:inform_current($event:mk_info(player, message));
      return reaction;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@enable-reaction" (any none none) owner: HACKER flags: "rd"
    "Enable a reaction. Usage: @enable-reaction <object>.<property_reaction>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @enable-reaction OBJECT.PROPERTY_reaction");
    endif
    try
      prop_spec = argstr:trim();
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property_reaction");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property_reaction");
      endif
      "Match object";
      target_obj = $match:match_object(target_name, player);
      valid(target_obj) || raise(E_INVARG, "Object not found");
      "Property must end with _reaction";
      prop_name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
      "Permission check";
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You don't own " + tostr(target_obj) + ".");
      endif
      "Check property exists and is a reaction";
      prop_key = $obj_utils:property_key(target_obj, prop_name);
      prop_key == E_PROPNF && raise(E_INVARG, "Property not found: " + prop_name);
      reaction = target_obj.(prop_key);
      typeof(reaction) == TYPE_FLYWEIGHT && reaction.delegate == $reaction || raise(E_INVARG, prop_name + " is not a reaction");
      "Enable the reaction";
      reaction.enabled = true;
      target_obj.(prop_key) = reaction;
      player:inform_current($event:mk_info(player, "Enabled " + tostr(target_obj) + "." + prop_name));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@disable-reaction" (any none none) owner: HACKER flags: "rd"
    "Disable a reaction. Usage: @disable-reaction <object>.<property_reaction>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @disable-reaction OBJECT.PROPERTY_reaction");
    endif
    try
      prop_spec = argstr:trim();
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property_reaction");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property_reaction");
      endif
      "Match object";
      target_obj = $match:match_object(target_name, player);
      valid(target_obj) || raise(E_INVARG, "Object not found");
      "Property must end with _reaction";
      prop_name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
      "Permission check";
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You don't own " + tostr(target_obj) + ".");
      endif
      "Check property exists and is a reaction";
      prop_key = $obj_utils:property_key(target_obj, prop_name);
      prop_key == E_PROPNF && raise(E_INVARG, "Property not found: " + prop_name);
      reaction = target_obj.(prop_key);
      typeof(reaction) == TYPE_FLYWEIGHT && reaction.delegate == $reaction || raise(E_INVARG, prop_name + " is not a reaction");
      "Disable the reaction";
      reaction.enabled = false;
      target_obj.(prop_key) = reaction;
      player:inform_current($event:mk_info(player, "Disabled " + tostr(target_obj) + "." + prop_name));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@par*ents @anc*estors" (any none none) owner: ARCH_WIZARD flags: "rxd"
    "HINT: <object> [--brief|--full] -- Show the ancestor chain of an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@parents OBJECT [--brief|--full]")));
      return;
    endif
    input = argstr:trim();
    tokens = input:words();
    mode = "normal";
    while (length(tokens) > 1)
      tok = tokens[$];
      if (tok == "--brief")
        mode = "brief";
        tokens = tokens[1..$ - 1];
      elseif (tok == "--full")
        mode = "full";
        tokens = tokens[1..$ - 1];
      else
        break;
      endif
    endwhile
    target_spec = tokens:join(" "):trim();
    if (!target_spec)
      player:inform_current($event:mk_error(player, $format.code:mk("@parents OBJECT [--brief|--full]")));
      return;
    endif
    fn render_obj(o, display_mode)
      if (display_mode == "brief")
        return tostr(o);
      endif
      name_txt = `o.name ! ANY => tostr(o)';
      if (display_mode == "full")
        owner_obj = `o.owner ! ANY => $nothing';
        owner_txt = owner_obj == $nothing ? "(unknown)" | tostr(owner_obj);
        p = `parent(o) ! ANY => #-1';
        if (valid(p))
          p_name = `p.name ! ANY => tostr(p)';
          ptxt = tostr(p_name, " (", p, ")");
        else
          ptxt = "(none)";
        endif
        return tostr(name_txt, " (", o, ") owner ", owner_txt, " parent ", ptxt);
      endif
      return tostr(name_txt, " (", o, ")");
    endfn
    try
      target_obj = this:_resolve_object_ref(target_spec, player, "object");
      ancestors_list = ancestors(target_obj);
      segments = {};
      for i in [1..length(ancestors_list)]
        a = ancestors_list[i];
        segments = {@segments, tostr("[", i, "] ", render_obj(a, mode))};
      endfor
      title = $format.title:mk(render_obj(target_obj, mode));
      summary = $format.code:mk(tostr("Ancestors: ", length(ancestors_list)));
      chain_text = segments ? segments:join(" -> ") | "none";
      chain = $format.code:mk(chain_text);
      content = $format.block:mk(title, summary, $format.title:mk("Chain:"), chain);
      player:inform_current($event:mk_info(player, content));
      return ancestors_list;
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return false;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return false;
    endtry
  endverb

  verb "@set-thumbnail @thumbnail" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> -- Set a thumbnail image for an object.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!dobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@set-thumbnail OBJECT")));
      return;
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to set thumbnail on " + tostr(target_obj) + ".");
      endif
      "Prompt for image upload";
      obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      result = player:upload_image("Upload thumbnail image for " + obj_name + ":", 5 * (1 << 20));
      if (!result)
        player:inform_current($event:mk_info(player, "Thumbnail upload cancelled."):with_audience('utility));
        return 0;
      endif
      {content_type, picbin} = result;
      target_obj:set_thumbnail(content_type, picbin);
      player:inform_current($event:mk_info(player, "Set thumbnail for " + obj_name + " (" + tostr(target_obj) + ")."));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method help_topics owner: ARCH_WIZARD
    "Return help topics for builder/programmer commands via configured help source.";
    {for_player, ?topic = ""} = args;
    source = this.help_source;
    if (valid(source))
      result = source:help_topics(for_player, topic);
      if (typeof(result) != TYPE_INT)
        return result;
      endif
    endif
    verb_help = $help_utils:verb_help_from_hint(this, topic, 'building);
    typeof(verb_help) != TYPE_INT && return verb_help;
    return 0;
  endmethod

  verb "@passage @passage-info @pinfo" (any none none) owner: ARCH_WIZARD flags: "rd"
    "Show detailed passage information for a direction.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    !argstr || !dobjstr && return player:inform_current($event:mk_error(player, $format.code:mk("@passage DIRECTION")));
    try
      direction = dobjstr:trim();
      current_room = player.location;
      !valid(current_room) && raise(E_INVARG, "You must be in a room.");
      area = current_room.location;
      !valid(area) && raise(E_INVARG, "Your current room is not in an area.");
      passage = area:find_passage_by_direction(current_room, direction);
      typeof(passage) != TYPE_FLYWEIGHT && typeof(passage) != TYPE_OBJ && raise(E_INVARG, "No passage found in direction '" + direction + "'.");
      other_room = passage:other_room(current_room);
      !valid(other_room) && raise(E_INVARG, "That passage does not connect to a valid destination from here.");
      "Build info table";
      rows = {{"Direction", passage:label_for(current_room)}, {"Aliases", passage:aliases_for(current_room):join(", ")}, {"Description", passage:description_for(current_room) || "(none)"}, {"Prose Style", tostr(passage:prose_style_for(current_room))}, {"Departure Phrase", passage:departure_phrase_for(current_room) || "(none)"}, {"Arrival Phrase", passage:arrival_phrase_for(current_room) || "(none)"}, {"", ""}, {"Destination", other_room.name + " (" + tostr(other_room) + ")"}, {"Return Direction", passage:label_for(other_room) || "(one-way)"}, {"Return Aliases", (passage:aliases_for(other_room) || {}):join(", ")}, {"Return Description", passage:description_for(other_room) || "(none)"}, {"", ""}, {"Open", passage.is_open ? "yes" | "no"}, {"Locked", `passage.is_locked ! E_PROPNF => false' ? "yes" | "no"}};
      table = $format.table:mk({"Property", "Value"}, rows);
      content = $format.block:mk($format.title:mk("Passage: " + direction, 3), table);
      player:inform_current($event:mk_info(player, content));
    except e (ANY)
      message = typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
    endtry
  endverb

  verb "@set-passage @setp" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Set a passage property: description, departure, arrival, style, aliases, door mode.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || length(args) < 3)
      lines = {"@set-passage DIRECTION PROPERTY VALUE", "", "Properties:", "  description DESC   - Ambient description (include direction word for links!)", "  departure PHRASE   - e.g., 'through the door'", "  arrival PHRASE     - e.g., 'from the kitchen'", "  style sentence|fragment", "  aliases A,B,C", "  is_door true|false - Enable/disable door operations (open/close/lock/unlock)"};
      return player:inform_current($event:mk_info(player, $format.block:mk(@lines)));
    endif
    try
      {direction, prop} = {args[1]:trim(), args[2]:trim():initial_lowercase()};
      "Get value - everything after first two args";
      offset = index(argstr, args[2]) + length(args[2]);
      value = argstr[offset..length(argstr)]:trim();
      current_room = player.location;
      !valid(current_room) && raise(E_INVARG, "You must be in a room.");
      area = current_room.location;
      !valid(area) && raise(E_INVARG, "Your room is not in an area.");
      passage = area:find_passage_by_direction(current_room, direction);
      typeof(passage) != TYPE_FLYWEIGHT && typeof(passage) != TYPE_OBJ && raise(E_INVARG, "No passage '" + direction + "' found.");
      "Check permissions";
      cap = player:find_capability_for(current_room, 'room);
      room_target = typeof(cap) == TYPE_FLYWEIGHT ? cap | current_room;
      room_target:check_can_dig_from();
      other_room = passage:other_room(current_room);
      new_passage = 0;
      "Apply the property change";
      if (prop in {"description", "desc", "d"})
        new_passage = passage:with_description_from(current_room, value):with_ambient_from(current_room, true);
        prop_display = "description";
      elseif (prop in {"departure", "depart", "leave"})
        new_passage = passage:with_departure_phrase_from(current_room, value);
        prop_display = "departure phrase";
      elseif (prop in {"arrival", "arrive", "enter"})
        new_passage = passage:with_arrival_phrase_from(current_room, value);
        prop_display = "arrival phrase";
      elseif (prop in {"style", "prose"})
        style_value = value:lowercase();
        style_value in {"sentence", "s"} && (new_passage = passage:with_prose_style_from(current_room, 'sentence));
        style_value in {"fragment", "f"} && (new_passage = passage:with_prose_style_from(current_room, 'fragment));
        typeof(new_passage) != TYPE_FLYWEIGHT && typeof(new_passage) != TYPE_OBJ && raise(E_INVARG, "Style must be 'sentence' or 'fragment'.");
        prop_display = "style to " + style_value;
      elseif (prop in {"aliases", "alias"})
        aliases = {};
        for a in (value:split(","))
          trimmed = a:trim();
          trimmed && (aliases = {@aliases, trimmed});
        endfor
        new_passage = passage:with_aliases_from(current_room, aliases);
        prop_display = "aliases to " + aliases:join(", ");
      elseif (prop in {"is_door", "door"})
        toggle = value:lowercase();
        if (toggle in {"true", "t", "yes", "y", "1", "on"})
          door_value = true;
        elseif (toggle in {"false", "f", "no", "n", "0", "off"})
          door_value = false;
        else
          raise(E_INVARG, "is_door must be true/false (also yes/no, 1/0, on/off).");
        endif
        props = passage:_extract_all();
        props['is_door] = door_value;
        new_passage = passage:_mk_from_props(props);
        prop_display = "is_door to " + (door_value ? "true" | "false");
      else
        raise(E_INVARG, "Unknown property. Use: description, departure, arrival, style, aliases, is_door");
      endif
      area:update_passage(current_room, other_room, new_passage);
      player:inform_current($event:mk_info(player, "Set " + direction + " " + prop_display + "."));
    except e (ANY)
      message = typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
    endtry
  endverb

  verb "#*" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "HINT: #<string>[.<property>...] [= <value>] [exit|player|inventory] [for <code>]";
    "Quick object lookup with optional property chain, assignment, and code evaluation.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    "Parse the verb name to extract object string and property chain";
    verb_part = verb[2..$];
    if (!verb_part)
      player:inform_current($event:mk_info(player, "Usage: #<name>[.<property>...] [= <value>] [exit|player|inventory] [for <code>]\nType 'help #' for details."));
      return;
    endif
    "Split by dots to get object name and property chain";
    parts = verb_part:split(".");
    obj_string = parts[1];
    prop_chain = length(parts) > 1 ? parts[2..$] | {};
    "Check for assignment (argstr starts with =)";
    rest = argstr:trim();
    assignment_value = 0;
    is_assignment = false;
    if (rest && rest[1] == "=")
      is_assignment = true;
      !player.programmer && raise(E_PERM, "Assignment requires programmer privileges.");
      !length(prop_chain) && raise(E_INVARG, "Assignment requires a property (e.g., #obj.prop = value)");
      value_str = rest[2..$]:trim();
      !value_str && raise(E_INVARG, "Missing value after =");
      try
        assignment_value = fromliteral(value_str);
      except e (E_INVARG, E_TYPE)
        raise(E_INVARG, "Invalid value: " + (length(e) >= 2 ? e[2] | toliteral(e)));
      endtry
      rest = "";
    endif
    "Parse remaining argstr for scope and for-clause";
    scope = "";
    for_code = "";
    if (rest)
      words = rest:split(" ");
      first_word = words[1]:lowercase();
      if (first_word in {"exit", "player", "inventory"})
        scope = first_word;
        rest = length(words) > 1 ? words[2..$]:join(" ") | "";
      endif
      rest = rest:trim();
      if (index(rest:lowercase(), "for ") == 1)
        for_code = rest[5..$]:trim();
      elseif (rest && !(first_word in {"exit", "player", "inventory"}))
        raise(E_INVARG, "Unknown argument: " + rest + ". Expected: exit, player, inventory, or 'for <code>'");
      endif
    endif
    "Helper to match exits - returns {passage, side} where side is 'a or 'b";
    fn match_exit(name)
      room = player.location;
      !valid(room) && return $failed_match;
      area = room.location;
      !valid(area) || !respond_to(area, 'passages_from) && return $failed_match;
      passages = area:passages_from(room);
      !length(passages) && return $failed_match;
      search_objects = {};
      keys = {};
      sides = {};
      for passage in (passages)
        if (passage.side_a_room == room)
          search_objects = {@search_objects, passage};
          keys = {@keys, passage.side_a_aliases};
          sides = {@sides, 'a};
        else
          search_objects = {@search_objects, passage};
          keys = {@keys, passage.side_b_aliases};
          sides = {@sides, 'b};
        endif
      endfor
      match = complex_match(name, search_objects, keys);
      match == $failed_match && return $failed_match;
      idx = match in search_objects;
      return {match, sides[idx]};
    endfn
    "Match the object based on scope";
    passage_side = 0;
    try
      if (scope == "player")
        thing = $match:match_player(obj_string, player);
      elseif (scope == "inventory")
        search_objects = player.contents;
        !length(search_objects) && raise(E_INVARG, "Your inventory is empty");
        keys = { {item.name, @item.aliases} for item in (search_objects) };
        thing = complex_match(obj_string, search_objects, keys);
        thing == $failed_match && raise(E_INVARG, "No object found matching '" + obj_string + "' in inventory");
      elseif (scope == "exit")
        match_result = match_exit(obj_string);
        match_result == $failed_match && raise(E_INVARG, "No exit found matching '" + obj_string + "'");
        {thing, passage_side} = match_result;
      else
        "Default: try room objects, then players, then exits";
        try
          thing = $match:match_object(obj_string, player);
        except e (E_INVARG)
          thing = $failed_match;
        endtry
        if (thing == $failed_match)
          try
            thing = $match:match_player(obj_string, player);
          except e (E_INVARG)
            thing = $failed_match;
          endtry
        endif
        if (thing == $failed_match)
          match_result = match_exit(obj_string);
          if (match_result != $failed_match)
            {thing, passage_side} = match_result;
          endif
        endif
        thing == $failed_match && raise(E_INVARG, "No match found for '" + obj_string + "'");
      endif
    except e (ANY)
      msg = typeof(e) == TYPE_LIST && length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, msg));
      return;
    endtry
    "Walk the property chain (for assignment, stop one short)";
    result = thing;
    current_prop = "";
    walk_to = is_assignment ? length(prop_chain) - 1 | length(prop_chain);
    try
      for i in [1..walk_to]
        current_prop = prop_chain[i];
        if (current_prop == "parent")
          result = parent(result);
        else
          result = result.(current_prop);
        endif
      endfor
    except e (E_PROPNF)
      player:inform_current($event:mk_error(player, "Property ." + current_prop + " not found on " + toliteral(result)));
      return;
    except e (E_INVIND)
      player:inform_current($event:mk_error(player, "Cannot access ." + current_prop + " on " + toliteral(result) + " (not an object)"));
      return;
    except e (ANY)
      msg = typeof(e) == TYPE_LIST && length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, msg));
      return;
    endtry
    "Handle assignment";
    if (is_assignment)
      target_prop = prop_chain[$];
      target_prop == "parent" && raise(E_INVARG, "Cannot assign to .parent (use @parent command)");
      try
        result.(target_prop) = assignment_value;
        player:inform_current($event:mk_info(player, toliteral(thing) + "." + prop_chain:join(".") + " = " + toliteral(assignment_value)));
        return assignment_value;
      except e (E_PROPNF)
        player:inform_current($event:mk_error(player, "Property ." + target_prop + " not found on " + toliteral(result)));
        return;
      except e (E_PERM)
        player:inform_current($event:mk_error(player, "Permission denied setting ." + target_prop + " on " + toliteral(result)));
        return;
      except e (ANY)
        msg = typeof(e) == TYPE_LIST && length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
        player:inform_current($event:mk_error(player, msg));
        return;
      endtry
    endif
    "If for-code given, evaluate it with %# substituted (programmers only)";
    if (for_code)
      !player.programmer && raise(E_PERM, "The 'for' clause requires programmer privileges.");
      code = strsub(for_code, "%#", toliteral(result));
      try
        final_result = eval("return " + code + ";");
        if (typeof(final_result) == TYPE_LIST && final_result[1])
          result = final_result[2];
        else
          player:inform_current($event:mk_error(player, "Eval error: " + toliteral(final_result)));
          return;
        endif
      except e (ANY)
        msg = typeof(e) == TYPE_LIST && length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
        player:inform_current($event:mk_error(player, "Eval error: " + msg));
        return;
      endtry
    endif
    "Display the result";
    if (passage_side && typeof(result) == TYPE_FLYWEIGHT && !prop_chain)
      "Passage result with no property chain - show passage info";
      passage = result;
      if (passage_side == 'a)
        from_room = passage.side_a_room;
        to_room = passage.side_b_room;
        label = passage.side_a_label;
        aliases = passage.side_a_aliases;
        desc = passage.side_a_description;
      else
        from_room = passage.side_b_room;
        to_room = passage.side_a_room;
        label = passage.side_b_label;
        aliases = passage.side_b_aliases;
        desc = passage.side_b_description;
      endif
      items = {{"Passage", label + " (" + aliases:join(", ") + ")"}, {"From", from_room.name + " (" + tostr(from_room) + ")"}, {"To", to_room.name + " (" + tostr(to_room) + ")"}, {"Open", passage.is_open ? "yes" | "no"}};
      desc && (items = {@items, {"Description", desc}});
      player:inform_current($event:mk_info(player, $format.deflist:mk(items)):with_presentation_hint('inset));
    elseif (typeof(result) == TYPE_OBJ && valid(result))
      owner = `result.owner ! ANY => #-1';
      loc = `result.location ! ANY => #-1';
      items = {{"Object", tostr(result)}, {"Name", `result.name ! ANY => "(no name)"'}, {"Owner", valid(owner) ? owner.name + " (" + tostr(owner) + ")" | "???"}, {"Location", valid(loc) ? loc.name + " (" + tostr(loc) + ")" | "nowhere"}};
      player:inform_current($event:mk_info(player, $format.deflist:mk(items)):with_presentation_hint('inset));
    elseif (typeof(result) == TYPE_OBJ)
      player:inform_current($event:mk_info(player, "=> " + tostr(result) + " (invalid)"));
    else
      player:inform_current($event:mk_info(player, "=> " + toliteral(result)));
    endif
    return result;
  endverb

  verb "@show-reaction @showr" (any none none) owner: HACKER flags: "rd"
    "HINT: <object>.<property_reaction> -- Show one reaction in detail.";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @show-reaction OBJECT.PROPERTY_reaction");
    endif
    try
      prop_spec = argstr:trim();
      parsed = $prog_utils:parse_target_spec(prop_spec);
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property_reaction");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property_reaction");
      endif
      target_obj = $match:match_object(target_name, player);
      valid(target_obj) || raise(E_INVARG, "Object not found");
      prop_name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
      reaction = E_PROPNF;
      for prop_info in ($obj_utils:reaction_properties(target_obj, player, true))
        if (tostr(prop_info[1]) == prop_name)
          reaction = prop_info[2];
          break;
        endif
      endfor
      reaction == E_PROPNF && raise(E_INVARG, "Property not found: " + prop_name);
      typeof(reaction) == TYPE_FLYWEIGHT && reaction.delegate == $reaction || raise(E_INVARG, prop_name + " is not a reaction");
      trigger_str = "??";
      if (typeof(reaction.trigger) == TYPE_SYM)
        trigger_str = tostr(reaction.trigger);
      elseif (typeof(reaction.trigger) == TYPE_LIST && length(reaction.trigger) >= 4 && reaction.trigger[1] == 'when)
        {_, p, op, v} = reaction.trigger;
        trigger_str = tostr(p) + " " + tostr(op) + " " + tostr(v);
      else
        trigger_str = toliteral(reaction.trigger);
      endif
      when_str = reaction.when == 0 ? "-" | $rule_engine:decompile_rule(reaction.when);
      effects_parts = {};
      for effect in (reaction.effects)
        if (typeof(effect) == TYPE_FLYWEIGHT && effect.type)
          effects_parts = {@effects_parts, tostr(effect.type)};
        elseif (typeof(effect) == TYPE_LIST && length(effect) > 0)
          effects_parts = {@effects_parts, tostr(effect[1])};
        endif
      endfor
      effects_summary = effects_parts ? effects_parts:join(", ") | "(none)";
      enabled_str = reaction.enabled ? "yes" | "no";
      rows = {{"Object", tostr(target_obj)}, {"Property", prop_name}, {"Trigger", trigger_str}, {"When", when_str}, {"Effects", effects_summary}, {"Enabled", enabled_str}};
      table = $format.table:mk({"Field", "Value"}, rows);
      player:inform_current($event:mk_info(player, table));
      return reaction;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb _matching_candidates (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Internal helper: list likely candidate objects for a plain-name token in context.";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {token, ?context = player} = args;
    typeof(token) == TYPE_STR || return {};
    valid(context) || return {};
    needle = token:trim():lowercase();
    !needle && return {};
    scope = {};
    scope = {@scope, @context.contents};
    if (valid(context.location))
      scope = {@scope, @context.location.contents};
    endif
    candidates = {};
    seen = [];
    for o in (scope)
      if (!(o in mapkeys(seen)))
        seen[o] = 1;
      else
        continue;
      endif
      names = {o.name};
      aliases = o.aliases;
      if (typeof(aliases) == TYPE_LIST)
        names = {@names, @aliases};
      endif
      matched = 0;
      for n in (names)
        if (typeof(n) == TYPE_STR)
          lower = n:lowercase();
          if (index(lower, needle) || index(needle, lower))
            matched = 1;
            break;
          endif
        endif
      endfor
      if (matched)
        candidates = {@candidates, o};
      endif
    endfor
    return candidates;
  endverb

  verb _resolve_object_ref (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Internal helper: resolve object references with better ambiguity diagnostics.";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    {ref_string, ?context = player, ?label = "object"} = args;
    typeof(ref_string) == TYPE_STR || raise(E_TYPE, "Object reference must be a string.");
    ref = ref_string:trim();
    ref || raise(E_INVARG, "Empty " + label + " reference.");
    if (ref[1] in {"#", "$", "@"} || ref in {"me", "player", "here"})
      return $match:match_object(ref, context);
    endif
    if (!valid(context))
      context = player;
    endif
    scope = {};
    scope = {@scope, @context.contents};
    if (valid(context.location))
      scope = {@scope, @context.location.contents};
    endif
    if (!scope)
      return $match:match_object(ref, context);
    endif
    result = $match:resolve_in_scope(ref, scope, ['fuzzy_threshold -> 0.5]);
    if (result == $failed_match)
      raise(E_INVARG, "No " + label + " found matching '" + ref + "'.");
    endif
    if (result == $ambiguous_match)
      candidates = this:_matching_candidates(ref, context);
      if (!candidates)
        raise(E_INVARG, "Ambiguous " + label + " reference '" + ref + "'.");
      endif
      formatted = {};
      max_show = 8;
      count = 0;
      for o in (candidates)
        count = count + 1;
        if (count > max_show)
          break;
        endif
        formatted = {@formatted, tostr(o.name, " (", o, ")")};
      endfor
      suffix = length(candidates) > max_show ? " ..." | "";
      msg = "Ambiguous " + label + " '" + ref + "'. Candidates: " + formatted:join(", ") + suffix;
      raise(E_INVARG, msg);
    endif
    return result;
  endverb

  method test_require_unique_room_name_detects_duplicate owner: ARCH_WIZARD
    "Unit test: duplicate room-name checks reject existing rooms in an area.";
    area = $area:create(true);
    room = $room:create(true);
    try
      room:set_name_aliases("duplicate build room", {});
      move(room, area);
      raised = false;
      try
        this:_require_unique_room_name(area, "duplicate build room");
      except (E_INVARG)
        raised = true;
      endtry
      $test_utils:assert_true(raised, "duplicate room name should be rejected");
      $test_utils:assert_true(this:_require_unique_room_name(area, "unique build room"), "unique room name should pass");
    finally
      $test_utils:destroy_if_valid(room);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod

  method test_matching_room_candidates_reads_names_and_aliases owner: ARCH_WIZARD
    "Unit test: @dig ambiguous-room candidate formatting reads room names and aliases.";
    area = $area:create(true);
    room = $room:create(true);
    try
      room:set_name_aliases("candidate room", {"candidate-alias"});
      move(room, area);
      name_matches = this:_matching_room_candidates(area, "candidate room");
      alias_matches = this:_matching_room_candidates(area, "candidate-alias");
      $test_utils:assert_eq(length(name_matches), 1, "room name should produce one candidate");
      $test_utils:assert_eq(length(alias_matches), 1, "room alias should produce one candidate");
      $test_utils:assert_true(index(name_matches[1], "candidate room") > 0, "candidate should include room name");
      $test_utils:assert_true(index(alias_matches[1], tostr(room)) > 0, "candidate should include room object ref");
    finally
      $test_utils:destroy_if_valid(room);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod
endobject
