object ROOM [
  import_export_id -> "room",
  import_export_hierarchy -> {"world"}
]
  name: "Generic Room"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property acoustic_debug_counts (owner: HACKER, flags: "rc") = ["suppressed_not_loud" -> 1];
  property acoustic_debug_enabled (owner: HACKER, flags: "rc") = 0;
  property acoustic_debug_log (owner: HACKER, flags: "rc") = {};
  property acoustic_neighbors (owner: HACKER, flags: "rc") = [];
  property engagements (owner: HACKER, flags: "rc") = [];

  override description (owner: HACKER, flags: "rc") = "Parent prototype for all rooms in the system, defining room behavior and event broadcasting.";

  method initialize owner: ARCH_WIZARD
    "Set sensible defaults for newly created rooms.";
    pass();
    set_task_perms(caller_perms());
    this.description = "An empty room awaiting a description.";
    set_object_metadata(this, "import_export_hierarchy", {"rooms"});
  endmethod

  verb emote (any any any) owner: HACKER flags: "rxd"
    this:announce(player:mk_emote_event(argstr));
  endverb

  verb say (any any any) owner: HACKER flags: "rxd"
    this:announce(player:mk_say_event(argstr));
  endverb

  verb "`* to" (any any any) owner: HACKER flags: "rd"
    "Directed say - say something to a specific person";
    "Usage: `target message  OR  to target message";
    if (verb[1] == "`")
      "Backtick syntax: target is in verb[2..]";
      if (length(verb) < 2)
        player:inform_current($event:mk_error(player, "Usage: `target message"));
        return;
      endif
      target_name = verb[2..length(verb)];
      message = argstr;
    else
      "to syntax: first word of argstr is target, rest is message";
      if (!argstr)
        player:inform_current($event:mk_error(player, "Usage: to <target> <message>"));
        return;
      endif
      space_idx = index(argstr, " ");
      if (space_idx > 0)
        target_name = argstr[1..space_idx - 1];
        message = argstr[space_idx + 1..$]:trim();
      else
        target_name = argstr;
        message = "";
      endif
    endif
    if (!message)
      player:inform_current($event:mk_error(player, "Say what to ", target_name, "?"));
      return;
    endif
    try
      target = $match:match_object(target_name, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "I don't see '", target_name, "' here."));
      return;
    endtry
    if (!valid(target) || typeof(target) != TYPE_OBJ)
      player:inform_current($event:mk_error(player, "I don't see '", target_name, "' here."));
      return;
    endif
    "Build the event with source/dest annotations";
    event = player:mk_stagetalk(target, message);
    event = event:with_metadata('source, player);
    event = event:with_metadata('target, target);
    event = event:with_metadata('message, message);
    "Announce to room";
    this:announce(event);
    "Notify target if it implements on_addressed protocol - fork so speech delivers first";
    if (respond_to(target, 'on_addressed))
      fork (0)
        `target:on_addressed(player, message, event) ! ANY';
      endfork
    endif
  endverb

  method confunc owner: HACKER
    "Called when a player connects in this room.";
    "Args: who, ?is_new_player, ?should_announce";
    {who, ?is_new_player = false, ?should_announce = true} = args;
    "Only announce if should_announce is true (not a quick reconnect)";
    if (should_announce)
      arrival_event = who:mk_connected_event(is_new_player):with_audience('utility);
      this:announce(arrival_event);
    endif
    "Always show the room to the connecting player";
    `who:emit_room_look(this) ! ANY';
  endmethod

  method disfunc owner: HACKER
    "Called when a player disconnects from this room.";
    "By the time this runs, all connections are gone (daemon handles soft detach).";
    discon_event = player:mk_disconnected_event():with_audience('utility);
    this:announce(discon_event);
  endmethod

  method enterfunc owner: HACKER
    "Show room description to arriving players and refresh room snapshot state.";
    {who} = args;
    valid(who) || return;
    "Defer expensive room presentation and fanout until after the move commits.";
    fork (0)
      if (is_player(who))
        `who:emit_room_look(this) ! ANY => 0';
        for thing in (this.contents)
          suspend_if_needed();
          if (thing != who)
            `thing:on_location_enter(who) ! ANY => 0';
          endif
        endfor
      endif
      for viewer in (this.contents)
        suspend_if_needed();
        if (!is_player(viewer))
          continue;
        endif
        `viewer:emit_room_snapshot_state(viewer, this) ! ANY => 0';
      endfor
    endfork
    `pass(@args) ! E_TYPE, E_VERBNF => 0';
  endmethod

  method exitfunc owner: HACKER
    "Notify room contents of departure, then refresh room snapshot state.";
    {who} = args;
    "Defer expensive room fanout until after the move commits.";
    fork (0)
      for item in (this.contents)
        suspend_if_needed();
        if (respond_to(item, 'on_location_exit))
          `item:on_location_exit(who) ! ANY => 0';
        endif
      endfor
      for viewer in (this.contents)
        suspend_if_needed();
        if (!is_player(viewer))
          continue;
        endif
        `viewer:emit_room_snapshot_state(viewer, this) ! ANY => 0';
      endfor
    endfork
    `pass(@args) ! E_TYPE, E_VERBNF => 0';
  endmethod

  method acceptable owner: HACKER
    "TODO: support locking/unlocking etc";
    return true;
  endmethod

  method match_scope_for owner: HACKER
    "Expose the room and visible occupants to the command scope.";
    {actor, ?context = []} = args;
    "In case we have a parent that wants to establish initial occupancy, ask it first...";
    entries = `pass(@args) ! E_TYPE, E_VERBNF => {}';
    "Add every visible occupant so they can be matched as direct objects.";
    "Contents come BEFORE the room itself so items match before room name.";
    visible = this:contents();
    for visible_obj in (visible)
      if (!valid(visible_obj))
        continue;
      endif
      entries = {@entries, visible_obj};
    endfor
    "Add the room itself last.";
    entries = {@entries, this};
    return entries;
  endmethod

  method maybe_handle_passage owner: ARCH_WIZARD
    "Let our location (area) potentially handle passage commands...";
    const {parsed} = args;
    const actor = caller_perms();
    actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(actor);
    if (!valid(this.location) || !respond_to(this.location, 'handle_passage_command))
      return false;
    endif
    return this.location:handle_passage_command(parsed);
  endmethod

  method maybe_handle_command owner: ARCH_WIZARD
    "Handle any potential commands that the command matcher didn't already handle on the player, for example for furniture or exits";
    const {pc} = args;
    const actor = caller_perms();
    actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(actor);
    return this:maybe_handle_passage(pc);
  endmethod

  method announce owner: HACKER
    {event} = args;
    "Local announcement";
    for who in (this:contents())
      suspend_if_needed();
      `who:tell(event) ! E_VERBNF';
    endfor
    "Check for acoustic propagation";
    if (typeof(event) != TYPE_FLYWEIGHT)
      return;
    endif
    slots = flyslots(event);
    raw_loudness = maphaskey(slots, 'loudness) ? slots['loudness] | 0;
    if (typeof(raw_loudness) == TYPE_INT || typeof(raw_loudness) == TYPE_FLOAT)
      loudness = raw_loudness;
    else
      loudness = 0;
      `this:_acoustic_debug_bump("bad_loudness_type") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
    endif
    raw_propagates = maphaskey(slots, 'propagates) ? slots['propagates] | false;
    if (typeof(raw_propagates) == TYPE_INT || typeof(raw_propagates) == TYPE_FLOAT)
      propagates = raw_propagates != 0;
    else
      propagates = raw_propagates ? true | false;
    endif
    origin_room = maphaskey(slots, 'origin_room) ? slots['origin_room] | this;
    if (!valid(origin_room))
      origin_room = this;
    endif
    source_room = maphaskey(slots, 'source_room) ? slots['source_room] | $nothing;
    depth_raw = maphaskey(slots, 'acoustic_depth) ? slots['acoustic_depth] | 0;
    max_depth_raw = maphaskey(slots, 'acoustic_max_depth) ? slots['acoustic_max_depth] | 3;
    depth = typeof(depth_raw) == TYPE_INT || typeof(depth_raw) == TYPE_FLOAT ? depth_raw | 0;
    max_depth = typeof(max_depth_raw) == TYPE_INT || typeof(max_depth_raw) == TYPE_FLOAT ? max_depth_raw | 3;
    if (max_depth < 0)
      max_depth = 0;
    endif
    if (depth >= max_depth)
      entry = ["reason" -> "depth_limit", "depth" -> depth, "max_depth" -> max_depth, "when" -> time()];
      `this:_acoustic_debug_bump("suppressed_depth_limit", entry) ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
      return;
    endif
    if (loudness == 0 && !propagates)
      `this:_acoustic_debug_bump("suppressed_not_loud") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
      return;
    endif
    "Propagate to acoustic neighbors";
    neighbors = this.acoustic_neighbors;
    if (typeof(neighbors) != TYPE_LIST || length(neighbors) == 0)
      `this:_acoustic_debug_bump("no_neighbors") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
      return;
    endif
    for neighbor_spec in (neighbors)
      if (typeof(neighbor_spec) != TYPE_MAP)
        `this:_acoustic_debug_bump("bad_neighbor_spec") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
        continue;
      endif
      room = `neighbor_spec['room] ! E_RANGE => $nothing';
      if (!valid(room))
        `this:_acoustic_debug_bump("invalid_neighbor_room") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
        continue;
      endif
      if (room == source_room || room == origin_room)
        `this:_acoustic_debug_bump("suppressed_loop_guard") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
        continue;
      endif
      "Check loudness threshold if specified";
      raw_threshold = `neighbor_spec['threshold] ! E_RANGE => 1';
      if (typeof(raw_threshold) == TYPE_INT || typeof(raw_threshold) == TYPE_FLOAT)
        threshold = raw_threshold;
      else
        threshold = 1;
        `this:_acoustic_debug_bump("bad_threshold_type") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
      endif
      if (loudness < threshold && !propagates)
        `this:_acoustic_debug_bump("suppressed_threshold") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
        continue;
      endif
      "Transform the event";
      transformer = `neighbor_spec['transformer] ! E_RANGE => ""';
      if (typeof(transformer) == TYPE_STR && length(transformer) > 0)
        "Custom transformer verb on this room";
        transformed = `this:(transformer)(event, room) ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND => 0';
        if (typeof(transformed) != TYPE_FLYWEIGHT)
          entry = ["reason" -> "transform_failed", "transformer" -> transformer, "target_room" -> room, "when" -> time()];
          `this:_acoustic_debug_bump("transform_failed", entry) ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
        endif
      else
        "Default transformation: use prefix";
        prefix = `neighbor_spec['prefix] ! E_RANGE => "From nearby"';
        transformed = this:_default_distant_transform(event, prefix);
      endif
      if (typeof(transformed) != TYPE_FLYWEIGHT)
        continue;
      endif
      transformed = transformed:with_metadata('source_room, this):with_metadata('origin_room, origin_room):with_metadata('acoustic_depth, depth + 1):with_metadata('acoustic_max_depth, max_depth);
      `room:announce_distant(transformed, this) ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
      `this:_acoustic_debug_bump("propagated") ! E_VERBNF, E_ARGS, E_TYPE, E_INVARG, E_INVIND, E_PERM';
    endfor
  endmethod

  method look_self owner: HACKER
    "Override look_self to include exit information and actions";
    "Get base look from parent";
    look_data = pass(@args);
    "Add exits if we're in an area with passages";
    area = this.location;
    exits = {};
    ambient_passages = {};
    if (valid(area) && respond_to(area, 'get_exit_info))
      {exits, ambient_passages} = area:get_exit_info(this);
    endif
    "Collect actions from objects in the room";
    actions = {};
    for item in (this.contents)
      suspend_if_needed();
      item_actions = `item:available_actions() ! E_VERBNF => {}';
      if (typeof(item_actions) == TYPE_LIST && length(item_actions) > 0)
        actions = {@actions, @item_actions};
      endif
    endfor
    "Build updated flyweight if we have exits, passages, or actions";
    if (length(exits) > 0)
      look_data.exits = exits;
    endif
    if (length(ambient_passages) > 0)
      look_data.ambient_passages = ambient_passages;
    endif
    if (length(actions) > 0)
      look_data.actions = actions;
    endif
    return look_data;
  endmethod

  method check_can_dig_from owner: ARCH_WIZARD
    "Check if caller can dig passages from this room. Wizard, owner, or 'dig_from capability.";
    actor = caller_perms();
    set_task_perms(actor);
    {this, perms} = this:check_permissions_as(actor, 'dig_from);
    return true;
  endmethod

  method check_can_dig_into owner: ARCH_WIZARD
    "Check if caller can dig passages into this room. Wizard, owner, or 'dig_into capability.";
    actor = caller_perms();
    set_task_perms(actor);
    {this, perms} = this:check_permissions_as(actor, 'dig_into);
    return true;
  endmethod

  verb "exits ways" (none none none) owner: ARCH_WIZARD flags: "rd"
    "List the ways out of this room.";
    "Note: removed set_task_perms(caller_perms()) - it breaks respond_to when caller_perms is #-1";
    "Get exits from our area";
    area = this.location;
    if (!valid(area) || !respond_to(area, 'get_exit_info))
      player:inform_current($event:mk_error(player, "You don't see any obvious ways out."):with_audience('utility));
      return;
    endif
    {exit_lines, ambient_passages} = area:get_exit_info(this);
    "Collect all exit directions - both regular exits and ambient passage labels";
    all_exits = {@exit_lines};
    for ap in (ambient_passages)
      "ambient_passages format: {description, prose_style, label}";
      if (typeof(ap) == TYPE_LIST && length(ap) >= 3)
        label = ap[3];
        if (label && typeof(label) == TYPE_STR)
          all_exits = {@all_exits, label};
        endif
      endif
    endfor
    "Normalize, dedupe, and sort for readability.";
    normalized = {};
    for candidate_dir in (all_exits)
      if (typeof(candidate_dir) != TYPE_STR)
        continue;
      endif
      if (length(candidate_dir) == 0)
        continue;
      endif
      if (!(candidate_dir in normalized))
        normalized = {@normalized, candidate_dir};
      endif
    endfor
    all_exits = `sort(normalized) ! ANY => normalized';
    if (length(all_exits) == 0)
      player:inform_current($event:mk_error(player, "You don't see any obvious ways out."):with_audience('utility));
      return;
    endif
    "Format exits with their room and passage identity.";
    exit_links = { $format.annotation:exit(this, listed_dir) for listed_dir in (all_exits) };
    exit_list = $format.list:mk(exit_links);
    exit_title = $format.title:mk("Ways out");
    content = $format.block:mk(exit_title, exit_list);
    event = $event:mk_info(player, content):with_audience('utility):with_metadata('preferred_content_types, {'text_html, 'text_plain}):with_presentation_hint('inset):with_group('exits, this);
    player:inform_current(event);
  endverb

  method exit_annotation owner: HACKER
    "Describe a registered passage and its ordinary command invocation.";
    const {direction} = args;
    const area = this.location;
    valid(area) && isa(area, $area) || return [];
    const passage = area:find_passage_by_direction(this, direction);
    typeof(passage) in {TYPE_OBJ, TYPE_FLYWEIGHT} || return [];
    const destination = passage:other_room(this);
    valid(destination) || return [];
    const identity = area:passage_link_id(this, destination);
    identity != "" || return [];
    return ["kind" -> "command", "command" -> "go " + direction, "action" -> $player:command_action("go", direction), "exit" -> ["source" -> $url_utils:to_curie_str(this), "destination" -> $url_utils:to_curie_str(destination), "passage" -> identity]];
  endmethod

  method action_go owner: ARCH_WIZARD
    "Action handler: make actor go in a direction from this room.";
    {who, context, direction} = args;
    who.location != this && return false;
    "Get the area and find passage for this direction";
    area = this.location;
    !valid(area) && return false;
    !respond_to(area, 'find_passage_by_direction) && return false;
    passage = area:find_passage_by_direction(this, direction);
    !passage && return false;
    "Traverse the passage";
    return passage:travel_from(who, this, []);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Room-local help topics for passage/door interaction.";
    {for_player, ?topic = ""} = args;
    doors_help = $help:mk("doors", "Door commands", "Door commands operate only on door-like passages.\n\nA passage is treated as a door when either:\n- `is_door` is true, or\n- it has an `unlock_rule`\n\nBuilders can toggle door behavior with:\n- `@set-passage <dir> is_door true|false`", {"door", "doors", "open", "close", "lock", "unlock"}, 'basics, {"movement", "exits"});
    open_help = $help:mk("open", "Open a door", "Open a door-like passage from this room.\n\nUsage:\n- `open <direction>`\n- `open door`\n\nIf the door is locked, open may auto-unlock when you have a matching key.", {"open door"}, 'basics, {"close", "lock", "unlock", "doors"});
    close_help = $help:mk("close", "Close a door", "Close a door-like passage from this room.\n\nUsage:\n- `close <direction>`\n- `close door`", {"close door"}, 'basics, {"open", "lock", "unlock", "doors"});
    lock_help = $help:mk("lock", "Lock a door", "Lock a lockable door-like passage.\n\nUsage:\n- `lock <direction>`\n- `lock <direction> with <key>`\n- `lock door`\n\nRequires a matching key and an `unlock_rule` on the passage.", {"lock door"}, 'basics, {"unlock", "open", "doors"});
    unlock_help = $help:mk("unlock", "Unlock a door", "Unlock a lockable door-like passage.\n\nUsage:\n- `unlock <direction>`\n- `unlock <direction> with <key>`\n- `unlock door`\n\nRequires a matching key and an `unlock_rule` on the passage.", {"unlock door"}, 'basics, {"lock", "open", "doors"});
    if (topic == "")
      return {doors_help, open_help, close_help, lock_help, unlock_help};
    endif
    doors_help:matches(topic) && return doors_help;
    open_help:matches(topic) && return open_help;
    close_help:matches(topic) && return close_help;
    lock_help:matches(topic) && return lock_help;
    unlock_help:matches(topic) && return unlock_help;
    return 0;
  endmethod

  method recycle owner: ARCH_WIZARD
    "Clean up passages when room is recycled.";
    const actor = caller_perms();
    actor == this.owner || (valid(actor) && actor.wizard) || raise(E_PERM);
    const area = this.location;
    if (valid(area) && respond_to(area, 'on_room_recycle))
      `area:on_room_recycle(this) ! ANY';
    endif
  endmethod

  method notify_pre_exit owner: HACKER
    "Notify contents before a player leaves (called by passage before departure message).";
    {who} = args;
    if (valid(who) && is_player(who))
      "Clear any engagement record for this actor.";
      this:disengage_actor(who);
      "Notify contents (furniture, etc.) that someone is leaving.";
      for thing in (this.contents)
        if (thing != who)
          `thing:on_location_exit(who) ! E_VERBNF => 0';
        endif
      endfor
    endif
  endmethod

  verb unlock (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Unlock a passage, or defer to an object's unlock verb.";
    "Usage: unlock <direction> [with <key>] OR unlock <object> OR unlock door";
    direction = dobjstr;
    if (!direction || direction == "")
      player:inform_current($event:mk_error(player, "Unlock what?"));
      return;
    endif
    "First check if this matches an object in scope with an unlock verb";
    scope = {@player.contents, @this.contents};
    target = $match:resolve_in_scope(direction, scope);
    if (target == $ambiguous_match)
      player:inform_current($event:mk_error(player, "I see more than one \"" + direction + "\" here."));
      return;
    endif
    if (valid(target) && respond_to(target, 'unlock))
      return target:unlock();
    endif
    "Get the area";
    area = this.location;
    if (!valid(area) || !isa(area, $area))
      player:inform_current($event:mk_error(player, "This room is not in an area."));
      return;
    endif
    "If 'door' was specified, find a door-type passage";
    passage = 0;
    if (direction == "door")
      passages = area:passages_from(this);
      for p in (passages)
        unlock_rule = p:_value("unlock_rule", 0);
        door_like = p:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
        if (door_like)
          passage = p;
          break;
        endif
      endfor
    else
      passage = area:find_passage_by_direction(this, direction);
    endif
    is_passage = typeof(passage) == TYPE_FLYWEIGHT || (typeof(passage) == TYPE_OBJ && valid(passage));
    if (!is_passage)
      direction_words = {"n", "north", "s", "south", "e", "east", "w", "west", "u", "up", "d", "down", "in", "out", "ne", "nw", "se", "sw"};
      if (valid(target))
        player:inform_current($event:mk_error(player, "You can't unlock that."));
      elseif (iobjstr && iobjstr != "")
        player:inform_current($event:mk_error(player, "I don't see \"" + direction + "\" here."));
      elseif (direction == "door")
        player:inform_current($event:mk_error(player, "There's no door here."));
      elseif (direction in direction_words)
        player:inform_current($event:mk_error(player, "There's no exit in that direction."));
      else
        player:inform_current($event:mk_error(player, "I don't see \"" + direction + "\" here."));
      endif
      return;
    endif
    unlock_rule = passage:_value("unlock_rule", 0);
    door_like = passage:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
    if (!door_like)
      player:inform_current($event:mk_error(player, "That exit can't be unlocked."));
      return;
    endif
    "Check if it's locked";
    if (!passage:_value("is_locked", false))
      door_name = passage:door_name_for(this);
      msg = "The " + door_name + " is already unlocked.";
      player:inform_current($event:mk_info(player, msg));
      return;
    endif
    "Find a key - either specified or auto-detect from inventory";
    key = #-1;
    to_room = passage:other_room(this);
    if (iobjstr && iobjstr != "")
      key = $match:resolve_in_scope(iobjstr, scope);
      if (!valid(key))
        player:inform_current($event:mk_error(player, "You don't have that."));
        return;
      endif
    else
      "Auto-detect key";
      if (typeof(unlock_rule) == TYPE_FLYWEIGHT)
        for item in (player.contents)
          bindings = ['Accessor -> player, 'This -> to_room, 'Key -> item, 'Passage -> passage];
          result = $rule_engine:evaluate(unlock_rule, bindings);
          if (typeof(result) == TYPE_MAP && result['success])
            key = item;
            break;
          endif
          bindings = ['Accessor -> player, 'This -> this, 'Key -> item, 'Passage -> passage];
          result = $rule_engine:evaluate(unlock_rule, bindings);
          if (typeof(result) == TYPE_MAP && result['success])
            key = item;
            break;
          endif
        endfor
      endif
    endif
    if (!valid(key))
      player:inform_current($event:mk_error(player, "You don't have a key that fits this lock."));
      return;
    endif
    "Verify the key works";
    if (typeof(unlock_rule) == TYPE_FLYWEIGHT)
      bindings = ['Accessor -> player, 'This -> to_room, 'Key -> key, 'Passage -> passage];
      result = $rule_engine:evaluate(unlock_rule, bindings);
      if (typeof(result) != TYPE_MAP || !result['success])
        bindings = ['Accessor -> player, 'This -> this, 'Key -> key, 'Passage -> passage];
        result = $rule_engine:evaluate(unlock_rule, bindings);
        if (typeof(result) != TYPE_MAP || !result['success])
          player:inform_current($event:mk_error(player, "That key doesn't fit this lock."));
          return;
        endif
      endif
    endif
    "Unlock the passage";
    new_passage = passage:with_locked(false);
    try
      area:update_passage(this, to_room, new_passage);
    except (E_PERM)
      player:inform_current($event:mk_error(player, "You don't have permission to change this passage."));
      return;
    endtry
    door_name = passage:door_name_for(this);
    unlock_msg = new_passage:_value("unlock_msg", 0);
    if (unlock_msg)
      player:inform_current($event:mk_info(player, unlock_msg));
    else
      msg = "You unlock the " + door_name + ".";
      player:inform_current($event:mk_info(player, msg));
    endif
    "Announce to room";
    announce_msg = player.name + " unlocks the " + door_name + ".";
    event = $event:mk_narrative(player, announce_msg);
    this:announce(event);
  endverb

  verb lock (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Lock a passage, or defer to an object's lock verb.";
    "Usage: lock <direction> [with <key>] OR lock <object> OR lock door";
    direction = dobjstr;
    if (!direction || direction == "")
      player:inform_current($event:mk_error(player, "Lock what?"));
      return;
    endif
    "First check if this matches an object in scope with a lock verb";
    scope = {@player.contents, @this.contents};
    target = $match:resolve_in_scope(direction, scope);
    if (target == $ambiguous_match)
      player:inform_current($event:mk_error(player, "I see more than one \"" + direction + "\" here."));
      return;
    endif
    if (valid(target) && respond_to(target, 'lock))
      return target:lock();
    endif
    "Get the area";
    area = this.location;
    if (!valid(area) || !isa(area, $area))
      player:inform_current($event:mk_error(player, "This room is not in an area."));
      return;
    endif
    "If 'door' was specified, find a door-type passage";
    passage = 0;
    if (direction == "door")
      passages = area:passages_from(this);
      for p in (passages)
        unlock_rule = p:_value("unlock_rule", 0);
        door_like = p:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
        if (door_like)
          passage = p;
          break;
        endif
      endfor
    else
      passage = area:find_passage_by_direction(this, direction);
    endif
    is_passage = typeof(passage) == TYPE_FLYWEIGHT || (typeof(passage) == TYPE_OBJ && valid(passage));
    if (!is_passage)
      if (direction == "door")
        player:inform_current($event:mk_error(player, "There's no door here."));
      else
        player:inform_current($event:mk_error(player, "There's no exit in that direction."));
      endif
      return;
    endif
    unlock_rule = passage:_value("unlock_rule", 0);
    door_like = passage:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
    if (!door_like)
      player:inform_current($event:mk_error(player, "That exit can't be locked."));
      return;
    endif
    "Check if it's already locked";
    if (passage:_value("is_locked", false))
      door_name = passage:door_name_for(this);
      msg = "The " + door_name + " is already locked.";
      player:inform_current($event:mk_info(player, msg));
      return;
    endif
    "Check if passage has a lock (unlock_rule)";
    if (typeof(unlock_rule) != TYPE_FLYWEIGHT)
      player:inform_current($event:mk_error(player, "That doesn't have a lock."));
      return;
    endif
    "Find a key - either specified or auto-detect from inventory";
    key = #-1;
    to_room = passage:other_room(this);
    if (iobjstr && iobjstr != "")
      key = $match:resolve_in_scope(iobjstr, scope);
      if (!valid(key))
        player:inform_current($event:mk_error(player, "You don't have that."));
        return;
      endif
    else
      "Auto-detect key";
      for item in (player.contents)
        bindings = ['Accessor -> player, 'This -> to_room, 'Key -> item, 'Passage -> passage];
        result = $rule_engine:evaluate(unlock_rule, bindings);
        if (typeof(result) == TYPE_MAP && result['success])
          key = item;
          break;
        endif
        bindings = ['Accessor -> player, 'This -> this, 'Key -> item, 'Passage -> passage];
        result = $rule_engine:evaluate(unlock_rule, bindings);
        if (typeof(result) == TYPE_MAP && result['success])
          key = item;
          break;
        endif
      endfor
    endif
    if (!valid(key))
      player:inform_current($event:mk_error(player, "You don't have a key that fits this lock."));
      return;
    endif
    "Verify the key works";
    bindings = ['Accessor -> player, 'This -> to_room, 'Key -> key, 'Passage -> passage];
    result = $rule_engine:evaluate(unlock_rule, bindings);
    if (typeof(result) != TYPE_MAP || !result['success])
      bindings = ['Accessor -> player, 'This -> this, 'Key -> key, 'Passage -> passage];
      result = $rule_engine:evaluate(unlock_rule, bindings);
      if (typeof(result) != TYPE_MAP || !result['success])
        player:inform_current($event:mk_error(player, "That key doesn't fit this lock."));
        return;
      endif
    endif
    "Close the door first if it's open, then lock it";
    new_passage = passage;
    if (passage:_value("is_open", true))
      new_passage = new_passage:with_open(false);
    endif
    new_passage = new_passage:with_locked(true);
    try
      area:update_passage(this, to_room, new_passage);
    except (E_PERM)
      player:inform_current($event:mk_error(player, "You don't have permission to change this passage."));
      return;
    endtry
    door_name = passage:door_name_for(this);
    lock_msg = new_passage:_value("lock_msg", 0);
    if (lock_msg)
      player:inform_current($event:mk_info(player, lock_msg));
    else
      msg = "You lock the " + door_name + ".";
      player:inform_current($event:mk_info(player, msg));
    endif
    "Announce to room";
    announce_msg = player.name + " locks the " + door_name + ".";
    event = $event:mk_narrative(player, announce_msg);
    this:announce(event);
  endverb

  verb open (any none none) owner: ARCH_WIZARD flags: "rxd"
    "Open a closed door/passage, or defer to an object's open verb.";
    "Usage: open <direction> OR open <object> OR open door";
    direction = dobjstr;
    if (!direction || direction == "")
      player:inform_current($event:mk_error(player, "Open what?"));
      return;
    endif
    "First check if this matches an object in scope with an open verb";
    scope = {@player.contents, @this.contents};
    target = $match:resolve_in_scope(direction, scope);
    if (target == $ambiguous_match)
      player:inform_current($event:mk_error(player, "I see more than one \"" + direction + "\" here."));
      return;
    endif
    if (valid(target) && respond_to(target, 'open))
      return target:open();
    endif
    "Get the area";
    area = this.location;
    if (!valid(area) || !isa(area, $area))
      player:inform_current($event:mk_error(player, "This room is not in an area."));
      return;
    endif
    "If 'door' was specified, find a door-type passage";
    passage = 0;
    if (direction == "door")
      passages = area:passages_from(this);
      for p in (passages)
        unlock_rule = p:_value("unlock_rule", 0);
        door_like = p:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
        if (door_like)
          passage = p;
          break;
        endif
      endfor
    else
      passage = area:find_passage_by_direction(this, direction);
    endif
    is_passage = typeof(passage) == TYPE_FLYWEIGHT || (typeof(passage) == TYPE_OBJ && valid(passage));
    if (!is_passage)
      if (direction == "door")
        player:inform_current($event:mk_error(player, "There's no door here."));
      else
        player:inform_current($event:mk_error(player, "There's no exit in that direction."));
      endif
      return;
    endif
    unlock_rule = passage:_value("unlock_rule", 0);
    door_like = passage:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
    if (!door_like)
      player:inform_current($event:mk_error(player, "That exit can't be opened."));
      return;
    endif
    "Check if it's already open";
    if (passage:_value("is_open", true))
      door_name = passage:door_name_for(this);
      msg = "The " + door_name + " is already open.";
      player:inform_current($event:mk_info(player, msg));
      return;
    endif
    "Check if it's locked";
    to_room = passage:other_room(this);
    if (passage:_value("is_locked", false))
      "Try to unlock it first - find a key";
      key = #-1;
      if (typeof(unlock_rule) == TYPE_FLYWEIGHT)
        for item in (player.contents)
          bindings = ['Accessor -> player, 'This -> to_room, 'Key -> item, 'Passage -> passage];
          result = $rule_engine:evaluate(unlock_rule, bindings);
          if (typeof(result) == TYPE_MAP && result['success])
            key = item;
            break;
          endif
          bindings = ['Accessor -> player, 'This -> this, 'Key -> item, 'Passage -> passage];
          result = $rule_engine:evaluate(unlock_rule, bindings);
          if (typeof(result) == TYPE_MAP && result['success])
            key = item;
            break;
          endif
        endfor
      endif
      if (!valid(key))
        door_name = passage:door_name_for(this);
        msg = "The " + door_name + " is locked.";
        player:inform_current($event:mk_error(player, msg));
        return;
      endif
      "Unlock it first";
      passage = passage:with_locked(false);
      door_name = passage:door_name_for(this);
      unlock_msg = passage:_value("unlock_msg", 0);
      if (unlock_msg)
        player:inform_current($event:mk_info(player, unlock_msg));
      else
        msg = "You unlock the " + door_name + " with your key.";
        player:inform_current($event:mk_info(player, msg));
      endif
    endif
    "Open the passage";
    new_passage = passage:with_open(true);
    try
      area:update_passage(this, to_room, new_passage);
    except (E_PERM)
      player:inform_current($event:mk_error(player, "You don't have permission to change this passage."));
      return;
    endtry
    door_name = passage:door_name_for(this);
    open_msg = new_passage:_value("open_msg", 0);
    if (open_msg)
      player:inform_current($event:mk_info(player, open_msg));
    else
      msg = "You open the " + door_name + ".";
      player:inform_current($event:mk_info(player, msg));
    endif
    "Announce to room";
    announce_msg = player.name + " opens the " + door_name + ".";
    event = $event:mk_narrative(player, announce_msg);
    this:announce(event);
  endverb

  verb close (any none none) owner: ARCH_WIZARD flags: "rxd"
    "Close an open door/passage, or defer to an object's close verb.";
    "Usage: close <direction> OR close <object> OR close door";
    direction = dobjstr;
    if (!direction || direction == "")
      player:inform_current($event:mk_error(player, "Close what?"));
      return;
    endif
    "First check if this matches an object in scope with a close verb";
    scope = {@player.contents, @this.contents};
    target = $match:resolve_in_scope(direction, scope);
    if (target == $ambiguous_match)
      player:inform_current($event:mk_error(player, "I see more than one \"" + direction + "\" here."));
      return;
    endif
    if (valid(target) && respond_to(target, 'close))
      return target:close();
    endif
    "Get the area";
    area = this.location;
    if (!valid(area) || !isa(area, $area))
      player:inform_current($event:mk_error(player, "This room is not in an area."));
      return;
    endif
    "If 'door' was specified, find a door-type passage";
    passage = 0;
    if (direction == "door")
      passages = area:passages_from(this);
      for p in (passages)
        unlock_rule = p:_value("unlock_rule", 0);
        door_like = p:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
        if (door_like)
          passage = p;
          break;
        endif
      endfor
    else
      passage = area:find_passage_by_direction(this, direction);
    endif
    is_passage = typeof(passage) == TYPE_FLYWEIGHT || (typeof(passage) == TYPE_OBJ && valid(passage));
    if (!is_passage)
      if (direction == "door")
        player:inform_current($event:mk_error(player, "There's no door here."));
      else
        player:inform_current($event:mk_error(player, "There's no exit in that direction."));
      endif
      return;
    endif
    unlock_rule = passage:_value("unlock_rule", 0);
    door_like = passage:_value("is_door", false) || typeof(unlock_rule) == TYPE_FLYWEIGHT;
    if (!door_like)
      player:inform_current($event:mk_error(player, "That exit can't be closed."));
      return;
    endif
    "Check if it's already closed";
    if (!passage:_value("is_open", true))
      door_name = passage:door_name_for(this);
      msg = "The " + door_name + " is already closed.";
      player:inform_current($event:mk_info(player, msg));
      return;
    endif
    "Close the passage";
    new_passage = passage:with_open(false);
    to_room = passage:other_room(this);
    try
      area:update_passage(this, to_room, new_passage);
    except (E_PERM)
      player:inform_current($event:mk_error(player, "You don't have permission to change this passage."));
      return;
    endtry
    door_name = passage:door_name_for(this);
    close_msg = new_passage:_value("close_msg", 0);
    if (close_msg)
      player:inform_current($event:mk_info(player, close_msg));
    else
      msg = "You close the " + door_name + ".";
      player:inform_current($event:mk_info(player, msg));
    endif
    "Announce to room";
    announce_msg = player.name + " closes the " + door_name + ".";
    event = $event:mk_narrative(player, announce_msg);
    this:announce(event);
  endverb

  method command_hints owner: ARCH_WIZARD
    "Return special commands available in this room for LLM suggestions.";
    "Returns list of maps: [{command -> 'press 1', description -> 'Go to floor 1'}, ...]";
    "Override in subclasses to expose room-specific commands handled by maybe_handle_command.";
    return {};
  endmethod

  method engage_actor owner: ARCH_WIZARD
    "Register an actor as engaged with an object (sitting, swimming, etc.).";
    "If actor is already engaged with something else, disengage them first.";
    {actor, object} = args;
    if (!valid(actor) || !valid(object))
      return false;
    endif
    if (actor.location != this || object.location != this)
      return false;
    endif
    "Check if actor is already engaged with something in this room.";
    current = `this.engagements[actor] ! E_RANGE => #-1';
    if (valid(current) && (current.location != this || current == actor))
      this:disengage_actor(actor);
      current = #-1;
    endif
    if (valid(current) && current != object)
      "Disengage from current object first.";
      if (respond_to(current, 'remove_occupant))
        current:remove_occupant(actor);
      endif
    endif
    "Record new engagement.";
    this.engagements[actor] = object;
    return true;
  endmethod

  method disengage_actor owner: ARCH_WIZARD
    "Remove an actor's engagement record from this room.";
    "Does NOT call remove_occupant - use this after the object has already removed them.";
    {actor} = args;
    this.engagements = `mapdelete(this.engagements, actor) ! E_RANGE => this.engagements';
  endmethod

  method get_engagement owner: ARCH_WIZARD
    "Return what object an actor is engaged with in this room, or #-1 if none.";
    {actor} = args;
    engaged = `this.engagements[actor] ! E_RANGE => #-1';
    if (!valid(engaged))
      return #-1;
    endif
    if (!valid(actor) || actor.location != this || engaged.location != this)
      this:disengage_actor(actor);
      return #-1;
    endif
    return engaged;
  endmethod

  method transport_destinations owner: ARCH_WIZARD
    "Return destinations reachable via transport objects in this room.";
    "Scans contents for objects implementing :transport_connections().";
    "Returns list of {destination_room, label, transport_object} tuples.";
    results = {};
    for item in (this.contents)
      if (!valid(item))
        continue;
      endif
      if (!respond_to(item, 'transport_connections))
        continue;
      endif
      connections = item:transport_connections();
      if (typeof(connections) == TYPE_LIST)
        for conn in (connections)
          results = {@results, conn};
        endfor
      endif
    endfor
    return results;
  endmethod

  method _default_distant_transform owner: ARCH_WIZARD
    "Transform an event for distant announcement. Returns new event or 0 to suppress.";
    {event, prefix} = args;
    "Get event info safely";
    slots = flyslots(event);
    actor_name = `slots['actor_name] ! ANY => "someone"';
    verb = `slots['verb] ! ANY => "does something"';
    sound = `slots['sound] ! ANY => ""';
    content = `slots['content] ! ANY => ""';
    "Build a distant description based on event type or sound";
    if (sound == "splash" || verb == "splash" || verb == "dive")
      msg = prefix + ", you hear a splash.";
    elseif (verb == "shout" || verb == "yell")
      if (content)
        msg = prefix + ", you hear " + actor_name + " shout, \"" + content + "\"";
      else
        msg = prefix + ", you hear " + actor_name + " shout.";
      endif
    elseif (verb == "say" || verb == "directed_say")
      msg = prefix + ", you hear muffled voices.";
    elseif (verb == "music" || verb == "record")
      msg = prefix + ", you hear music playing.";
    elseif (verb == "emote")
      msg = prefix + ", you hear sounds of activity.";
    else
      msg = prefix + ", you hear something.";
    endif
    const narrative = $document_template:mk('emphasis, $format:literal_inline(msg));
    return $event:mk_distant(false, narrative):with_location(this);
  endmethod

  method announce_distant owner: ARCH_WIZARD
    "Receive a distant event from another room and announce it locally.";
    {event, source_room} = args;
    if (typeof(event) == TYPE_FLYWEIGHT && valid(source_room))
      event = event:with_metadata('source_room, source_room);
    endif
    "Tell everyone in this room about the distant event";
    for who in (this:contents())
      suspend_if_needed();
      `who:tell(event) ! E_VERBNF';
    endfor
  endmethod

  verb "shout yell" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Shout something loudly - propagates to acoustic neighbors.";
    !argstr && return player:inform_current($event:mk_error(player, "Shout what?"));
    this:announce(player:mk_shout_event(argstr));
  endverb

  method rewrite_announced owner: ARCH_WIZARD
    "Rewrite a previously announced rewritable event for all players in this room.";
    "Args: rewrite_id, new_event";
    "Usage: room:rewrite_announced(rewrite_id, $event:mk_info(actor, new_content))";
    {rewrite_id, new_event} = args;
    for who in (this:contents())
      if (!valid(who) || !is_player(who))
        continue;
      endif
      "Get this player's connections - we have permission since we're the room";
      try
        conns = who:_connections();
        if (!conns || length(conns) == 0)
          continue;
        endif
        "Rewrite for each connection this player has";
        for conn_info in (conns)
          conn = conn_info[1];
          who:rewrite_event(rewrite_id, new_event, conn);
        endfor
      except e (E_PERM, E_INVARG, E_INVIND, E_VERBNF)
        "Skip players we can't rewrite for; let unexpected errors bubble.";
      endtry
    endfor
  endmethod

  method input_placeholders owner: ARCH_WIZARD
    "Return a list of placeholder strings for the input field, or false for default.";
    "Override on specific rooms to provide context-sensitive hints.";
    return false;
  endmethod

  verb _acoustic_debug_bump (this none none) owner: ARCH_WIZARD flags: "rxd"
    {reason, ?entry = 0} = args;
    enabled = `#7.acoustic_debug_enabled ! E_PROPNF => 0';
    if (typeof(enabled) != TYPE_INT && typeof(enabled) != TYPE_FLOAT)
      enabled = enabled ? 1 | 0;
    endif
    !enabled && return;
    owner_info = `property_info(#7, "acoustic_debug_counts") ! E_INVARG => {#2, "rc"}';
    prop_owner = typeof(owner_info) == TYPE_LIST && length(owner_info) >= 1 ? owner_info[1] | #2;
    set_task_perms(prop_owner);
    counts = `#7.acoustic_debug_counts ! E_PROPNF, E_TYPE => []';
    if (typeof(counts) != TYPE_MAP)
      counts = [];
    endif
    prev = `counts[reason] ! E_RANGE, E_TYPE => 0';
    counts[reason] = prev + 1;
    `#7.acoustic_debug_counts = counts ! E_PERM';
    if (entry != 0)
      log = `#7.acoustic_debug_log ! E_PROPNF, E_TYPE => {}';
      if (typeof(log) != TYPE_LIST)
        log = {};
      endif
      if (typeof(entry) == TYPE_MAP)
        entry["room"] = this;
      endif
      log = {entry, @log};
      if (length(log) > 20)
        log = log[1..20];
      endif
      `#7.acoustic_debug_log = log ! E_PERM';
    endif
  endverb

  method room_snapshot owner: HACKER
    "Return structured room state for OOB clients.";
    "{?viewer = player, ?look_data = this:look_self()}";
    {?viewer = player, ?look_data = 0} = args;
    if (typeof(look_data) != TYPE_FLYWEIGHT)
      look_data = this:look_self();
    endif
    exits = `look_data.exits ! E_PROPNF => {}';
    ambient_passages = `look_data.ambient_passages ! E_PROPNF => {}';
    all_exits = {@exits};
    for ap in (ambient_passages)
      suspend_if_needed();
      if (typeof(ap) == TYPE_LIST && length(ap) >= 3)
        label = ap[3];
        if (typeof(label) == TYPE_STR && label && !(label in all_exits))
          all_exits = {@all_exits, label};
        endif
      endif
    endfor
    const exit_links = { ["label" -> direction, "annotation" -> this:exit_annotation(direction)] for direction in (all_exits) };
    available_actions = `look_data.actions ! E_PROPNF => {}';
    actors = {};
    things = {};
    for o in (this:contents())
      suspend_if_needed();
      if (!valid(o) || o == viewer)
        continue;
      endif
      name = `o:name() ! E_VERBNF => o.name';
      if (o:is_actor())
        status = `look_data:actor_idle_status(o) ! ANY => ""';
        actors = {@actors, ["object" -> o, "name" -> name, "status" -> status]};
      else
        things = {@things, ["object" -> o, "name" -> name, "objectKind" -> o:reference_kind()]};
      endif
    endfor
    return ["room" -> this, "title" -> this:name(), "description" -> this:description(), "exits" -> all_exits, "exit_links" -> exit_links, "ambient_passages" -> ambient_passages, "actions" -> available_actions, "actors" -> actors, "things" -> things];
  endmethod
endobject
