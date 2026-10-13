// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_WORLD_SCENARIOS
  name: "World Boundary Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true
  override import_export_id = "headless_world_scenarios";
  override import_export_hierarchy = {"tests", "headless"};
  method test_view_and_take_policies_are_independent owner: ARCH_WIZARD
    "An item-specific take rule does not break looking, and both take paths emit the same events.";
    const room = create($room, #90100, 2);
    const box = create($container, #90100, 2);
    const item = create($thing, #90100, 2);
    const npc = create($actor, #90102, 2);
    try
      move(box, room);
      move(npc, room);
      move(item, box);
      box.take_rule = $rule_engine:parse_expression("isa(object: input.Dobj, prototype: " + toliteral($thing) + ")",
        'items_only, $rule_engine:policy_scope('take_rule));
      $test_utils:assert_true(box:can_view_contents(npc), "Viewing needs no item input");
      add_property(box, "takes", 0, {#90100, "r"});
      add_property(item, "gets", 0, {#90100, "r"});
      $reaction:install(box, "take_reaction", $reaction:mk('on_take, 0, {{'increment, 'takes}}));
      $reaction:install(item, "get_reaction", $reaction:mk('on_get, "input.Location == " + toliteral(room), {{'increment, 'gets}}));
      $test_utils:assert_true(this:_action_as_guest(item, "action_get", npc, []), "Owned NPC takes an item");
      $test_utils:assert_eq({item.location, box.takes, item.gets}, {npc, 1, 1}, "Both completed events fire once");
      move(item, box);
      box.view_rule = $rule_engine:parse_expression("FALSE", 'hidden, $rule_engine:policy_scope('view_rule));
      $test_utils:assert_false(box:can_view_contents(npc), "Viewing has its own policy");
      $test_utils:assert_true(this:_action_as_guest(box, "action_take_from", npc, [], item), "Taking retains its own policy");
      $test_utils:assert_eq({box.takes, item.gets}, {2, 2}, "The second entry produces the same two events");
      move(item, box);
      box.take_rule = $rule_engine:parse_expression("FALSE", 'keep, $rule_engine:policy_scope('take_rule));
      add_verb(box, {#90100, "rxd", "can_take_from"}, {"this", "none", "this"});
      set_verb_code(box, "can_take_from", {"return ['allowed -> true];"});
      $test_utils:assert_false(this:_action_as_guest(item, "action_get", npc, []), "An override cannot bypass the actual rule");
      $test_utils:assert_eq({item.location, box.takes, item.gets}, {box, 2, 2}, "Denial leaves custody and events unchanged");
      box.take_rule = 0;
      move(npc, #-1);
      $test_utils:assert_false(this:_action_as_guest(box, "action_take_from", npc, [], item), "A remote actor cannot take");
    finally
      recycle(item);
      recycle(box);
      recycle(npc);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_named_actions_preserve_actor_authority owner: ARCH_WIZARD
    "Named key arguments work for an owner's NPC, and denial differs from an invalid request.";
    const npc = create($actor, #90102, 2);
    const box = create($container, #90100, 2);
    const key = create($thing, #90102, 2);
    try
      move(box, npc);
      move(key, npc);
      box.locked = true;
      box.unlock_rule = $rule_engine:parse_expression("input.Key == " + toliteral(key),
        'key_only, $rule_engine:policy_scope('unlock_rule));
      const arguments = ['target -> box, 'actor -> npc, 'key -> key];
      const applied = this:_action_as_guest($reaction, "perform_action", 'unlock, arguments);
      $test_utils:assert_eq(applied['status], 'applied, "An owned NPC can use an authorized key");
      $test_utils:assert_false(box.locked, "The operation changes the lock");
      const denied = this:_action_as_guest($reaction, "perform_action", 'unlock, arguments);
      $test_utils:assert_eq(denied['status], 'denied, "An already completed operation is a normal denial");
      const missing = this:_action_as_guest($reaction, "perform_action", 'unlock, mapdelete(arguments, 'key));
      $test_utils:assert_eq(missing['status], 'error, "A missing required argument is an error");
      const forged = this:_action_as_guest($reaction, "perform_action", 'unlock,
        mapmerge(arguments, ['actor -> #90100]));
      $test_utils:assert_eq(forged['error][1], E_PERM, "An event participant cannot grant another person's authority");
      box.locked = true;
      $reaction:install(npc, "unlock_reaction", $reaction:mk('on_lock, 0,
        {{'action, 'unlock, ['target -> ['kind -> 'literal, 'value -> box],
          'actor -> ['kind -> 'input, 'name -> 'This], 'key -> ['kind -> 'input, 'name -> 'Key]]}}));
      this:_action_as_guest(npc, "fire_trigger", 'on_lock, ['Actor -> #90102, 'Key -> key]);
      $test_utils:assert_false(box.locked, "Installed effects preserve the owner principal and pass named arguments");
    finally
      recycle(key);
      recycle(box);
      recycle(npc);
    endtry
    return true;
  endmethod

  method test_wear_and_remove_emit_completed_events owner: ARCH_WIZARD
    "Wearing and removal authenticate the actor, emit one event, and preserve worn-item custody.";
    const npc = create($actor, #90102, 2);
    const item = create($wearable, #90100, 2);
    try
      move(item, npc);
      add_property(item, "wears", 0, {#90100, "r"});
      add_property(item, "removes", 0, {#90100, "r"});
      $reaction:install(item, "wear_reaction", $reaction:mk('on_wear, 0, {{'increment, 'wears}}));
      $reaction:install(item, "remove_reaction", $reaction:mk('on_remove, 0, {{'increment, 'removes}}));
      $test_utils:assert_true(this:_action_as_guest(item, "action_wear", npc, []), "The owner can direct an NPC to wear a held item");
      $test_utils:assert_eq({npc.wearing, item.wears}, {{item}, 1}, "One completed wear");
      $test_utils:assert_false(this:_action_as_guest(item, "action_wear", npc, []), "A repeated wear is denied");
      $test_utils:assert_eq(item.wears, 1, "Denial emits no completed event");
      $test_utils:assert_false(this:_action_as_guest(item, "action_drop", npc, []), "A worn item cannot be dropped");
      $test_utils:assert_true(this:_action_as_guest(item, "action_remove", npc, []), "Removal uses the same actor authority");
      $test_utils:assert_eq({npc.wearing, item.removes, item.location}, {{}, 1, npc}, "One removal leaves the item carried");
    finally
      recycle(item);
      recycle(npc);
    endtry
    return true;
  endmethod

  method _action_as_guest owner: RUNTIME_PLAYER
    "Exercise action authority as a non-programmer who owns an NPC.";
    caller == this || raise(E_PERM);
    const {target, method, @parameters} = args;
    return target:(method)(@parameters);
  endmethod

  method test_passage_policy_has_explicit_sides_and_optional_keys owner: ARCH_WIZARD
    "A policy without Key works with an empty inventory; From and To keep their declared meaning.";
    const from = create($room, #90100, 2);
    const to = create($room, #90100, 2);
    const npc = create($actor, #90102, 2);
    const key = create($thing, #90102, 2);
    try
      let passage = $passage:mk(from, "east", {}, "", true, to, "west", {}, "", true);
      passage = passage:with_locked(true);
      const side_rule = $rule_engine:parse_expression("input.From == " + toliteral(from),
        'one_way, $rule_engine:passage_scope());
      passage = passage:with_unlock_rule(side_rule);
      $test_utils:assert_true(passage:check_unlock(npc, from), "A key-independent rule needs no inventory item");
      $test_utils:assert_false(passage:check_unlock(npc, to), "The other endpoint is not silently tried as From");
      const key_rule = $rule_engine:parse_expression("input.Key == " + toliteral(key),
        'key_only, $rule_engine:passage_scope());
      passage = passage:with_unlock_rule(key_rule);
      $test_utils:assert_false(passage:check_unlock(npc, from), "A key-dependent rule needs a key");
      move(key, npc);
      $test_utils:assert_true(passage:check_unlock(npc, from), "A carried matching key grants access");
    finally
      recycle(key);
      recycle(npc);
      recycle(from);
      recycle(to);
    endtry
    return true;
  endmethod

  method test_foreign_area_cleanup_denied owner: ARCH_WIZARD
    "Only authorized lifecycle code may remove another area's passages.";
    const area = create($area, $arch_wizard, 2);
    const room_a = create($room, $arch_wizard, 2);
    const room_b = create($room, $arch_wizard, 2);
    try
      let passage = <$passage, .side_a_room = room_a, .side_b_room = room_b, .is_open = true>;
      passage = area:set_passage(room_a, room_b, passage);
      $test_utils:assert_eq(length(area:passages()), 1, "fixture has a real passage");
      try
        this:_as_programmer("area_cleanup", area, room_a);
      except (E_PERM)
      endtry
      $test_utils:assert_eq(length(area:passages()), 1, "foreign cleanup must preserve the passage");
    finally
      recycle(room_a);
      recycle(room_b);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_passage_foreign_movement_denied owner: ARCH_WIZARD
    "Public passage construction does not authorize moving another owner's object.";
    const room_a = create($room, $arch_wizard, 2);
    const room_b = create($room, $arch_wizard, 2);
    const victim = create($actor, $arch_wizard, 2);
    try
      move(victim, room_a);
      try
        this:_as_programmer("passage_move", victim, room_a, room_b);
      except (E_PERM)
      endtry
      $test_utils:assert_eq(victim.location, room_a, "foreign passage traversal must not move the victim");
    finally
      recycle(victim);
      recycle(room_a);
      recycle(room_b);
    endtry
    return true;
  endmethod

  method _as_programmer owner: RUNTIME_PROGRAMMER
    "Run the public operation as a verified nonwizard programmer.";
    caller == this && this == #90014 || raise(E_PERM);
    const {operation, subject, @rest} = args;
    if (operation == "reaction")
      return $reaction:execute_effect({'set, 'audit_value, 7}, ['Actor -> #90101, 'This -> subject], subject);
    elseif (operation == "area_cleanup")
      return subject:on_room_recycle(rest[1]);
    elseif (operation == "passage_move")
      const {from_room, to_room} = rest;
      const passage = $passage:mk(from_room, "east", {}, "", true, to_room, "west", {}, "", true);
      return passage:travel_from(subject, from_room, []);
    endif
    raise(E_INVARG);
  endmethod
  method test_owner_area_and_room_lifecycle owner: ARCH_WIZARD
    "Nonwizard owners can initialize an area and recycle an owned room in a foreign area.";
    const owned_area = create($area, #90101, 2);
    const foreign_area = create($area, #90100, 2);
    const room_a = create($room, #90101, 2);
    const room_b = create($room, #90100, 2);
    try
      $test_utils:assert_false(this:call_as_programmer(owned_area, "passage_for", room_a, room_b),
        "The owner can query a new area's empty relation");
      $test_utils:assert_false(this:call_as_programmer(foreign_area, "passage_for", room_a, room_b),
        "Public connection discovery also works before a room has any passages");
      let passage = <$passage, .side_a_room = room_a, .side_b_room = room_b, .is_open = true>;
      $test_utils:assert_eq(`this:call_as_programmer(owned_area, "set_passage", room_a, room_b, passage)
        ! E_PERM => E_PERM', E_PERM, "Area ownership does not grant authority over a foreign endpoint");
      $root:grant_capability(room_b, {'dig_from, 'dig_into}, #90101, 'room);
      passage = this:call_as_programmer(owned_area, "set_passage", room_a, room_b, passage);
      $test_utils:assert_eq(length(owned_area:passages()), 1, "owner can register passage");
      $test_utils:assert_eq(this:call_as_programmer(owned_area, "passage_for", room_a, room_b), passage,
        "The owner can query a registered passage");
      this:call_as_programmer(owned_area, "on_room_recycle", room_a);
      $test_utils:assert_eq(length(owned_area:passages()), 0, "owner cleanup succeeds");
      $test_utils:assert_false(this:call_as_programmer(owned_area, "passage_for", room_a, room_b),
        "The owner sees no passage after its removal");
      passage = foreign_area:set_passage(room_a, room_b, passage);
      move(room_a, foreign_area);
      this:call_as_programmer(room_a, "destroy");
      $test_utils:assert_eq(length(foreign_area:passages()), 0, "actual room recycle cleans foreign area's member link");
    finally
      valid(room_a) && recycle(room_a);
      recycle(room_b);
      owned_area:destroy();
      foreign_area:destroy();
    endtry
  endmethod

  method call_as_programmer owner: RUNTIME_PROGRAMMER
    "Invoke a world entry with verified nonwizard permissions.";
    const {target, method, @parameters} = args;
    return target:(method)(@parameters);
  endmethod

  method test_container_rejects_supplied_foreign_actor owner: ARCH_WIZARD
    "A programmer cannot select the owner's identity to open a restricted container.";
    const container = create($container, #90100, 2);
    const prior_location = #90100.location;
    move(#90100, #-1);
    try
      container.open = false;
      container.open_rule = $rule_engine:parse_expression("owner_is(object: input.This, owner: input.Accessor)", 'owner_open, $rule_engine:policy_scope('open_rule));
      const result = `this:call_as_programmer(container, "action_open", #90100, []) ! E_PERM => E_PERM';
      $test_utils:assert_eq(result, E_PERM, "supplied foreign actor denied");
      $test_utils:assert_eq(container.open, false, "denial preserves closed state");
    finally
      move(#90100, prior_location);
      recycle(container);
    endtry
  endmethod

  method test_container_put_rechecks_custody_after_acceptance_yield owner: ARCH_WIZARD
    "A suspended acceptance callback cannot authorize moving an item that its actor no longer holds.";
    const container = create($container, #90100, 2);
    const item = create($thing, #90101, 2);
    const prior_location = #90101.location;
    move(#90101, #-1);
    try
      move(container, #90101);
      move(item, #90101);
      add_verb(container, {#90100, "rxd", "acceptable"}, {"this", "none", "this"});
      set_verb_code(container, "acceptable", {"const {item} = args; move(item, #90103); suspend(0); return true;"});
      const result = this:call_as_programmer(container, "action_put_into", #90101, [], item);
      $test_utils:assert_eq(result, false, "stale custody denies put");
      $test_utils:assert_eq(item.location, #90103, "callback transfer remains in its chosen location");
    finally
      move(#90101, prior_location);
      recycle(item);
      recycle(container);
    endtry
  endmethod

  method test_container_public_actions_and_restricted_denial owner: ARCH_WIZARD
    "Default public open and held-item actions succeed while configured denial preserves state.";
    const container = create($container, #90100, 2);
    const item = create($thing, #90101, 2);
    const prior_location = #90101.location;
    move(#90101, #-1);
    try
      move(container, #90101);
      move(item, #90101);
      $test_utils:assert_eq(this:call_as_programmer(container, "action_close", #90101, []), true, "public close succeeds");
      $test_utils:assert_eq(container.open, false, "close applied");
      $test_utils:assert_eq(this:call_as_programmer(container, "action_open", #90101, []), true, "public open succeeds");
      $test_utils:assert_eq(this:call_as_programmer(container, "action_put_into", #90101, [], item), true, "held put succeeds");
      $test_utils:assert_eq(item.location, container, "item deposited");
      $test_utils:assert_eq(this:call_as_programmer(container, "action_take_from", #90101, [], item), true, "public take succeeds");
      $test_utils:assert_eq(item.location, #90101, "item received");
      container.open = false;
      container.open_rule = $rule_engine:parse_expression("owner_is(object: input.This, owner: input.Accessor)", 'owner_open, $rule_engine:policy_scope('open_rule));
      $test_utils:assert_eq(this:call_as_programmer(container, "action_open", #90101, []), false, "restricted actor denied");
      $test_utils:assert_eq(container.open, false, "restricted denial preserves state");
    finally
      move(#90101, prior_location);
      recycle(item);
      recycle(container);
    endtry
  endmethod

  method test_foreign_area_destroy_preserves_relation owner: ARCH_WIZARD
    "Check area recycle authority before touching its child relation.";
    const area = create($area, #90100, 2);
    const relation = area.passages_rel;
    try
      const denied = `this:call_as_programmer(area, "destroy") ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "foreign area destroy denied");
      $test_utils:assert_true(valid(area) && valid(relation), "both area and child relation survive denial");
    finally
      if (valid(area))
        area:destroy();
      elseif (valid(relation))
        relation:destroy();
      endif
    endtry
  endmethod

  method test_passage_failed_accept_preserves_actor_location owner: ARCH_WIZARD
    "A real destination rejection propagates without moving the authorized traveler.";
    const from_room = create($room, #90100, 2);
    const to_room = create($room, #90100, 2);
    const traveler = create($actor, #90101, 2);
    const area = create($area, #90100, 2);
    try
      move(traveler, from_room);
      const passage = area:create_passage(from_room, to_room,
        $passage:mk(from_room, "east", {}, "", true, to_room, "west", {}, "", true));
      add_verb(to_room, {#90100, "rxd", "accept"}, {"this", "none", "this"});
      set_verb_code(to_room, "accept", {"return false;"});
      const rejected = `this:call_as_programmer(passage, "travel_from", traveler, from_room, []) ! E_NACC => E_NACC';
      $test_utils:assert_eq(rejected, E_NACC, "native destination rejection preserved");
      $test_utils:assert_eq(traveler.location, from_room, "rejection preserves actor location");
    finally
      recycle(traveler);
      recycle(from_room);
      recycle(to_room);
      area:destroy();
    endtry
  endmethod

  method test_passage_helpers_reject_hostile_self_call owner: ARCH_WIZARD
    "An ordinary child cannot choose a wizard principal for inherited relation helpers.";
    const area = create($area, #90101, 2);
    const room_a = create($room, #90100, 2);
    const room_b = create($room, #90100, 2);
    try
      add_verb(area, {#90101, "rxd", "attack_passage"}, {"this", "none", "this"});
      set_verb_code(area, "attack_passage", {"const {a, b} = args; return this:_do_create_passage(a, b, <$passage, .side_a_room = a, .side_b_room = b>, $arch_wizard);"});
      const denied = `area:attack_passage(room_a, room_b) ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "self-call cannot supply a wizard principal");
      $test_utils:assert_eq(length(area:passages()), 0, "denied helper preserves empty relation");
    finally
      recycle(room_a);
      recycle(room_b);
      area:destroy();
    endtry
  endmethod

  method test_passage_authorization_overrides_cannot_grant owner: ARCH_WIZARD
    "A foreign area's permission override cannot authorize raw passage creation.";
    const area = create($area, #90100, 2);
    const room_a = create($room, #90100, 2);
    const room_b = create($room, #90100, 2);
    try
      add_verb(area, {#90101, "rxd", "check_permissions_as"}, {"this", "none", "this"});
      set_verb_code(area, "check_permissions_as", {"return {this, $arch_wizard};"});
      const passage = <$passage, .side_a_room = room_a, .side_b_room = room_b, .side_b_label = "", .side_b_aliases = {}>;
      const denied = `this:call_as_programmer(area, "create_passage", room_a, room_b, passage) ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "foreign override cannot forge authority");
      $test_utils:assert_eq(length(area:passages()), 0, "denied creation preserves relation");
    finally
      recycle(room_a);
      recycle(room_b);
      area:destroy();
    endtry
  endmethod

  method test_passage_update_ignores_fake_existing_link owner: ARCH_WIZARD
    "Source digging authority cannot create a link through an overridden lookup.";
    const area = create($area, #90100, 2);
    const room_a = create($room, #90101, 2);
    const room_b = create($room, #90100, 2);
    try
      add_verb(area, {#90101, "rxd", "passage_for"}, {"this", "none", "this"});
      set_verb_code(area, "passage_for", {"const {a, b} = args; return <$passage, .side_a_room = a, .side_b_room = b>;"});
      const replacement = <$passage, .side_a_room = room_a, .side_b_room = room_b>;
      const denied = `this:call_as_programmer(area, "update_passage", room_a, room_b, replacement) ! E_INVARG => E_INVARG';
      $test_utils:assert_eq(denied, E_INVARG, "update requires actual stored link");
      $test_utils:assert_eq(length(area:passages()), 0, "denied update cannot create a link");
    finally
      recycle(room_a);
      recycle(room_b);
      area:destroy();
    endtry
  endmethod


  method call_as_test_player owner: ARCH_WIZARD
    "Invoke an interaction with the test task player's permissions.";
    caller == this && this == #90014 || raise(E_PERM);
    const {target, method, @parameters} = args;
    set_task_perms(player);
    return target:(method)(@parameters);
  endmethod

  method test_inspection_transfer_reachability owner: ARCH_WIZARD
    "Old inspection actions must neither take remote items nor bypass container access.";
    const room = create($room, #90100, 2);
    const remote = create($room, #90100, 2);
    const item = create($thing, #90100, 2);
    const box = create($container, #90101, 2);
    const who = player;
    const prior_location = who.location;
    "Keep announcements inside the fixture; headless actors have no network connections.";
    add_verb(room, {#90100, "rxd", "announce"}, {"this", "none", "this"});
    set_verb_code(room, "announce", {"return;"});
    try
      move(who, room);
      move(item, room);
      const nearby = item:inspection(who);
      $test_utils:assert_eq(nearby["actions"][2]["label"], "Take", "nearby item offers Take");
      $test_utils:assert_eq(nearby["actions"][2]["command"], "get " + tostr(item),
        "Take names a stable object through the command parser");
      move(item, remote);
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, remote, "stale Take cannot fetch a remote item");
      const distant = item:inspection(who);
      $test_utils:assert_eq(length(distant["actions"]), 1, "remote inspection offers only Examine");
      move(item, room);
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, who, "nearby Take succeeds");
      const held = item:inspection(who);
      $test_utils:assert_eq(held["actions"][2]["label"], "Drop", "fresh inspection reflects custody");
      $test_utils:assert_eq(held["actions"][2]["command"], "drop " + tostr(item),
        "Drop is an ordinary command");
      this:call_as_test_player(item, "drop");
      $test_utils:assert_eq(item.location, room, "held Drop succeeds");
      move(box, room);
      const container_actions = box:inspection(who);
      $test_utils:assert_eq(container_actions["actions"][2]["command"], "get " + tostr(box),
        "taking the container is distinct from taking something out of it");
      move(item, box);
      box.open = false;
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, box, "closed container cannot be bypassed by Take");
      box.open = true;
      box.take_rule = $rule_engine:parse_expression("owner_is(object: input.This, owner: input.Accessor)", 'owner_take, $rule_engine:policy_scope('take_rule));
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, box, "container take policy still applies");
      box.take_rule = 0;
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, who, "open public nearby container permits Take");
      $test_utils:assert_eq(held["state"], {"Carrying"}, "inspection reflects custody");
      for action in (container_actions["actions"])
        $test_utils:assert_type(action["command"], TYPE_STR, "all actions are commands");
        $test_utils:assert_false(maphaskey(action, "verb"), "actions do not invoke methods");
      endfor
    finally
      move(who, prior_location);
      recycle(item);
      recycle(box);
      recycle(room);
      recycle(remote);
    endtry
  endmethod

  method test_inspection_transfers_recheck_after_acceptance_yield owner: ARCH_WIZARD
    "Transfer acceptance callbacks may suspend; final custody and reachability must be checked again.";
    const room = create($room, #90100, 2);
    const item = create($thing, #90100, 2);
    const who = player;
    const prior_location = who.location;
    "Keep announcements inside the fixture; headless actors have no network connections.";
    add_verb(room, {#90100, "rxd", "announce"}, {"this", "none", "this"});
    set_verb_code(room, "announce", {"return;"});
    try
      move(who, room);
      move(item, room);
      add_verb(who, {#90100, "rxd", "acceptable"}, {"this", "none", "this"});
      set_verb_code(who, "acceptable", {"move(args[1], #90103); suspend(0); return true;"});
      this:call_as_test_player(item, "get");
      $test_utils:assert_eq(item.location, #90103, "Take rechecks reachability after acceptance yield");
      delete_verb(who, "acceptable");
      move(item, who);
      add_verb(room, {#90100, "rxd", "acceptable"}, {"this", "none", "this"});
      set_verb_code(room, "acceptable", {"move(args[1], #90103); suspend(0); return true;"});
      this:call_as_test_player(item, "drop");
      $test_utils:assert_eq(item.location, #90103, "Drop rechecks custody after acceptance yield");
    finally
      `delete_verb(who, "acceptable") ! E_VERBNF';
      move(who, prior_location);
      recycle(item);
      recycle(room);
    endtry
  endmethod
  method test_exit_command_annotations owner: ARCH_WIZARD
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      move(source, area);
      move(destination, area);
      let passage = $passage:mk(source, "east", {"e"}, "", false, destination, "west", {"w"}, "", false);
      passage.is_door = true;
      passage = area:set_passage(source, destination, passage);
      const descriptor = source:exit_annotation("east");
      $test_utils:assert_eq(descriptor["command"], "go east", "Cowbell supplies the precise normal command");
      $test_utils:assert_eq(descriptor["exit"]["source"], $url_utils:to_curie_str(source), "source is explicit");
      $test_utils:assert_eq(descriptor["exit"]["destination"], $url_utils:to_curie_str(destination), "destination is explicit");
      $test_utils:assert_eq(source:exit_annotation("e")["exit"], descriptor["exit"], "aliases describe the same passage");
      const snapshot = source:room_snapshot(player);
      $test_utils:assert_eq(mapvalues(snapshot["annotations"])[1], descriptor, "pinned room uses the same command descriptor");
      area:update_passage(source, destination, passage:with_open(false));
      $test_utils:assert_eq(source:exit_annotation("east")["exit"], descriptor["exit"], "door state does not change identity");
      $test_utils:assert_eq(source:exit_annotation("east")["command"], "open east", "closed doors offer opening");
      passage = area:set_passage(source, destination, passage);
      $test_utils:assert_true(source:exit_annotation("east")["exit"]["passage"] != descriptor["exit"]["passage"], "replacement has a new semantic identity");
    finally
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_travel_rechecks_after_pre_exit_yield owner: ARCH_WIZARD
    "A pre-exit callback cannot replace or close the checked passage before movement.";
    const who = player;
    const prior_location = who.location;
    const area = create($area, #90100, 2);
    const source = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    try
      move(source, area);
      move(destination, area);
      move(who, source);
      let passage = $passage:mk(source, "east", {}, "", false, destination, "west", {}, "", false);
      passage = area:set_passage(source, destination, passage);
      add_property(source, "test_destination", destination, {#90100, "r"});
      add_verb(source, {#90100, "rxd", "notify_pre_exit"}, {"this", "none", "this"});
      set_verb_code(source, "notify_pre_exit", {
        "const area = this.location; const destination = this.test_destination;",
        "let passage = area:passage_for(this, destination);",
        "area:update_passage(this, destination, passage:with_open(false)); suspend(0);"
      });
      const closed = this:call_as_test_player(area, "handle_passage_command", ['verb -> "go", 'dobjstr -> "east"]);
      $test_utils:assert_true(closed, "A matched direction is handled even when its passage closes");
      $test_utils:assert_eq(who.location, source, "closing during callback preserves actor location");
      area:update_passage(source, destination, passage);
      set_verb_code(source, "notify_pre_exit", {
        "const area = this.location; const destination = this.test_destination;",
        "let passage = area:passage_for(this, destination);",
        "area:set_passage(this, destination, passage); suspend(0);"
      });
      const replaced = this:call_as_test_player(area, "handle_passage_command", ['verb -> "go", 'dobjstr -> "east"]);
      $test_utils:assert_true(replaced, "A matched direction is handled even when its passage is replaced");
      $test_utils:assert_eq(who.location, source, "replacement during callback preserves actor location");
    finally
      move(who, prior_location);
      recycle(source);
      recycle(destination);
      area:destroy();
    endtry
  endmethod

  method test_contextual_suggestions owner: ARCH_WIZARD
    "Suggestions share scopes and command matching, preserve aliases, and never execute candidate verbs.";
    const prior = player.location;
    const prior_wearing = player.wearing;
    const room = create($room, #90100, 2);
    const box = create($container, #90100, 2);
    const alpha = create($thing, player, 2);
    const beta = create($thing, player, 2);
    try
      move(player, room);
      move(box, room);
      move(alpha, player);
      player.wearing = {#-1};
      move(beta, player);
      alpha.name = "Silver token";
      alpha.aliases = {"coin"};
      beta.name = "Copper token";
      add_verb(alpha, {player, "rd", "polish"}, {"this", "none", "none"});
      set_verb_code(alpha, "polish", {"raise(E_ASSERT, \"Suggestion executed a command\");"});
      const composer = player:command_input_context("polish ", "");
      $test_utils:assert_eq(composer["source"]["context"]["template"], "polish {input}", "main composer context comes from the parser");
      $test_utils:assert_eq(composer["before"], "polish ", "context preserves the exact raw prefix");
      const middle = player:command_input_context("fit tok", "en with " + tostr(alpha));
      $test_utils:assert_eq(middle["query"], "token", "cursor in a word identifies the whole argument");
      $test_utils:assert_eq(middle["after"], " with " + tostr(alpha), "bound counterpart survives unchanged");
      $test_utils:assert_eq(player:command_input_context("fit two  tok", "ens with " + tostr(alpha)), [], "normalized ranges are not guessed");
      const filtered = player:suggestions("inventory", "token", $match:input_context("polish {input}"));
      $test_utils:assert_eq({row["value"] for row in (filtered["items"])}, {tostr(alpha)}, "argspec matcher excludes objects without the command");
      $test_utils:assert_eq(player:suggestions("inventory", "token", $match:input_context("polish {input} with " + tostr(box)))["items"], {}, "preposition mismatch is not suggested");
      $test_utils:assert_eq(player:suggestions("inventory", "COIN")["items"][1]["value"], tostr(alpha), "aliases match without case sensitivity");
      const limited = player:suggestions("inventory", "token", [], 1);
      $test_utils:assert_eq(length(limited["items"]), 1, "limit bounds the response");
      $test_utils:assert_eq(limited["more"], true, "additional choices are indicated");
      const entries = $match:object_suggestions({alpha, {alpha, "scope alias"}});
      $test_utils:assert_eq(length(entries), 1, "duplicate scope entries collapse");
      $test_utils:assert_eq($match:rank_suggestions(entries, "scope alias", 12)["items"][1]["value"], tostr(alpha), "environment aliases survive duplicate entries");
      add_verb(alpha, {player, "rd", "fit"}, {"any", "with", "this"});
      set_verb_code(alpha, "fit", {"raise(E_ASSERT, \"Suggestion executed a command\");"});
      $test_utils:assert_eq(player:suggestions("inventory", "token", $match:input_context("fit " + tostr(box) + " with {input}"))["items"][1]["value"], tostr(alpha), "indirect argument slots use the same matcher");
      const partial_direct = $match:input_context("fit {dobj} with {iobj}", "dobj");
      const partial_indirect = $match:input_context("fit {dobj} with {iobj}", "iobj");
      $test_utils:assert_eq(length(player:suggestions("inventory", "token", partial_direct)["items"]), 2, "any direct argspec permits either candidate while receiver is unbound");
      $test_utils:assert_eq({partial_row["value"] for partial_row in (player:suggestions("inventory", "token", partial_indirect)["items"])}, {tostr(alpha)}, "unbound direct argument does not erase the indirect this constraint");
      const narrowed = $match:input_context("fit {dobj} with {iobj}", "dobj", ["iobj" -> tostr(beta)]);
      $test_utils:assert_eq(player:suggestions("inventory", "", narrowed)["items"], {}, "binding an incompatible receiver invalidates dependent choices");
      const beta_ref = $url_utils:to_curie_str(beta);
      $test_utils:assert_true(player:suggestion_eligibility("inventory", {beta_ref})[beta_ref]["eligible"], "explicit eligibility checks the full scope, not the first ranked page");
      move(beta, box);
      $test_utils:assert_eq(box:suggestions("contents", "", $match:input_context("get {input} from " + tostr(box)))["items"][1]["value"], tostr(beta), "container scope can expand beyond ordinary nearby matches");
      box.open = false;
      $test_utils:assert_true(!box:suggestion_eligibility("contents", {beta_ref})[beta_ref]["eligible"], "closed content eligibility reveals no label");
      $test_utils:assert_eq(box:suggestions("contents", "")["items"], {}, "closed contents stay private");
      box.open = true;
      box.view_rule = $rule_engine:parse_expression("owner_is(object: input.This, owner: input.Accessor)", 'owner_view, $rule_engine:policy_scope('view_rule));
      box.owner = player;
      $test_utils:assert_eq({owner_row["value"] for owner_row in (box:suggestions("contents", "")["items"])}, {tostr(beta)}, "viewing rules allow the owner to see contents");
      $test_utils:assert_true(box:suggestion_eligibility("contents", {beta_ref})[beta_ref]["eligible"], "owner-visible contents remain eligible");
      box.owner = player == #90101 ? #90100 | #90101;
      $test_utils:assert_eq(box:suggestions("contents", "")["items"], {}, "viewing rules deny nonowner suggestions");
      const denied = box:suggestion_eligibility("contents", {beta_ref})[beta_ref];
      $test_utils:assert_false(denied["eligible"], "viewing rules deny nonowner eligibility");
      $test_utils:assert_false(maphaskey(denied, "label"), "denied eligibility does not reveal the item name");
      box.view_rule = 0;
      move(box, #90103);
      $test_utils:assert_eq(box:suggestions("contents", "")["items"], {}, "distant containers do not expose contents");
      const foreign = `#90101:suggestions("inventory", "") ! E_PERM => E_PERM';
      $test_utils:assert_eq(foreign, E_PERM, "another player's inventory is not queryable");
      $test_utils:assert_true(length(player:suggestions("commands", "loo")["items"]) > 0, "command names use the existing verb catalog");
    finally
      player.wearing = prior_wearing;
      move(player, prior);
      recycle(alpha);
      recycle(beta);
      recycle(box);
      recycle(room);
    endtry
  endmethod

endobject

object #90108
  name: "Runtime World Area"
  parent: AREA
  owner: RUNTIME_WIZARD
endobject

object #90109
  name: "Runtime World West Room"
  parent: ROOM
  owner: RUNTIME_WIZARD
  location: #90108
endobject

object #90110
  name: "Runtime World East Room"
  parent: ROOM
  owner: RUNTIME_WIZARD
  location: #90108
endobject

object #90111
  name: "Runtime World Container"
  parent: CONTAINER
  owner: RUNTIME_WIZARD
  location: RUNTIME_ROOM
endobject

object #90112
  name: "Runtime World Item"
  parent: THING
  owner: RUNTIME_PLAYER
  location: RUNTIME_PLAYER

endobject
