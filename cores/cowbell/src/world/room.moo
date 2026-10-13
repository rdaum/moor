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
  property passage_areas (owner: ARCH_WIZARD, flags: "r") = {};
  property exit_revision (owner: ARCH_WIZARD, flags: "r") = 0;
  property exit_providers (owner: ARCH_WIZARD, flags: "r") = [];

  override description (owner: HACKER, flags: "rc") = "Parent prototype for all rooms in the system, defining room behavior and event broadcasting.";

  method passages owner: ARCH_WIZARD
    "Find connections at an endpoint, including stores outside its containing area.";
    set_task_perms(caller_perms());
    const {?room = this} = args;
    valid(room) && isa(room, $room) || raise(E_TYPE);
    let passages = {};
    for area in (room.passage_areas)
      if (valid(area) && isa(area, $area))
        passages = {@passages, @$area:stored_passages_from(area, room)};
      endif
    endfor
    return passages;
  endmethod

  method find_connection owner: ARCH_WIZARD
    "Resolve an installed identity at this endpoint to {storing area, current passage}.";
    set_task_perms(caller_perms());
    const {room, identity} = args;
    typeof(identity) == TYPE_STR && identity != "" || return {};
    valid(room) && isa(room, $room) || return {};
    for area in (room.passage_areas)
      if (!valid(area) || !isa(area, $area))
        continue;
      endif
      for passage in ($area:stored_passages_from(area, room))
        if (passage.connection_id == identity)
          return {area, passage};
        endif
      endfor
    endfor
    return {};
  endmethod

  method find_passage owner: ARCH_WIZARD
    "Match a direction against all of a room's passages, rejecting ambiguous names.";
    set_task_perms(caller_perms());
    const {room, direction} = args;
    typeof(direction) == TYPE_STR || raise(E_TYPE);
    let matches = {};
    for passage in ($room:passages(room))
      if ($passage:allows_direction(passage, room) && passage:matches_command(room, direction))
        matches = {@matches, passage};
      endif
    endfor
    length(matches) <= 1 || raise(E_INVARG, "Several exits match '" + direction + "'. Use a more specific name.");
    return length(matches) ? matches[1] | false;
  endmethod

  method passage_to owner: ARCH_WIZARD
    "Select a connection to another room; parallel connections need an identity.";
    set_task_perms(caller_perms());
    const {room, destination, ?identity = ""} = args;
    room == destination && return false;
    let matches = {};
    for passage in ($room:passages(room))
      if (passage:other_room(room) == destination && (identity == "" || passage.connection_id == identity))
        matches = {@matches, passage};
      endif
    endfor
    length(matches) <= 1 || raise(E_INVARG, "Several passages connect those rooms. Choose a direction or connection identity.");
    return length(matches) ? matches[1] | false;
  endmethod

  method initialize owner: ARCH_WIZARD
    "Set sensible defaults for newly created rooms.";
    pass();
    set_task_perms(caller_perms());
    this.description = "An empty room awaiting a description.";
    set_object_metadata(this, "import_export_hierarchy", {"rooms"});
  endmethod

  verb emote (any any any) owner: HACKER flags: "rxd"
    "Usage: emote <action>. Announce an action to the room.";
    if (!length(argstr:trim()))
      player:inform_current($event:mk_error(player, "Emote what?"));
      return;
    endif
    this:announce(player:mk_emote_event(argstr));
  endverb

  verb say (any any any) owner: HACKER flags: "rxd"
    "Usage: say <message>. Speak to everyone in the room.";
    if (!length(argstr:trim()))
      player:inform_current($event:mk_error(player, "Say what?"));
      return;
    endif
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
        if (!is_player(viewer) || viewer == who)
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
    this:disengage_actor(who);
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
    return $area:handle_passage_command(parsed);
  endmethod

  method maybe_handle_command owner: ARCH_WIZARD
    "Handle any potential commands that the command matcher didn't already handle on the player, for example for furniture or exits";
    const {pc} = args;
    const actor = caller_perms();
    actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(actor);
    const command = pc['verb] + (pc['argstr] == "" ? "" | " " + pc['argstr]);
    for option in ($room:exit_options(this, player))
      if (maphaskey(option, 'provider) && command in option['commands])
        if (option['state] == 'unavailable)
          player:inform_current($event:mk_info(player, option['reason]));
        else
          $room:use_exit_action(player, option['descriptor]["command"], option['descriptor]["exit"]);
        endif
        return true;
      endif
    endfor
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

  method look_self owner: ARCH_WIZARD
    "Collect authored room content, passages, and actions from one contents list.";
    set_task_perms(caller_perms());
    const revision = this.exit_revision;
    let look = pass(@args);
    look.exit_revision = revision;
    look.exit_options = $room:exit_options(this, player);
    look.ambient_passages = {{option['description], option['prose_style],
      option['label], option['destination]} for option in (look.exit_options) if (option['ambient])};
    let actions = {};
    for item in ({this, @flycontents(look)})
      const available = `item:available_actions() ! E_VERBNF => {}';
      typeof(available) == TYPE_LIST || raise(E_TYPE, "Actions must be a list");
      actions = {@actions, @available};
    endfor
    look.actions = actions;
    return look;
  endmethod

  method exit_options owner: ARCH_WIZARD
    "List visible connections, including closed doors, independently of the containing area.";
    "Returns maps with identity, label, caption, state, descriptor, description, and destination.";
    "An open state permits a travel attempt; the passage still checks access when used.";
    set_task_perms(caller_perms());
    const {room, viewer} = args;
    let options = {};
    for passage in ($room:passages(room))
      if (!$passage:allows_direction(passage, room))
        continue;
      endif
      const prefix = passage.side_a_room == room ? "side_a_" | "side_b_";
      const label = passage.(prefix + "label");
      if (label == "")
        continue;
      endif
      let state = passage.is_open ? 'open | 'closed;
      passage.is_locked && (state = 'locked);
      const caption = label + (state == 'open ? "" | " (" + tostr(state) + ")");
      const description = $passage:description_content(passage, room);
      const ambient = passage.(prefix + "ambient") && length(description) > 0;
      options = {@options, ['identity -> passage.connection_id, 'label -> label, 'caption -> caption,
        'state -> state, 'destination -> passage.(prefix == "side_a_" ? "side_b_room" | "side_a_room"),
        'passage -> passage, 'descriptor -> $room:_passage_annotation(room, passage, label),
        'description -> description, 'ambient -> ambient, 'prose_style -> passage.(prefix + "prose_style")]};
    endfor
    for provider in (mapkeys(room.exit_providers))
      try
        const custom = $room:_provider_options(room, viewer, provider, options);
        options = {@options, @custom};
      except error (ANY)
        "One faulty provider must not erase the room's other exits.";
        $room:_log_provider_error(room, provider, error);
        options = {@options, ['identity -> "", 'label -> "Exit provider " + tostr(provider),
          'caption -> "Exit provider " + tostr(provider) + " (error; contact the room's owner)",
          'state -> 'unavailable, 'descriptor -> [], 'description -> {}, 'destination -> #-1,
          'ambient -> false, 'prose_style -> 'sentence, 'error -> error]};
      endtry
    endfor
    return options;
  endmethod

  method register_exit_provider owner: ARCH_WIZARD
    "Approve an object's travel_options/use_exit methods. Requires dig_from on the room.";
    const {subject, provider} = args;
    const {room, _} = $root:_check_permissions_as(subject, caller_perms(), 'dig_from);
    valid(room) && isa(room, $room) || raise(E_TYPE);
    typeof(provider) == TYPE_OBJ && valid(provider) && !is_anonymous(provider) || raise(E_TYPE);
    respond_to(provider, 'travel_options) && respond_to(provider, 'use_exit)
      || raise(E_INVARG, "An exit provider needs travel_options and use_exit methods.");
    const registration = ['token -> uuid(), 'room_owner -> room.owner, 'provider_owner -> provider.owner];
    room.exit_providers[provider] = registration;
    $room:connections_changed({room});
    return registration['token];
  endmethod

  method unregister_exit_provider owner: ARCH_WIZARD
    "Remove approval and invalidate old links. Requires dig_from on the room.";
    const {subject, provider} = args;
    const {room, _} = $root:_check_permissions_as(subject, caller_perms(), 'dig_from);
    valid(room) && isa(room, $room) || raise(E_TYPE);
    maphaskey(room.exit_providers, provider) || return false;
    room.exit_providers = mapdelete(room.exit_providers, provider);
    $room:connections_changed({room});
    return true;
  endmethod

  method exit_options_changed owner: ARCH_WIZARD
    "Refresh room panels after a provider changes. The registered provider or room author may call.";
    const {room, provider} = args;
    const principal = caller_perms();
    const registration = $room:_exit_registration(room, provider);
    length(registration) || raise(E_INVARG, "This exit provider needs the room owner's approval.");
    if (caller != provider || principal != provider.owner)
      $root:_check_permissions_as(room, principal, 'dig_from);
    endif
    $room:connections_changed({room});
  endmethod

  method _exit_registration owner: ARCH_WIZARD
    "Return current approval, or an empty map after removal or an ownership change.";
    set_task_perms(caller_perms());
    const {room, provider} = args;
    typeof(room) == TYPE_OBJ && valid(room) && isa(room, $room) || return [];
    typeof(provider) == TYPE_OBJ && valid(provider) || return [];
    const registration = `room.exit_providers[provider] ! E_RANGE => []';
    length(registration) || return [];
    (registration['room_owner] == room.owner && registration['provider_owner] == provider.owner)
      || return [];
    return registration;
  endmethod

  method _log_provider_error owner: ARCH_WIZARD
    "Retain a broken provider's diagnostic without lending wizard authority to its callback.";
    caller == $room || raise(E_PERM);
    const {room, provider, error} = args;
    server_log("Exit provider " + tostr(provider) + " at " + tostr(room) + ": " + toliteral(error));
  endmethod

  method _provider_options owner: ARCH_WIZARD
    "Validate a provider's visible options under the caller's authority; no movement happens here.";
    set_task_perms(caller_perms());
    const {room, viewer, provider, existing} = args;
    const registration = $room:_exit_registration(room, provider);
    length(registration) || return {};
    const offered = provider:travel_options(room, viewer);
    typeof(offered) == TYPE_LIST && length(offered) <= 64
      || raise(E_INVARG, "travel_options must return at most 64 option maps.");
    equal(registration, $room:_exit_registration(room, provider)) || return {};
    let options = {};
    let identities = {};
    let labels = {option['label] for option in (existing)};
    let commands = {};
    for option in (existing)
      if (maphaskey(option, 'commands))
        commands = {@commands, @option['commands]};
      elseif (maphaskey(option, 'passage))
        const names = $passage:expand_direction_aliases($passage:command_names(option['passage], room));
        commands = {@commands, @names};
        for operation in ({"go", "open", "close", "lock", "unlock"})
          commands = {@commands, @{operation + " " + name for name in (names)}};
        endfor
      endif
    endfor
    fn text(value, limit)
      return typeof(value) == TYPE_STR && length(value) > 0 && length(value) <= limit
        && value == value:trim() && !index(value, "\n") && !index(value, "\r");
    endfn
    for offered_option in (offered)
      typeof(offered_option) == TYPE_MAP || raise(E_TYPE, "An exit option must be a map.");
      const id = offered_option['id];
      const label = offered_option['label];
      const command = offered_option['command];
      text(id, 128) && text(label, 128) && text(command, 256)
        || raise(E_INVARG, "Exit id, label, and command must be nonempty single-line text.");
      !(id in identities) && !(label in labels) || raise(E_INVARG, "Duplicate exit id or label.");
      const aliases = `offered_option['aliases] ! E_RANGE => {}';
      typeof(aliases) == TYPE_LIST && length(aliases) <= 16 || raise(E_TYPE, "Invalid exit aliases.");
      const names = {command, @aliases};
      for name in (names)
        text(name, 256) && !(name in commands) || raise(E_INVARG, "Conflicting exit command: " + toliteral(name));
        commands = {@commands, name};
      endfor
      const available = `offered_option['available] ! E_RANGE => true';
      typeof(available) == TYPE_BOOL || raise(E_TYPE, "Exit available must be a boolean.");
      const reason = `offered_option['reason] ! E_RANGE => "That way is currently unavailable."';
      text(reason, 512) || raise(E_TYPE, "Invalid exit availability explanation.");
      const destination = `offered_option['destination] ! E_RANGE => #-1';
      (destination == #-1 || (typeof(destination) == TYPE_OBJ && valid(destination) && isa(destination, $room)))
        || raise(E_TYPE, "An exit destination must be a room or #-1.");
      const description = $passage:authored_description(
        `offered_option['description] ! E_RANGE => ""', `offered_option['description_kind] ! E_RANGE => 'literal');
      const ambient = `offered_option['ambient] ! E_RANGE => false';
      const style = `offered_option['prose_style] ! E_RANGE => 'sentence';
      typeof(ambient) == TYPE_BOOL && style in {'sentence, 'fragment}
        || raise(E_TYPE, "Invalid exit prose layout.");
      let reference = ["source" -> $url_utils:to_curie_str(room),
        "provider" -> $url_utils:to_curie_str(provider), "registration" -> registration['token], "option" -> id];
      valid(destination) && (reference["destination"] = $url_utils:to_curie_str(destination));
      const descriptor = available ? ["kind" -> "command", "command" -> command, "exit" -> reference] | [];
      options = {@options, ['identity -> id, 'label -> label, 'commands -> names, 'provider -> provider,
        'caption -> label + (available ? "" | " (" + reason + ")"), 'reason -> reason,
        'state -> available ? 'open | 'unavailable, 'descriptor -> descriptor, 'destination -> destination,
        'description -> description, 'ambient -> ambient && length(description) > 0, 'prose_style -> style]};
      identities = {@identities, id};
      labels = {@labels, label};
    endfor
    return options;
  endmethod

  method _passage_annotation owner: ARCH_WIZARD
    "Build the action for a captured passage value. Closed doors offer an opening attempt.";
    set_task_perms(caller_perms());
    const {room, passage, direction} = args;
    const destination = passage.side_a_room == room ? passage.side_b_room | passage.side_a_room;
    valid(destination) && passage.connection_id != "" || return [];
    const door = passage.is_door || typeof(passage.unlock_rule) == TYPE_FLYWEIGHT;
    !passage.is_open && !door && return [];
    const operation = passage.is_open ? "go" | "open";
    const action = operation == "go" ? $player:command_action("go", direction)
      | $room:command_action("open", direction);
    return ["kind" -> "command", "command" -> operation + " " + direction, "action" -> action,
      "exit" -> ["source" -> $url_utils:to_curie_str(room),
        "destination" -> $url_utils:to_curie_str(destination), "passage" -> passage.connection_id]];
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
    "List the visible ways out, including closed doors and their current actions.";
    caller == player || raise(E_PERM);
    set_task_perms(player);
    const options = $room:exit_options(this, player);
    if (!length(options))
      return player:inform_current($event:mk_info(player,
        "You don't see any obvious ways out."):with_audience('utility));
    endif
    const links = {length(option['descriptor])
      ? $format.annotation:mk(option['caption], option['descriptor]) | option['caption]
      for option in (options)};
    const content = $format.block:mk($format.title:mk("Ways out"), $format.list:mk(links));
    const event = $event:mk_info(player, content):with_audience('utility)
      :with_presentation_hint('inset):with_group('exits, this);
    player:inform_current(event);
  endverb

  method exit_annotation owner: ARCH_WIZARD
    "Describe the current action for a registered passage.";
    set_task_perms(caller_perms());
    const {direction} = args;
    for option in ($room:exit_options(this, player))
      if (option['label] == direction)
        return option['descriptor];
      endif
    endfor
    const passage = $room:find_passage(this, direction);
    return typeof(passage) in {TYPE_OBJ, TYPE_FLYWEIGHT}
      ? $room:_passage_annotation(this, passage, direction) | [];
  endmethod

  method use_exit_action owner: ARCH_WIZARD
    "Use the connection shown in a client action, without matching its command against another exit.";
    const {who, command, reference} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    set_task_perms(principal);
    fn stale()
      if (respond_to(who, 'inform_current))
        who:inform_current($event:mk_info(who, "That exit has changed or is no longer here. Look again."));
      endif
      return false;
    endfn
    typeof(reference) == TYPE_MAP && typeof(command) == TYPE_STR || return stale();
    const source = `reference["source"] ! E_RANGE => ""';
    const identity = `reference["passage"] ! E_RANGE => ""';
    const destination = `reference["destination"] ! E_RANGE => ""';
    const room = who.location;
    valid(room) && source == $url_utils:to_curie_str(room) || return stale();
    if (maphaskey(reference, "provider"))
      identity == "" || return stale();
      for option in ($room:exit_options(room, who))
        if (maphaskey(option, 'provider) && option['state] == 'open
            && command == option['descriptor]["command"] && equal(reference, option['descriptor]["exit"]))
          const provider = option['provider];
          const registration = $room:_exit_registration(room, provider);
          (who.location == room && length(registration)
            && registration['token] == reference["registration"]) || return stale();
          $root:check_actor(who, principal);
          const destination = provider:use_exit(who, room, option['identity]);
          typeof(destination) == TYPE_OBJ && valid(destination) && isa(destination, $room) || return false;
          (who.location == room && equal(registration, $room:_exit_registration(room, provider)))
            || return stale();
          if (maphaskey(reference, "destination"))
            reference["destination"] == $url_utils:to_curie_str(destination) || return stale();
          endif
          $root:check_actor(who, principal);
          move(who, destination);
          return who.location == destination;
        endif
      endfor
      return stale();
    endif
    const found = $room:find_connection(room, identity);
    length(found) || return stale();
    const passage = found[2];
    $passage:allows_direction(passage, room) || return stale();
    const near = passage.side_a_room == room;
    const other = near ? passage.side_b_room | passage.side_a_room;
    const names = $passage:expand_direction_aliases($passage:command_names(passage, room));
    valid(other) && destination == $url_utils:to_curie_str(other) || return stale();
    if (command in {"go " + name for name in (names)})
      return passage:travel_from(who, room, []);
    elseif (command in {"open " + door_name for door_name in (names)}
        && (passage.is_door || typeof(passage.unlock_rule) == TYPE_FLYWEIGHT))
      const result = $passage:operate(passage, 'open, who, room);
      if (result['status] != 'applied)
        who:inform_current($event:mk_info(who, result['reason]));
      endif
      return result['status] == 'applied;
    endif
    return stale();
  endmethod

  method action_go owner: ARCH_WIZARD
    "Use a passage from this room for an authenticated actor; return whether movement completed.";
    const principal = caller_perms();
    const {who, context, direction} = args;
    $root:check_actor(who, principal);
    set_task_perms(principal);
    who.location == this || return false;
    const passage = $room:find_passage(this, direction);
    typeof(passage) in {TYPE_OBJ, TYPE_FLYWEIGHT} || return false;
    const destination = passage:other_room(this);
    passage:travel_from(who, this, context);
    return who.location == destination;
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Room-local help topics for passage/door interaction.";
    {for_player, ?topic = ""} = args;
    doors_help = $help:mk("doors", "Door commands", "Door commands operate only on door-like passages.\n\nA passage is treated as a door when either:\n\n- `is_door` is true, or\n- it has an `unlock_rule`\n\nBuilders can toggle door behavior with:\n\n- `@set-passage <dir> is_door true|false`", {"door", "doors", "open", "close", "lock", "unlock"}, 'basics, {"movement", "exits"}, 'djot_blocks);
    open_help = $help:mk("open", "Open a door", "Open a door-like passage from this room.\n\nUsage:\n\n- `open <direction>`\n- `open door`\n\nIf the door is locked, open may auto-unlock when you have a matching key.", {"open door"}, 'basics, {"close", "lock", "unlock", "doors"}, 'djot_blocks);
    close_help = $help:mk("close", "Close a door", "Close a door-like passage from this room.\n\nUsage:\n\n- `close <direction>`\n- `close door`", {"close door"}, 'basics, {"open", "lock", "unlock", "doors"}, 'djot_blocks);
    lock_help = $help:mk("lock", "Lock a door", "Lock a lockable door-like passage.\n\nUsage:\n\n- `lock <direction>`\n- `lock <direction> with <key>`\n- `lock door`\n\nRequires a matching key and an `unlock_rule` on the passage.", {"lock door"}, 'basics, {"unlock", "open", "doors"}, 'djot_blocks);
    unlock_help = $help:mk("unlock", "Unlock a door", "Unlock a lockable door-like passage.\n\nUsage:\n\n- `unlock <direction>`\n- `unlock <direction> with <key>`\n- `unlock door`\n\nRequires a matching key and an `unlock_rule` on the passage.", {"unlock door"}, 'basics, {"lock", "open", "doors"}, 'djot_blocks);
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
    for area in (this.passage_areas)
      if (valid(area))
        area:on_room_recycle(this);
      endif
    endfor
  endmethod

  method notify_pre_exit owner: HACKER
    "Optional pre-move hook. Completed departure notifications belong to exitfunc.";
    return true;
  endmethod

  verb unlock (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Unlock an exit or a nearby object. A named key must be carried.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player
      || (valid(principal) && principal.wizard)) || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    return $room:door_command(this, 'unlock, player, dobjstr, iobjstr);
  endverb

  verb lock (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Lock an exit or a nearby object. A named key must be carried.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player
      || (valid(principal) && principal.wizard)) || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    return $room:door_command(this, 'lock, player, dobjstr, iobjstr);
  endverb

  verb open (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Open an exit or a nearby object. A named key must be carried.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player
      || (valid(principal) && principal.wizard)) || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    return $room:door_command(this, 'open, player, dobjstr, iobjstr);
  endverb

  verb close (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Close an exit or a nearby object. A named key must be carried.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player
      || (valid(principal) && principal.wizard)) || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    return $room:door_command(this, 'close, player, dobjstr, iobjstr);
  endverb

  method command_hints owner: ARCH_WIZARD
    "Return special commands available in this room for LLM suggestions.";
    "Returns list of maps: [{command -> 'press 1', description -> 'Go to floor 1'}, ...]";
    "Override in subclasses to expose room-specific commands handled by maybe_handle_command.";
    return {};
  endmethod

  method door_command owner: ARCH_WIZARD
    "Resolve a local object or one passage, then use the shared checked operation.";
    const principal = caller_perms();
    const {room, operation, who, direction, ?key_name = ""} = args;
    $root:check_actor(who, principal);
    operation in {'open, 'close, 'lock, 'unlock} || raise(E_INVARG, "Unknown door operation.");
    set_task_perms(principal);
    who.location == room || raise(E_PERM, "You must be in that room to operate its doors.");
    const inform = {message} => who:inform_current($event:mk_info(who, message));
    direction != "" || return inform(tostr(operation):capitalize() + " what?");
    if (key_name != "" && prepstr != "with")
      return inform("Use '" + tostr(operation) + " EXIT with KEY'.");
    endif
    const scope = {@who.contents, @room.contents};
    const target = $match:resolve_in_scope(direction, scope);
    if (target == $ambiguous_match)
      return inform("Several objects match '" + direction + "'. Use a more specific name.");
    endif
    if (valid(target) && target in scope && respond_to(target, operation))
      return target:(operation)();
    endif
    let passage = false;
    if (direction == "door")
      const doors = {candidate for candidate in ($room:passages(room))
        if ($passage:allows_direction(candidate, room)
          && (candidate.is_door || typeof(candidate.unlock_rule) == TYPE_FLYWEIGHT))};
      if (length(doors) > 1)
        const names = {candidate:label_for(room) for candidate in (doors)};
        return inform("Which door? Specify " + names:join(", ") + ".");
      endif
      passage = length(doors) ? doors[1] | false;
    else
      passage = $room:find_passage(room, direction);
    endif
    typeof(passage) == TYPE_FLYWEIGHT || return inform("There is no exit called '" + direction + "' here.");
    let key = #-1;
    if (key_name != "")
      key = $match:resolve_in_scope(key_name, who.contents);
      if (key == $ambiguous_match)
        return inform("Several carried objects match that key. Use a more specific name.");
      endif
      valid(key) && key.location == who || return inform("You are not carrying that key.");
    endif
    const result = $passage:operate(passage, operation, who, room, key);
    if (result['status] != 'applied)
      inform(result['reason]);
    endif
    return result;
  endmethod

  method refresh_snapshots owner: ARCH_WIZARD
    "Publish current room state after an authorized connection change.";
    caller_perms().wizard || raise(E_PERM);
    const {room} = args;
    valid(room) || return;
    for viewer in (room.contents)
      if (is_player(viewer))
        viewer:emit_room_snapshot_state(viewer, room);
      endif
    endfor
  endmethod

  method connections_changed owner: ARCH_WIZARD
    "Invalidate older exit captures in this transaction, then refresh occupied room panels.";
    caller_perms().wizard || raise(E_PERM);
    const {rooms} = args;
    for room in (rooms)
      if (valid(room))
        room.exit_revision = room.exit_revision + 1;
      endif
    endfor
    const occupied = {candidate for candidate in (rooms) if (valid(candidate)
      && length({who for who in (connected_players()) if (who.location == candidate)}))};
    if (length(occupied))
      fork (0)
        for changed_room in (occupied)
          $room:refresh_snapshots(changed_room);
        endfor
      endfork
    endif
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
    "Return {destination, label, provider, command} for available, registered custom exits.";
    set_task_perms(caller_perms());
    const {?viewer = player} = args;
    return {{option['destination], option['label], option['provider], option['descriptor]["command"]}
      for option in ($room:exit_options(this, viewer)) if (maphaskey(option, 'provider)
        && option['state] == 'open && valid(option['destination]))};
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

  method room_snapshot owner: ARCH_WIZARD
    "Capture a viewer's current room panel. Combined delivery captures once in the receiver.";
    set_task_perms(caller_perms());
    const {?viewer = player} = args;
    return $look:panel(this:look_self():capture(viewer));
  endmethod
endobject
