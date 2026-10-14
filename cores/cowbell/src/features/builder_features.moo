object BUILDER_FEATURES [
  import_export_id -> "builder_features",
  import_export_hierarchy -> {"features"}
]
  name: "Builder Features"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  property help_source (owner: ARCH_WIZARD, flags: "rc") = BUILDER_HELP_TOPICS;

  override description (owner: HACKER, flags: "rc") = "Provides building commands (@create, @build, @dig, etc.) for builders.";

  method _require_builder_command owner: ARCH_WIZARD
    caller == this || raise(E_PERM);
    {who} = args;
    who.is_builder || raise(E_PERM, "Builder features required.");
  endmethod

  verb "@add-message" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<property> <template> -- Add an entry to a message bag.";
    "Add a compiled message template entry to a message bag property. Usage: @add-message <object>.<prop> <template>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    if (length(args) < 2)
      player:inform_current($event:mk_error(player, "Usage: @add-message <object>.<prop> <template>"):with_audience('utility));
      return;
    endif
    const prop_spec = args[1];
    const parsed = $prog_utils:parse_target_spec(prop_spec);
    let target_name;
    let prop_name;
    if (parsed && parsed['type] == 'property)
      target_name = parsed['object_str];
      prop_name = parsed['item_name];
    elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
      const sel = parsed['selectors][1];
      sel['kind] == 'property || raise(E_INVARG, "Property must be in the form object.property");
      target_name = parsed['object_str];
      prop_name = sel['item_name];
    else
      raise(E_INVARG, "Property must be in the form object.property");
    endif
    const target = $match:match_object(target_name, player);
    valid(target) || raise(E_INVARG, "Object not found");
    prop_name:ends_with("_msg_bag") || prop_name:ends_with("_msgs") || raise(E_INVARG, "Property must end with _msg_bag or _msgs");
    "Preserve full template text after target spec";
    const offset = index(argstr, prop_spec) + length(prop_spec);
    const text = argstr[offset..length(argstr)]:trim();
    !text && raise(E_INVARG, "Usage: @add-message <object>.<prop> <template>");
    const {success, compiled} = $obj_utils:validate_and_compile_template(text, target, prop_name);
    if (!success)
      player:inform_current($event:mk_error(player, "Template compilation failed: " + compiled):with_audience('utility));
      return;
    endif
    const prop_key = $obj_utils:property_key(target, prop_name);
    const grants = prop_key == E_PROPNF ? {{"property_define", target}} | {{"property_write", target, prop_key}};
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
          new_room = area_target:make_room_in(parent_obj, room_name);
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
    "Use the shared area name check before reporting a duplicate room.";
    set_task_perms(caller_perms());
    return $area:check_room_name(@args);
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
      "Resolve local names or an explicit room reference in another area.";
      if (typeof(iobj) == TYPE_OBJ && valid(iobj) && isa(iobj, $room))
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
      "Create the passage flyweight";
      if (is_oneway)
        passage = $passage:mk(current_room, from_dir[1], from_dir, "", true, target_room, "", {}, "", false, true);
      else
        passage = $passage:mk(current_room, from_dir[1], from_dir, "", true, target_room, to_dir[1], to_dir, "", true, true);
      endif
      "Register with area using capability if we have one";
      area_cap = player:find_capability_for(area, 'area);
      area_target = typeof(area_cap) == TYPE_FLYWEIGHT ? area_cap | area;
      "Dry-run mode: report planned operation and stop before mutation";
      if (dry_run)
        area_target:create_passage(from_room_target, to_room_target, passage, allow_parallel, true);
        mode_str = is_oneway ? "one-way" | "bidirectional";
        flags_str = allow_parallel ? "--allow-parallel" | "(no extra flags)";
        preview = "Dry-run OK: " + mode_str + " passage from " + from_dir:join(",") + (is_oneway ? "" | " | " + to_dir:join(",")) + " to " + tostr(target_room) + " " + flags_str + ".";
        player:inform_current($event:mk_info(player, preview));
        return true;
      endif
      passage = area_target:create_passage(from_room_target, to_room_target, passage, allow_parallel);
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
      result = area_target:remove_passage(from_room_target, target_room, passage.connection_id);
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
    const directions = $passage:parse_directions(dir_spec, "", is_oneway);
    return mapmerge(directions, ['dry_run -> dry_run, 'allow_parallel -> allow_parallel]);
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
    "Prefix the object or exit with --djot or --literal. Passage descriptions also accept --template.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || !dobjstr || (prepstr && prepstr != "as") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@describe OBJECT_OR_DIRECTION as DESCRIPTION")));
      return;
    endif
    try
      "Try to match as object first, then fall through to passage matching on ordinary not-found errors.";
      const {target_spec, description_kind} = $builder_features:description_target(dobjstr);
      target_obj = false;
      try
        target_obj = $match:match_object(target_spec, player);
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
        this:_do_describe_object(target_obj, new_description,
          typeof(description_kind) == TYPE_SYM ? description_kind | target_obj.description_kind);
        obj_name = `target_obj.name ! ANY => tostr(target_obj)';
        message = "Set description of \"" + obj_name + "\" (" + tostr(target_obj) + ").";
        player:inform_current($event:mk_info(player, message));
        return 1;
      else
        "Not an object - try to match as a passage direction";
        result = this:_do_describe_passage(target_spec, iobjstr,
          description_kind == 'djot_blocks ? 'djot_inline | description_kind);
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

  method description_target owner: HACKER
    "Parse an optional description format flag without interpreting the source text.";
    const {text} = args;
    for entry in ({{"--djot ", 'djot_blocks}, {"--literal ", 'literal}, {"--template ", 'template}})
      if (text:starts_with(entry[1]))
        return {text[length(entry[1]) + 1..$]:trim(), entry[2]};
      endif
    endfor
    return {text:trim(), false};
  endmethod

  verb "@edit-description @edit-d" (any none none) owner: ARCH_WIZARD flags: "rd"
    "Edit an object's or passage's source. Optional --literal, --djot, and passage --template select syntax.";
    caller == player || raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    try
      const {spec, requested_kind} = $builder_features:description_target(dobjstr);
      length(spec) || raise(E_INVARG, "Use @edit-description OBJECT_OR_DIRECTION.");
      let target = `$match:match_object(spec, player) ! E_INVARG, E_PROPNF => #-1';
      let identity = "";
      let source;
      let old_kind;
      let title;
      if (typeof(target) == TYPE_OBJ && valid(target))
        $root:_check_permissions_as(target, player, 'set_description);
        source = target.description;
        old_kind = target.description_kind;
        title = target.name;
      else
        target = player.location;
        const passage = $room:find_passage(target, spec);
        typeof(passage) == TYPE_FLYWEIGHT || raise(E_INVARG, "No object or exit called '" + spec + "' here.");
        $area:_check_room_right(player, $area:_subject_for(player, target, 'room), 'dig_from);
        identity = passage.connection_id;
        source = passage:description_for(target);
        old_kind = passage:description_kind_for(target);
        title = passage:label_for(target) + " at " + target.name;
      endif
      typeof(source) == TYPE_STR
        || raise(E_INVARG, "This description is built in code. Use @describe to replace it with editable text.");
      let kind = typeof(requested_kind) == TYPE_SYM ? requested_kind | old_kind;
      identity != "" && kind == 'djot_blocks && (kind = 'djot_inline);
      if (identity == "")
        kind in {'literal, 'djot_blocks} || raise(E_INVARG, "Object descriptions use --literal or --djot.");
      endif
      const conn = connection();
      const state = ['connection -> conn, 'passage -> identity, 'format -> kind,
        'original -> source, 'original_format -> old_kind];
      const session_id = player:start_edit_session(target, "edit_description", state);
      const content_type = kind in {'djot_blocks, 'djot_inline} ? "text/djot" | "text/plain";
      $builder_features:_present_description_editor(player, session_id, source, title, content_type);
    except problem (E_INVARG, E_PERM, E_TYPE)
      player:inform_current($event:mk_error(player, problem[2]));
    endtry
  endverb

  method _present_description_editor owner: ARCH_WIZARD
    "Present only on the authenticated editor's bound connection; presentation needs native authority.";
    const principal = caller_perms();
    const {who, session_id, source, title, content_type} = args;
    principal == who || (valid(principal) && principal.wizard) || raise(E_PERM);
    const session = who.editing_sessions[session_id];
    session['verb] == "edit_description" || raise(E_PERM);
    const conn = session['args]['connection];
    conn in {entry[1] for entry in (connections(who))} || raise(E_PERM);
    present(conn, session_id, content_type, "text-editor", source,
      {{"object", $url_utils:to_curie_str($builder_features)}, {"verb", "receive_description_edit"},
      {"title", "Edit Description: " + title}, {"text_mode", "string"}, {"session_id", session_id}});
  endmethod

  method receive_description_edit owner: ARCH_WIZARD
    "Save under the editor's current authority, rejecting replacement exits and conflicting source edits.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player
      || (valid(principal) && principal.wizard)) || raise(E_PERM);
    set_task_perms(player);
    const {session_id, content} = args;
    const session = player:get_edit_session(session_id);
    session['verb] == "edit_description" || raise(E_PERM);
    const state = session['args];
    const conn = state['connection];
    const request_conn = `connection() ! E_INVARG => #-1';
    request_conn == #-1 || request_conn == conn || raise(E_PERM);
    request_conn == #-1 || conn in {entry[1] for entry in (connections(player))} || raise(E_PERM);
    if (content == 'close)
      player:end_edit_session(session_id);
      return;
    endif
    typeof(content) == TYPE_STR || raise(E_TYPE, "Description source must be text.");
    const target = session['target];
    valid(target) || raise(E_INVARG, "The edited object no longer exists.");
    if (state['passage] == "")
      $root:_check_permissions_as(target, player, 'set_description);
      equal(target.description, state['original]) && target.description_kind == state['original_format]
        || raise(E_INVARG, "The description changed while this editor was open. Reopen it before saving.");
      target:set_description(content, state['format]);
    else
      const found = $room:find_connection(target, state['passage]);
      length(found) || raise(E_INVARG, "The edited passage is no longer connected. Reopen the editor.");
      const {area, passage} = found;
      const subject = $area:_subject_for(player, target, 'room);
      equal(passage:description_for(target), state['original])
        && passage:description_kind_for(target) == state['original_format]
        || raise(E_INVARG, "The description changed while this editor was open. Reopen it before saving.");
      const updated = passage:with_description_from(target, content, state['format]);
      equal(player:get_edit_session(session_id), session) || raise(E_PERM, "The editing session changed.");
      area:update_passage(subject, passage:other_room(target), updated);
    endif
    let saved = session;
    saved['args]['original] = content;
    saved['args]['original_format] = state['format];
    player.editing_sessions[session_id] = saved;
    player:inform_connection(conn, $event:mk_info(player, "Description updated for " + target.name + "."));
  endmethod

  method _do_describe_object owner: ARCH_WIZARD
    "Internal helper to set object description with elevated permissions";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    const {target_obj, new_description, ?kind = target_obj.description_kind} = args;
    target_obj:set_description(new_description, kind);
  endmethod

  method _do_describe_passage owner: ARCH_WIZARD
    "Describe one local connection using the builder's endpoint authority. Return false if absent.";
    caller == this || raise(E_PERM);
    set_task_perms(player);
    const {direction, source, ?kind = false} = args;
    const description = source:trim();
    length(description) || raise(E_INVARG, "Description cannot be blank.");
    const room = player.location;
    valid(room) && isa(room, $room) || return false;
    const passage = $room:find_passage(room, direction:trim());
    typeof(passage) == TYPE_FLYWEIGHT || return false;
    const {area, current} = $room:find_connection(room, passage.connection_id);
    const subject = $area:_subject_for(player, room, 'room);
    const format = typeof(kind) == TYPE_SYM ? kind | current:description_kind_for(room);
    const updated = current:with_description_from(room, description, format):with_ambient_from(room, true);
    area:update_passage(subject, current:other_room(room), updated);
    player:inform_current($event:mk_info(player, "Set ambient description for '", direction, "' passage."));
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
      const moving_player = is_player(target_obj);
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
      if (moving_player && valid(old_loc))
        old_loc:announce_all_but({target_obj}, obj_name + " disappears suddenly for parts unknown.");
      endif
      if (moving_player && valid(dest_loc))
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

  verb "@set-m*essage @setm" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Set a custom message template on an object property.";
    "Usage: @set-message OBJECT.PROPERTY template string...";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    if (!argstr || length(args) < 2)
      raise(E_INVARG, "Usage: @set-message OBJECT.PROPERTY template string...");
    endif
    try
      const target_spec = args[1];
      "Parse property reference";
      const parsed = $prog_utils:parse_target_spec(target_spec);
      if (!parsed)
        player:inform_current($event:mk_error(player, "Usage: @set-message <object>.<prop-name> template..."));
        return;
      endif
      "Handle both old 'property type and new 'compound type";
      let object_str;
      let prop_name;
      if (parsed['type] == 'property)
        object_str = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed['type] == 'compound && length(parsed['selectors]) > 0)
        const sel = parsed['selectors][1];
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
      let target_obj;
      try
        target_obj = $match:match_object(object_str, player);
      except e (E_INVARG, E_PROPNF)
        player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
        return;
      except e (ANY)
        raise(e[1], "Object lookup failed: " + toliteral(e));
      endtry
      "Get remainder after target spec";
      const offset = index(argstr, target_spec) + length(target_spec);
      const template_string = argstr[offset..length(argstr)]:trim();
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object does not exist.");
      "Check if property is writable";
      const {writable, error_msg} = $obj_utils:check_message_property_writable(target_obj, prop_name, player);
      if (!writable)
        raise(E_PERM, error_msg);
      endif
      "Compile and validate the template";
      const {success, result} = $obj_utils:validate_and_compile_template(template_string, target_obj, prop_name);
      if (!success)
        raise(E_INVARG, "Template compilation failed: " + result);
      endif
      const compiled_list = result;
      const obj_name = `target_obj.name ! ANY => tostr(target_obj)';
      const success_message = "Set message template on " + obj_name + " (" + tostr(target_obj) + ")." + prop_name + ".";
      "Set the compiled message";
      const prop_key = $obj_utils:property_key(target_obj, prop_name);
      prop_key == E_PROPNF && raise(E_PROPNF, "Property not found: " + prop_name);
      $obj_utils:set_compiled_message(target_obj, prop_key, compiled_list, player, {{"property_write", target_obj, prop_key}});
      player:inform_current($event:mk_info(player, success_message));
      return true;
    except e (ANY)
      const message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  verb "@message-bindings @preview-message" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: OBJECT.PROPERTY [TEMPLATE] -- Explain bindings or preview a draft without saving.";
    caller == player || raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    length(args) >= 1 || raise(E_INVARG, "Use " + verb + " OBJECT.PROPERTY.");
    const parsed = $prog_utils:parse_target_spec(args[1]);
    (parsed && parsed['type] == 'compound && length(parsed['selectors]) == 1
      && parsed['selectors][1]['kind] == 'property)
      || raise(E_INVARG, "Use an object.property reference.");
    const target = $match:match_object(parsed['object_str], player);
    const property = parsed['selectors][1]['item_name];
    const bindings = target:message_bindings(property);
    let lines = {"Bindings for " + args[1] + ":"};
    if (verb == "@preview-message")
      length(args) > 1 || raise(E_INVARG, "Use @preview-message OBJECT.PROPERTY TEMPLATE.");
      const template = argstr[length(args[1]) + 1..$]:trim();
      lines = $sub_utils:preview(template, bindings, target);
    else
      length(args) == 1 || raise(E_INVARG, "Use @message-bindings OBJECT.PROPERTY.");
      for name in (mapkeys(bindings))
        lines = {@lines, "{" + tostr(name) + "}: " + bindings[name]['description]
          + " (" + tostr(bindings[name]['kind]) + ")"};
      endfor
      if (!length(bindings))
        lines = {@lines, "This message has no declared bindings. Literal text is allowed.",
          "Its programmer must declare any participants before you can use their names or pronouns."};
      endif
    endif
    for line in (lines)
      player:inform_current($event:mk_info(player, line):with_audience('utility));
    endfor
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

  verb "@set-r*ule" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Set a rule property on an object. Usage: @set-rule <object>.<rule-property> <expression>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
    endif
    if (!args[1])
      raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
    endif
    try
      "args[1] is the object.property part, rest of argstr is the rule expression";
      const prop_spec = args[1];
      const rule_expr = argstr[length(prop_spec) + 1..$]:trim();
      if (!rule_expr || rule_expr == "")
        raise(E_INVARG, "Usage: @set-rule OBJECT.RULE_PROPERTY EXPRESSION");
      endif
      const parsed = $prog_utils:parse_target_spec(prop_spec);
      let target_name;
      let prop_name;
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        const sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property");
      endif
      "Match object";
      const target = $match:match_object(target_name, player);
      valid(target) || raise(E_INVARG, "Object not found");
      "Property must end with _rule to prevent accidents";
      prop_name:ends_with("_rule") || raise(E_INVARG, "Rule properties must end with '_rule'");
      "Permission check";
      if (!player.wizard && target.owner != player)
        raise(E_PERM, "You don't own " + tostr(target) + ".");
      endif
      "Parse and validate rule";
      const rule = $rule_engine:parse_expression(rule_expr, tosym(prop_name), $rule_engine:policy_scope(prop_name));
      "Validate for bounded negation violations without evaluating";
      const validation = $rule_engine:validate_rule(rule);
      if (length(validation['warnings]) > 0)
        for warning in (validation['warnings])
          player:inform_current($event:mk_error(player, "Warning: " + warning));
        endfor
        if (!validation['valid])
          raise(E_INVARG, "Rule has errors - fix bounded negation issues.");
        endif
      endif
      target.(prop_name) = rule;
      const message = "Set rule on " + tostr(target) + "." + prop_name + ": \"" + rule_expr + "\"";
      player:inform_current($event:mk_info(player, message));
      return rule;
    except e (ANY)
      const error_message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, error_message));
      return 0;
    endtry
  endverb

  verb "@clear-rule" (any none none) owner: ARCH_WIZARD flags: "rd"
    "Clear a rule property. Usage: @clear-rule <object>.<rule-property>";
    caller != player && raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    if (!argstr)
      raise(E_INVARG, "Usage: @clear-rule OBJECT.RULE_PROPERTY");
    endif
    try
      const prop_spec = argstr:trim();
      const parsed = $prog_utils:parse_target_spec(prop_spec);
      let target_name;
      let prop_name;
      if (parsed && parsed['type] == 'property)
        target_name = parsed['object_str];
        prop_name = parsed['item_name];
      elseif (parsed && parsed['type] == 'compound && length(parsed['selectors]) > 0)
        const sel = parsed['selectors][1];
        sel['kind] == 'property || raise(E_INVARG, "Property must be object.property");
        target_name = parsed['object_str];
        prop_name = sel['item_name];
      else
        raise(E_INVARG, "Property must be object.property");
      endif
      "Match object";
      const target = $match:match_object(target_name, player);
      valid(target) || raise(E_INVARG, "Object not found");
      "Property must end with _rule";
      prop_name:ends_with("_rule") || raise(E_INVARG, "Rule properties must end with '_rule'");
      "Permission check";
      if (!player.wizard && target.owner != player)
        raise(E_PERM, "You don't own " + tostr(target) + ".");
      endif
      "Clear the rule";
      target.(prop_name) = 0;
      const message = "Cleared rule on " + tostr(target) + "." + prop_name;
      player:inform_current($event:mk_info(player, message));
      return true;
    except e (ANY)
      const error_message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, error_message));
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
    if (topic in {"@reaction", "@state", "@rule", "@facts", "reactions", "rules"})
      return <$help, .name = topic, .content_kind = 'djot_blocks,
        .content = $builder_features:reaction_help():join("\n\n"), .category = 'building,
        .summary = "Make objects respond without writing code", .aliases = {}, .see_also = {}>;
    endif
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
    "Inspect an exit's configuration and preview its room prose and movement messages.";
    caller == player || raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    try
      const direction = dobjstr:trim();
      length(direction) || raise(E_INVARG, "Use @passage DIRECTION.");
      const room = player.location;
      const passage = $room:find_passage(room, direction);
      typeof(passage) == TYPE_FLYWEIGHT || raise(E_INVARG, "No exit called '" + direction + "' here.");
      const destination = passage:other_room(room);
      const source = passage:description_for(room);
      const label = passage:label_for(room);
      let rows = {
        {"Direction", label}, {"Aliases", passage:aliases_for(room):join(", ")},
        {"Description source", typeof(source) == TYPE_STR ? source | toliteral(source)},
        {"Description syntax", tostr(passage:description_kind_for(room))},
        {"In room prose", passage.(room == passage.side_a_room ? "side_a_ambient" | "side_b_ambient") ? "yes" | "no"},
        {"Prose style", tostr(passage:prose_style_for(room))},
        {"Departure phrase", passage:departure_phrase_for(room)},
        {"Arrival phrase", passage:arrival_phrase_for(room)},
        {"Destination", destination.name + " (" + tostr(destination) + ")"},
        {"Return direction", $passage:allows_direction(passage, destination) ? passage:label_for(destination) | "one-way"},
        {"Open", passage.is_open ? "yes" | "no"}, {"Locked", passage.is_locked ? "yes" | "no"}
      };
      const rule = passage.unlock_rule;
      rows = {@rows, {"Access condition", typeof(rule) == TYPE_FLYWEIGHT
        ? $builder_features:_author_condition_text(rule.condition) | "none"}};
      for property in ({"open_msg", "close_msg", "lock_msg", "unlock_msg"})
        const message = passage.(property);
        const authored = typeof(message) in {TYPE_STR, TYPE_LIST, TYPE_FLYWEIGHT};
        rows = {@rows, {property, authored && message != "" && message != {}
          ? $sub_utils:decompile(message) | "default message"}};
      endfor
      const context = $event:mk_look(player):with_dobj(room);
      const prose = $look:_ambient({{$passage:description_content(passage, room),
        passage:prose_style_for(room), label, destination}}, [label -> room:exit_annotation(label)], context, player);
      const departure = $passage:movement_from(passage, player, room, destination, false):resolve(player);
      const arrival = $passage:movement_from(passage, player, room, destination, true):resolve(player);
      const content = $format.block:mk($format.title:mk("Passage: " + label, 3),
        $format.table:mk({"Property", "Value"}, rows), $format.title:mk("Room prose", 4), @prose,
        $format.title:mk("Leaving this room", 4), @flycontents(departure),
        $format.title:mk("Arriving at the other room", 4), @flycontents(arrival));
      player:inform_current($event:mk_info(player, content));
    except problem (E_INVARG, E_PERM, E_TYPE)
      player:inform_current($event:mk_error(player, problem[2]));
    endtry
  endverb

  verb "@set-passage @setp" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Set exit prose, aliases, door mode, access conditions, or door messages.";
    caller != player && raise(E_PERM);
    player.is_builder || raise(E_PERM, "Builder features required.");
    set_task_perms(player);
    if (!argstr || length(args) < 3)
      lines = {"@set-passage DIRECTION PROPERTY VALUE", "", "Properties:", "  description DESC   - Ambient description (links the complete phrase)", "  departure PHRASE   - e.g., 'through the door'", "  arrival PHRASE     - e.g., 'from the kitchen'", "  style sentence|fragment", "  aliases A,B,C", "  is_door true|false - Enable/disable door operations (open/close/lock/unlock)", "  access CONDITION - e.g., key is \"brass key\"; clear restores an unlocked passage", "  open_msg|close_msg|lock_msg|unlock_msg TEMPLATE - use clear for the default message"};
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
      elseif (prop == "access")
        if (value == "clear")
          new_passage = passage:with_unlock_rule(0):with_locked(false);
        else
          const scope = $rule_engine:passage_scope();
          const tree = $builder_features:_author_condition($builder_features:_author_words(value), scope, player);
          new_passage = passage:with_unlock_rule($rule:mk('unlock_rule, tree, scope));
        endif
        prop_display = "access condition";
      elseif (prop in {"open_msg", "close_msg", "lock_msg", "unlock_msg"})
        const message = value == "clear" ? {} | $sub_utils:compile(value, $passage:message_bindings(prop));
        props = passage:_extract_all();
        props[tosym(prop)] = message;
        new_passage = passage:_mk_from_props(props);
        prop_display = prop;
      else
        raise(E_INVARG, "Unknown property. Use: description, departure, arrival, style, aliases, is_door, access, open_msg, close_msg, lock_msg, unlock_msg");
      endif
      area:update_passage(current_room, other_room, new_passage);
      player:inform_current($event:mk_info(player, "Set " + direction + " " + prop_display + "."));
    except e (ANY)
      message = typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
    endtry
  endverb

  verb "#*" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Look up #NAME[.PROPERTY...] [SCOPE] [for EXPRESSION], or assign with = EXPRESSION.";
    caller == player || raise(E_PERM);
    const is_builder = player.is_builder;
    set_task_perms(player);
    try
      is_builder || raise(E_PERM, "Builder features required.");
      const parts = verb[2..$]:split(".");
      let reference = parts[1];
      const properties = length(parts) > 1 ? parts[2..$] | {};
      if (!length(reference))
        return player:inform_current($event:mk_info(player,
          "Use #NAME[.PROPERTY...] [exit|player|inventory] [for EXPRESSION], or #NAME.PROPERTY = EXPRESSION. See help #."));
      endif
      for property in (properties)
        length(property) || raise(E_INVARG, "A dot must be followed by a property name.");
      endfor
      let rest = argstr:trim();
      const assignment = length(rest) > 0 && rest[1] == "=";
      let expression = "";
      let scope = "";
      if (assignment)
        player.programmer || raise(E_PERM, "Assignment requires programmer privileges.");
        length(properties) || raise(E_INVARG, "Assignment needs a property: #NAME.PROPERTY = VALUE.");
        expression = rest[2..$]:trim();
        length(expression) || raise(E_INVARG, "Missing value after =.");
      elseif (length(rest))
        const words = rest:split(" ");
        if (words[1] in {"exit", "player", "inventory"})
          scope = words[1];
          rest = rest[length(scope) + 1..$]:trim();
        endif
        if (length(rest))
          rest == "for" && raise(E_INVARG, "Missing expression after for.");
          index(rest, "for ") == 1 || raise(E_INVARG,
            "Use exit, player, inventory, or for EXPRESSION after the object name.");
          expression = rest[5..$]:trim();
          length(expression) || raise(E_INVARG, "Missing expression after for.");
          player.programmer || raise(E_PERM, "The for clause requires programmer privileges.");
        endif
      endif
      fn match_player(name)
        if (name[1] == "#" || name in {"me", "myself"})
          return $match:match_player(name, player);
        endif
        return $builder_features:_hash_match(name, players());
      endfn
      fn match_exit(name)
        const room = player.location;
        valid(room) || return {$failed_match, 0};
        const area = room.location;
        (valid(area) && respond_to(area, 'passages_from)) || return {$failed_match, 0};
        const passages = area:passages_from(room);
        length(passages) || return {$failed_match, 0};
        const aliases = {$passage:command_names(passage, room) for passage in (passages)};
        const matched = $builder_features:_hash_match(name, passages, aliases);
        if (typeof(matched) != TYPE_FLYWEIGHT)
          return {matched, 0};
        endif
        return {matched, matched.side_a_room == room ? 'a | 'b};
      endfn
      let target = $failed_match;
      let passage_side = 0;
      "Keep the leading # on numbered and UUID references; named lookup uses it as the command prefix.";
      if ($str_proto:match_objid("#" + reference, true))
        reference = "#" + reference;
      endif
      if (scope == "player")
        target = match_player(reference);
      elseif (scope == "inventory")
        target = $builder_features:_hash_match(reference, player.contents);
      elseif (scope == "exit")
        {target, passage_side} = match_exit(reference);
      else
        if (reference[1] == "@")
          length(reference) > 1 || raise(E_INVARG, "Missing player name after @.");
          target = match_player(reference[2..$]);
        elseif (reference[1] in {"#", "$"} || reference in {"me", "player", "here"})
          target = $match:match_object(reference, player);
        else
          const nearby = valid(player.location) ? player.location.contents | {};
          target = $builder_features:_hash_match(reference, {@player.contents, @nearby});
          if (target == $failed_match)
            target = match_player(reference);
          endif
          if (target == $failed_match)
            {target, passage_side} = match_exit(reference);
          endif
        endif
      endif
      target == $failed_match && raise(E_INVARG, "No match found for '" + reference + "'.");
      let result = target;
      const walk_to = assignment ? length(properties) - 1 | length(properties);
      for property in (properties[1..walk_to])
        try
          result = property == "parent" ? parent(result) | result.(property);
        except problem (E_PROPNF)
          raise(E_PROPNF, "Property ." + property + " not found on " + toliteral(result));
        except problem (E_INVIND)
          raise(E_INVIND, "Cannot access ." + property + " on " + toliteral(result));
        endtry
      endfor
      if (assignment)
        const property = properties[$];
        property == "parent" && raise(E_INVARG, "Use @parent to change an object's parent.");
        const value = $prog_utils:eval_expression(expression);
        try
          result.(property) = value;
        except problem (E_PROPNF)
          raise(E_PROPNF, "Property ." + property + " not found on " + toliteral(result));
        endtry
        player:inform_current($event:mk_info(player,
          toliteral(target) + "." + properties:join(".") + " = " + toliteral(value)));
        return value;
      endif
      if (length(expression))
        result = $prog_utils:eval_expression(expression, result);
      endif
      if (passage_side && typeof(result) == TYPE_FLYWEIGHT && !length(properties))
        const room = player.location;
        const destination = result:other_room(room);
        const label = passage_side == 'a ? result.side_a_label | result.side_b_label;
        let items = {{"Passage", label + " (" + result:aliases_for(room):join(", ") + ")"},
          {"From", room.name + " (" + tostr(room) + ")"},
          {"To", destination.name + " (" + tostr(destination) + ")"},
          {"Open", result.is_open ? "yes" | "no"}};
        const description = result:description_for(room);
        if (description)
          items = {@items, {"Description", description}};
        endif
        player:inform_current($event:mk_info(player, $format.deflist:mk(items))
          :with_presentation_hint('inset));
      elseif (typeof(result) == TYPE_OBJ && valid(result))
        const owner = `result.owner ! E_PERM => #-1';
        const location = `result.location ! E_PERM => #-1';
        const items = {{"Object", tostr(result)}, {"Name", result.name},
          {"Owner", valid(owner) ? owner.name + " (" + tostr(owner) + ")" | "???"},
          {"Location", valid(location) ? location.name + " (" + tostr(location) + ")" | "nowhere"}};
        player:inform_current($event:mk_info(player, $format.deflist:mk(items))
          :with_presentation_hint('inset));
      else
        player:inform_current($event:mk_info(player, "=> " + toliteral(result)));
      endif
      return result;
    except problem (ANY)
      "User expressions may raise any error; report it at the command boundary.";
      player:inform_current($event:mk_error(player, problem[2]));
      return false;
    endtry
  endverb

  method _hash_match owner: ARCH_WIZARD
    "Return a unique best match for # lookup; reject ambiguity before reading or writing properties.";
    set_task_perms(caller_perms());
    const {name, objects, ?keys = {}} = args;
    const names = length(keys) ? keys | {{object.name, @object.aliases} for object in (objects)};
    const matches = complex_matches(name, objects, names);
    length(matches) <= 1 || raise(E_INVARG,
      "More than one match for '" + name + "'. Use a more specific name or object number.");
    return length(matches) ? matches[1] | $failed_match;
  endmethod

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
  method _author_words owner: HACKER
    "Split command text while preserving quoted names and messages. Do not evaluate code.";
    const {text} = args;
    return {token['text] for token in ($condition:_tokens(text))};
  endmethod

  method _author_text owner: HACKER
    "Unquote one text token. All other words remain text.";
    const {word} = args;
    return word:starts_with("\"") ? fromliteral(word) | word;
  endmethod

  method _author_object owner: ARCH_WIZARD
    "Resolve an object from the author's perspective, under the author's read permissions.";
    set_task_perms(caller_perms());
    const {word, who} = args;
    const object = $match:match_object($builder_features:_author_text(word), who);
    typeof(object) == TYPE_OBJ && valid(object) || raise(E_INVARG, "No object matches " + word + ".");
    return object;
  endmethod

  method _author_type owner: HACKER
    "Translate the small builder vocabulary for settings and state types.";
    const {word} = args;
    const types = ["number" -> 'integer, "decimal" -> 'float, "text" -> 'string,
      "flag" -> 'boolean, "object" -> 'object, "choice" -> 'string];
    maphaskey(types, word) || raise(E_INVARG, "Use number, decimal, text, flag, object, or choice.");
    return types[word];
  endmethod

  method _author_value owner: ARCH_WIZARD
    "Resolve a builder value to the shared literal or reference form, with an optional expected type.";
    set_task_perms(caller_perms());
    const {word, scope, who, ?expected = 'any} = args;
    const aliases = ["this" -> 'This, "actor" -> maphaskey(scope['input], 'Actor) ? 'Actor | 'Accessor,
      "place" -> 'Location, "item" -> maphaskey(scope['input], 'Item) ? 'Item | 'Dobj,
      "key" -> 'Key, "food" -> 'Food, "who" -> 'Who, "from" -> 'From, "to" -> 'To,
      "exit" -> 'Direction, "connection" -> 'Connection, "passage" -> 'Passage];
    if (maphaskey(aliases, word) && maphaskey(scope['input], aliases[word]))
      return ['kind -> 'input, 'name -> aliases[word]];
    endif
    for namespace in ({'state, 'parameter, 'binding})
      if (maphaskey(scope[namespace], tosym(word)))
        return ['kind -> namespace, 'name -> tosym(word)];
      endif
    endfor
    let value;
    if (expected == 'object || word in {"me", "here"} || word:starts_with("#") || word:starts_with("$"))
      value = $builder_features:_author_object(word, who);
    elseif (expected == 'string)
      value = $builder_features:_author_text(word);
    elseif (expected == 'symbol)
      value = tosym($builder_features:_author_text(word));
    elseif (word in {"true", "yes", "false", "no"})
      value = !!(word in {"true", "yes"});
    elseif (pcre_match(word, "^-?[0-9]+([.][0-9]+)?$"))
      value = fromliteral(word);
      expected == 'float && typeof(value) == TYPE_INT && (value = tofloat(value));
    else
      value = $builder_features:_author_text(word);
    endif
    $behavior_schema:_type(value, expected, "value " + word);
    return ['kind -> 'literal, 'value -> value];
  endmethod

  method _author_relation owner: ARCH_WIZARD
    "Resolve a relation object or setting and read its fields under the author's permissions.";
    set_task_perms(caller_perms());
    const {word, scope, who, parameters} = args;
    const reference = $builder_features:_author_value(word, scope, who, 'object);
    const target = $condition:relation_target(reference, parameters);
    return {reference, $relation:schema(target)['types]};
  endmethod

  method _author_condition owner: ARCH_WIZARD
    "Parse builder conditions into the same tree used by rules. Preserve grouping and precedence.";
    set_task_perms(caller_perms());
    const {words, scope, who, ?depth = 0, ?parameters = []} = args;
    depth <= 16 || raise(E_QUOTA, "The condition has too many nested groups.");
    length(words) > 0 || raise(E_INVARG, "Supply a condition, or use always.");
    let nesting = 0;
    for word in (words)
      word == "(" && (nesting = nesting + 1);
      word == ")" && (nesting = nesting - 1);
      nesting >= 0 || raise(E_INVARG, "A condition has an extra closing parenthesis.");
    endfor
    nesting == 0 || raise(E_INVARG, "A condition needs a closing parenthesis.");
    for operator in ({"OR", "AND"})
      nesting = 0;
      for i in [1..length(words)]
        words[i] == "(" && (nesting = nesting + 1);
        words[i] == ")" && (nesting = nesting - 1);
        if (nesting == 0 && words[i] == operator)
          const left = $builder_features:_author_condition(words[1..i - 1], scope, who, depth + 1, parameters);
          let right_scope = scope;
          if (operator == "AND")
            right_scope['binding] = $behavior_schema:condition(left, scope, $rule_engine:schemas())['bindings];
          endif
          const right = $builder_features:_author_condition(words[i + 1..$], right_scope, who, depth + 1, parameters);
          return ['kind -> operator == "AND" ? 'all | 'any, 'children -> {left, right}];
        endif
      endfor
    endfor
    if (words[1] == "(")
      words[$] == ")" || raise(E_INVARG, "Unexpected text after a condition group.");
      return $builder_features:_author_condition(words[2..$ - 1], scope, who, depth + 1, parameters);
    elseif (words[1] == "NOT")
      return ['kind -> 'not, 'child -> $builder_features:_author_condition(words[2..$], scope, who, depth + 1, parameters)];
    elseif (length(words) == 1 && words[1] in {"always", "never"})
      return ['kind -> words[1] == "always" ? 'all | 'any, 'children -> {}];
    elseif (words[1] == "fact")
      if (length(words) >= 2 && words[2] == "in")
        length(words) >= 3 || raise(E_INVARG, "Use fact in RELATION FIELD=VALUE or FIELD as NAME.");
        const {reference, fields} = $builder_features:_author_relation(words[3], scope, who, parameters);
        (length(words) - 3) % 3 == 0 || raise(E_INVARG, "Use FIELD=VALUE or FIELD as NAME.");
        let values = [];
        let bind = [];
        for i in [1..(length(words) - 3) / 3]
          const offset = i * 3;
          const field = tosym(words[offset + 1]);
          maphaskey(fields, field) || raise(E_INVARG, "Unknown relation field: " + tostr(field));
          (!maphaskey(values, field) && !maphaskey(bind, field))
            || raise(E_INVARG, "Repeated relation field: " + tostr(field));
          if (words[offset + 2] == "=")
            values[field] = $builder_features:_author_value(words[offset + 3], scope, who, fields[field]);
          elseif (words[offset + 2] == "as")
            const name = tosym(words[offset + 3]);
            $behavior_schema:_name(name, "match name");
            !(words[offset + 3] in {"this", "actor", "place", "item", "key", "food", "who", "me", "here", "from", "to"})
              && !maphaskey(scope['state], name) && !maphaskey(scope['parameter], name)
              || raise(E_INVARG, "Choose a match name that differs from event inputs, state, and settings.");
            bind[field] = name;
          else
            raise(E_INVARG, "Use FIELD=VALUE or FIELD as NAME.");
          endif
        endfor
        return ['kind -> 'relation, 'relation -> reference, 'fields -> fields,
          'arguments -> values, 'bind -> bind];
      endif
      length(words) >= 2 || raise(E_INVARG, "Use fact NAME FIELD=VALUE, or fact in RELATION FIELD=VALUE.");
      const name = tosym(words[2]);
      const schemas = $rule_engine:schemas();
      maphaskey(schemas, name) || raise(E_INVARG, "Unknown fact: " + words[2] + ". Use @reaction facts.");
      const values = $builder_features:_author_arguments(words[3..$], schemas[name]['inputs], scope, who);
      return ['kind -> 'predicate, 'name -> name, 'arguments -> values, 'bind -> []];
    elseif (words[1] == "find")
      (length(words) == 6 && words[2] == "occupant" && words[3] == "in" && words[5] == "as")
        || raise(E_INVARG, "Use find occupant in PLACE as NAME.");
      $behavior_schema:_name(tosym(words[6]), "match name");
      !(words[6] in {"this", "actor", "place", "item", "key", "food", "who", "me", "here", "from", "to"})
        && !maphaskey(scope['state], tosym(words[6])) && !maphaskey(scope['parameter], tosym(words[6]))
        || raise(E_INVARG, "Choose a match name that differs from event inputs, state, and settings.");
      return ['kind -> 'predicate, 'name -> 'occupants,
        'arguments -> ['location -> $builder_features:_author_value(words[4], scope, who, 'object)],
        'bind -> ['object -> tosym(words[6])]];
    endif
    length(words) >= 3 || raise(E_INVARG, "Use a comparison such as visits >= 4 or actor is me.");
    if (words[2] == "is" && words[3] == "not")
      return ['kind -> 'not, 'child -> $builder_features:_author_condition({words[1], "is", @words[4..$]}, scope, who, depth + 1, parameters)];
    endif
    let predicate = 'is;
    let values = [];
    if (words[2] == "is" && length(words) == 3 && words[3] in {"open", "locked", "portable"})
      predicate = tosym("is_" + words[3]);
      values = ['object -> $builder_features:_author_value(words[1], scope, who, 'object)];
    elseif (words[2] == "is" && length(words) == 4 && words[3] in {"in", "a"})
      predicate = words[3] == "in" ? 'location_is | 'isa;
      const field = words[3] == "in" ? 'location | 'prototype;
      values = ['object -> $builder_features:_author_value(words[1], scope, who, 'object),
        field -> $builder_features:_author_value(words[4], scope, who, 'object)];
    elseif (length(words) == 5 && words[2..4] == {"is", "owned", "by"})
      predicate = 'owner_is;
      values = ['object -> $builder_features:_author_value(words[1], scope, who, 'object),
        'owner -> $builder_features:_author_value(words[5], scope, who, 'object)];
    else
      const ops = ["is" -> 'eq, "=" -> 'eq, "==" -> 'eq, "!=" -> 'ne,
        ">" -> 'gt, "<" -> 'lt, ">=" -> 'ge, "<=" -> 'le];
      (length(words) == 3 && maphaskey(ops, words[2]))
        || raise(E_INVARG, "Unknown condition. Use @reaction conditions for examples.");
      const left = $builder_features:_author_value(words[1], scope, who);
      if (left['kind] == 'literal && typeof(left['value]) == TYPE_STR && !words[1]:starts_with("\""))
        raise(E_INVARG, "Unknown name: " + words[1] + ". Use @state OBJECT to see declared state, or add a named setting with with.");
      endif
      const kind = $behavior_schema:expression(left, scope);
      return ['kind -> 'compare, 'op -> ops[words[2]], 'left -> left,
        'right -> $builder_features:_author_value(words[3], scope, who, kind)];
    endif
    return ['kind -> 'predicate, 'name -> predicate, 'arguments -> values, 'bind -> []];
  endmethod

  method _author_arguments owner: ARCH_WIZARD
    "Read exact named arguments from FIELD=VALUE pairs. The shared type checker validates references.";
    set_task_perms(caller_perms());
    const {words, schema, scope, who} = args;
    length(words) % 3 == 0 || raise(E_INVARG, "Use name=value for each argument. Quote names that contain spaces.");
    let values = [];
    for i in [1..length(words) / 3]
      const offset = (i - 1) * 3;
      const name = tosym(words[offset + 1]);
      words[offset + 2] == "=" || raise(E_INVARG, "Use name=value for each argument.");
      maphaskey(schema, name) || raise(E_INVARG, "Unknown argument: " + tostr(name));
      !maphaskey(values, name) || raise(E_INVARG, "Repeated argument: " + tostr(name));
      values[name] = $builder_features:_author_value(words[offset + 3], scope, who, schema[name]);
    endfor
    $behavior_schema:_arguments(values, schema, scope, "arguments", 0);
    return values;
  endmethod

  method reaction_steps owner: HACKER
    "Supply the accepted step names and their command help from one catalog.";
    return [
      'announce -> "announce \"MESSAGE\" — show a message in the event's room",
      'tell -> "tell actor \"MESSAGE\" — send a private message to the participant",
      'emote -> "emote \"MESSAGE\" — describe what this object does",
      'set -> "set STATE VALUE — change a declared state value",
      'add -> "add COUNTER AMOUNT — increase a declared counter",
      'subtract -> "subtract COUNTER AMOUNT — decrease a declared counter",
      'schedule -> "schedule NAME SECONDS STEP — replace a timer with this name",
      'cancel -> "cancel NAME — cancel a timer on this object",
      'record -> "record in RELATION FIELD=VALUE ... — store a fact once",
      'forget -> "forget in RELATION FIELD=VALUE ... — remove a fact if present",
      'do -> "do ACTION target=OBJECT actor=ACTOR ... — request a normal world action"
    ];
  endmethod

  method _author_step owner: ARCH_WIZARD
    "Compile one readable step. State changes are restricted to the host's declared fields.";
    set_task_perms(caller_perms());
    const {words, scope, who, host, ?depth = 0, ?parameters = []} = args;
    depth <= 16 || raise(E_QUOTA, "Steps are nested too deeply.");
    length(words) > 0 || raise(E_INVARG, "Supply a step. Use @reaction steps for examples.");
    const kind = tosym(words[1]);
    maphaskey($builder_features:reaction_steps(), kind)
      || raise(E_INVARG, "Unknown step: " + words[1] + ". Use @reaction steps.");
    if (kind in {'announce, 'emote})
      length(words) == 2 || raise(E_INVARG, "Put the entire message in double quotes.");
      return $reaction:parse_effect({kind, $builder_features:_author_text(words[2])});
    elseif (kind == 'tell)
      (length(words) == 3 && words[2] == "actor") || raise(E_INVARG, "Use tell actor \"MESSAGE\".");
      return $reaction:parse_effect({'tell, 'Actor, $builder_features:_author_text(words[3])});
    elseif (kind in {'set, 'add, 'subtract})
      length(words) == 3 || raise(E_INVARG, "Supply a state name and a value.");
      const field = tosym(words[2]);
      maphaskey(scope['state], field) || raise(E_INVARG, "Declare " + words[2] + " with @state first.");
      const expected = scope['state][field];
      kind == 'set || expected in {'integer, 'float} || raise(E_INVARG, "Only a number can be increased or decreased.");
      const value = $builder_features:_author_value(words[3], $rule_engine:scope(), who, expected)['value];
      kind == 'set && $reaction:_check_state_value(value, host.reaction_state_fields[field]);
      return $reaction:parse_effect({kind == 'set ? 'set | (kind == 'add ? 'increment | 'decrement), field, value});
    elseif (kind == 'schedule)
      length(words) >= 5 || raise(E_INVARG, "Use schedule NAME SECONDS STEP.");
      const seconds = $builder_features:_author_value(words[3], $rule_engine:scope(), who, 'integer)['value];
      const name = tosym(words[2]);
      $behavior_schema:_name(name, "timer name");
      seconds >= 0 && seconds <= 86400 || raise(E_INVARG, "Timers must wait zero seconds through one day.");
      return <$reaction, .type = 'schedule, .name = name, .seconds = seconds,
        .effect = $builder_features:_author_step(words[4..$], scope, who, host, depth + 1, parameters)>;
    elseif (kind == 'cancel)
      length(words) == 2 || raise(E_INVARG, "Use cancel NAME.");
      return $reaction:parse_effect({'cancel, tosym(words[2])});
    endif
    if (kind in {'record, 'forget})
      (length(words) >= 3 && words[2] == "in")
        || raise(E_INVARG, "Use record in RELATION FIELD=VALUE, or forget in RELATION FIELD=VALUE.");
      const {reference, fields} = $builder_features:_author_relation(words[3], scope, who, parameters);
      return $reaction:parse_effect({kind, reference,
        $builder_features:_author_arguments(words[4..$], fields, scope, who), fields});
    endif
    length(words) >= 2 || raise(E_INVARG, "Use do ACTION name=value. See @reaction actions.");
    const schemas = $reaction:action_schemas();
    const action = tosym(words[2]);
    maphaskey(schemas, action) || raise(E_INVARG, "Unknown action: " + words[2]);
    const values = $builder_features:_author_arguments(words[3..$], schemas[action]['inputs], scope, who);
    return $reaction:parse_effect({'action, action, values});
  endmethod
  method _author_value_text owner: ARCH_WIZARD
    "Show a value or reference in the builder command vocabulary without executing object methods.";
    set_task_perms(caller_perms());
    const {expression} = args;
    if (expression['kind] == 'literal)
      const value = expression['value];
      if (typeof(value) == TYPE_OBJ)
        return valid(value) ? value.name + " (" + tostr(value) + ")" | tostr(value) + " (removed)";
      endif
      typeof(value) == TYPE_SYM && return tostr(value);
      return typeof(value) == TYPE_BOOL ? (value ? "yes" | "no") | toliteral(value);
    endif
    const name = expression['name];
    if (expression['kind] == 'input)
      const aliases = ['This -> "this", 'Actor -> "actor", 'Accessor -> "actor", 'Location -> "place",
        'Item -> "item", 'Dobj -> "item", 'Key -> "key", 'Food -> "food", 'Who -> "who", 'From -> "from", 'To -> "to",
        'Direction -> "exit", 'Connection -> "connection", 'Passage -> "passage"];
      return `aliases[name] ! E_RANGE => tostr(name)';
    endif
    return tostr(name);
  endmethod

  method _author_condition_text owner: ARCH_WIZARD
    "Describe the actual condition tree, preserving groups and selected names.";
    set_task_perms(caller_perms());
    const {tree} = args;
    const kind = tree['kind];
    if (kind in {'all, 'any})
      length(tree['children]) == 0 && return kind == 'all ? "always" | "never";
      return "(" + {$builder_features:_author_condition_text(child) for child in (tree['children])}
        :join(kind == 'all ? " AND " | " OR ") + ")";
    elseif (kind == 'not)
      return "NOT (" + $builder_features:_author_condition_text(tree['child]) + ")";
    elseif (kind == 'compare)
      const ops = ['eq -> "is", 'ne -> "is not", 'lt -> "<", 'le -> "<=", 'gt -> ">", 'ge -> ">="];
      return $builder_features:_author_value_text(tree['left]) + " " + ops[tree['op]] + " "
        + $builder_features:_author_value_text(tree['right]);
    endif
    if (kind == 'relation)
      let parts = {"fact in", $builder_features:_author_value_text(tree['relation])};
      for field in (mapkeys(tree['arguments]))
        parts = {@parts, tostr(field) + "=" + $builder_features:_author_value_text(tree['arguments][field])};
      endfor
      for field in (mapkeys(tree['bind]))
        parts = {@parts, tostr(field) + " as " + tostr(tree['bind][field])};
      endfor
      return parts:join(" ");
    endif
    const name = tree['name];
    const values = tree['arguments];
    const text = {field} => $builder_features:_author_value_text(values[field]);
    if (name == 'occupants && maphaskey(tree['bind], 'object))
      return "find occupant in " + text('location) + " as " + tostr(tree['bind]['object]);
    elseif (name in {'is_open, 'is_locked, 'is_portable})
      return text('object) + " is " + tostr(name)[4..$];
    elseif (name in {'location_is, 'nearby})
      return text('object) + " is in " + text('location);
    elseif (name == 'owner_is)
      return text('object) + " is owned by " + text('owner);
    elseif (name == 'isa)
      return text('object) + " is a " + text('prototype);
    endif
    let parts = {"fact", tostr(name)};
    for field in (mapkeys(values))
      parts = {@parts, tostr(field) + "=" + text(field)};
    endfor
    for field in (mapkeys(tree['bind]))
      parts = {@parts, "select " + tostr(field) + " as " + tostr(tree['bind][field])};
    endfor
    return parts:join(" ");
  endmethod

  method _author_step_text owner: ARCH_WIZARD
    "Describe every argument in an executable step. An optional environment resolves references for preview.";
    set_task_perms(caller_perms());
    const {raw, ?environment = 0} = args;
    const effect = typeof(raw) == TYPE_LIST ? $reaction:parse_effect(raw) | raw;
    const kind = effect.type;
    if (kind in {'announce, 'emote, 'tell})
      const message = typeof(effect.msg) == TYPE_SYM ? "message named " + tostr(effect.msg)
        | toliteral($sub_utils:decompile(effect.msg));
      return tostr(kind) + (kind == 'tell ? " " + tostr(effect.target) | "") + " " + message;
    elseif (kind in {'set, 'increment, 'decrement})
      const words = ['set -> "set", 'increment -> "add", 'decrement -> "subtract"];
      return words[kind] + " " + tostr(effect.prop) + " " + toliteral(kind == 'set ? effect.value | effect.by);
    elseif (kind == 'schedule)
      return "schedule " + tostr(effect.name) + " " + tostr(effect.seconds) + " "
        + $builder_features:_author_step_text(effect.effect, environment);
    elseif (kind == 'cancel)
      return "cancel " + tostr(effect.name);
    elseif (kind in {'action, 'record, 'forget})
      let parts;
      if (kind == 'action)
        parts = {"do", tostr(effect.action)};
      else
        const reference = typeof(environment) == TYPE_MAP
          ? ['kind -> 'literal, 'value -> $condition:resolve(effect.relation, environment)] | effect.relation;
        parts = {tostr(kind), "in", $builder_features:_author_value_text(reference)};
      endif
      for field in (mapkeys(effect.arguments))
        let expression = effect.arguments[field];
        if (typeof(environment) == TYPE_MAP)
          expression = ['kind -> 'literal, 'value -> $condition:resolve(expression, environment)];
        endif
        parts = {@parts, tostr(field) + "=" + $builder_features:_author_value_text(expression)};
      endfor
      return parts:join(" ");
    elseif (kind == 'move)
      return "move to " + tostr(effect.destination);
    elseif (kind == 'trigger)
      return "send event " + tostr(effect.event) + " to " + tostr(effect.target);
    endif
    raise(E_INVARG, "Unknown step: " + tostr(kind));
  endmethod

  method _author_trigger_text owner: HACKER
    "Describe an event or threshold with the command vocabulary.";
    const {trigger} = args;
    if (typeof(trigger) == TYPE_SYM)
      const name = tostr(trigger);
      return "after " + (name:starts_with("on_") ? name[4..$] | name);
    endif
    const ops = ['eq -> "=", 'ne -> "!=", 'lt -> "<", 'le -> "<=", 'gt -> ">", 'ge -> ">="];
    return "when " + tostr(trigger[2]) + " " + ops[trigger[3]] + " " + toliteral(trigger[4]);
  endmethod

  method reaction_help owner: ARCH_WIZARD
    "Build command help from the same event, step, action, and fact definitions used for validation.";
    set_task_perms(caller_perms());
    const {?topic = "help"} = args;
    if (topic == "events")
      let lines = {"These events come from core operations. Programmers can also send custom events with fire_trigger.",
        "Use after NAME for a custom event named on_NAME. Event names belong to the receiving object.",
        "Custom events supply actor and place; their producer defines when they happen."};
      for event in ($reaction.event_triggers)
        const scope = $rule_engine:trigger_scope(event);
        lines = {@lines, $builder_features:_author_trigger_text(event) + ": "
          + {$builder_features:_author_value_text(['kind -> 'input, 'name -> field]) for field in (mapkeys(scope['input]))}:join(", ")};
      endfor
      return lines;
    elseif (topic == "steps")
      return mapvalues($builder_features:reaction_steps());
    elseif (topic == "actions" || topic == "facts")
      const schemas = topic == "actions" ? $reaction:action_schemas() | $rule_engine:schemas();
      let lines = topic == "facts" ? {
        "Stored facts: fact in RELATION FIELD=VALUE. Use FIELD as NAME to select a value.",
        "RELATION is an object or a reaction setting. @facts OBJECT show lists its fields and facts.",
        "General world conditions:"} | {};
      for name in (mapkeys(schemas))
        const entry = schemas[name];
        const arguments = {tostr(field) + "=" + tostr(entry['inputs][field]) for field in (mapkeys(entry['inputs]))};
        lines = {@lines, (topic == "actions" ? "do " | "fact ") + tostr(name) + " " + arguments:join(" ")
          + " — " + entry['description]};
      endfor
      return lines;
    elseif (topic == "conditions")
      return {"A condition decides whether a reaction runs. Use always to remove a condition.",
        "Examples: actor is me; this is open; actor is in place; visits >= 4; mood is sleepy.",
        "AND requires both conditions. OR accepts either condition. NOT reverses a condition.",
        "Use parentheses for groups: (mood is awake OR visits >= 4) AND actor is me.",
        "this is the object with the reaction. actor is the participant. place is the event's room.",
        "State names come from @state. Settings come from with. Put text with spaces in double quotes.",
        "find occupant in place as visitor selects nearby objects. Add AND visitor is a $actor to select actors.",
        "Choose select each by visitor to run steps for each match, or select one to require exactly one match.",
        "Select once runs the steps once if anything matches, without exposing found objects to those steps.",
        "Select one requires exactly one match. Select each runs the steps for every distinct match.",
        "Selection orders object references by identity. It never relies on arrival order.",
        "Use fact in RELATION FIELD=VALUE to check stored facts. Use FIELD as NAME to select a value."};
    endif
    return {"A reaction makes an object respond to an event. You choose when it runs and the steps it takes.",
      "Example: @reaction cupboard welcome after open",
      "Then: @reaction cupboard welcome then announce \"The cupboard smells of cedar.\"",
      "Inspect: @reaction cupboard welcome show",
      "Preview: @reaction cupboard welcome preview actor=me",
      "Start: @reaction cupboard welcome enable",
      "New reactions start disabled. Their steps run in numbered order. Reactions for the same event run by name. A denied step stops later steps.",
      "Use @reaction OBJECT to list reactions. Quote an object name if it contains spaces.",
      "@reaction OBJECT NAME after EVENT — create a reaction or change its event",
      "@reaction OBJECT NAME when COUNTER >= NUMBER — respond when a value crosses a threshold",
      "@reaction OBJECT NAME if CONDITION — replace its condition",
      "@reaction OBJECT NAME then STEP — append a step",
      "@reaction OBJECT NAME step NUMBER STEP — replace a numbered step",
      "@reaction OBJECT NAME remove-step NUMBER — delete a step",
      "@reaction OBJECT NAME with SETTING TYPE VALUE — choose a reusable setting",
      "@reaction OBJECT NAME select once|one|each [by NAME ...] — choose how matches are used",
      "@reaction OBJECT NAME show|check|preview|enable|disable|remove",
      "@reaction OBJECT NAME trace on|off — record its latest run; trace shows that report",
      "@reaction OBJECT NAME copy to OBJECT [as NAME] — copy disabled, with fresh state",
      "@reaction events|conditions|steps|actions|facts — discover the available parts",
      "@state OBJECT NAME number 0 — declare a counter; use choice awake sleepy for named states",
      "@state OBJECT — show declared state; @state OBJECT NAME set VALUE changes it",
      "@rule OBJECT OPERATION CONDITION — control open, take, put, view, or another supported operation",
      "@facts OBJECT define FIELD:TYPE ... — define fields on this relation",
      "@facts OBJECT add FIELD=VALUE ... — record a fact; use remove to delete it, or show to read it",
      "Disabling, replacing, or removing a reaction cancels its timers. Earlier completed steps remain completed.",
      "A preview reads current facts and shows planned steps. It sends no messages and changes no state.",
      "Preview does not run action permission checks or extension conditions. Actual actions can still be denied."};
  endmethod

  method _author_inspect owner: ARCH_WIZARD
    "Show a complete installed definition and current declared values to its owner.";
    set_task_perms(caller_perms());
    const {host, name, reaction} = args;
    const property = name + "_reaction";
    const installed = `host.(property) ! E_PROPNF => 0';
    const active = reaction.enabled && installed != 0
      && $reaction:_authority(host, property, installed) != 0;
    const status = !reaction.enabled ? "disabled" | (active ? "enabled" | "inactive until you enable it on this object");
    let lines = {name + " on " + host.name + ": " + status,
      $builder_features:_author_trigger_text(reaction.trigger),
      "if " + (reaction.when == 0 ? "always" | $builder_features:_author_condition_text(reaction.when.condition))};
    const parameters = `reaction.parameters ! E_PROPNF => []';
    for field in (mapkeys(parameters))
      lines = {@lines, "Setting " + tostr(field) + ": " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> parameters[field]])};
    endfor
    const state = $reaction:state_values(host);
    for field in (mapkeys(state))
      lines = {@lines, "State " + tostr(field) + ": " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> state[field]])};
    endfor
    const selection = `reaction.selection ! E_PROPNF => ['mode -> 'one, 'order -> {}, 'limit -> 256]';
    lines = {@lines, "Matches: " + tostr(selection['mode]) + (length(selection['order]) > 0
      ? " by " + {tostr(ordered_name) for ordered_name in (selection['order])}:join(", ") | "")};
    for i in [1..length(reaction.effects)]
      lines = {@lines, "Step " + tostr(i) + ": " + $builder_features:_author_step_text(reaction.effects[i])};
    endfor
    const timers = $reaction:timers(host);
    if (length(timers) == 0)
      lines = {@lines, "Pending timers: none."};
    endif
    for timer in (mapkeys(timers))
      lines = {@lines, "Pending timer: " + tostr(timer)};
    endfor
    return lines;
  endmethod

  method _author_preview owner: ARCH_WIZARD
    "Plan effects using read-only canonical queries. Never invoke an action, message recipient, or extension predicate.";
    set_task_perms(caller_perms());
    const {host, reaction, context} = args;
    let lines = {"Preview only: nothing changes. This reaction is " + (reaction.enabled ? "enabled." | "disabled."),
      $builder_features:_author_trigger_text(reaction.trigger),
      "Condition: " + (reaction.when == 0 ? "always" | $builder_features:_author_condition_text(reaction.when.condition))};
    const initial_state = $reaction:state_values(host);
    for name in (mapkeys(initial_state))
      lines = {@lines, "Current " + tostr(name) + ": " + $builder_features:_author_value_text(
        ['kind -> 'literal, 'value -> initial_state[name]])};
    endfor
    for name in (mapkeys(reaction.parameters))
      lines = {@lines, "Setting " + tostr(name) + ": " + $builder_features:_author_value_text(
        ['kind -> 'literal, 'value -> reaction.parameters[name]])};
    endfor
    const rows = $reaction:_select(reaction, context, true);
    lines = {@lines, "Condition " + (length(rows) > 0 ? "matched." | "did not match. No steps would run.")};
    for row in (rows)
      let environment = ['input -> context, 'parameter -> reaction.parameters,
        'state -> $reaction:state_values(host), 'binding -> row];
      for field in (mapkeys(row))
        lines = {@lines, "Selected " + tostr(field) + ": " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> row[field]])};
      endfor
      for i in [1..length(reaction.effects)]
        const effect = reaction.effects[i];
        lines = {@lines, "Would run step " + tostr(i) + ": " + $builder_features:_author_step_text(effect, environment)};
        lines = {@lines, @$reaction:preview_message(effect, host)};
        if (effect.type in {'set, 'increment, 'decrement})
          const old = environment['state][effect.prop];
          let amount = effect.type == 'set ? effect.value | effect.by;
          typeof(old) == TYPE_FLOAT && typeof(amount) == TYPE_INT && (amount = tofloat(amount));
          environment['state][effect.prop] = effect.type == 'set ? amount
            | (effect.type == 'increment ? old + amount | old - amount);
        endif
      endfor
    endfor
    return {@lines, "State changes can start other reactions; this preview does not expand those later reactions.",
      "Actions and timer rechecks can fail when the reaction actually runs. No actions were attempted."};
  endmethod
  verb "@reaction @reactions @state @rule @facts" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: help -- Author reactions, state, and access rules without programming privileges.";
    caller == player || raise(E_PERM);
    this:_require_builder_command(player);
    set_task_perms(player);
    try
      const lines = $builder_features:author(tosym(verb == "@reactions" ? "reaction" | verb[2..$]), argstr, player);
      player:inform_current($event:mk_info(player, @$format:literal_inline(lines:join("\n"))));
    except error (ANY)
      player:inform_current($event:mk_error(player, error[2]));
    endtry
  endverb

  method author owner: ARCH_WIZARD
    "Shared command and tool service. Authenticate the builder before interpreting any request.";
    const {command, text, who} = args;
    const principal = caller_perms();
    (principal == who || principal.wizard) || raise(E_PERM);
    who.is_builder || raise(E_PERM, "Builder privileges are required. Programmer privileges are not.");
    set_task_perms(who);
    const words = $builder_features:_author_words(text);
    if (length(words) == 0 || words[1] in {"help", "events", "steps", "actions", "facts", "conditions"})
      return $builder_features:reaction_help(length(words) == 0 ? "help" | words[1]);
    endif
    const host = $builder_features:_author_object(words[1], who);
    (host.owner == who || who.wizard) || raise(E_PERM, "You do not own " + host.name + ".");
    if (command == 'state)
      return $builder_features:_author_state(host, words[2..$], who);
    elseif (command == 'rule)
      return $builder_features:_author_rule(host, words[2..$], who);
    elseif (command == 'facts)
      return $builder_features:_author_facts(host, words[2..$], who);
    endif
    command == 'reaction || raise(E_INVARG, "Unknown authoring command.");
    if (length(words) == 1)
      const reactions = $obj_utils:reaction_properties(host, who, true);
      let lines = {"Reactions on " + host.name + ":"};
      for entry in (reactions)
        lines = {@lines, tostr(entry[1])[1..$ - 9] + " — "
          + $builder_features:_author_trigger_text(entry[2].trigger) + " — "
          + (entry[2].enabled ? "enabled" | "disabled")};
      endfor
      return length(reactions) > 0 ? lines | {"No reactions. Use @reaction OBJECT NAME after EVENT to create one."};
    endif
    const name = tosym(words[2]);
    $behavior_schema:_name(name, "reaction name");
    const property = tostr(name) + "_reaction";
    const operation = length(words) < 3 ? "show" | words[3];
    const rest = length(words) > 3 ? words[4..$] | {};
    const previous = `host.(property) ! E_PROPNF => 0';
    let reaction;
    if (previous == 0)
      operation in {"after", "when"} || raise(E_INVARG, "No reaction named " + tostr(name) + ". Start with after EVENT.");
      reaction = $reaction:mk('on_open, 0, {}, $reaction:state_types(host));
      reaction.enabled = false;
      reaction.parameters = [];
      reaction.parameter_types = [];
      reaction.selection = ['mode -> 'once, 'order -> {}, 'limit -> 256];
      reaction.trace = false;
    else
      reaction = $reaction:_definition(previous, false);
      reaction.state_types = $reaction:state_types(host);
    endif
    let scope = $reaction:author_scope(reaction);
    if (operation == "show")
      length(rest) == 0 || raise(E_INVARG, "Show takes no extra arguments.");
      return $builder_features:_author_inspect(host, tostr(name), reaction);
    elseif (operation == "check")
      length(rest) == 0 || raise(E_INVARG, "Check takes no extra arguments.");
      $reaction:_definition(reaction);
      return {"Valid: " + tostr(name) + ". Action permissions are checked when each step runs."};
    elseif (operation == "preview-message")
      length(rest) > 0 || raise(E_INVARG, "Use preview-message announce, emote, or tell actor followed by a quoted message.");
      const effect = $builder_features:_author_step(rest, scope, who, host, 0, reaction.parameters);
      effect.type in {'announce, 'emote, 'tell} || raise(E_INVARG, "Choose announce, emote, or tell actor.");
      return $reaction:preview_message(effect, host);
    elseif (operation == "preview")
      const supplied = length(rest) == 0 ? []
        | $builder_features:_author_preview_inputs(rest, scope, who);
      const location = isa(host, $room) ? host | host.location;
      const context = mapmerge(['This -> host, 'Actor -> who, 'Location -> location], supplied);
      return $builder_features:_author_preview(host, reaction, context);
    elseif (operation == "trace")
      if (length(rest) == 0)
        const report = $reaction:report(host, property);
        return length(report) > 0 ? report | {"No recorded run. Use trace on, then cause the event."};
      endif
      (length(rest) == 1 && rest[1] in {"on", "off"}) || raise(E_INVARG, "Use trace on, trace off, or trace.");
      reaction.trace = rest[1] == "on";
    elseif (operation == "after")
      length(rest) == 1 || raise(E_INVARG, "Use after EVENT. @reaction events lists the events.");
      const event = tosym("on_" + rest[1]);
      $behavior_schema:_name(tosym(rest[1]), "event name");
      reaction.trigger = event;
    elseif (operation == "when")
      length(rest) == 3 || raise(E_INVARG, "Use when COUNTER >= NUMBER.");
      const field = tosym(rest[1]);
      maphaskey(scope['state], field) || raise(E_INVARG, "Declare the counter with @state first.");
      const ops = ["=" -> 'eq, "==" -> 'eq, "!=" -> 'ne, ">=" -> 'ge, "<=" -> 'le, ">" -> 'gt, "<" -> 'lt];
      maphaskey(ops, rest[2]) || raise(E_INVARG, "Use =, !=, <, <=, >, or >=.");
      const value = $builder_features:_author_value(rest[3], $rule_engine:scope(), who, scope['state][field])['value];
      reaction.trigger = {'when, field, ops[rest[2]], value};
    elseif (operation == "if")
      scope['binding] = [];
      const tree = $builder_features:_author_condition(rest, scope, who, 0, reaction.parameters);
      reaction.when = $rule:mk(name, tree, scope, reaction.parameters);
    elseif (operation == "then" || operation == "step")
      length(rest) > 0 || raise(E_INVARG, "Supply a step. Use @reaction steps for examples.");
      const index = operation == "then" ? length(reaction.effects) + 1
        | $builder_features:_author_value(rest[1], $rule_engine:scope(), who, 'integer)['value];
      index > 0 && index <= length(reaction.effects) + (operation == "then" ? 1 | 0)
        || raise(E_INVARG, "That step number does not exist. Use show to see the step numbers.");
      const effect_words = operation == "then" ? rest | rest[2..$];
      const effect = $builder_features:_author_step(effect_words, scope, who, host, 0, reaction.parameters);
      reaction.effects = operation == "then" ? {@reaction.effects, effect}
        | {@reaction.effects[1..index - 1], effect, @reaction.effects[index + 1..$]};
    elseif (operation == "remove-step")
      length(rest) == 1 || raise(E_INVARG, "Use remove-step NUMBER.");
      const index = $builder_features:_author_value(rest[1], $rule_engine:scope(), who, 'integer)['value];
      index > 0 && index <= length(reaction.effects) || raise(E_INVARG, "That step number does not exist.");
      reaction.effects = listdelete(reaction.effects, index);
    elseif (operation == "with")
      length(rest) == 3 || raise(E_INVARG, "Use with NAME TYPE VALUE, such as with cat object Henri.");
      const setting = tosym(rest[1]);
      $behavior_schema:_name(setting, "setting name");
      !(rest[1] in {"this", "actor", "place", "item", "key", "food", "who", "me", "here", "from", "to"})
        && !maphaskey(scope['state], setting) || raise(E_INVARG, "That name already identifies an event input or state.");
      const kind = $builder_features:_author_type(rest[2]);
      const value = $builder_features:_author_value(rest[3], $rule_engine:scope(), who, kind)['value];
      if (maphaskey(reaction.parameter_types, setting))
        reaction.parameter_types[setting] == kind || raise(E_INVARG, "An existing setting must keep its type.");
      endif
      reaction.parameter_types[setting] = kind;
      reaction.parameters[setting] = value;
    elseif (operation == "select")
      length(rest) >= 1 && rest[1] in {"once", "one", "each"} || raise(E_INVARG, "Choose select once, select one, or select each by NAME.");
      length(rest) == 1 || (length(rest) >= 3 && rest[2] == "by") || raise(E_INVARG, "Use by NAME to order matches.");
      let ordering = {};
      if (length(rest) > 1)
        for match_name in (rest[3..$])
          ordering = {@ordering, tosym(match_name)};
        endfor
      endif
      reaction.selection = ['mode -> tosym(rest[1]), 'order -> ordering, 'limit -> 256];
    elseif (operation in {"enable", "disable"})
      length(rest) == 0 || raise(E_INVARG, "Enable and disable take no extra arguments.");
      operation == "enable" && length(reaction.effects) == 0 && raise(E_INVARG, "Add at least one step before enabling the reaction.");
      reaction.enabled = operation == "enable";
    elseif (operation == "remove")
      length(rest) == 0 || raise(E_INVARG, "Remove takes no extra arguments.");
      $reaction:remove(host, property);
      return {"Removed " + tostr(name) + ". Its pending timers are canceled."};
    elseif (operation == "copy")
      (length(rest) == 2 || (length(rest) == 4 && rest[3] == "as")) && rest[1] == "to"
        || raise(E_INVARG, "Use copy to OBJECT, or copy to OBJECT as NAME.");
      const destination = $builder_features:_author_object(rest[2], who);
      (destination.owner == who || who.wizard) || raise(E_PERM, "You do not own the destination.");
      const copy_name = length(rest) == 4 ? tosym(rest[4]) | name;
      $behavior_schema:_name(copy_name, "reaction name");
      const copy_property = tostr(copy_name) + "_reaction";
      `destination.(copy_property) ! E_PROPNF => 0' == 0 || raise(E_INVARG, "That reaction name is already in use on the destination.");
      for field in (mapkeys(reaction.state_types))
        if (maphaskey(destination.reaction_state_fields, field))
          equal(destination.reaction_state_fields[field], host.reaction_state_fields[field])
            || raise(E_INVARG, "The destination declares different choices or initial state for " + tostr(field) + ".");
        else
          $obj_utils:property_key(destination, tostr(field)) == E_PROPNF
            || raise(E_INVARG, "The destination already uses the state name " + tostr(field) + ".");
        endif
      endfor
      for field in (mapkeys(reaction.state_types))
        if (!maphaskey(destination.reaction_state_fields, field))
          const definition = host.reaction_state_fields[field];
          $reaction:declare_state(destination, field, definition['type], definition['initial], definition['choices]);
        endif
      endfor
      reaction.enabled = false;
      $reaction:install(destination, copy_property, reaction);
      return {"Copied " + tostr(copy_name) + " to " + destination.name + ", disabled. Review its settings with show, then enable it."};
    else
      raise(E_INVARG, "Unknown reaction command: " + operation + ". Use @reaction help.");
    endif
    if (reaction.when != 0)
      let condition = reaction.when;
      condition.scope = $rule_engine:trigger_scope(reaction.trigger, reaction.state_types);
      condition.scope['parameter] = reaction.parameter_types;
      reaction.when = condition;
    endif
    $reaction:install(host, property, reaction);
    return {"Saved " + tostr(name) + " on " + host.name + " (" + (reaction.enabled ? "enabled" | "disabled") + ")."};
  endmethod

  method _author_preview_inputs owner: ARCH_WIZARD
    "Read optional simulated event inputs by their builder names. Do not permit overriding this.";
    set_task_perms(caller_perms());
    const {words, scope, who} = args;
    length(words) % 3 == 0 || raise(E_INVARG, "Use actor=NAME, item=NAME, or key=NAME for preview inputs.");
    let values = [];
    for i in [1..length(words) / 3]
      const offset = (i - 1) * 3;
      const reference = $builder_features:_author_value(words[offset + 1], scope, who);
      reference['kind] == 'input && reference['name] != 'This || raise(E_INVARG, "That is not an event input.");
      words[offset + 2] == "=" || raise(E_INVARG, "Use name=value for each preview input.");
      const field = reference['name];
      !maphaskey(values, field) || raise(E_INVARG, "Repeated preview input.");
      values[field] = $builder_features:_author_value(words[offset + 3], $rule_engine:scope(), who, scope['input][field])['value];
    endfor
    return values;
  endmethod

  method _author_state owner: ARCH_WIZARD
    "Declare or inspect state on an owned host, or apply a typed change that emits thresholds.";
    set_task_perms(caller_perms());
    const {host, words, who} = args;
    if (length(words) == 0)
      const values = $reaction:state_values(host);
      return length(values) == 0 ? {"No declared state. Use @state OBJECT NAME number 0."}
        | {tostr(field) + ": " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> values[field]])
          + (length(host.reaction_state_fields[field]['choices]) > 0
            ? " (choices: " + host.reaction_state_fields[field]['choices]:join(", ") + ")" | "") for field in (mapkeys(values))};
    endif
    length(words) >= 3 || raise(E_INVARG, "Use @state OBJECT NAME TYPE VALUE, or @state OBJECT NAME set VALUE.");
    const name = tosym(words[1]);
    $behavior_schema:_name(name, "state name");
    !(words[1] in {"this", "actor", "place", "item", "key", "food", "who", "me", "here", "from", "to"})
      || raise(E_INVARG, "That name already identifies an event input.");
    if (words[2] == "set")
      length(words) == 3 || raise(E_INVARG, "Use set VALUE. Quote text that contains spaces.");
      maphaskey(host.reaction_state_fields, name) || raise(E_INVARG, "Declare the state first.");
      const kind = host.reaction_state_fields[name]['type];
      const value = $builder_features:_author_value(words[3], $rule_engine:scope(), who, kind)['value];
      $reaction:change(host, name, 'set, value, ['Actor -> who]);
      return {"Set " + words[1] + " to " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> value]) + "."};
    endif
    !maphaskey(host.reaction_state_fields, name) || raise(E_INVARG, "That state is already declared. Use set to change its value.");
    for entry in ($obj_utils:reaction_properties(host, who, true))
      const settings = `entry[2].parameters ! E_PROPNF => []';
      !maphaskey(settings, name) || raise(E_INVARG, "A reaction already uses that name for a setting.");
    endfor
    const kind = $builder_features:_author_type(words[2]);
    words[2] == "choice" || length(words) == 3 || raise(E_INVARG, "Supply one initial value, or use choice for several choices.");
    let choices = {};
    if (words[2] == "choice")
      for option in (words[3..$])
        choices = {@choices, $builder_features:_author_text(option)};
      endfor
    endif
    const value = $builder_features:_author_value(words[3], $rule_engine:scope(), who, kind)['value];
    $reaction:declare_state(host, name, kind, value, choices);
    return {"Declared " + words[1] + ". Its initial value is " + $builder_features:_author_value_text(['kind -> 'literal, 'value -> value]) + "."};
  endmethod

  method _author_rule owner: ARCH_WIZARD
    "Set an existing operation policy using readable conditions and the operation's actual input schema.";
    set_task_perms(caller_perms());
    const {host, words, who} = args;
    length(words) >= 2 || raise(E_INVARG, "Use @rule OBJECT OPERATION CONDITION, show, or clear.");
    const property = words[1] + "_rule";
    const scope = $rule_engine:policy_scope(property);
    length(scope['input]) > 0 && $obj_utils:property_key(host, property) != E_PROPNF
      || raise(E_INVARG, "That object has no rule for " + words[1] + ".");
    if (length(words) == 2 && words[2] == "show")
      const rule = host.(property);
      return {words[1] + ": " + (rule == 0 ? "default access" | $builder_features:_author_condition_text(rule.condition))};
    elseif (length(words) == 2 && words[2] == "clear")
      host.(property) = 0;
      return {"Restored default access for " + words[1] + "."};
    endif
    const tree = $builder_features:_author_condition(words[2..$], scope, who);
    host.(property) = $rule:mk(tosym(property), tree, scope);
    return {"Set " + words[1] + " rule: " + $builder_features:_author_condition_text(tree)};
  endmethod
  method _author_facts owner: ARCH_WIZARD
    "Define fields on one relation and edit its facts without registering a global name.";
    set_task_perms(caller_perms());
    const {host, words, who} = args;
    isa(host, $relation) || raise(E_INVARG, "Use @create $relation named NAME to create a place for these facts.");
    if (length(words) > 0 && words[1] == "define")
      (length(words) >= 4 && (length(words) - 1) % 3 == 0)
        || raise(E_INVARG, "Use @facts OBJECT define person:object group:text.");
      let columns = {};
      let types = [];
      for i in [1..(length(words) - 1) / 3]
        const offset = (i - 1) * 3 + 1;
        const field = tosym(words[offset + 1]);
        words[offset + 2] == ":" || raise(E_INVARG, "Use field:type for each column.");
        $behavior_schema:_name(field, "fact field");
        !(field in columns) || raise(E_INVARG, "A field name can appear only once.");
        columns = {@columns, field};
        types[field] = $builder_features:_author_type(words[offset + 3]);
      endfor
      $relation:define_fields(host, columns, types);
      return {"Defined fields on " + host.name + ". Use @facts " + tostr(host) + " add FIELD=VALUE to record a fact."};
    endif
    const schema = $relation:schema(host);
    if (length(words) == 0 || words == {"show"})
      const definitions = {tostr(field) + ":" + tostr(schema['types][field]) for field in (schema['columns])};
      let lines = {"Facts in " + host.name + " (" + tostr(host) + ")", "Fields: " + definitions:join(", ")};
      const tuples = $relation:tuples(host);
      for tuple in (tuples)
        $relation:_check_tuple(tuple, schema);
        let fields = {};
        for i in [1..length(schema['columns])]
          fields = {@fields, tostr(schema['columns][i]) + "="
            + $builder_features:_author_value_text(['kind -> 'literal, 'value -> tuple[i]])};
        endfor
        lines = {@lines, fields:join(", ")};
      endfor
      return length(tuples) == 0 ? {@lines, "No facts yet."} | lines;
    endif
    words[1] in {"add", "remove"} || raise(E_INVARG, "Use @facts OBJECT add FIELD=VALUE, remove FIELD=VALUE, or show.");
    const expressions = $builder_features:_author_arguments(words[2..$], schema['types], $rule_engine:scope(), who);
    let values = [];
    for field in (mapkeys(expressions))
      values[field] = expressions[field]['value];
    endfor
    $relation:change_fact(host, schema['types], values, words[1] == "add" ? 'record | 'forget);
    return {words[1] == "add" ? "Recorded fact. Repeating it does not create a duplicate." | "Removed fact if present."};
  endmethod
endobject
