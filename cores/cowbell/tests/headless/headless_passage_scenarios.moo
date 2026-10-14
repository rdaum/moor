// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_PASSAGE_SCENARIOS
  name: "Passage Boundaries"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true
  override import_export_id = "headless_passage_scenarios";
  override import_export_hierarchy = {"tests", "headless"};
  property policy_probe (owner: ARCH_WIZARD, flags: "r") = [];

  method _as_programmer owner: RUNTIME_PROGRAMMER
    "Call an operation as the fixture programmer, without granting wizard authority.";
    caller == this && this == #90019 || raise(E_PERM);
    const {target, operation, @arguments} = args;
    return target:(operation)(@arguments);
  endmethod

  method _as_visitor owner: RUNTIME_PLAYER
    "Call an operation as an ordinary visitor who does not own the provider.";
    caller == this && this == #90019 || raise(E_PERM);
    const {target, operation, @arguments} = args;
    return target:(operation)(@arguments);
  endmethod

  method test_nonowner_can_use_programmer_owned_portal owner: ARCH_WIZARD
    "The core performs the checked move; a provider never needs authority over the visitor.";
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const provider = this:_make_exit_provider(destination);
    const previous = #90102.location;
    try
      this:_as_programmer($room, "register_exit_provider", source, provider);
      move(#90102, source);
      const action = this:_as_visitor($room, "exit_options", source, #90102)[1]['descriptor];
      $test_utils:assert_eq(provider.query_principal, #90102, "Discovery retains the actual querying principal");
      $test_utils:assert_true(this:_as_visitor($room, "use_exit_action", #90102,
        action["command"], action["exit"]), "A visitor can use another programmer's portal");
      $test_utils:assert_eq(#90102.location, destination, "The visitor arrives");
      $test_utils:assert_false(provider.owner.wizard, "The provider's owner is not a wizard");
      $test_utils:assert_eq(`this:_as_programmer(provider, "use_exit", #90102, source, "portal")
        ! E_PERM => E_PERM', E_PERM, "Direct calls do not impersonate the checked room action");
    finally
      move(#90102, previous);
      recycle(provider);
      recycle(source);
      recycle(destination);
    endtry
    return true;
  endmethod

  method test_go_keeps_actor_checks_and_local_scope owner: ARCH_WIZARD
    "Destination lookup uses only local approved exits and cannot authorize another actor.";
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const remote = create($room, #90101, 2);
    const provider = this:_make_exit_provider(destination);
    const previous = #90102.location;
    try
      destination.name = "The Garden";
      remote.name = "Distant Room";
      this:_as_programmer($room, "register_exit_provider", source, provider);
      move(#90102, source);
      $test_utils:assert_eq(`this:_as_programmer(source, "go", #90102, "garden")
        ! E_PERM => E_PERM', E_PERM, "The room's owner cannot move another player");
      $test_utils:assert_eq(this:_as_visitor($room, "match_exit", source, #90102, tostr(remote)),
        [], "An explicit object ID must be a local exit destination");
      $test_utils:assert_eq(this:_as_visitor($room, "match_exit", source, #90102, "distant"),
        [], "Name matching cannot select a remote room");
      $test_utils:assert_true(this:_as_visitor(source, "go", #90102, "garden"),
        "A visitor can use the local provider by destination name");
      $test_utils:assert_eq(#90102.location, destination, "Checked travel reaches the destination");
      $test_utils:assert_eq(provider.query_principal, #90102,
        "Matching and action discovery retain the initiating principal");
      move(#90102, source);
      this:_as_programmer($room, "unregister_exit_provider", source, provider);
      $test_utils:assert_eq(this:_as_visitor($room, "match_exit", source, #90102, "garden"),
        [], "Unapproved providers cannot be selected by destination");
    finally
      move(#90102, previous);
      recycle(provider);
      recycle(source);
      recycle(destination);
      recycle(remote);
    endtry
    return true;
  endmethod

  method _make_exit_provider owner: ARCH_WIZARD
    "Create a programmer-owned portal fixture with the public provider interface.";
    caller == this || raise(E_PERM);
    const {destination} = args;
    const provider = create($root, #90101, 2);
    add_property(provider, "offered", {['id -> "portal", 'label -> "portal", 'command -> "enter portal",
      'destination -> destination, 'description -> "A blue arch shimmers.", 'ambient -> true]}, {#90101, "r"});
    add_property(provider, "query_principal", #-1, {#90101, "r"});
    add_property(provider, "destination", destination, {#90101, "r"});
    add_verb(provider, {#90101, "rxd", "travel_options"}, {"this", "none", "this"});
    set_verb_code(provider, "travel_options", {
      "this.query_principal = caller_perms(); return this.offered;"
    });
    add_verb(provider, {#90101, "rxd", "use_exit"}, {"this", "none", "this"});
    set_verb_code(provider, "use_exit", {
      "caller == $room || raise(E_PERM); const {who, source, option} = args;",
      "who.location == source && option == \"portal\" || return false;",
      "return this.destination;"
    });
    return provider;
  endmethod

  method test_custom_exits_share_discovery_and_routes owner: ARCH_WIZARD
    "Approved custom travel appears in prose and routes; dropped objects do not become exits.";
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const provider = this:_make_exit_provider(destination);
    try
      move(provider, source);
      $test_utils:assert_eq($room:exit_options(source, #90101), {}, "Contents alone are not exits");
      this:_as_programmer($room, "register_exit_provider", source, provider);
      let options = this:_as_programmer($room, "exit_options", source, #90101);
      $test_utils:assert_eq(provider.query_principal, #90101, "Provider receives the caller's authority");
      $test_utils:assert_eq(length(options), 1, "The registered portal is visible");
      $test_utils:assert_eq(options[1]['descriptor]["command"], "enter portal", "The provider supplies its action");
      $document_schema:_descriptor_from_wire(options[1]['descriptor]);
      add_verb(source, {#90101, "rxd", "available_actions"}, {"this", "none", "this"});
      set_verb_code(source, "available_actions", {"return {{\"press bell\", \"Ring the bell\"}};"});
      const view = this:_as_programmer(source, "look_self"):capture(#90101);
      $test_utils:assert_eq(view['actions][1]['label], "Ring the bell", "Room-defined actions are discoverable");
      $test_utils:assert_eq(view['exits][1]['label], "portal", "Room capture includes custom travel");
      const route = this:_as_programmer($area, "find_path", source, destination);
      $test_utils:assert_eq(route[1][2], {'transport, "portal", provider, "enter portal"},
        "Routes use the same option and command");
      $test_utils:assert_false(this:_as_programmer($area, "find_path", source, destination, true, false),
        "A custom exit is not an automatically walkable passage");
      provider.offered[1] = mapdelete(provider.offered[1], 'destination);
      options = this:_as_programmer($room, "exit_options", source, #90101);
      $test_utils:assert_false(maphaskey(options[1]['descriptor]["exit"], "destination"),
        "An unknown destination is not invented");
      $document_schema:_descriptor_from_wire(options[1]['descriptor]);
      $test_utils:assert_eq(source:transport_destinations(#90101), {}, "Unknown destinations are not route edges");
      provider.offered[1]['available] = false;
      provider.offered[1]['reason] = "The portal is recharging.";
      options = this:_as_programmer($room, "exit_options", source, #90101);
      $test_utils:assert_eq(options[1]['caption], "portal (The portal is recharging.)", "Unavailable travel explains why");
      $test_utils:assert_eq(options[1]['descriptor], [], "Unavailable travel has no clickable action");
    finally
      recycle(provider);
      recycle(source);
      recycle(destination);
    endtry
    return true;
  endmethod

  method test_custom_exit_actions_reject_stale_approval owner: ARCH_WIZARD
    "Removing, replacing, or transferring a provider invalidates a captured action.";
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const provider = this:_make_exit_provider(destination);
    const who = create($actor, #90101, 2);
    try
      move(who, source);
      this:_as_programmer($room, "register_exit_provider", source, provider);
      const descriptor = this:_as_programmer($room, "exit_options", source, who)[1]['descriptor];
      this:_as_programmer($room, "register_exit_provider", source, provider);
      $test_utils:assert_false(this:_as_programmer($room, "use_exit_action", who,
        descriptor["command"], descriptor["exit"]), "Reapproval invalidates old links");
      const current = this:_as_programmer($room, "exit_options", source, who)[1]['descriptor];
      const forged = mapmerge(current["exit"], ["destination" -> $url_utils:to_curie_str(source)]);
      $test_utils:assert_false(this:_as_programmer($room, "use_exit_action", who, "enter portal", forged),
        "Destination changes cannot reuse a captured action");
      $test_utils:assert_true(this:_as_programmer($room, "use_exit_action", who,
        current["command"], current["exit"]), "A current action reaches the provider as the actor's principal");
      $test_utils:assert_eq(who.location, destination, "The room performs the selected travel");
      $test_utils:assert_false(this:_as_programmer($room, "use_exit_action", who,
        current["command"], current["exit"]), "A link cannot be used from another room");
      move(who, source);
      provider.owner = #90100;
      $test_utils:assert_false(this:_as_programmer($room, "use_exit_action", who,
        current["command"], current["exit"]), "Changed provider ownership needs approval");
      this:_as_programmer($room, "unregister_exit_provider", source, provider);
      $test_utils:assert_eq($room:exit_options(source, who), {}, "Removal clears discovery");
      source.owner = #90100;
      $test_utils:assert_eq(`this:_as_programmer($room, "register_exit_provider", source, provider)
        ! E_PERM => E_PERM', E_PERM, "Only a room author can approve a provider");
    finally
      recycle(who);
      recycle(provider);
      recycle(source);
      recycle(destination);
    endtry
    return true;
  endmethod

  method test_broken_exit_provider_preserves_other_exits owner: ARCH_WIZARD
    "A malformed or conflicting provider is diagnosed without hiding a passage.";
    const area = create($area, #90100, 2);
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const provider = this:_make_exit_provider(destination);
    try
      area:set_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true));
      this:_as_programmer($room, "register_exit_provider", source, provider);
      for invalid in ({false, {[]}, {['id -> "portal", 'label -> "east", 'command -> "enter portal"]},
          {['id -> "portal", 'label -> "portal", 'command -> "go e"]}})
        provider.offered = invalid;
        const options = this:_as_programmer($room, "exit_options", source, #90101);
        $test_utils:assert_eq(options[1]['label], "east", "The passage survives a broken provider");
        $test_utils:assert_true(maphaskey(options[2], 'error), "The provider's failure remains visible");
        $test_utils:assert_eq(options[2]['descriptor], [], "Malformed output offers no action");
      endfor
    finally
      recycle(provider);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_area_lookup_cannot_retrieve_another_principals_grant owner: ARCH_WIZARD
    "An ordinary caller cannot obtain another principal's bearer from the area helper.";
    const room = create($room, #90100, 2);
    try
      $root:grant_capability(room, {'dig_from}, #90100, 'room);
      $test_utils:assert_eq(`this:_as_programmer($area, "_subject_for", #90100, room, 'room)
        ! E_PERM => E_PERM', E_PERM, "A supplied principal does not grant access to their credentials");
      $root:grant_capability(room, {'dig_from}, #90101, 'room);
      const own = this:_as_programmer($area, "_subject_for", #90101, room, 'room);
      $test_utils:assert_eq(own.delegate, room, "The actual caller can still use their own grant");
    finally
      $root:revoke_capability(room, #90100, 'room);
      $root:revoke_capability(room, #90101, 'room);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_building_tools_use_room_names_and_shared_directions owner: ARCH_WIZARD
    "Tools reject duplicate names and unknown reverse directions, and can select parallel passages.";
    const area = create($area, #90101, 2);
    const holder = create($room, #90101, 2);
    let source = #-1;
    let destination = #-1;
    try
      move(area, holder);
      const tools = $agent_building_tools;
      const build = ["name" -> "tool entrance", "area" -> tostr(area)];
      this:_as_programmer(tools, "build_room", build, #90101);
      source = area.contents[1];
      $test_utils:assert_eq(source.name, "tool entrance", "A valid area can itself have a location");
      $test_utils:assert_eq(`this:_as_programmer(tools, "build_room", build, #90101)
        ! E_INVARG => E_INVARG', E_INVARG, "Tools reject a duplicate room name");
      $test_utils:assert_eq(length(area.contents), 1, "A rejected build creates no room");
      $test_utils:assert_eq(`this:_as_programmer(tools, "build_room",
        mapmerge(build, ["area" -> tostr(source)]), #90101) ! E_INVARG => E_INVARG', E_INVARG,
        "A room is not an area even when it has no suitable contents");
      destination = this:_as_programmer(area, "make_room_in", $room, "tool landing");
      const dig = ["source_room" -> tostr(source), "target_room" -> tostr(destination), "direction" -> "ne"];
      this:_as_programmer(tools, "dig_passage", dig, #90101);
      $test_utils:assert_type($room:find_passage(destination, "sw"), TYPE_FLYWEIGHT,
        "Tools infer diagonal reverse directions like @dig");
      $test_utils:assert_eq(`this:_as_programmer(tools, "dig_passage",
        mapmerge(dig, ["direction" -> "stairs", "allow_parallel" -> true]), #90101)
        ! E_INVARG => E_INVARG', E_INVARG, "A named direction requires an explicit return name");
      this:_as_programmer(tools, "dig_passage", mapmerge(dig,
        ["direction" -> "stairs", "return_direction" -> "landing", "allow_parallel" -> true]), #90101);
      $test_utils:assert_eq(length($room:passages(source)), 2, "Parallel tool creation preserves the first passage");
      this:_as_programmer(tools, "remove_passage", mapmerge(dig, ["direction" -> "stairs"]), #90101);
      $test_utils:assert_eq(length($room:passages(source)), 1, "Removal selects the named connection");
      $test_utils:assert_type($room:find_passage(source, "ne"), TYPE_FLYWEIGHT,
        "Removing the stairs preserves the original passage");
    finally
      valid(source) && source:destroy();
      valid(destination) && destination:destroy();
      area:destroy();
      recycle(holder);
    endtry
    return true;
  endmethod

  method test_connection_previews_use_creation_validation owner: ARCH_WIZARD
    "A dry run rejects the same conflicting vocabulary without changing the connection store.";
    const area = create($area, #90101, 2);
    const source = create($room, #90101, 2);
    const destination = create($room, #90101, 2);
    const other = create($room, #90101, 2);
    const foreign_area = create($area, #90100, 2);
    try
      const passage = $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true);
      $test_utils:assert_true(this:_as_programmer(area, "create_passage", source, destination, passage, false, true),
        "A valid preview succeeds");
      $test_utils:assert_eq($room:passages(source), {}, "Preview does not install a passage");
      this:_as_programmer(area, "create_passage", source, destination, passage);
      const conflicting = $passage:mk(source, "e", {}, "", true, other, "west", {}, "", true);
      for preview in ({true, false})
        $test_utils:assert_eq(`this:_as_programmer(area, "create_passage", source, other, conflicting, false, preview)
          ! E_INVARG => E_INVARG', E_INVARG, "Both paths reject the expanded alias collision");
      endfor
      const moved = `this:_as_programmer($agent_building_tools, "move_object",
        ["object" -> tostr(source), "destination" -> tostr(foreign_area)], #90101) ! E_NACC => E_NACC';
      $test_utils:assert_eq(moved, E_NACC, "A denied move is an error, not a successful tool response");
      $test_utils:assert_eq(source.location, #-1, "Denied area admission does not relocate the room");
    finally
      source:destroy();
      destination:destroy();
      other:destroy();
      area:destroy();
      foreign_area:destroy();
    endtry
    return true;
  endmethod

  method test_exit_actions_authenticate_the_actor owner: ARCH_WIZARD
    "A reference to a passage does not grant control of another player.";
    const before = #90102.location;
    $test_utils:assert_eq(`this:_as_programmer($room, "use_exit_action", #90102, "go east", [])
      ! E_PERM => E_PERM', E_PERM, "Foreign players cannot be supplied as the actor");
    $test_utils:assert_eq(#90102.location, before, "Rejected actions leave the actor in place");
    return true;
  endmethod

  method test_reaction_door_actions_share_checked_operations owner: ARCH_WIZARD
    "Declared actions use the named door and retain actor, location, and access checks.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    const stranger = create($actor, #90100, 2);
    try
      move(who, source);
      move(stranger, source);
      let draft = $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true, false);
      draft.is_door = true;
      draft = draft:with_unlock_rule($rule_engine:parse_expression(
        "input.Accessor == " + toliteral(who), 'owner, $rule_engine:passage_scope()));
      area:set_passage(source, destination, draft);
      const values = ['target -> source, 'actor -> who, 'exit -> "east"];
      for name in ({'open_exit, 'close_exit, 'lock_exit, 'unlock_exit})
        const result = this:_as_programmer($reaction, "perform_action", name, values);
        $test_utils:assert_eq(result['status], 'applied, tostr(name) + " uses the door operation");
        const passage = area:passage_for(source, destination);
        $test_utils:assert_eq(passage.is_open, name == 'open_exit, "The selected operation changes open state");
        $test_utils:assert_eq(passage.is_locked, name == 'lock_exit, "The selected operation changes lock state");
      endfor
      const foreign = this:_as_programmer($reaction, "perform_action", 'open_exit,
        mapmerge(values, ['actor -> stranger]));
      $test_utils:assert_eq(foreign['status], 'error, "A reaction cannot borrow another actor");
      $test_utils:assert_eq(foreign['error][1], E_PERM, "Actor authority is required");
      const missing = this:_as_programmer($reaction, "perform_action", 'open_exit,
        mapmerge(values, ['exit -> "north"]));
      $test_utils:assert_eq(missing['status], 'denied, "A missing exit does not select another door");
      move(who, destination);
      const remote = this:_as_programmer($reaction, "perform_action", 'open_exit, values);
      $test_utils:assert_eq(remote['status], 'denied, "The actor must be at the target room");
      $test_utils:assert_false(area:passage_for(source, destination).is_open,
        "Denied actions leave the door closed");
    finally
      recycle(who);
      recycle(stranger);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_travel_rechecks_rule_facts_after_pre_exit owner: ARCH_WIZARD
    "A departure hook can revoke access without changing the passage or carried objects.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    try
      add_property(source, "entry_allowed", true, {#90100, "r"});
      add_verb(source, {#90100, "rxd", "notify_pre_exit"}, {"this", "none", "this"});
      set_verb_code(source, "notify_pre_exit", {"this.entry_allowed = false; suspend(0);"});
      move(who, source);
      const rule = $rule_engine:parse_expression(
        "property_is(object: input.From, field: 'entry_allowed, value: true)",
        'entry, $rule_engine:passage_scope());
      const passage = area:set_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true)
          :with_locked(true):with_unlock_rule(rule));
      $test_utils:assert_true(passage:check_unlock(who, source), "Access starts allowed");
      $test_utils:assert_false(this:_as_programmer(passage, "travel_from", who, source, []),
        "Revoking a fact during the departure hook blocks movement");
      $test_utils:assert_eq(who.location, source, "The denied actor stays in the source room");
    finally
      recycle(who);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_travel_reports_redirection_during_native_movement owner: ARCH_WIZARD
    "An entry callback can redirect a traveler; the passage must not report arrival at its destination.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const elsewhere = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    try
      move(who, source);
      add_property(destination, "redirect_to", elsewhere, {#90100, "r"});
      add_verb(destination, {#90100, "rxd", "enterfunc"}, {"this", "none", "this"});
      set_verb_code(destination, "enterfunc", {"move(args[1], this.redirect_to); suspend(0);"});
      const passage = area:create_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true));
      $test_utils:assert_false(this:_as_programmer(passage, "travel_from", who, source, []),
        "A redirected move did not complete the requested passage step");
      $test_utils:assert_eq(who.location, elsewhere, "The callback's relocation remains in effect");
    finally
      recycle(who);
      recycle(source);
      recycle(destination);
      recycle(elsewhere);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_room_look_rejects_departed_viewers owner: ARCH_WIZARD
    "Arrival presentation must not send a former room after movement, including a yielding capture.";
    const source = create($room, #90100, 2);
    const elsewhere = create($room, #90100, 2);
    const receiver = create($event_receiver, #90100, 2);
    try
      add_property(receiver, "received", {}, {#90100, "r"});
      add_verb(receiver, {#90100, "rxd", "inform_current"}, {"this", "none", "this"});
      set_verb_code(receiver, "inform_current", {"this.received = {@this.received, args[1]};"});
      move(receiver, source);
      receiver:emit_room_look(source);
      $test_utils:assert_eq(length(receiver.received), 1, "A current room look reaches its viewer");
      receiver.received = {};
      move(receiver, elsewhere);
      receiver:emit_room_look(source);
      $test_utils:assert_eq(receiver.received, {}, "A delayed arrival look does not describe the old room");
      move(receiver, source);
      add_property(source, "receiver", receiver, {#90100, "r"});
      add_property(source, "redirect_to", elsewhere, {#90100, "r"});
      add_verb(source, {#90100, "rxd", "look_self"}, {"this", "none", "this"});
      set_verb_code(source, "look_self", {"const look = pass(@args); move(this.receiver, this.redirect_to); suspend(0); return look;"});
      receiver:emit_room_look(source);
      $test_utils:assert_eq(receiver.location, elsewhere, "The producer moved the viewer during capture");
      $test_utils:assert_eq(receiver.received, {}, "A capture that finishes after departure sends no old narrative");
    finally
      recycle(receiver);
      recycle(source);
      recycle(elsewhere);
    endtry
    return true;
  endmethod

  method test_configure_cannot_mutate_the_shared_prototype owner: ARCH_WIZARD
    "A public convenience constructor cannot reconfigure the world's shared passage prototype.";
    const before = $passage:configuration($passage);
    $test_utils:assert_eq(`this:_as_programmer($passage, "configure", #90103, "east", {}, "", true,
      #90110, "west", {}, "", true) ! E_PERM => E_PERM', E_PERM,
      "Configuring a persistent passage requires its owner or a wizard");
    $test_utils:assert_eq($passage:configuration($passage), before, "A denied call changes no defaults");
    const value = <$passage>;
    const configured = this:_as_programmer(value, "configure", #90103, "east", {}, "An arch.", true,
      #90110, "", {}, "", true);
    $test_utils:assert_true(configured.side_a_enabled && !configured.side_b_enabled,
      "A configured one-way value has the same explicit direction flags as mk");
    return true;
  endmethod

  method test_side_edits_cannot_reconnect_or_change_the_other_side owner: ARCH_WIZARD
    "Source ownership authorizes local prose, but not another destination or shared door state.";
    const area = create($area, #90100, 2);
    const source = create($room, #90101, 2);
    const destination = create($room, #90100, 2);
    const elsewhere = create($room, #90100, 2);
    try
      const original = area:set_passage(source, destination, $passage:mk(source, "east", {"e"}, "", true,
        destination, "west", {"w"}, "", true));
      const identity = area:passage_link_id(source, destination);
      const redirected = $passage:mk(source, "east", {}, "", true,
        elsewhere, "west", {}, "", true);
      $test_utils:assert_eq(`this:_as_programmer(area, "update_passage",
        source, destination, redirected) ! E_INVARG => E_INVARG', E_INVARG,
        "A replacement must retain the registered endpoints");
      for replacement in ({original:with_label_from(destination, "hidden"),
          original:with_locked(true), original:with_open(false)})
        $test_utils:assert_eq(`this:_as_programmer(area, "update_passage",
          source, destination, replacement) ! E_PERM => E_PERM', E_PERM,
          "Source ownership cannot change another side or shared state");
      endfor
      $test_utils:assert_eq(area:passage_for(source, destination), original,
        "Rejected edits preserve the installed passage");
      const described = original:with_description_from(source, "an east archway");
      this:_as_programmer(area, "update_passage", source, destination, described);
      $test_utils:assert_eq(area:passage_for(source, destination), described,
        "Source ownership permits a local description edit");
      $test_utils:assert_eq(area:passage_link_id(source, destination), identity,
        "Side edits retain the connection identity");
    finally
      recycle(source);
      recycle(destination);
      recycle(elsewhere);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_supplied_bearer_authorizes_only_its_side owner: ARCH_WIZARD
    "A supplied dig_from capability works without installing it in the caller's stored grants.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      const original = area:set_passage(source, destination, $passage:mk(source, "east", {}, "", true,
        destination, "west", {}, "", true));
      const capability = $root:issue_capability(source, {'dig_from});
      const described = original:with_description_from(source, "an east doorway");
      this:_as_programmer(area, "update_passage", capability, destination, described);
      $test_utils:assert_eq(area:passage_for(source, destination), described,
        "The operation validates the supplied bearer");
      $test_utils:assert_eq(`this:_as_programmer(area, "update_passage", capability,
        destination, described:with_open(false)) ! E_PERM => E_PERM', E_PERM,
        "A side capability cannot operate the shared door");
    finally
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_invalid_pairs_and_values_preserve_connections owner: ARCH_WIZARD
    "Same-room requests and invalid replacements cannot remove a neighboring connection.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      const original = area:set_passage(source, destination, $passage:mk(source, "east", {}, "", true,
        destination, "west", {}, "", true));
      const identity = area:passage_link_id(source, destination);
      $test_utils:assert_false(area:passage_for(source, source), "A room has no implicit self-loop");
      $test_utils:assert_eq(`area:clear_passage(source, source) ! E_INVARG => E_INVARG',
        E_INVARG, "Same-room removal is rejected");
      $test_utils:assert_eq(`area:set_passage(source, destination, $thing) ! E_TYPE => E_TYPE',
        E_TYPE, "A thing is not a passage");
      $test_utils:assert_eq(area:passage_for(source, destination), original,
        "Rejected mutations leave the connection intact");
      $test_utils:assert_eq(area:passage_link_id(source, destination), identity,
        "Rejected mutations preserve identity as well as contents");
    finally
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_passage_actions_preserve_actor_authority owner: ARCH_WIZARD
    "An owner can move an NPC through an open passage; another actor cannot supply that authority.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const owned = create($actor, #90101, 2);
    const foreign = create($actor, #90100, 2);
    try
      move(source, area);
      move(destination, area);
      move(owned, source);
      move(foreign, source);
      area:set_passage(source, destination, $passage:mk(source, "east", {}, "", true,
        destination, "west", {}, "", true));
      $test_utils:assert_true(this:_as_programmer(owned, "action_go", owned, [], "east"),
        "The public action traverses a real passage");
      $test_utils:assert_eq(owned.location, destination, "The owned NPC arrives");
      $test_utils:assert_eq(`this:_as_programmer(foreign, "action_go", foreign, [], "east")
        ! E_PERM => E_PERM', E_PERM, "The actor entry rejects foreign authority");
      $test_utils:assert_eq(`this:_as_programmer(source, "action_go", foreign, [], "east")
        ! E_PERM => E_PERM', E_PERM, "The room entry also rejects foreign authority");
      $test_utils:assert_eq(foreign.location, source, "The foreign NPC stays in place");
    finally
      recycle(owned);
      recycle(foreign);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_passage_edits_preserve_messages_and_custom_behavior owner: ARCH_WIZARD
    "Ordinary passage edits retain all four door messages, the custom delegate, slots, and contents.";
    const prototype = create($passage, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      let passage = prototype:mk(source, "east", {}, "", true,
        destination, "west", {}, "", true);
      passage = toflyweight(prototype, flyslots(passage), {"custom contents"});
      passage.custom_state = ['color -> "brass"];
      passage = passage:with_open_msg("Creak!"):with_close_msg("Thud!")
        :with_lock_msg("Click!"):with_unlock_msg("Clink!");
      const edited = passage:with_open(false):with_aliases_from(source, {"e", "brass door"})
        :with_description_from(source, "a brass door");
      $test_utils:assert_eq({edited.open_msg, edited.close_msg, edited.lock_msg, edited.unlock_msg},
        {"Creak!", "Thud!", "Click!", "Clink!"}, "Messages survive later edits");
      $test_utils:assert_eq(edited.delegate, prototype, "The custom delegate survives");
      $test_utils:assert_eq(edited.custom_state, ['color -> "brass"], "Extension slots survive");
      $test_utils:assert_eq(flycontents(edited), {"custom contents"}, "Contents survive");
      $test_utils:assert_false(edited.is_open, "The intended edit still applies");
    finally
      recycle(source);
      recycle(destination);
      recycle(prototype);
    endtry
    return true;
  endmethod
  method test_connections_cross_areas_and_survive_room_transfer owner: ARCH_WIZARD
    "Both endpoints discover a connection; routes continue through a second area's own passages.";
    const left = create($area, #90100, 2);
    const right = create($area, #90100, 2);
    const third = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const onward = create($room, #90100, 2);
    try
      move(source, left);
      move(destination, right);
      move(onward, right);
      const crossing = left:create_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true));
      const continuation = right:create_passage(destination, onward,
        $passage:mk(destination, "north", {}, "", true, onward, "south", {}, "", true));
      $test_utils:assert_eq($room:find_passage(source, "east"), crossing, "The source finds the crossing");
      $test_utils:assert_eq($room:find_passage(destination, "west"), crossing, "The other area finds its return exit");
      const route = left:find_path(source, onward);
      $test_utils:assert_eq({route[1][2], route[2][2]}, {crossing, continuation},
        "Routing continues through the destination area's connections");
      third:add_room(destination);
      $test_utils:assert_eq($room:passages(destination), {crossing, continuation},
        "Changing area membership preserves both connections");
      $test_utils:assert_eq(length(third:find_path(destination, source)), 2,
        "Return routing survives transfer");
      recycle(destination);
      $test_utils:assert_eq($room:passages(source), {}, "Recycling removes the far-side crossing index");
      $test_utils:assert_eq($room:passages(onward), {}, "Recycling also cleans the other storing area");
    finally
      if (valid(destination))
        recycle(destination);
      endif
      recycle(source);
      recycle(onward);
      left:destroy();
      right:destroy();
      third:destroy();
    endtry
    return true;
  endmethod

  method test_closed_doors_remain_visible_at_both_endpoints owner: ARCH_WIZARD
    "Closed cross-area doors retain their descriptions and offer opening from either side.";
    const first_area = create($area, #90100, 2);
    const second_area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      move(source, first_area);
      move(destination, second_area);
      let door = $passage:mk(source, "east", {}, "A brass door stands here.", true,
        destination, "west", {}, "A brass door leads back.", true, false);
      door.is_door = true;
      door = first_area:create_passage(source, destination, door);
      const near = this:_as_programmer($room, "exit_options", source, #90101)[1];
      const far = this:_as_programmer($room, "exit_options", destination, #90101)[1];
      $test_utils:assert_eq({near['caption], far['caption]}, {"east (closed)", "west (closed)"},
        "Both sides show the closed door even though only one area stores it");
      $test_utils:assert_eq({near['descriptor]["command"], far['descriptor]["command"]},
        {"open east", "open west"}, "Both sides offer their local opening command");
      $test_utils:assert_eq(near['identity], far['identity], "Both sides identify one connection");
      const view = source:look_self():capture(#90102);
      for format in ({'text_plain, 'text_html, 'text_djot})
        const panel = $look:panel(view, format);
        const narrative = $document:render($look:narrative(view):resolve(#90102), format);
        const state_text = format == 'text_djot ? "\\(closed\\)" | "(closed)";
        $test_utils:assert_false(index(narrative['body], "You can go "),
          "A closed door retains its state without a duplicate exit list");
        for body in ({panel["content"], narrative['body]})
          $test_utils:assert_true(index(body, state_text) > 0,
            tostr("Exit state survives ", format, ": ", body));
          $test_utils:assert_true(index(body, "A brass door stands here") > 0,
            "Closing a door does not remove its authored description");
        endfor
      endfor
      const revisions = {source.exit_revision, destination.exit_revision};
      door = first_area:update_passage(source, destination, door:with_open(true));
      $test_utils:assert_true(source.exit_revision > revisions[1]
        && destination.exit_revision > revisions[2], "Changing shared state invalidates both captures");
      $test_utils:assert_eq($room:exit_options(source, #90102)[1]['caption], "east",
        "Fresh discovery sees the open door");
      $test_utils:assert_true(index($look:panel(view, 'text_plain)["content"], "east (closed)") > 0,
        "Historical content retains its captured state");
      door = first_area:update_passage(source, destination, door:with_open(false):with_locked(true));
      $test_utils:assert_eq($room:exit_options(destination, #90102)[1]['caption], "west (locked)",
        "Locked state remains discoverable at the far endpoint");
      door.side_b_enabled = false;
      first_area:update_passage(source, destination, door);
      $test_utils:assert_eq($room:exit_options(destination, #90102), {},
        "A disabled return direction is not advertised as an exit");
      move(source, #-1);
      $test_utils:assert_eq($room:exit_options(source, #90102)[1]['identity], door.connection_id,
        "Discovery does not require a containing area");
    finally
      recycle(source);
      recycle(destination);
      first_area:destroy();
      second_area:destroy();
    endtry
    return true;
  endmethod

  method test_parallel_passages_have_independent_state_and_identity owner: ARCH_WIZARD
    "Two connections between the same rooms can be edited and removed separately.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      const door = area:create_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true));
      const stairs = area:create_passage(source, destination,
        $passage:mk(source, "up", {}, "", true, destination, "down", {}, "", true), true);
      $test_utils:assert_true(door.connection_id != stairs.connection_id, "Parallel connections have different identities");
      $test_utils:assert_eq(`area:passage_for(source, destination) ! E_INVARG => E_INVARG', E_INVARG,
        "A room pair cannot silently select one of two passages");
      const closed = area:update_passage(source, destination, door:with_open(false));
      $test_utils:assert_eq(closed.connection_id, door.connection_id, "A state edit preserves identity");
      $test_utils:assert_eq($room:find_passage(source, "up"), stairs, "The other connection remains unchanged");
      $test_utils:assert_eq(length(area:passages_from(source)), 2, "Editing does not remove the parallel passage");
      area:remove_passage(source, destination, closed.connection_id);
      $test_utils:assert_eq(area:passages_from(destination), {stairs}, "Removal selects only the requested connection");
      $test_utils:assert_eq(`area:update_passage(source, destination,
        stairs:with_aliases_from(source, {"east", "up"})) ! ANY => E_ASSERT',
        stairs:with_aliases_from(source, {"east", "up"}), "A removed passage releases its aliases");
    finally
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_oneway_and_removed_connections_cannot_be_traversed owner: ARCH_WIZARD
    "Movement and routes respect direction and reject saved values after removal or reinstallation.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    try
      move(who, source);
      const draft = $passage:mk(source, "chute", {}, "", true, destination, "", {}, "", false);
      $test_utils:assert_false(this:_as_programmer(draft, "travel_from", who, source, []),
        "A draft value does not authorize travel");
      const installed = area:create_passage(source, destination, draft);
      $test_utils:assert_true(this:_as_programmer(installed, "travel_from", who, source, []),
        "The enabled side permits movement");
      $test_utils:assert_eq(who.location, destination, "The actor reaches the destination");
      $test_utils:assert_false(this:_as_programmer(installed, "travel_from", who, destination, []),
        "The disabled side cannot be invoked directly");
      $test_utils:assert_false(area:find_path(destination, source), "Routing does not invent a return direction");
      move(who, source);
      area:remove_passage(source, destination, installed.connection_id);
      $test_utils:assert_false(this:_as_programmer(installed, "travel_from", who, source, []),
        "A removed connection cannot be traversed");
      const replacement = area:create_passage(source, destination, installed);
      $test_utils:assert_true(replacement.connection_id != installed.connection_id, "Reinstallation gets a fresh identity");
      $test_utils:assert_false(this:_as_programmer(installed, "travel_from", who, source, []),
        "A saved value cannot select a replacement");
      area:update_passage(source, destination, replacement:with_open(false));
      $test_utils:assert_false(this:_as_programmer(replacement, "travel_from", who, source, []),
        "An old open value cannot bypass the current closed state");
      $test_utils:assert_eq(who.location, source, "Denials leave the actor in place");
    finally
      recycle(who);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod
  method test_native_area_admission_respects_stored_and_supplied_grants owner: ARCH_WIZARD
    "Owning a room does not permit moving it into a foreign area; an add_room grant does.";
    const area = create($area, #90100, 2);
    const room = create($room, #90101, 2);
    try
      $test_utils:assert_eq(this:_as_programmer(room, "moveto", area), E_NACC,
        "Native admission rejects a room owner without area permission");
      $test_utils:assert_eq(room.location, #-1, "Denied movement leaves membership unchanged");
      const capability = $root:issue_capability(area, {'add_room});
      this:_as_programmer(capability, "add_room", room);
      $test_utils:assert_eq(room.location, area, "An unstored bearer authorizes admission");
      move(room, #-1);
      $root:grant_capability(area, {'add_room}, #90101, 'area);
      this:_as_programmer(room, "moveto", area);
      $test_utils:assert_eq(room.location, area, "A stored grant also permits direct room movement");
      move(room, #-1);
      $root:revoke_capability(area, #90101, 'area);
      $test_utils:assert_eq(this:_as_programmer(room, "moveto", area), E_NACC,
        "Revoked admission cannot be bypassed by moving the room directly");
    finally
      recycle(room);
      area:destroy();
    endtry
    return true;
  endmethod
  method test_old_passage_stores_can_be_rebuilt_without_partial_conversion owner: ARCH_WIZARD
    "Validate legacy records before conversion, then index both endpoints and retain a backup.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const other = create($room, #90100, 2);
    const old_relation = area.passages_rel;
    try
      const old = <$passage, .side_a_room = source, .side_b_room = destination, .side_a_label = "east">;
      old_relation:assert($area:_canonical_tuple(source, destination, old));
      const conflict = <$passage, .side_a_room = source, .side_b_room = other, .side_a_label = "east">;
      const conflict_tuple = $area:_canonical_tuple(source, other, conflict);
      old_relation:assert(conflict_tuple);
      $test_utils:assert_eq(`area:rebuild_passages() ! E_INVARG => E_INVARG', E_INVARG,
        "Conflicting names must be resolved before conversion");
      $test_utils:assert_eq(area.passages_rel, old_relation, "Rejected conversion preserves the old store");
      $test_utils:assert_eq(source.passage_areas, {}, "Rejected conversion writes no endpoint indexes");
      old_relation:retract(conflict_tuple);
      $test_utils:assert_eq(`this:_as_programmer(area, "rebuild_passages") ! E_PERM => E_PERM', E_PERM,
        "Only a wizard can install legacy records without fresh endpoint grants");
      $test_utils:assert_eq(area:rebuild_passages(), old_relation, "The conversion returns its retained backup");
      const installed = $room:find_passage(source, "east");
      $test_utils:assert_true(installed.connection_id != "", "Converted connections have identities");
      $test_utils:assert_eq($room:passages(destination), {installed}, "The far endpoint discovers the same value");
      $test_utils:assert_false(area:find_path(destination, source), "An unnamed old return side stays disabled");
      $test_utils:assert_true(valid(old_relation), "The old relation is available for inspection");
    finally
      recycle(source);
      recycle(destination);
      recycle(other);
      area:destroy();
      if (valid(old_relation))
        old_relation:destroy();
      endif
    endtry
    return true;
  endmethod
  method test_far_endpoint_owner_can_disconnect_a_cross_area_passage owner: ARCH_WIZARD
    "A room owner can withdraw its connection without needing ownership of the other area's store.";
    const store = create($area, #90100, 2);
    const other_area = create($area, #90101, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90101, 2);
    try
      move(source, store);
      move(destination, other_area);
      const passage = store:create_passage(source, destination,
        $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true));
      $test_utils:assert_eq(`this:_as_programmer(other_area, "update_passage", destination,
        source, passage:with_open(false)) ! E_PERM => E_PERM', E_PERM,
        "Owning one side does not authorize changing shared configuration");
      $test_utils:assert_true(this:_as_programmer(other_area, "remove_passage", destination,
        source, passage.connection_id), "The far-side owner can disconnect its own endpoint");
      $test_utils:assert_eq($room:passages(source), {}, "The other endpoint loses the removed connection too");
    finally
      recycle(source);
      recycle(destination);
      store:destroy();
      other_area:destroy();
    endtry
    return true;
  endmethod
  method test_door_operations_use_passage_scope_and_carried_keys owner: ARCH_WIZARD
    "Door operations share keyless and keyed policies with traversal, and notify both endpoint rooms.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    const key = create($thing, #90101, 2);
    const wrong_key = create($thing, #90101, 2);
    try
      move(who, source);
      for room in ({source, destination})
        add_property(room, "open_count", 0, {#90100, "r"});
        $reaction:install(room, "opening_reaction", $reaction:mk('on_passage_open,
          "input.From == " + toliteral(source), {{'increment, 'open_count, 1}}));
      endfor
      let draft = $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true, false);
      draft = draft:with_locked(true):with_unlock_rule($rule_engine:parse_expression(
        "input.From == " + toliteral(source) + " AND input.To == " + toliteral(destination),
        'from_here, $rule_engine:passage_scope()));
      const passage = area:create_passage(source, destination, draft);
      const opened = this:_as_programmer($passage, "operate", passage, 'open, who, source);
      $test_utils:assert_eq(opened['status], 'applied, "A keyless rule permits opening with empty inventory");
      $test_utils:assert_true(opened['passage].is_open && !opened['passage].is_locked,
        "Opening performs the permitted unlock and open together");
      $test_utils:assert_eq({source.open_count, destination.open_count}, {1, 1},
        "Each room receives one completed open event");
      const again = this:_as_programmer($passage, "operate", passage, 'open, who, source);
      $test_utils:assert_eq(again['status], 'denied, "Repeating an operation reports its current state");
      $test_utils:assert_eq(source.open_count, 1, "An unchanged door does not fire another event");
      let keyed = opened['passage]:with_open(false):with_locked(true):with_unlock_rule(
        $rule_engine:parse_expression("input.Key == " + toliteral(key), 'brass_key, $rule_engine:passage_scope()));
      keyed = area:update_passage(source, destination, keyed);
      move(key, source);
      $test_utils:assert_eq(this:_as_programmer($passage, "operate", keyed, 'unlock, who, source, key)['status],
        'denied, "A key on the floor cannot be supplied as a carried key");
      $test_utils:assert_false(keyed:check_unlock(who, source), "Traversal rejects the same uncarried key");
      move(wrong_key, who);
      $test_utils:assert_eq(this:_as_programmer($passage, "operate", keyed, 'unlock, who,
        source, wrong_key)['status], 'denied, "Carrying the wrong key does not satisfy the policy");
      $test_utils:assert_false(keyed:check_unlock(who, source), "Traversal rejects the same wrong key");
      move(key, who);
      $test_utils:assert_true(keyed:check_unlock(who, source), "Traversal accepts the carried key");
      $test_utils:assert_eq(this:_as_programmer($passage, "operate", keyed, 'unlock, who, source)['status],
        'applied, "Automatic key selection finds the same carried key");
      const locked = this:_as_programmer($passage, "operate", keyed, 'lock, who, source, key);
      $test_utils:assert_eq(locked['status], 'applied, "The same policy controls locking");
      $test_utils:assert_true(locked['passage].is_locked && !locked['passage].is_open,
        "Locking leaves a closed, locked door");
      move(who, destination);
      $test_utils:assert_eq(this:_as_programmer($passage, "operate", keyed, 'open, who, source)['status],
        'denied, "A supplied source room cannot authorize remote operation");
      $test_utils:assert_eq(`this:_as_programmer($passage, "operate", keyed, 'open, #90100,
        source) ! E_PERM => E_PERM', E_PERM, "Another actor's identity grants no authority");
    finally
      recycle(wrong_key);
      recycle(key);
      recycle(who);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method query_door_race owner: ARCH_WIZARD
    "A trusted test provider changes the connection during a policy callback.";
    this.policy_probe['principal] = caller_perms();
    const {_, inputs, limit} = args;
    const probe = this.policy_probe;
    if (probe['mode] == 'remove)
      probe['area]:remove_passage(probe['source], probe['destination]);
    else
      move(probe['key], probe['source]);
    endif
    suspend(0);
    return ['rows -> {[]}, 'complete -> true];
  endmethod

  method test_door_policy_yields_recheck_connection_and_key_custody owner: ARCH_WIZARD
    "A policy callback cannot resurrect an unlinked door or use a key removed during evaluation.";
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const who = create($actor, #90101, 2);
    const key = create($thing, #90101, 2);
    try
      move(who, source);
      move(key, who);
      $rule_engine:register_predicate('test_door_race,
        ['description -> "Change a door during evaluation.", 'inputs -> ['actor -> 'object], 'outputs -> []],
        this, "query_door_race");
      const rule = $rule_engine:parse_expression("test_door_race(actor: input.Accessor)",
        'race, $rule_engine:passage_scope());
      const draft = $passage:mk(source, "east", {}, "", true, destination, "west", {}, "", true, false)
        :with_locked(true):with_unlock_rule(rule);
      const passage = area:create_passage(source, destination, draft);
      this.policy_probe = ['area -> area, 'source -> source, 'destination -> destination, 'key -> key, 'mode -> 'drop];
      const dropped = this:_as_programmer($passage, "operate", passage, 'open, who, source, key);
      $test_utils:assert_eq(dropped['status], 'denied, "A key that leaves the inventory cannot unlock the door");
      $test_utils:assert_eq(this.policy_probe['principal], #90101, "The predicate receives the initiating principal");
      $test_utils:assert_false(area:passage_for(source, destination).is_open, "Denied access leaves the door closed");
      this.policy_probe['mode] = 'remove;
      const removed = this:_as_programmer($passage, "operate", passage, 'open, who, source);
      $test_utils:assert_eq(removed['status], 'denied, "Removal during evaluation prevents the write");
      $test_utils:assert_false(area:passage_for(source, destination), "The operation does not recreate the removed passage");
    finally
      $rule_engine:unregister_predicate('test_door_race);
      this.policy_probe = [];
      recycle(key);
      recycle(who);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod
endobject
