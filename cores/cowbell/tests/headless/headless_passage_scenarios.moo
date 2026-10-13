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
        const state_text = format == 'text_djot ? "east \\(closed\\)" | "east (closed)";
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
