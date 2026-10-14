object PASSAGE [
  import_export_id -> "passage",
  import_export_hierarchy -> {"world"}
]
  name: "Generic Passage"
  parent: ROOT
  owner: HACKER
  fertile: true
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
  property close_msg (owner: HACKER, flags: "rc") = 0;
  property connection_id (owner: HACKER, flags: "rc") = "";
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
  property is_door (owner: HACKER, flags: "rc") = false;
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
  property side_a_description_kind (owner: HACKER, flags: "rc") = 'literal;
  property side_a_enabled (owner: HACKER, flags: "rc") = true;
  property side_a_label (owner: HACKER, flags: "rc") = "";
  property side_a_leave_msg (owner: HACKER, flags: "rc") = {};
  property side_a_prose_style (owner: HACKER, flags: "rc") = 'sentence;
  property side_a_room (owner: HACKER, flags: "rc") = #-1;
  property side_b_aliases (owner: HACKER, flags: "rc") = {};
  property side_b_ambient (owner: HACKER, flags: "rc") = true;
  property side_b_arrival_phrase (owner: HACKER, flags: "rc") = "";
  property side_b_arrive_msg (owner: HACKER, flags: "rc") = {};
  property side_b_departure_phrase (owner: HACKER, flags: "rc") = "";
  property side_b_description (owner: HACKER, flags: "rc") = "";
  property side_b_description_kind (owner: HACKER, flags: "rc") = 'literal;
  property side_b_enabled (owner: HACKER, flags: "rc") = true;
  property side_b_label (owner: HACKER, flags: "rc") = "";
  property side_b_leave_msg (owner: HACKER, flags: "rc") = {};
  property side_b_prose_style (owner: HACKER, flags: "rc") = 'sentence;
  property side_b_room (owner: HACKER, flags: "rc") = #-1;
  property unlock_msg (owner: HACKER, flags: "rc") = 0;
  property unlock_rule (owner: HACKER, flags: "rc") = 0;

  override description (owner: HACKER, flags: "rc") = "Bidirectional passage configuration.";

  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Passage descriptions",
    "A passage connects two rooms. Each side has its own name, aliases, description, and movement wording.",
    "Use @describe east as A brass door stands beside the fireplace. to describe your east exit. You need permission to edit that side.",
    "@describe --literal east as TEXT treats punctuation and braces as text. --djot selects inline Djot. --template selects message substitutions.",
    "For example: @describe --template east as An arch leads {direction}. The direction binding contains this side's exit name.",
    "Template bindings also include the reader as actor, this room as this_obj and from_room, and the destination as to_room.",
    "Without a format flag, edits preserve the current syntax. New passages use literal text.",
    "@edit-description east opens the source editor. A removed connection, changed description, or lost editing permission prevents a stale save.",
    "@passage east shows the source and previews the room prose, departure message, and arrival message without moving anyone.",
    "Sentence descriptions retain their spelling and punctuation. @set-passage east style fragment joins a short phrase into a You see sentence.",
    "The complete ambient phrase links to its exit. Explicit document references can link a selected phrase instead. Direction names remain listed separately.",
    "@set-passage east departure through the brass door controls departure wording. The arrival setting describes arrival on this side.",
    "@set-passage east access key is \"brass key\" sets a readable access condition. The door must be locked for access to restrict passage travel.",
    "@set-passage east access clear removes the condition and unlocks the passage. Shared changes require authority at both ends.",
    "Use @set-passage east open_msg TEMPLATE (or close_msg, lock_msg, unlock_msg) for door messages; clear restores the standard message.",
    "Ambient descriptions do not supply movement wording. Use the dedicated phrases or side_a_leave_msg, side_b_leave_msg, side_a_arrive_msg, and side_b_arrive_msg.",
    "For MOO code, with_description_from(room, source, kind) validates and retains source. Kinds are literal, djot_inline, and template. Omit kind to preserve it.",
    "Literal mode also accepts explicit inline document content and compiled substitution lists. mk preserves these values; it does not turn them into empty text."
  };

  method allows_direction owner: HACKER
    "Whether a passage permits movement from this endpoint, independently of door state.";
    const {passage, room} = args;
    if (room == passage.side_a_room)
      return passage.side_a_enabled;
    elseif (room == passage.side_b_room)
      return passage.side_b_enabled;
    endif
    return false;
  endmethod

  method command_names owner: HACKER
    "Return validated, nonempty names for one side of a passage.";
    const {passage, room} = args;
    const prefix = room == passage.side_a_room ? "side_a_" | "side_b_";
    const label = passage.(prefix + "label");
    const aliases = passage.(prefix + "aliases");
    typeof(label) == TYPE_STR && typeof(aliases) == TYPE_LIST || raise(E_TYPE);
    let names = label != "" ? {label:trim()} | {};
    for alias in (aliases)
      typeof(alias) == TYPE_STR && alias:trim() != "" || raise(E_INVARG, "Exit names must be nonempty text.");
      names = setadd(names, alias:trim());
    endfor
    return names;
  endmethod

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"open_msg", "close_msg", "lock_msg", "unlock_msg"})
      let bindings = $sub_utils:standard_bindings("The person operating the door.");
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room where the person stands.",
        'example -> 'location];
      bindings['direction] = ['kind -> 'text, 'description -> "The exit name on that side.",
        'example -> "east"];
      bindings['passage] = ['kind -> 'text, 'description -> "The door name on that side.",
        'example -> "east door"];
      return bindings;
    endif
    if (tostr(property) in {"side_a_arrive_msg", "side_b_arrive_msg", "side_a_leave_msg", "side_b_leave_msg"})
      let bindings = $sub_utils:standard_bindings("The person travelling.");
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room receiving this message.",
        'example -> 'location];
      bindings['from_room] = ['kind -> 'object, 'description -> "The room the traveller leaves.",
        'example -> 'from_room];
      bindings['to_room] = ['kind -> 'object, 'description -> "The room the traveller enters.",
        'example -> 'to_room];
      bindings['direction] = ['kind -> 'text, 'description -> "The direction seen from this side.",
        'example -> "north"];
      bindings['from_direction] = ['kind -> 'text, 'description -> "The departure direction.",
        'example -> "north"];
      bindings['to_direction] = ['kind -> 'text, 'description -> "The arrival direction.",
        'example -> "south"];
      bindings['passage] = ['kind -> 'text, 'description -> "The passage name.",
        'example -> "doorway"];
      return bindings;
    endif
    return pass(@args);
  endmethod

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
    $passage:authored_description(description_a, 'literal);
    $passage:authored_description(description_b, 'literal);
    ambient_a = ambient_a ? true | false;
    ambient_b = ambient_b ? true | false;
    is_open = is_open ? true | false;
    const enabled_a = label_a != "" || length(aliases_a) > 0;
    const enabled_b = label_b != "" || length(aliases_b) > 0;
    return <this, .side_a_enabled = enabled_a, .side_b_enabled = enabled_b, .side_a_room = room_a, .side_a_label = label_a, .side_a_aliases = aliases_a, .side_a_description = description_a, .side_a_ambient = ambient_a, .side_a_leave_msg = {}, .side_a_arrive_msg = {}, .side_a_prose_style = 'sentence, .side_a_departure_phrase = "", .side_a_arrival_phrase = "", .side_b_room = room_b, .side_b_label = label_b, .side_b_aliases = aliases_b, .side_b_description = description_b, .side_b_ambient = ambient_b, .side_b_leave_msg = {}, .side_b_arrive_msg = {}, .side_b_prose_style = 'sentence, .side_b_departure_phrase = "", .side_b_arrival_phrase = "", .is_open = is_open>;
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

  method configure owner: ARCH_WIZARD
    "Configure a value, or an owned passage object, after validating both descriptions.";
    const principal = caller_perms();
    if (typeof(this) == TYPE_OBJ)
      principal == this.owner || (valid(principal) && principal.wizard) || raise(E_PERM);
    endif
    set_task_perms(principal);
    const fields = flyslots($passage:mk(@args));
    for name in (mapkeys(fields))
      this.(name) = fields[name];
    endfor
    this.side_a_description_kind = 'literal;
    this.side_b_description_kind = 'literal;
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
    "Traverse a registered connection using current state; return whether movement completed.";
    const {traveler, from_room, parsed} = args;
    const actor = caller_perms();
    valid(traveler) && valid(actor) || raise(E_INVARG);
    $root:check_actor(traveler, actor);
    set_task_perms(actor);
    valid(traveler) || return false;
    valid(from_room) || return false;
    traveler.location == from_room || return false;
    const registration = $room:find_connection(from_room, this.connection_id);
    length(registration) || return false;
    const current = registration[2];
    current.side_a_room == this.side_a_room && current.side_b_room == this.side_b_room || return false;
    this = current;
    $passage:allows_direction(this, from_room) || return false;
    this:_value("is_open", true) || return this:_notify_blocked(traveler, from_room);
    const access_state = {this.is_locked, this.unlock_rule, traveler.contents};
    this:check_unlock(traveler, from_room) || return this:_notify_locked(traveler, from_room);
    const to_room = this:other_room(from_room);
    valid(to_room) || return false;
    "Notify room contents before departure (e.g., stand up from furniture)";
    `from_room:notify_pre_exit(traveler) ! E_VERBNF => 0';
    "Actually move the traveler before any expensive presentation work.";
    "Pre-exit callbacks may suspend or change the actor's location.";
    traveler.location == from_room || return false;
    "A hook can change rule facts without changing the passage or inventory.";
    this:check_unlock(traveler, from_room) || return this:_notify_locked(traveler, from_room);
    $root:check_actor(traveler, actor);
    "Callbacks may yield; normal travel must not use a replaced passage or outdated access state.";
    const after_callback = $room:find_connection(from_room, this.connection_id);
    length(after_callback) && equal(after_callback[2], this) || return false;
    this.is_open && {this.is_locked, this.unlock_rule, traveler.contents} == access_state || return false;
    set_task_perms(actor);
    this:_move_actor(traveler, to_room);
    "Native movement callbacks can redirect or recycle the traveler.";
    valid(traveler) && traveler.location == to_room || return false;
    "Render and announce movement asynchronously after the move commits.";
    fork (0)
      const departure = $passage:movement_from(this, traveler, from_room, to_room, false):with_audience('narrative);
      const arrival = $passage:movement_from(this, traveler, from_room, to_room, true):with_audience('narrative);
      from_room:announce(departure);
      to_room:announce(arrival);
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
    if (respond_to(player, 'inform_current))
      player:inform_current($event:mk_error(player, message));
    endif
    return false;
  endmethod

  method parse_directions owner: HACKER
    "Parse command or tool directions with the same reverse inference and one-way rules.";
    const {outward, ?inward = "", ?oneway = false} = args;
    typeof(outward) == TYPE_STR && typeof(inward) == TYPE_STR || raise(E_TYPE);
    typeof(oneway) == TYPE_BOOL || raise(E_TYPE);
    let source = outward;
    let reverse = inward;
    if (index(source, "|"))
      reverse == "" || raise(E_INVARG, "Supply the return direction only once.");
      const parts = source:split("|");
      length(parts) == 2 || raise(E_INVARG, "Direction spec must be 'dir' or 'dir|returndir'.");
      source = parts[1];
      reverse = parts[2];
    endif
    const names = {name:trim() for name in (strsub(source, ":", ","):split(","))
      if (name:trim() != "")};
    length(names) || raise(E_INVARG, "Missing direction aliases.");
    let returns = {reverse_name:trim() for reverse_name in (strsub(reverse, ":", ","):split(","))
      if (reverse_name:trim() != "")};
    if (oneway)
      !length(returns) || raise(E_INVARG, "A one-way passage has no return direction.");
    elseif (!length(returns))
      for name in (names)
        maphaskey($passage.direction_opposites, name)
          || raise(E_INVARG, "Can't infer opposite direction for '" + source + "'. Use 'dir|returndir' syntax.");
        returns = {@returns, $passage.direction_opposites[name]};
      endfor
    endif
    return ['oneway -> oneway, 'from_dir -> $passage:expand_direction_aliases(names),
      'to_dir -> $passage:expand_direction_aliases(returns)];
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

  method configuration owner: ARCH_WIZARD
    "Capture supported fields and extension slots without invoking passage overrides.";
    set_task_perms(caller_perms());
    const {passage} = args;
    let fields = typeof(passage) == TYPE_FLYWEIGHT ? flyslots(passage) | [];
    for name in ({"connection_id", "is_open", "is_locked", "is_door", "unlock_rule", "locked_msg",
        "open_msg", "close_msg", "lock_msg", "unlock_msg"})
      fields[tosym(name)] = passage.(name);
    endfor
    for side in ({"a", "b"})
      for attribute in ({"room", "enabled", "label", "aliases", "description", "description_kind", "ambient", "leave_msg",
          "arrive_msg", "prose_style", "departure_phrase", "arrival_phrase"})
        const name = "side_" + side + "_" + attribute;
        fields[tosym(name)] = passage.(name);
      endfor
    endfor
    return fields;
  endmethod

  method _extract_all owner: HACKER
    "Capture a value for editing, preserving its delegate, contents, and extension slots.";
    const fields = $passage:configuration(this);
    let props = ['prototype -> typeof(this) == TYPE_FLYWEIGHT ? this.delegate | this,
      'fields -> fields, 'contents -> typeof(this) == TYPE_FLYWEIGHT ? flycontents(this) | {}];
    for name in ({'is_open, 'is_locked, 'is_door, 'unlock_rule, 'locked_msg,
        'open_msg, 'close_msg, 'lock_msg, 'unlock_msg})
      props[name] = fields[name];
    endfor
    for side in ({"a", "b"})
      for attribute in ({"room", "label", "aliases", "description", "description_kind", "ambient", "leave_msg",
          "arrive_msg", "prose_style", "departure_phrase", "arrival_phrase"})
        const key = (attribute == "description" ? "desc" | attribute) + "_" + side;
        props[tosym(key)] = fields[tosym("side_" + side + "_" + attribute)];
      endfor
    endfor
    return props;
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

  method description_kind_for owner: HACKER
    "Return the selected source format on one connected side.";
    const {room} = args;
    room in {this.side_a_room, this.side_b_room} || raise(E_INVARG, "Room is not connected.");
    return room == this.side_a_room ? this.side_a_description_kind | this.side_b_description_kind;
  endmethod

  method description_bindings owner: HACKER
    "Declare the reader, endpoint rooms, and exit name available to passage descriptions.";
    let bindings = $sub_utils:standard_bindings("The person looking at the exit.");
    for name in ({'this_obj, 'dobj, 'from_room})
      bindings[name] = ['kind -> 'object, 'description -> "The room being described.", 'example -> 'location];
    endfor
    bindings['to_room] = ['kind -> 'object, 'description -> "The room reached through this exit.",
      'example -> 'to_room];
    for name in ({'direction, 'passage})
      bindings[name] = ['kind -> 'text, 'description -> "The exit name on this side.", 'example -> "east"];
    endfor
    return bindings;
  endmethod

  method authored_description owner: HACKER
    "Validate explicitly selected passage source and return inline content. Never infer templates.";
    const {source, kind} = args;
    kind in {'literal, 'djot_inline, 'template}
      || raise(E_INVARG, "Passage descriptions use literal, djot_inline, or template syntax.");
    if (kind == 'template)
      return $sub_utils:compile(source, $passage:description_bindings());
    elseif (kind == 'djot_inline)
      return $format.djot:inline(source);
    endif
    return $format:inlines({source});
  endmethod

  method description_content owner: ARCH_WIZARD
    "Read one side's description as inline content under the caller's authority.";
    set_task_perms(caller_perms());
    const {passage, room} = args;
    room in {passage.side_a_room, passage.side_b_room} || raise(E_INVARG, "Room is not connected.");
    const prefix = room == passage.side_a_room ? "side_a_" | "side_b_";
    return $passage:authored_description(passage.(prefix + "description"), passage.(prefix + "description_kind"));
  endmethod

  method with_description_from owner: HACKER
    "Validate and retain one side's source. An omitted format preserves the side's current choice.";
    const {room, description, ?kind = this:description_kind_for(room)} = args;
    room in {this.side_a_room, this.side_b_room} || raise(E_INVARG, "Room is not connected.");
    $passage:authored_description(description, kind);
    let props = this:_extract_all();
    const side = room == this.side_a_room ? "a" | "b";
    props[tosym("desc_" + side)] = description;
    props[tosym("description_kind_" + side)] = kind;
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

  method movement_from owner: ARCH_WIZARD
    "Build actual or preview movement wording from dedicated messages and phrases, never ambient prose.";
    set_task_perms(caller_perms());
    const {passage, actor, from_room, to_room, ?arriving = false} = args;
    const source_side = from_room == passage.side_a_room ? "side_a_" | "side_b_";
    const target_side = to_room == passage.side_a_room ? "side_a_" | "side_b_";
    const from_label = passage.(source_side + "label");
    const to_label = passage.(target_side + "label");
    const value = passage.((arriving ? target_side | source_side) + (arriving ? "arrive_msg" | "leave_msg"));
    const parts = typeof(value) == TYPE_LIST ? value | (typeof(value) == TYPE_FLYWEIGHT
      || (typeof(value) == TYPE_STR && value != "") ? {value} | {});
    if (length(parts))
      return passage:movement_event(actor, from_room, to_room, from_label, to_label, parts, arriving);
    endif
    if (arriving)
      return actor:mk_arrival_event(to_room, to_label, "", from_room, passage.(target_side + "arrival_phrase"));
    endif
    return actor:mk_departure_event(from_room, from_label, "", to_room, passage.(source_side + "departure_phrase"));
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
    "Rebuild an edited passage without dropping messages or custom behavior.";
    const {props} = args;
    let fields = props['fields];
    for name in ({'is_open, 'is_locked, 'is_door, 'unlock_rule, 'locked_msg,
        'open_msg, 'close_msg, 'lock_msg, 'unlock_msg})
      fields[name] = props[name];
    endfor
    for side in ({"a", "b"})
      for attribute in ({"room", "label", "aliases", "description", "description_kind", "ambient", "leave_msg",
          "arrive_msg", "prose_style", "departure_phrase", "arrival_phrase"})
        const key = (attribute == "description" ? "desc" | attribute) + "_" + side;
        fields[tosym("side_" + side + "_" + attribute)] = props[tosym(key)];
      endfor
    endfor
    return toflyweight(props['prototype], fields, props['contents]);
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
    "Check traversal with the same policy inputs and carried-key rules as door operations.";
    set_task_perms(caller_perms());
    const {who, from_room} = args;
    !this.is_locked && return true;
    return $passage:policy_access(this, who, from_room)['allowed];
  endmethod

  method policy_access owner: ARCH_WIZARD
    "Evaluate the passage rule once per eligible key, or once without a key when it is unused.";
    set_task_perms(caller_perms());
    const {passage, who, from_room, ?key = #-1} = args;
    const denied = ['allowed -> false, 'key -> #-1];
    const rule = passage.unlock_rule;
    typeof(rule) == TYPE_FLYWEIGHT || return denied;
    if (key != #-1 && (!valid(key) || key.location != who))
      return denied;
    endif
    const destination = from_room == passage.side_a_room ? passage.side_b_room | passage.side_a_room;
    let inputs = ['Accessor -> who, 'From -> from_room, 'To -> destination, 'Passage -> passage];
    const references = $condition:references(rule.condition);
    if (!('Key in references['input]))
      return ['allowed -> $rule_engine:evaluate(rule, inputs)['success], 'key -> key];
    endif
    const candidates = key == #-1 ? who.contents | {key};
    for candidate in (candidates)
      if (!valid(candidate) || candidate.location != who)
        continue;
      endif
      inputs['Key] = candidate;
      if ($rule_engine:evaluate(rule, inputs)['success])
        return ['allowed -> valid(candidate) && candidate.location == who, 'key -> candidate];
      endif
    endfor
    return denied;
  endmethod

  method _door_access owner: ARCH_WIZARD
    "Run rule providers with the initiating principal's authority, not mutation authority.";
    const source = callers()[1];
    source[4] == $passage && source[2] == "operate" || raise(E_PERM);
    const {passage, who, from_room, key, principal} = args;
    set_task_perms(principal);
    return $passage:policy_access(passage, who, from_room, key);
  endmethod

  method operate owner: ARCH_WIZARD
    "Open, close, lock, or unlock one installed connection for an authenticated local actor.";
    const principal = caller_perms();
    const {requested, operation, who, from_room, ?key = #-1, ?reaction_context = []} = args;
    $root:check_actor(who, principal);
    operation in {'open, 'close, 'lock, 'unlock} || raise(E_INVARG, "Unknown door operation.");
    const denied = {reason} => ['status -> 'denied, 'reason -> {reason}];
    who.location == from_room || return denied("You must be beside that door.");
    const found = $room:find_connection(from_room, requested.connection_id);
    length(found) || return denied("That passage is no longer connected.");
    const {area, passage} = found;
    (passage.side_a_room == requested.side_a_room && passage.side_b_room == requested.side_b_room)
      || return denied("That passage leads somewhere else now.");
    $passage:allows_direction(passage, from_room) || return denied("There is no exit on this side.");
    (passage.is_door || typeof(passage.unlock_rule) == TYPE_FLYWEIGHT)
      || return denied("That exit is not a door.");
    const label = $passage:door_label(passage, from_room);
    const state = operation == 'open ? passage.is_open | (operation == 'close ? !passage.is_open
      | (operation == 'lock ? passage.is_locked | !passage.is_locked));
    const state_name = ['open -> "open", 'close -> "closed", 'lock -> "locked", 'unlock -> "unlocked"][operation];
    state && return denied("The " + label + " is already " + state_name + ".");
    if (key != #-1 && (!valid(key) || key.location != who))
      return denied("You are not carrying that key.");
    endif
    const custody = {who.owner, who.location, who.contents, from_room.owner,
      passage.side_a_room.owner, passage.side_b_room.owner, area.owner};
    const needs_access = operation in {'lock, 'unlock} || (operation == 'open && passage.is_locked);
    let access = ['allowed -> true, 'key -> key];
    if (needs_access)
      access = $passage:_door_access(passage, who, from_room, key, principal);
      access['allowed] || return denied(key == #-1 ? "You cannot " + tostr(operation) + " the " + label + "."
        | "That key does not permit this operation.");
    endif
    $root:check_actor(who, principal);
    const current = $room:find_connection(from_room, passage.connection_id);
    length(current) && equal(current[2], passage)
      || return denied("The passage changed. Try again.");
    equal(custody, {who.owner, who.location, who.contents, from_room.owner,
      passage.side_a_room.owner, passage.side_b_room.owner, area.owner})
      || return denied("The room, actor, or carried items changed. Try again.");
    const used_key = access['key];
    if (used_key != #-1 && (!valid(used_key) || used_key.location != who))
      return denied("You are no longer carrying that key.");
    endif
    let updated = passage;
    if (operation == 'open)
      updated.is_open = true;
      updated.is_locked = false;
    elseif (operation == 'close)
      updated.is_open = false;
    elseif (operation == 'lock)
      updated.is_open = false;
      updated.is_locked = true;
    else
      updated.is_locked = false;
    endif
    updated = area:_do_create_passage(passage.side_a_room, passage.side_b_room, updated, passage, true);
    const destination = from_room == passage.side_a_room ? passage.side_b_room | passage.side_a_room;
    let context = ['Actor -> who, 'From -> from_room, 'To -> destination, 'Passage -> updated,
      'Connection -> updated.connection_id, 'Key -> used_key,
      '_reaction_chain -> `reaction_context['_reaction_chain] ! E_RANGE => {}'];
    for room in ({from_room, destination})
      context['Location] = room;
      context['Direction] = room == passage.side_a_room ? passage.side_a_label | passage.side_b_label;
      $reaction:_dispatch(room, tosym("on_passage_" + tostr(operation)), context);
    endfor
    set_task_perms(principal);
    const custom = updated.(tostr(operation) + "_msg");
    let parts = typeof(custom) == TYPE_LIST ? custom | (typeof(custom) == TYPE_FLYWEIGHT
      || (typeof(custom) == TYPE_STR && custom != "") ? {custom} | {});
    if (!length(parts))
      const action = operation == 'open && passage.is_locked ? "unlock and open" | tostr(operation);
      const other_action = operation == 'open && passage.is_locked ? "unlocks and opens" | tostr(operation) + "s";
      parts = {<$sub, .type = 'actor, .capitalize = true>, " ",
        <$sub, .type = 'self_alt, .for_self = action, .for_others = other_action>, " the ", label, "."};
    endif
    const event = $event:mk_info(who, @parts):with_this(from_room)
      :with_text_binding('direction, from_room == passage.side_a_room ? passage.side_a_label | passage.side_b_label)
      :with_text_binding('passage, label);
    from_room:announce(event);
    return ['status -> 'applied, 'reason -> {}, 'passage -> updated];
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
    if (respond_to(player, 'inform_current))
      player:inform_current($event:mk_error(player, message));
    endif
    return false;
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

  method door_name_for owner: HACKER
    "Name the selected side so feedback distinguishes multiple doors.";
    const {from_room} = args;
    return $passage:door_label(this, from_room);
  endmethod

  method door_label owner: HACKER
    "Name a door from stored side fields without calling its custom delegate.";
    const {passage, from_room} = args;
    const label = from_room == passage.side_a_room ? passage.side_a_label | passage.side_b_label;
    if (label:lowercase() in {"north", "south", "east", "west", "northeast", "northwest",
        "southeast", "southwest", "up", "down", "in", "out"})
      return label + " door";
    endif
    return label != "" ? label | "door";
  endmethod

  method test_description_formats_are_explicit owner: HACKER
    "Literal braces stay text; explicit templates validate their available bindings before an edit.";
    const passage = this:mk($room, "east", {}, "", true, $root, "west", {}, "", true);
    const literal = passage:with_description_from($room, "broken {template", 'literal);
    $test_utils:assert_eq(literal.side_a_description, "broken {template", "Braces do not imply a template");
    $test_utils:assert_raises(E_INVARG, passage, "with_description_from", {$room, "broken {template", 'template},
      "An explicit malformed template is rejected");
    $test_utils:assert_raises(E_INVARG, passage, "with_description_from", {$room, "{unknown_binding}", 'template},
      "An explicit template cannot name an unavailable binding");
    const source = "a {direction} door";
    const authored = passage:with_description_from($room, source, 'template);
    $test_utils:assert_eq(authored.side_a_description, source, "Template source stays editable");
    $test_utils:assert_eq(authored.side_a_description_kind, 'template, "The syntax is explicit");
    const parts = $sub_utils:compile(source);
    const structured = this:mk($room, "east", {}, parts, true, $root, "west", {}, "", true);
    $test_utils:assert_eq(structured.side_a_description, parts, "Construction retains structured descriptions");
    return true;
  endmethod
endobject
