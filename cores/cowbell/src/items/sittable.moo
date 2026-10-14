object SITTABLE [
  import_export_id -> "sittable",
  import_export_hierarchy -> {"items"}
]
  name: "Sittable"
  parent: THING
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property already_sitting_msg (owner: HACKER, flags: "rc") = {
    "You're already sitting on ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property cant_reach_msg (owner: HACKER, flags: "rc") = {
    "You can't reach ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    " from here."
  };
  property dumped_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'dobj, .subject_form = 'name, .plural_form = "are", .singular_form = "is">,
    " dumped off ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property no_room_msg (owner: HACKER, flags: "rc") = {
    "There's no room on ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property not_sitting_msg (owner: HACKER, flags: "rc") = {
    "You aren't sitting on ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property seats (owner: HACKER, flags: "rc") = 2;
  property sit_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "sit", .for_others = "sits">,
    " on ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property sitting (owner: HACKER, flags: "r") = {};
  property sitting_prep (owner: HACKER, flags: "rc") = "on";
  property sitting_verb (owner: HACKER, flags: "rc") = "sitting";
  property squeeze (owner: HACKER, flags: "rc") = 0;
  property squeezed_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'dobj, .subject_form = 'name, .plural_form = "are", .singular_form = "is">,
    " squeezed off ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };
  property stand_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "stand", .for_others = "stands">,
    " up from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    "."
  };

  override aliases (owner: HACKER, flags: "rc") = {"sittable"};
  override description (owner: HACKER, flags: "rc") = "A prototype for objects that can be sat upon. Set .seats to control capacity, .squeeze for extra squeezable spots (-1 for rigid furniture), .sitting_verb and .sitting_prep for customization.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Sittable Objects",
    "",
    "## Overview",
    "",
    "Sittable objects are furniture that players can sit on. They track who is sitting and manage capacity with optional squeeze mechanics. Inherits from Thing.",
    "",
    "## Properties",
    "",
    "### seats",
    "",
    "Maximum number of players who can sit comfortably (default: 2).",
    "",
    "### squeeze",
    "",
    "Extra spots that can be squeezed in beyond capacity. When exceeded, the first sitter is dumped.",
    "",
    "- `0`: No squeezing allowed (default)",
    "- `-1`: Rigid furniture, no one can exceed capacity",
    "- `N`: Allow N extra players before squeezing someone off",
    "",
    "### sitting_verb and sitting_prep",
    "",
    "Customize how sitting is described:",
    "",
    "- `sitting_verb`: Verb used in description (default: \"sitting\")",
    "- `sitting_prep`: Preposition used (default: \"on\")",
    "",
    "Examples: \"perched on\", \"lounging in\", \"curled up on\"",
    "",
    "### Message Properties",
    "",
    "All messages use $sub template substitution and can be customized via @set-message:",
    "",
    "- `sit_msg`: Shown when sitting (default: \"Alice sits on the couch.\")",
    "- `stand_msg`: Shown when standing (default: \"Alice stands up from the couch.\")",
    "- `already_sitting_msg`: Error when already sitting",
    "- `not_sitting_msg`: Error when not sitting",
    "- `no_room_msg`: Error when furniture is full",
    "- `cant_reach_msg`: Error when not in same room",
    "- `squeezed_msg`: Shown when someone is squeezed off",
    "- `dumped_msg`: Shown when someone is dumped off (furniture moves)",
    "",
    "## Commands",
    "",
    "### sit on <furniture>",
    "",
    "```",
    "sit on couch",
    "```",
    "",
    "Sit on the furniture. You must be in the same room.",
    "",
    "### stand from <furniture>",
    "",
    "```",
    "stand from couch",
    "```",
    "",
    "Stand up from the furniture.",
    "",
    "## Behavior",
    "",
    "- Sitters are dumped when furniture moves to a different room",
    "- Sitters are removed from the list when they leave the room",
    "- The description automatically shows who is sitting",
    "- Fires `'on_sit` and `'on_stand` triggers for reactions",
    "",
    "## Example: Creating a Cozy Armchair",
    "",
    "```moo",
    "chair = create($sittable);",
    "chair:set_name(\"a cozy armchair\");",
    "chair.description = \"A worn leather armchair with plush cushions.\";",
    "chair.seats = 1;",
    "chair.squeeze = -1;  // Rigid - only one person fits",
    "chair.sitting_verb = \"curled up\";",
    "chair.sitting_prep = \"in\";",
    "```"
  };

  verb "sit climb" (none on this) owner: ARCH_WIZARD flags: "rd" [ ui -> ["prefix" -> "Sit on"] ]
    "Sit on nearby furniture as the person who entered the command.";
    caller == player || raise(E_PERM);
    set_task_perms(player);
    let message = {};
    if (player in this.sitting)
      message = this.already_sitting_msg;
    elseif (player.location != this.location)
      message = this.cant_reach_msg;
    elseif (this.squeeze < 0 && length(this.sitting) >= this.seats)
      message = this.no_room_msg;
    endif
    if (length(message))
      player:inform_current($event:mk_error(player, @message):with_this(this));
      return false;
    endif
    return this:add_sitter(player);
  endverb

  verb stand (none from this) owner: ARCH_WIZARD flags: "rd" [ ui -> ["prefix" -> "Stand up from"] ]
    "Stand from furniture as the person who entered the command.";
    caller == player || raise(E_PERM);
    set_task_perms(player);
    if (!(player in this.sitting))
      player:inform_current($event:mk_error(player, @this.not_sitting_msg):with_this(this));
      return false;
    endif
    return this:remove_sitter(player);
  endverb

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"already_sitting_msg", "cant_reach_msg", "no_room_msg", "not_sitting_msg", "sit_msg", "stand_msg"})
      let bindings = $sub_utils:standard_bindings("The person sitting or standing.");
      bindings['this_obj] = ['kind -> 'object, 'description -> "The furniture.",
        'example -> 'host];
      return bindings;
    endif
    if (tostr(property) in {"dumped_msg", "squeezed_msg"})
      let bindings = $sub_utils:standard_bindings("The person removed from the furniture.");
      bindings['this_obj] = ['kind -> 'object, 'description -> "The furniture.",
        'example -> 'host];
      bindings['dobj] = ['kind -> 'object, 'description -> "The same person removed from the furniture.",
        'example -> 'actor];
      return bindings;
    endif
    return pass(@args);
  endmethod

  method add_sitter owner: ARCH_WIZARD
    "Seat the caller or an owned NPC, then announce and publish the completed action.";
    const principal = caller_perms();
    const {who, ?silent = false, ?context = []} = args;
    $root:check_actor(who, principal);
    if (who in this.sitting || who.location != this.location)
      return false;
    endif
    if (this.squeeze < 0 && length(this.sitting) >= this.seats)
      return false;
    endif
    const room = this.location;
    valid(room) && isa(room, $room) || return false;
    const before = {this.owner, room, this.seats, this.squeeze, who.owner, who.location};
    room:engage_actor(who, this);
    "Disengaging other furniture can call authored code and suspend.";
    $root:check_actor(who, principal);
    equal(before, {this.owner, this.location, this.seats, this.squeeze, who.owner, who.location})
      || return false;
    who in this.sitting && return false;
    this.sitting = {@this.sitting, who};
    if (!silent)
      room:announce($event:mk_act(who, @this.sit_msg):with_this(this));
    endif
    this:fire_trigger('on_sit, mapmerge(context, ['Actor -> who, 'Location -> room]));
    if (length(this.sitting) > this.seats + max(0, this.squeeze))
      this:dump_sitter(this.sitting[1]);
    endif
    return true;
  endmethod

  method remove_sitter owner: ARCH_WIZARD
    "Stand the caller or an owned NPC, and clear its room engagement.";
    const principal = caller_perms();
    const {who, ?silent = false, ?context = []} = args;
    $root:check_actor(who, principal);
    const position = who in this.sitting;
    position || return false;
    const room = this.location;
    this.sitting = listdelete(this.sitting, position);
    if (valid(room) && isa(room, $room))
      if (room:get_engagement(who) == this)
        room:disengage_actor(who);
      endif
      if (!silent)
        room:announce($event:mk_act(who, @this.stand_msg):with_this(this));
      endif
    endif
    this:fire_trigger('on_stand, mapmerge(context, ['Actor -> who, 'Location -> room]));
    return true;
  endmethod

  method has_room owner: HACKER
    "Check if furniture has room for another sitter.";
    if (this.squeeze < 0)
      return length(this.sitting) < this.seats;
    endif
    return true;
  endmethod

  method is_sitting owner: HACKER
    "Check if a specific actor is sitting on this furniture.";
    {who} = args;
    return who in this.sitting;
  endmethod

  method action_sit owner: ARCH_WIZARD
    "Action handler for reactions: make actor sit on this furniture.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    $root:check_actor(who);
    return this:add_sitter(who, false, context);
  endmethod

  method action_stand owner: ARCH_WIZARD
    "Action handler for reactions: make actor stand from this furniture.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    $root:check_actor(who);
    return this:remove_sitter(who, false, context);
  endmethod

  verb dump_sitter (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Let the furniture owner or core operations remove a sitter.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    const {who, ?use_dumped = false, ?room = this.location} = args;
    const position = who in this.sitting;
    position || return false;
    this.sitting = listdelete(this.sitting, position);
    if (valid(room) && isa(room, $room))
      if (room:get_engagement(who) == this)
        room:disengage_actor(who);
      endif
      const message = use_dumped ? this.dumped_msg | this.squeezed_msg;
      room:announce($event:mk_info(who, @message):with_dobj(who):with_this(this));
    endif
    this:fire_trigger(use_dumped ? 'on_sittable_dump | 'on_sittable_squeeze,
      ['Actor -> who, 'Location -> room]);
    return true;
  endverb

  verb dump_all_sitters (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Let the furniture owner clear its seats, including after a completed move.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    const {?room = this.location} = args;
    for who in (this.sitting)
      this:dump_sitter(who, true, room);
    endfor
  endverb

  verb sitters_string (none none none) owner: HACKER flags: "rxd"
    "Return a string describing who is sitting, or empty if nobody.";
    length(this.sitting) == 0 && return "";
    names = { sitter:name() for sitter in (this.sitting) };
    name_list = $list_proto:english_list(names);
    verb = length(this.sitting) == 1 ? "is" | "are";
    return name_list + " " + verb + " " + this.sitting_verb + " " + this.sitting_prep + " it.";
  endverb

  verb description (none none none) owner: HACKER flags: "rxd"
    "Return description, appending sitter info if any.";
    desc = pass(@args);
    sitters = this:sitters_string();
    length(sitters) == 0 && return desc;
    typeof(desc) == TYPE_LIST && return {@desc, sitters};
    return desc + " " + sitters;
  endverb

  verb moveto (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Preserve the mover's authority and empty seats only after a successful move.";
    const principal = caller_perms();
    const {destination} = args;
    const source = this.location;
    set_task_perms(principal);
    const result = pass(@args);
    typeof(result) == TYPE_ERR && return result;
    if (this.location != source)
      $sittable:_complete_move(this, source);
    endif
    return result;
  endverb

  method _complete_move owner: ARCH_WIZARD
    "Clear real furniture after moveto authorizes and completes an owner or capability move.";
    const {receiver, source} = args;
    const frame = callers()[1];
    frame[1] == receiver && frame[4] == $sittable && frame[2] == "moveto" || raise(E_PERM);
    const furniture = typeof(receiver) == TYPE_FLYWEIGHT ? receiver.delegate | receiver;
    furniture:dump_all_sitters(source);
  endmethod

  verb test_sitting (none none none) owner: HACKER flags: "rxd"
    "Test basic sitting functionality.";
    test_obj = this:create(true);
    test_obj.name = "test bench";
    test_obj.seats = 2;
    test_obj.squeeze = 0;
    "Initially empty";
    length(test_obj.sitting) == 0 || raise(E_ASSERT, "Should start empty");
    "Add a sitter directly";
    test_obj.sitting = {player};
    length(test_obj.sitting) == 1 || raise(E_ASSERT, "Should have 1 sitter");
    "Check sitters_string";
    sitter_desc = test_obj:sitters_string();
    index(sitter_desc, "sitting on it") || raise(E_ASSERT, "Should mention sitting: " + sitter_desc);
    "Remove sitter";
    test_obj.sitting = {};
    test_obj:sitters_string() == "" || raise(E_ASSERT, "Should be empty string");
    return true;
  endverb

  method help_topics owner: ARCH_WIZARD
    "Return help topics for sittable furniture.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("sit", "Sit down", "Use 'sit on <furniture>' to sit down.", {}, 'commands, {"stand"}), $help:mk("stand", "Stand up", "Use 'stand from <furniture>' to get up.", {"get up"}, 'commands, {"sit"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

  method on_location_exit owner: ARCH_WIZARD
    "Remove a departed sitter only for the room's departure callback.";
    const {who} = args;
    const source = callers()[1];
    (caller == this.location && source[4] == $room && source[2] == "exitfunc")
      || raise(E_PERM);
    if (who.location != this.location && who in this.sitting)
      this:remove_sitter(who);
    endif
  endmethod

  method remove_occupant owner: ARCH_WIZARD
    "Allow the room to switch seats; other callers must control the departing actor.";
    const source = callers()[1];
    const room_callback = caller == this.location && source[4] == $room
      && source[2] == "engage_actor";
    if (!room_callback)
      set_task_perms(caller_perms());
    endif
    return this:remove_sitter(@args);
  endmethod
endobject
