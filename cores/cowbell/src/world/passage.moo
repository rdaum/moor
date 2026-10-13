object PASSAGE [
  import_export_id -> "passage",
  import_export_hierarchy -> {"world"}
]
  name: "Generic Passage"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  property close_msg (owner: HACKER, flags: "rc") = 0;
  property direction_abbrevs (owner: HACKER, flags: "rc") = [
    "d" -> "down",
    "down" -> "d",
    "downstairs" -> "d",
    "e" -> "east",
    "east" -> "e",
    "i" -> "in",
    "in" -> "i",
    "n" -> "north",
    "ne" -> "northeast",
    "north" -> "n",
    "northeast" -> "ne",
    "northwest" -> "nw",
    "nw" -> "northwest",
    "o" -> "out",
    "out" -> "o",
    "s" -> "south",
    "se" -> "southeast",
    "south" -> "s",
    "southeast" -> "se",
    "southwest" -> "sw",
    "sw" -> "southwest",
    "u" -> "up",
    "up" -> "u",
    "upstairs" -> "u",
    "w" -> "west",
    "west" -> "w"
  ];
  property is_locked (owner: HACKER, flags: "rc") = 0;
  property is_open (owner: HACKER, flags: "rc") = true;
  property lock_msg (owner: HACKER, flags: "rc") = 0;
  property locked_msg (owner: HACKER, flags: "rc") = "";
  property open_msg (owner: HACKER, flags: "rc") = 0;
  property side_a_aliases (owner: HACKER, flags: "rc") = {};
  property side_a_ambient (owner: HACKER, flags: "rc") = true;
  property side_a_arrival_phrase (owner: HACKER, flags: "rc") = "";
  property side_a_arrive_msg (owner: HACKER, flags: "rc") = {};
  property side_a_departure_phrase (owner: HACKER, flags: "rc") = "";
  property side_a_description (owner: HACKER, flags: "rc") = "";
  property side_a_label (owner: HACKER, flags: "rc") = "";
  property side_a_leave_msg (owner: HACKER, flags: "rc") = {};
  property side_a_prose_style (owner: HACKER, flags: "rc") = 'fragment;
  property side_a_room (owner: HACKER, flags: "rc") = #-1;
  property side_b_aliases (owner: HACKER, flags: "rc") = {};
  property side_b_ambient (owner: HACKER, flags: "rc") = true;
  property side_b_arrival_phrase (owner: HACKER, flags: "rc") = "";
  property side_b_arrive_msg (owner: HACKER, flags: "rc") = {};
  property side_b_departure_phrase (owner: HACKER, flags: "rc") = "";
  property side_b_description (owner: HACKER, flags: "rc") = "";
  property side_b_label (owner: HACKER, flags: "rc") = "";
  property side_b_leave_msg (owner: HACKER, flags: "rc") = {};
  property side_b_prose_style (owner: HACKER, flags: "rc") = 'fragment;
  property side_b_room (owner: HACKER, flags: "rc") = #-1;
  property unlock_msg (owner: HACKER, flags: "rc") = 0;
  property unlock_rule (owner: HACKER, flags: "rc") = 0;

  override description (owner: HACKER, flags: "rc") = "Bidirectional passage configuration.";

  method mk owner: HACKER
    {room_a, label_a, aliases_a, description_a, ambient_a, room_b, label_b, aliases_b, description_b, ambient_b, ?is_open = true} = args;
    typeof(room_a) == TYPE_OBJ && typeof(room_b) == TYPE_OBJ || raise(E_TYPE);
    label_a = typeof(label_a) == TYPE_STR ? label_a | "";
    label_b = typeof(label_b) == TYPE_STR ? label_b | "";
    if (typeof(aliases_a) == TYPE_STR)
      aliases_a = aliases_a ? {aliases_a} | {};
    elseif (typeof(aliases_a) != TYPE_LIST)
      aliases_a = {};
    endif
    if (typeof(aliases_b) == TYPE_STR)
      aliases_b = aliases_b ? {aliases_b} | {};
    elseif (typeof(aliases_b) != TYPE_LIST)
      aliases_b = {};
    endif
    description_a = typeof(description_a) == TYPE_STR ? description_a | "";
    description_b = typeof(description_b) == TYPE_STR ? description_b | "";
    ambient_a = ambient_a ? true | false;
    ambient_b = ambient_b ? true | false;
    is_open = is_open ? true | false;
    return <this, .side_a_room = room_a, .side_a_label = label_a, .side_a_aliases = aliases_a, .side_a_description = description_a, .side_a_ambient = ambient_a, .side_a_leave_msg = {}, .side_a_arrive_msg = {}, .side_a_prose_style = 'fragment, .side_a_departure_phrase = "", .side_a_arrival_phrase = "", .side_b_room = room_b, .side_b_label = label_b, .side_b_aliases = aliases_b, .side_b_description = description_b, .side_b_ambient = ambient_b, .side_b_leave_msg = {}, .side_b_arrive_msg = {}, .side_b_prose_style = 'fragment, .side_b_departure_phrase = "", .side_b_arrival_phrase = "", .is_open = is_open>;
  endmethod

  method _value owner: HACKER
    {prop, default} = args;
    typeof(prop) == TYPE_STR || raise(E_TYPE);
    if (typeof(this) == TYPE_OBJ)
      try
        return this.(prop);
      except (E_PROPNF)
        return default;
      endtry
    endif
    if (typeof(this) == TYPE_FLYWEIGHT)
      try
        return this.(prop);
      except (E_PROPNF)
        return default;
      endtry
    endif
    return default;
  endmethod

  method _side_lookup owner: HACKER
    {side, attr} = args;
    prop = "";
    default = 0;
    if (side == 'a)
      if (attr == 'room)
        prop = "side_a_room";
        default = #-1;
      elseif (attr == 'label)
        prop = "side_a_label";
        default = "";
      elseif (attr == 'aliases)
        prop = "side_a_aliases";
        default = {};
      elseif (attr == 'description)
        prop = "side_a_description";
        default = "";
      elseif (attr == 'ambient)
        prop = "side_a_ambient";
        default = false;
      elseif (attr == 'leave_msg)
        prop = "side_a_leave_msg";
        default = {};
      elseif (attr == 'arrive_msg)
        prop = "side_a_arrive_msg";
        default = {};
      elseif (attr == 'prose_style)
        prop = "side_a_prose_style";
        default = 'fragment;
      elseif (attr == 'departure_phrase)
        prop = "side_a_departure_phrase";
        default = "";
      elseif (attr == 'arrival_phrase)
        prop = "side_a_arrival_phrase";
        default = "";
      endif
    elseif (side == 'b)
      if (attr == 'room)
        prop = "side_b_room";
        default = #-1;
      elseif (attr == 'label)
        prop = "side_b_label";
        default = "";
      elseif (attr == 'aliases)
        prop = "side_b_aliases";
        default = {};
      elseif (attr == 'description)
        prop = "side_b_description";
        default = "";
      elseif (attr == 'ambient)
        prop = "side_b_ambient";
        default = false;
      elseif (attr == 'leave_msg)
        prop = "side_b_leave_msg";
        default = {};
      elseif (attr == 'arrive_msg)
        prop = "side_b_arrive_msg";
        default = {};
      elseif (attr == 'prose_style)
        prop = "side_b_prose_style";
        default = 'fragment;
      elseif (attr == 'departure_phrase)
        prop = "side_b_departure_phrase";
        default = "";
      elseif (attr == 'arrival_phrase)
        prop = "side_b_arrival_phrase";
        default = "";
      endif
    endif
    if (!prop)
      return default;
    endif
    return this:_value(prop, default);
  endmethod

  method configure owner: HACKER
    {room_a, label_a, aliases_a, description_a, ambient_a, room_b, label_b, aliases_b, description_b, ambient_b} = args;
    typeof(room_a) == TYPE_OBJ && typeof(room_b) == TYPE_OBJ || raise(E_TYPE);
    this.side_a_room = room_a;
    this.side_a_label = typeof(label_a) == TYPE_STR ? label_a | "";
    if (typeof(aliases_a) == TYPE_LIST)
      this.side_a_aliases = aliases_a;
    elseif (typeof(aliases_a) == TYPE_STR && aliases_a)
      this.side_a_aliases = {aliases_a};
    else
      this.side_a_aliases = {};
    endif
    this.side_a_description = typeof(description_a) == TYPE_STR ? description_a | "";
    this.side_a_ambient = ambient_a ? true | false;
    this.side_b_room = room_b;
    this.side_b_label = typeof(label_b) == TYPE_STR ? label_b | "";
    if (typeof(aliases_b) == TYPE_LIST)
      this.side_b_aliases = aliases_b;
    elseif (typeof(aliases_b) == TYPE_STR && aliases_b)
      this.side_b_aliases = {aliases_b};
    else
      this.side_b_aliases = {};
    endif
    this.side_b_description = typeof(description_b) == TYPE_STR ? description_b | "";
    this.side_b_ambient = ambient_b ? true | false;
    return this;
  endmethod

  method includes owner: HACKER
    {room} = args;
    return room == this:_side_lookup('a, 'room) || room == this:_side_lookup('b, 'room);
  endmethod

  method side_for owner: HACKER
    {room} = args;
    room == this:_side_lookup('a, 'room) && return 'a;
    room == this:_side_lookup('b, 'room) && return 'b;
    return 'none;
  endmethod

  method other_room owner: HACKER
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('b, 'room);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('a, 'room);
    else
      return #-1;
    endif
  endmethod

  method label_for owner: HACKER
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('a, 'label);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('b, 'label);
    else
      return "";
    endif
  endmethod

  method aliases_for owner: HACKER
    {room} = args;
    aliases = {};
    if (room == this:_side_lookup('a, 'room))
      aliases = this:_side_lookup('a, 'aliases);
    elseif (room == this:_side_lookup('b, 'room))
      aliases = this:_side_lookup('b, 'aliases);
    endif
    typeof(aliases) == TYPE_LIST || (aliases = typeof(aliases) == TYPE_STR ? {aliases} | {});
    return aliases;
  endmethod

  method description_for owner: HACKER
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('a, 'description);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('b, 'description);
    else
      return "";
    endif
  endmethod

  method matches_command owner: HACKER
    "Check if command matches this passage's label or aliases from given room.";
    "Also handles direction abbreviations (n/north, e/east, etc).";
    {room, command} = args;
    typeof(command) == TYPE_STR || return false;
    command = command:trim():lowercase();
    !command && return false;
    "Build list of commands to check (original + any expanded direction)";
    commands_to_check = {command};
    if (maphaskey(this.direction_abbrevs, command))
      expanded = this.direction_abbrevs[command];
      if (typeof(expanded) == TYPE_STR)
        expanded = expanded:trim():lowercase();
        expanded && !(expanded in commands_to_check) && (commands_to_check = {@commands_to_check, expanded});
      endif
    endif
    "Check each command variant against aliases and label";
    for cmd in (commands_to_check)
      for alias in (this:aliases_for(room))
        if (typeof(alias) == TYPE_STR && alias:trim():lowercase() == cmd)
          return true;
        endif
      endfor
      label = this:label_for(room);
      if (typeof(label) == TYPE_STR && label:trim():lowercase() == cmd)
        return true;
      endif
    endfor
    return false;
  endmethod

  method travel_from owner: ARCH_WIZARD
    "Traverse a passage using current policies and return command-handled status.";
    const {traveler, from_room, parsed} = args;
    const actor = caller_perms();
    valid(traveler) && valid(actor) || raise(E_INVARG);
    actor == traveler || actor == traveler.owner || actor.wizard || raise(E_PERM);
    valid(traveler) || return false;
    valid(from_room) || return false;
    traveler.location == from_room || return false;
    this:_value("is_open", true) || return this:_notify_blocked(traveler, from_room);
    const access_state = {this.is_locked, this.unlock_rule, traveler.contents};
    const area = from_room.location;
    const registration = valid(area) && isa(area, $area) ? area:passage_link_id(from_room, this:other_room(from_room)) | "";
    this:check_unlock(traveler, from_room) || return this:_notify_locked(traveler, from_room);
    const to_room = this:other_room(from_room);
    valid(to_room) || return false;
    "Get passage metadata";
    const from_label = this:label_for(from_room);
    const to_label = this:label_for(to_room);
    "Notify room contents before departure (e.g., stand up from furniture)";
    `from_room:notify_pre_exit(traveler) ! E_VERBNF => 0';
    "Actually move the traveler before any expensive presentation work.";
    "Pre-exit callbacks may suspend or change the actor's location.";
    traveler.location == from_room || return false;
    "Callbacks may yield; normal travel must not use a replaced passage or outdated access state.";
    if (registration != "")
      valid(area) && from_room.location == area || return false;
      area:passage_link_id(from_room, to_room) == registration && area:passage_for(from_room, to_room) == this || return false;
    endif
    this.is_open && {this.is_locked, this.unlock_rule, traveler.contents} == access_state || return false;
    set_task_perms(actor);
    this:_move_actor(traveler, to_room);
    "Render and announce movement asynchronously after the move commits.";
    fork (0)
      let departure = 0;
      let arrival = 0;
      let desc_lower = "";
      "Render departure event";
      const from_side = from_room == this:_side_lookup('a, 'room) ? 'a | 'b;
      const leave_msg = this:_side_lookup(from_side, 'leave_msg);
      if (typeof(leave_msg) == TYPE_LIST && length(leave_msg) > 0)
        departure = this:movement_event(traveler, from_room, to_room,
          from_label, to_label, leave_msg, false);
      else
        let from_description = this:description_for(from_room);
        const departure_phrase = this:departure_phrase_for(from_room);
        if (!departure_phrase || departure_phrase == "" && typeof(from_description) == TYPE_STR)
          desc_lower = from_description:lowercase();
          if (desc_lower:starts_with("through "))
            from_description = from_description[9..length(from_description)];
          endif
        endif
        departure = `traveler:mk_departure_event(from_room, from_label, from_description, to_room, departure_phrase) ! E_VERBNF => 0';
      endif
      if (typeof(departure) == TYPE_FLYWEIGHT)
        departure = departure:with_audience('narrative);
        `from_room:announce(departure) ! ANY => 0';
      endif
      "Render arrival event";
      const to_side = to_room == this:_side_lookup('a, 'room) ? 'a | 'b;
      const arrive_msg = this:_side_lookup(to_side, 'arrive_msg);
      if (typeof(arrive_msg) == TYPE_LIST && length(arrive_msg) > 0)
        arrival = this:movement_event(traveler, from_room, to_room,
          from_label, to_label, arrive_msg, true);
      else
        let to_description = this:description_for(to_room);
        const arrival_phrase = this:arrival_phrase_for(to_room);
        if (!arrival_phrase || arrival_phrase == "" && typeof(to_description) == TYPE_STR)
          desc_lower = to_description:lowercase();
          if (desc_lower:starts_with("from "))
            to_description = to_description[6..length(to_description)];
          elseif (desc_lower:starts_with("through "))
            to_description = to_description[9..length(to_description)];
          endif
        endif
        arrival = `traveler:mk_arrival_event(to_room, to_label, to_description, from_room, arrival_phrase) ! E_VERBNF => 0';
      endif
      if (typeof(arrival) == TYPE_FLYWEIGHT)
        arrival = arrival:with_audience('narrative);
        `to_room:announce(arrival) ! ANY => 0';
      endif
    endfork
    return true;
  endmethod

  method _move_actor owner: ARCH_WIZARD
    "Privileged final movement step for checked passage traversal.";
    caller == this || (typeof(this) == TYPE_FLYWEIGHT && caller == this.delegate) || raise(E_PERM);
    const {actor, destination} = args;
    valid(actor) && valid(destination) || raise(E_INVARG);
    const principal = caller_perms();
    principal == actor || principal == actor.owner || (valid(principal) && principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    return move(actor, destination);
  endmethod

  method _notify_blocked owner: HACKER
    "Notify player that passage is blocked/closed.";
    {player, from_room} = args;
    close_msg = this:_value("close_msg", "");
    if (typeof(close_msg) == TYPE_STR && close_msg != "")
      message = close_msg;
    else
      door_name = this:door_name_for(from_room);
      message = "The " + door_name + " is closed.";
    endif
    player:inform_current($event:mk_error(player, message));
    return true;
  endmethod

  method expand_direction_aliases owner: HACKER
    "Expand common directions to include standard aliases.";
    {directions} = args;
    result = {};
    for dir in (directions)
      result = {@result, dir};
      "Add abbreviation if it exists";
      if (maphaskey(this.direction_abbrevs, dir))
        abbrev = this.direction_abbrevs[dir];
        if (abbrev && !(abbrev in result))
          result = {@result, abbrev};
        endif
      endif
    endfor
    return result;
  endmethod

  method side_info_for owner: HACKER
    "Get label, description, ambient flag, and prose_style for a given room side.";
    "Returns [label, description, ambient, prose_style] or empty list if room not found.";
    {room} = args;
    label = this:label_for(room);
    description = this:description_for(room);
    side = this:side_for(room);
    if (side == 'none)
      return {};
    endif
    ambient = this:_side_lookup(side, 'ambient);
    prose_style = this:_side_lookup(side, 'prose_style);
    return {label, description, ambient, prose_style};
  endmethod

  method _extract_all owner: HACKER
    "Extract all passage properties into a map. Internal helper for transformer verbs.";
    return ['room_a -> this:_side_lookup('a, 'room), 'room_b -> this:_side_lookup('b, 'room), 'label_a -> this:_side_lookup('a, 'label), 'label_b -> this:_side_lookup('b, 'label), 'aliases_a -> this:_side_lookup('a, 'aliases), 'aliases_b -> this:_side_lookup('b, 'aliases), 'desc_a -> this:_side_lookup('a, 'description), 'desc_b -> this:_side_lookup('b, 'description), 'ambient_a -> this:_side_lookup('a, 'ambient), 'ambient_b -> this:_side_lookup('b, 'ambient), 'leave_msg_a -> this:_side_lookup('a, 'leave_msg), 'leave_msg_b -> this:_side_lookup('b, 'leave_msg), 'arrive_msg_a -> this:_side_lookup('a, 'arrive_msg), 'arrive_msg_b -> this:_side_lookup('b, 'arrive_msg), 'prose_style_a -> this:_side_lookup('a, 'prose_style), 'prose_style_b -> this:_side_lookup('b, 'prose_style), 'departure_phrase_a -> this:_side_lookup('a, 'departure_phrase), 'departure_phrase_b -> this:_side_lookup('b, 'departure_phrase), 'arrival_phrase_a -> this:_side_lookup('a, 'arrival_phrase), 'arrival_phrase_b -> this:_side_lookup('b, 'arrival_phrase), 'is_open -> this:_value("is_open", true), 'is_locked -> this:_value("is_locked", false), 'is_door -> this:_value("is_door", false), 'unlock_rule -> this:_value("unlock_rule", 0), 'locked_msg -> this:_value("locked_msg", "")];
  endmethod

  method with_label_from owner: HACKER
    "Update the label visible from a given room. Returns new passage flyweight.";
    {room, label} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    typeof(label) == TYPE_STR || raise(E_TYPE);
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['label_a] = label;
    else
      props['label_b] = label;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_description_from owner: HACKER
    "Update the description visible from a given room. Returns new passage flyweight.";
    {room, description} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    "Allow strings or precompiled lists (with $sub flyweights)";
    if (typeof(description) == TYPE_STR && ("{" in description || "}" in description))
      try
        description = $sub_utils:compile(description);
      except e (ANY)
        message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
        raise(E_INVARG, "Description template compilation failed: " + message);
      endtry
    elseif (typeof(description) != TYPE_STR && typeof(description) != TYPE_LIST)
      raise(E_TYPE);
    endif
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['desc_a] = description;
    else
      props['desc_b] = description;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_aliases_from owner: HACKER
    "Update the aliases visible from a given room. Accepts string or list. Returns new passage flyweight.";
    {room, aliases} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    "Normalize aliases (same as mk does)";
    if (typeof(aliases) == TYPE_STR)
      aliases = aliases ? {aliases} | {};
    elseif (typeof(aliases) != TYPE_LIST)
      aliases = {};
    endif
    props = this:_extract_all();
    if (side == 'a)
      props['aliases_a] = aliases;
    else
      props['aliases_b] = aliases;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_ambient_from owner: HACKER
    "Update the ambient flag for the side visible from a given room. Returns new passage flyweight.";
    {room, is_ambient} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['ambient_a] = is_ambient ? true | false;
    else
      props['ambient_b] = is_ambient ? true | false;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_open owner: HACKER
    "Update whether the passage is open/traversable. Returns new passage flyweight.";
    {is_open} = args;
    props = this:_extract_all();
    props['is_open] = is_open ? true | false;
    return this:_mk_from_props(props);
  endmethod

  method movement_event owner: ARCH_WIZARD
    "Capture a passage message template and its movement facts without resolving it for the traveler.";
    set_task_perms(caller_perms());
    const {actor, from_room, to_room, from_direction, to_direction, parts, ?arriving = false} = args;
    const location = arriving ? to_room | from_room;
    const direction = arriving ? to_direction | from_direction;
    return $event:mk_move(actor, @parts):with_location(location):with_this(location)
      :with_object_binding('from_room, from_room):with_object_binding('to_room, to_room)
      :with_text_binding('direction, direction):with_text_binding('from_direction, from_direction)
      :with_text_binding('to_direction, to_direction):with_text_binding('passage, this:name());
  endmethod

  method prose_style_for owner: HACKER
    "Get the prose style for the side facing a given room.";
    "Returns 'sentence (include as-is) or 'fragment (wrap in 'You see X').";
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('a, 'prose_style);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('b, 'prose_style);
    else
      return 'fragment;
    endif
  endmethod

  method departure_phrase_for owner: HACKER
    "Get the departure phrase for the side facing a given room.";
    "Used in messages like 'heads [direction] through [phrase]'.";
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('a, 'departure_phrase);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('b, 'departure_phrase);
    else
      return "";
    endif
  endmethod

  method arrival_phrase_for owner: HACKER
    "Get the arrival phrase for the side facing a given room.";
    "Used in messages like 'arrives from [phrase]'.";
    {room} = args;
    if (room == this:_side_lookup('a, 'room))
      return this:_side_lookup('a, 'arrival_phrase);
    elseif (room == this:_side_lookup('b, 'room))
      return this:_side_lookup('b, 'arrival_phrase);
    else
      return "";
    endif
  endmethod

  method _mk_from_props owner: HACKER
    "Construct a passage flyweight from a props map. Internal helper for transformer verbs.";
    {props} = args;
    return <$passage, .side_a_room = props['room_a], .side_a_label = props['label_a], .side_a_aliases = props['aliases_a], .side_a_description = props['desc_a], .side_a_ambient = props['ambient_a], .side_a_leave_msg = props['leave_msg_a], .side_a_arrive_msg = props['arrive_msg_a], .side_a_prose_style = props['prose_style_a], .side_a_departure_phrase = props['departure_phrase_a], .side_a_arrival_phrase = props['arrival_phrase_a], .side_b_room = props['room_b], .side_b_label = props['label_b], .side_b_aliases = props['aliases_b], .side_b_description = props['desc_b], .side_b_ambient = props['ambient_b], .side_b_leave_msg = props['leave_msg_b], .side_b_arrive_msg = props['arrive_msg_b], .side_b_prose_style = props['prose_style_b], .side_b_departure_phrase = props['departure_phrase_b], .side_b_arrival_phrase = props['arrival_phrase_b], .is_open = props['is_open], .is_locked = props['is_locked], .is_door = props['is_door], .unlock_rule = props['unlock_rule], .locked_msg = props['locked_msg]>;
  endmethod

  method with_prose_style_from owner: HACKER
    "Update the prose style for the side visible from a given room. Returns new passage flyweight.";
    "'sentence means include description as-is; 'fragment means wrap in 'You see X'.";
    {room, style} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    style in {'sentence, 'fragment} || raise(E_INVARG, "prose_style must be 'sentence or 'fragment");
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['prose_style_a] = style;
    else
      props['prose_style_b] = style;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_departure_phrase_from owner: HACKER
    "Update the departure phrase for the side visible from a given room. Returns new passage flyweight.";
    "Used in messages like 'heads [direction] through [phrase]'.";
    {room, phrase} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    typeof(phrase) == TYPE_STR || raise(E_TYPE);
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['departure_phrase_a] = phrase;
    else
      props['departure_phrase_b] = phrase;
    endif
    return this:_mk_from_props(props);
  endmethod

  method with_arrival_phrase_from owner: HACKER
    "Update the arrival phrase for the side visible from a given room. Returns new passage flyweight.";
    "Used in messages like 'arrives from [phrase]'.";
    {room, phrase} = args;
    typeof(room) == TYPE_OBJ || raise(E_TYPE);
    typeof(phrase) == TYPE_STR || raise(E_TYPE);
    side = this:side_for(room);
    side == 'none && raise(E_INVARG, "Room not connected by this passage");
    props = this:_extract_all();
    if (side == 'a)
      props['arrival_phrase_a] = phrase;
    else
      props['arrival_phrase_b] = phrase;
    endif
    return this:_mk_from_props(props);
  endmethod

  verb with_locked (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Update whether the passage is locked. Returns new passage flyweight.";
    {is_locked} = args;
    props = this:_extract_all();
    props['is_locked] = is_locked ? true | false;
    return this:_mk_from_props(props);
  endverb

  verb with_unlock_rule (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the unlock rule for the passage. Pass a compiled rule or 0 to clear.";
    {rule} = args;
    props = this:_extract_all();
    props['unlock_rule] = rule;
    return this:_mk_from_props(props);
  endverb

  verb with_locked_msg (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the message shown when passage is locked.";
    {msg} = args;
    props = this:_extract_all();
    props['locked_msg] = msg;
    return this:_mk_from_props(props);
  endverb

  method check_unlock owner: ARCH_WIZARD
    "Check a locked passage's policy with explicit From and To rooms and optional carried Key.";
    set_task_perms(caller_perms());
    const {who, from_room} = args;
    !this:_value("is_locked", false) && return true;
    const rule = this:_value("unlock_rule", 0);
    rule == 0 && return false;
    const to_room = this:other_room(from_room);
    valid(to_room) || return false;
    let inputs = ['Accessor -> who, 'From -> from_room, 'To -> to_room, 'Passage -> this];
    const references = $condition:references(rule.condition);
    if (!('Key in references['input]))
      return $rule_engine:evaluate(rule, inputs)['success];
    endif
    for key in (who.contents)
      inputs['Key] = key;
      $rule_engine:evaluate(rule, inputs)['success] && return true;
    endfor
    return false;
  endmethod

  verb _notify_locked (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Notify player that passage is locked.";
    {player, from_room} = args;
    locked_msg = this:_value("locked_msg", "");
    if (locked_msg != "")
      message = locked_msg;
    else
      door_name = this:door_name_for(from_room);
      message = "The " + door_name + " is locked.";
    endif
    player:inform_current($event:mk_error(player, message));
    return true;
  endverb

  verb with_open_msg (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the message shown when passage is opened.";
    {msg} = args;
    props = this:_extract_all();
    props['open_msg] = msg;
    return this:_mk_from_props(props);
  endverb

  verb with_close_msg (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the message shown when passage is closed.";
    {msg} = args;
    props = this:_extract_all();
    props['close_msg] = msg;
    return this:_mk_from_props(props);
  endverb

  verb with_lock_msg (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the message shown when passage is locked.";
    {msg} = args;
    props = this:_extract_all();
    props['lock_msg] = msg;
    return this:_mk_from_props(props);
  endverb

  verb with_unlock_msg (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set the message shown when passage is unlocked.";
    {msg} = args;
    props = this:_extract_all();
    props['unlock_msg] = msg;
    return this:_mk_from_props(props);
  endverb

  method door_name_for owner: ARCH_WIZARD
    "Return a display name for this passage when used as a door.";
    "If the passage has open/locked state, it's a door - return 'door'.";
    "Otherwise return the directional label.";
    {from_room} = args;
    "Check if this passage acts as a door (has is_open property)";
    is_door = this:_value("is_open", "not_set") != "not_set";
    if (is_door)
      return "door";
    endif
    "Not a door, use the directional label";
    return this:label_for(from_room);
  endmethod

  method test_with_description_from_reports_template_errors owner: HACKER
    "Unit test: passage description updates reject malformed substitution templates.";
    passage = this:mk($room, "east", {"east"}, "", true, $room, "west", {"west"}, "", true);
    $test_utils:assert_raises(E_INVARG, passage, "with_description_from", {$room, "broken {template"}, "with_description_from should reject malformed templates");
    updated = passage:with_description_from($room, "{the direction}");
    $test_utils:assert_type(updated.side_a_description, TYPE_LIST, "valid substitution descriptions should be compiled");
    return true;
  endmethod
endobject
