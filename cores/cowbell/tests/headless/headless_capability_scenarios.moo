object HEADLESS_CAPABILITY_SCENARIOS
  name: "Headless Capability Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for capability issuance, grants, merges, revocation, and denial paths.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_capability_scenarios";

  verb _test_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Return the deterministic PASETO key used by capability scenarios.";
    return "dGVzdHRlc3R0ZXN0dGVzdHRlc3R0ZXN0dGVzdHRlc3Q=";
  endverb

  verb test_headless_llm_public_methods_deny_non_owner (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Public LLM-agent state methods reject a non-owner without wizard permissions.";
    agent = #-1;
    try
      agent = $llm_agent:create(false);
      agent.owner = $hacker;
      $test_utils:assert_false($player.wizard, "fixture caller must not be a wizard");
      $test_utils:assert_true(agent.owner != $player, "fixture caller must not own the agent");
      for invocation in ({{agent, "get_project"}, {agent, "reset_tool_failures"},
                         {agent:get_project(), "get_task_status"}, {agent:get_project(), "get_todos"}})
        {subject, method} = invocation;
        denied = false;
        try
          this:_llm_public_call_as_player(subject, method);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, method + " should deny a non-owner caller");
      endfor
    finally
      valid(agent) && agent:destroy();
    endtry
    return true;
  endverb

  verb _llm_public_call_as_player (this none this) owner: PLAYER flags: "rxd"
    caller == this && this == #90005 || raise(E_PERM);
    {agent, method} = args;
    method in {"get_project", "get_task_status", "get_todos", "reset_tool_failures"} || raise(E_PERM);
    return agent:(method)();
  endverb

  verb test_headless_capability_issue_and_deny (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: issued capabilities validate granted permissions and deny invalid requests.";
    key = this:_test_key();
    target = #-1;
    try
      target = create($thing);
      cap = target:issue_capability(target, {'read, 'write}, 0, 0, key);
      $test_utils:assert_type(cap, TYPE_FLYWEIGHT, "issued capability should be a flyweight");
      $test_utils:assert_eq(cap.delegate, target, "issued capability delegate should match target");
      $test_utils:assert_true(maphaskey(flyslots(cap), 'token), "issued capability should include a token slot");
      {validated_target, run_as} = cap:challenge_for_with_key({'read}, key);
      $test_utils:assert_eq(validated_target, target, "challenge should return the capability target");
      $test_utils:assert_eq(run_as, $hacker, "plain capability should run as $hacker");
      missing_denied = false;
      try
        cap:challenge_for_with_key({'delete}, key);
      except (E_PERM)
        missing_denied = true;
      endtry
      $test_utils:assert_true(missing_denied, "missing permission should be denied");
      expired_cap = target:issue_capability(target, {'read}, time() - 1, 0, key);
      expired_denied = false;
      try
        expired_cap:challenge_for_with_key({'read}, key);
      except (E_PERM)
        expired_denied = true;
      endtry
      $test_utils:assert_true(expired_denied, "expired capability should be denied");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_tampered_token_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: capability validation rejects tampered token slots.";
    key = this:_test_key();
    target = #-1;
    try
      target = create($thing);
      cap = target:issue_capability(target, {'read}, 0, 0, key);
      {validated_target, _} = cap:challenge_for_with_key({'read}, key);
      $test_utils:assert_eq(validated_target, target, "untampered capability should validate before tamper test");
      slots = flyslots(cap);
      original_token = slots['token];
      last_char = original_token[$..$];
      replacement_char = last_char == "A" ? "B" | "A";
      slots['token] = original_token[1..$ - 1] + replacement_char;
      tampered_cap = toflyweight(cap.delegate, slots, flycontents(cap));
      denied = false;
      try
        tampered_cap:challenge_for_with_key({'read}, key);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "tampered capability token should be denied");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_grant_merge_and_revoke (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: stored grants merge capabilities, can be found, and can be revoked.";
    key = this:_test_key();
    test_area = #-1;
    test_room = #-1;
    test_player = #-1;
    try
      test_area = create($area);
      test_room = create($room);
      test_player = create($player);
      area_cap = $root:grant_capability(test_area, {'add_room}, test_player, 'area, key);
      $test_utils:assert_type(area_cap, TYPE_FLYWEIGHT, "grant should return a capability flyweight");
      $test_utils:assert_eq(area_cap.delegate, test_area, "area grant delegate should match target");
      $test_utils:assert_eq(test_player:find_capability_for(test_area, 'area), area_cap, "stored area grant should be discoverable");
      $root:grant_capability(test_area, {'create_passage}, test_player, 'area, key);
      merged_cap = test_player:find_capability_for(test_area, 'area);
      {merged_target, _} = merged_cap:challenge_for_with_key({'add_room, 'create_passage}, key);
      $test_utils:assert_eq(merged_target, test_area, "merged grant should validate both area permissions");
      room_cap = $root:grant_capability(test_room, {'dig_from}, test_player, 'room, key);
      $test_utils:assert_eq(test_player:find_capability_for(test_room, 'room), room_cap, "room grant should be stored separately");
      $test_utils:assert_true($root:revoke_capability(test_area, test_player, 'area), "revoke should remove an existing grant");
      $test_utils:assert_false(test_player:find_capability_for(test_area, 'area), "revoked area grant should not be found");
      $test_utils:assert_false($root:revoke_capability(test_area, test_player, 'area), "revoking a missing grant should return false");
      $test_utils:assert_eq(test_player:find_capability_for(test_room, 'room), room_cap, "revoking area grant should not affect room grants");
    finally
      valid(test_player) && test_player:destroy();
      valid(test_room) && test_room:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_grant_allows_non_owner_validation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a non-owner can find and validate a stored grant.";
    key = this:_test_key();
    test_area = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'add_room}, $player, 'area, key);
      {target, run_as} = this:_validate_area_grant_as_player(test_area, key);
      $test_utils:assert_eq(target, test_area, "non-owner should validate stored area grant target");
      $test_utils:assert_eq(run_as, $hacker, "plain stored grant should run as $hacker");
    finally
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_grant_requires_target_owner_or_wizard (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: non-owners cannot grant capabilities for objects they do not control.";
    key = this:_test_key();
    test_area = #-1;
    try
      test_area = create($area);
      denied = false;
      try
        this:_grant_area_capability_as_player(test_area, {'add_room}, $player, 'area, key);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner grant_capability should be denied");
      $test_utils:assert_false($player:find_capability_for(test_area, 'area), "denied grant should not be stored");
    finally
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_owner_can_grant_for_owned_target (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a non-wizard owner can grant capabilities for an owned target.";
    key = this:_test_key();
    test_area = #-1;
    try
      test_area = create($area);
      test_area:set_owner($player);
      cap = this:_grant_area_capability_as_player(test_area, {'add_room}, $player, 'area, key);
      $test_utils:assert_type(cap, TYPE_FLYWEIGHT, "owner grant should return a capability");
      $test_utils:assert_eq($player:find_capability_for(test_area, 'area), cap, "owner grant should be stored");
      {target, _} = cap:challenge_for_with_key({'add_room}, key);
      $test_utils:assert_eq(target, test_area, "owner grant should validate target");
    finally
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_grant_allows_non_owner_room_creation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a non-owner with an area add_room grant can create a room in that area.";
    test_area = #-1;
    created = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'add_room}, $player, 'area);
      created = this:_make_room_with_area_grant_as_player(test_area);
      $test_utils:assert_true(valid(created), "stored area grant should create a valid room");
      $test_utils:assert_eq(created.location, test_area, "stored area grant should place created room in area");
      $test_utils:assert_true(created in test_area:contents(), "stored area grant should add created room to area contents");
    finally
      valid(created) && created:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_missing_add_room_denies_room_creation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: an area grant without add_room must not create rooms.";
    test_area = #-1;
    created = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'create_passage}, $player, 'area);
      denied = false;
      try
        created = this:_make_room_with_area_grant_as_player(test_area);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "area grant missing add_room should deny room creation");
      $test_utils:assert_false(valid(created), "denied missing-add_room grant should not leave a created room");
    finally
      valid(created) && created:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_wrong_category_denies_room_creation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: an add_room grant stored under the wrong category must not authorize area room creation.";
    test_area = #-1;
    created = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'add_room}, $player, 'room);
      denied = false;
      try
        created = this:_make_room_with_area_grant_as_player(test_area);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "wrong-category add_room grant should not be found as an area grant");
      $test_utils:assert_false(valid(created), "denied wrong-category grant should not leave a created room");
    finally
      valid(created) && created:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_direct_bearer_allows_non_owner_room_creation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a bearer area add_room capability can create a room when used by a non-owner.";
    test_area = #-1;
    created = #-1;
    try
      test_area = create($area);
      cap = $root:issue_capability(test_area, {'add_room});
      created = this:_make_room_with_cap_as_player(cap);
      $test_utils:assert_true(valid(created), "bearer add_room capability should create a valid room");
      $test_utils:assert_eq(created.location, test_area, "bearer add_room capability should place created room in area");
    finally
      valid(created) && created:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_passage_creation_requires_area_and_room_grants (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: passage creation by non-owner requires area and room capabilities together.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      area_cap = $root:grant_capability(test_area, {'create_passage}, $player, 'area);
      from_cap = $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      to_cap = $root:grant_capability(room_b, {'dig_into}, $player, 'room);
      passage = $passage:mk(room_a, "east", {"east", "e"}, "", true, room_b, "", {}, "", false, true);
      created = this:_create_passage_with_caps_as_player(area_cap, from_cap, to_cap, passage);
      $test_utils:assert_true(created.connection_id != "", "Registration assigns an identity");
      $test_utils:assert_eq(created.side_a_room, room_a, "Registration preserves the authorized source");
      $test_utils:assert_eq(created.side_b_room, room_b, "Registration preserves the authorized destination");
      $test_utils:assert_eq(test_area:passage_for(room_a, room_b), created, "Creation returns the installed passage");
    finally
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_passage_creation_missing_room_grant_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: area create_passage and source dig_from are not enough without destination dig_into.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      area_cap = $root:grant_capability(test_area, {'create_passage}, $player, 'area);
      from_cap = $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      passage = $passage:mk(room_a, "east", {"east", "e"}, "", true, room_b, "", {}, "", false, true);
      denied = false;
      try
        this:_create_passage_with_caps_as_player(area_cap, from_cap, room_b, passage);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "missing destination dig_into should deny passage creation");
      $test_utils:assert_false(test_area:passage_for(room_a, room_b), "denied passage creation should not register a passage");
    finally
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_passage_removal_requires_area_and_room_grants (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: passage removal by non-owner requires area remove_passage and source room dig_from.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      passage = $passage:mk(room_a, "east", {"east", "e"}, "", true, room_b, "", {}, "", false, true);
      passage = test_area:create_passage(room_a, room_b, passage);
      $test_utils:assert_eq(test_area:passage_for(room_a, room_b), passage, "wizard setup should register passage");
      area_cap = $root:grant_capability(test_area, {'remove_passage}, $player, 'area);
      from_cap = $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      result = this:_remove_passage_with_caps_as_player(area_cap, from_cap, room_b);
      $test_utils:assert_true(result, "capability passage removal should report success");
      $test_utils:assert_false(test_area:passage_for(room_a, room_b), "capability passage removal should clear the passage");
    finally
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_update_passage_requires_source_room_grant (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: updating an existing passage requires only source-room dig_from capability.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      original = $passage:mk(room_a, "east", {"east", "e"}, "", true, room_b, "", {}, "", false, true);
      original = test_area:create_passage(room_a, room_b, original);
      replacement = original:with_label_from(room_a, "portal"):with_aliases_from(room_a, {"portal", "p"});
      $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      updated = this:_update_passage_as_player(test_area, room_a, room_b, replacement);
      $test_utils:assert_eq(updated, replacement, "capability update_passage should return the replacement passage");
      $test_utils:assert_eq(test_area:passage_for(room_a, room_b), replacement, "capability update_passage should replace the existing passage");
    finally
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_update_passage_missing_source_grant_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: update_passage must deny a non-owner without source-room dig_from capability.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      original = $passage:mk(room_a, "east", {"east", "e"}, "", true, room_b, "", {}, "", false, true);
      original = test_area:create_passage(room_a, room_b, original);
      replacement = original:with_label_from(room_a, "portal"):with_aliases_from(room_a, {"portal", "p"});
      denied = false;
      try
        this:_update_passage_as_player(test_area, room_a, room_b, replacement);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "missing source dig_from should deny update_passage");
      $test_utils:assert_eq(test_area:passage_for(room_a, room_b), original, "denied update_passage should preserve original passage");
    finally
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_agent_building_tools_use_stored_grants (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: agent building tools find stored area and room capabilities for delegated building.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    built_room = #-1;
    old_player_location = $player.location;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      room_a:set_name_aliases("agent source room", {"agent-source"});
      room_b:set_name_aliases("agent target room", {"agent-target"});
      $player:moveto(room_a);
      $root:grant_capability(test_area, {'add_room, 'create_passage, 'remove_passage}, $player, 'area);
      $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      $root:grant_capability(room_b, {'dig_into}, $player, 'room);
      build_result = $llm_building_tools:build_room(["name" -> "agent cap room", "area" -> tostr(test_area)], $player);
      built_room = this:_find_room_named_in_area(test_area, "agent cap room");
      $test_utils:assert_true(valid(built_room), "agent build_room should create a room through stored area grant");
      $test_utils:assert_true(index(build_result, "Created \"agent cap room\"") == 1, "agent build_room should report creation");
      dig_result = $llm_building_tools:dig_passage(["source_room" -> tostr(room_a), "direction" -> "north,n", "target_room" -> tostr(room_b), "oneway" -> true], $player);
      $test_utils:assert_true(index(dig_result, "Dug passage: north,n") == 1, "agent dig_passage should report passage creation");
      $test_utils:assert_true(typeof(test_area:passage_for(room_a, room_b)) == TYPE_FLYWEIGHT, "agent dig_passage should register passage");
      remove_result = $llm_building_tools:remove_passage(["source_room" -> room_a, "target_room" -> room_b], $player);
      $test_utils:assert_true(index(remove_result, "Removed passage") == 1, "agent remove_passage should report removal");
      $test_utils:assert_false(test_area:passage_for(room_a, room_b), "agent remove_passage should clear passage");
    finally
      valid(built_room) && built_room:destroy();
      valid(old_player_location) && $player:moveto(old_player_location);
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_agent_build_room_missing_grant_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: agent build_room denies non-owner room creation without add_room.";
    test_area = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'create_passage}, $player, 'area);
      result = $llm_building_tools:build_room(["name" -> "agent denied room", "area" -> tostr(test_area)], $player);
      $test_utils:assert_true(result.status == 'failure && result.code == "permission_denied", "agent build_room should report denied add_room");
      $test_utils:assert_false(valid(this:_find_room_named_in_area(test_area, "agent denied room")), "denied agent build_room should not create a room");
    finally
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_agent_dig_passage_missing_grants_deny (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: agent dig_passage denies missing area, source-room, or destination-room grants.";
    area_missing = #-1;
    from_missing = #-1;
    to_missing = #-1;
    try
      area_missing = create($area);
      room_a = area_missing:make_room_in($room);
      room_b = area_missing:make_room_in($room);
      $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      $root:grant_capability(room_b, {'dig_into}, $player, 'room);
      denied = false;
      try
        $llm_building_tools:dig_passage(["source_room" -> tostr(room_a), "direction" -> "north", "target_room" -> tostr(room_b), "oneway" -> true], $player);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "agent dig_passage should deny missing area create_passage");
      $test_utils:assert_false(area_missing:passage_for(room_a, room_b), "denied missing-area dig should not register passage");
      from_missing = create($area);
      room_c = from_missing:make_room_in($room);
      room_d = from_missing:make_room_in($room);
      $root:grant_capability(from_missing, {'create_passage}, $player, 'area);
      $root:grant_capability(room_d, {'dig_into}, $player, 'room);
      result = $llm_building_tools:dig_passage(["source_room" -> tostr(room_c), "direction" -> "east", "target_room" -> tostr(room_d), "oneway" -> true], $player);
      $test_utils:assert_true(result.status == 'failure && result.code == "permission_denied", "agent dig_passage should report denied source dig_from");
      $test_utils:assert_false(from_missing:passage_for(room_c, room_d), "denied missing-source dig should not register passage");
      to_missing = create($area);
      room_e = to_missing:make_room_in($room);
      room_f = to_missing:make_room_in($room);
      $root:grant_capability(to_missing, {'create_passage}, $player, 'area);
      $root:grant_capability(room_e, {'dig_from}, $player, 'room);
      result = $llm_building_tools:dig_passage(["source_room" -> tostr(room_e), "direction" -> "west", "target_room" -> tostr(room_f), "oneway" -> true], $player);
      $test_utils:assert_true(result.status == 'failure && result.code == "permission_denied", "agent dig_passage should report denied destination dig_into");
      $test_utils:assert_false(to_missing:passage_for(room_e, room_f), "denied missing-destination dig should not register passage");
    finally
      valid(to_missing) && to_missing:destroy();
      valid(from_missing) && from_missing:destroy();
      valid(area_missing) && area_missing:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_agent_remove_passage_missing_grants_deny (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: agent remove_passage denies missing area or source-room grants.";
    area_missing = #-1;
    from_missing = #-1;
    old_player_location = $player.location;
    try
      area_missing = create($area);
      room_a = area_missing:make_room_in($room);
      room_b = area_missing:make_room_in($room);
      passage = $passage:mk(room_a, "north", {"north"}, "", true, room_b, "", {}, "", false, true);
      passage = area_missing:create_passage(room_a, room_b, passage);
      $test_utils:assert_eq(room_a.location, area_missing, "remove fixture source should be in area");
      $test_utils:assert_eq(room_b.location, area_missing, "remove fixture target should be in area");
      $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      $player:moveto(room_a);
      denied = false;
      try
        $llm_building_tools:remove_passage(["source_room" -> room_a, "target_room" -> room_b], $player);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "agent remove_passage should deny missing area remove_passage");
      $test_utils:assert_eq(area_missing:passage_for(room_a, room_b), passage, "denied missing-area remove should preserve passage");
      from_missing = create($area);
      room_c = from_missing:make_room_in($room);
      room_d = from_missing:make_room_in($room);
      passage2 = $passage:mk(room_c, "east", {"east"}, "", true, room_d, "", {}, "", false, true);
      passage2 = from_missing:create_passage(room_c, room_d, passage2);
      $root:grant_capability(from_missing, {'remove_passage}, $player, 'area);
      $player:moveto(room_c);
      result = $llm_building_tools:remove_passage(["source_room" -> room_c, "target_room" -> room_d], $player);
      $test_utils:assert_true(result.status == 'failure && result.code == "permission_denied", "agent remove_passage should report denied source dig_from");
      $test_utils:assert_eq(from_missing:passage_for(room_c, room_d), passage2, "denied missing-source remove should preserve passage");
    finally
      valid(old_player_location) && $player:moveto(old_player_location);
      valid(from_missing) && from_missing:destroy();
      valid(area_missing) && area_missing:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_create_child_allows_non_owner_nonfertile_create (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: create_child capability lets a non-owner create from a non-fertile parent.";
    parent_obj = #-1;
    child = #-1;
    try
      parent_obj = create($thing);
      parent_obj.f = 0;
      cap = $root:issue_capability(parent_obj, {'create_child});
      child = this:_create_child_with_cap_as_player(cap);
      $test_utils:assert_true(valid(child), "create_child capability should create a valid child");
      $test_utils:assert_eq(parent(child), parent_obj, "create_child capability should create under the target parent");
      $test_utils:assert_eq(child.owner, $player, "capability child creation should still own the child to the caller");
    finally
      valid(child) && child:destroy();
      valid(parent_obj) && parent_obj:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_description_allows_non_owner_object_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: set_description capability lets a non-owner update only the delegated object.";
    target = #-1;
    try
      target = create($thing);
      target:set_description("unchanged");
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      cap = $root:issue_capability(target, {'set_description});
      this:_set_description_with_cap_as_player(cap, "capability description");
      $test_utils:assert_eq(target.description, "capability description", "set_description capability should update target description");
      $test_utils:assert_true(target.owner != $player, "capability use should not transfer ownership");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_move_and_recycle_install_runtime_grants (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: root mutation capabilities install narrow runtime grants for delegated movement and recycle operations.";
    target = #-1;
    new_home = #-1;
    doomed = #-1;
    try
      target = create($thing);
      new_home = create($room);
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      move_cap = $root:issue_capability(target, {'move});
      this:_move_with_cap_as_player(move_cap, new_home);
      $test_utils:assert_true(target.owner != $player, "capability use should not transfer ownership");
      $test_utils:assert_eq(target.location, new_home, "move capability should update location");
      doomed = create($thing);
      $test_utils:assert_true(doomed.owner != $player, "fixture player should not own the doomed object");
      recycle_cap = $root:issue_capability(doomed, {'recycle});
      this:_destroy_with_cap_as_player(recycle_cap);
      $test_utils:assert_false(valid(doomed), "recycle capability should destroy the target");
      doomed = #-1;
    finally
      valid(doomed) && doomed:destroy();
      valid(target) && target:destroy();
      valid(new_home) && new_home:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_owner_allows_non_owner_object_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a verified set_owner capability lets a non-owner transfer ownership.";
    target = #-1;
    try
      target = create($thing);
      original_owner = target.owner;
      add_property(target, "set_owner_probe", 1, {original_owner, "rc"});
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      cap = $root:issue_capability(target, {'set_owner});
      this:_set_owner_with_cap_as_player(cap, $player);
      $test_utils:assert_eq(target.owner, $player, "set_owner capability should update owner");
      $test_utils:assert_eq(property_info(target, "set_owner_probe")[1], $player, "set_owner capability should retitle clobber properties");
      $test_utils:assert_eq(property_info(target, "description_kind")[1], $player,
        "ownership transfer includes inherited properties with the c flag");
      $test_utils:assert_eq(property_info($root, "description_kind")[1], $hacker,
        "ownership transfer must not change the ancestor's property owner");
      $test_utils:assert_true(original_owner != target.owner, "set_owner test fixture should actually change ownership");
      $test_utils:assert_false($player.wizard, "capability use should not make actor a wizard");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_owner_rejects_invalid_authority (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Changing ownership requires the target owner, wizard, or a valid target-bound capability.";
    target = #-1;
    other = #-1;
    try
      target = create($thing);
      other = create($thing);
      original_owner = target.owner;
      add_property(target, "set_owner_probe", 1, {original_owner, "rc"});
      wrong_cap = target:issue_capability(target, {'set_description});
      expired_cap = target:issue_capability(target, {'set_owner}, time() - 1);
      revoked_cap = target:issue_capability(target, {'set_owner});
      $root:_revoke_capability_token(revoked_cap);
      other_cap = other:issue_capability(other, {'set_owner});
      rebound_cap = toflyweight(target, flyslots(other_cap), flycontents(other_cap));
      for attempt in ({{target, "no capability"}, {wrong_cap, "wrong permission"}, {expired_cap, "expired"}, {revoked_cap, "revoked"}, {rebound_cap, "other target"}})
        denied = false;
        try
          this:_set_owner_with_cap_as_player(attempt[1], $player);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, attempt[2] + " should not change owner");
        $test_utils:assert_eq(target.owner, original_owner, attempt[2] + " changed target owner");
        $test_utils:assert_eq(property_info(target, "set_owner_probe")[1], original_owner, attempt[2] + " changed property owner");
      endfor
      $test_utils:assert_eq(other.owner, original_owner, "other target grant should not change its own target");
    finally
      valid(other) && other:destroy();
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_owner_ignores_overridden_checker (this none this) owner: ARCH_WIZARD flags: "rxd"
    "An inherited wizard setter must not trust a child object's authorization override.";
    target = #-1;
    try
      target = create($thing);
      original_owner = target.owner;
      add_property(target, "set_owner_probe", 1, {original_owner, "rc"});
      add_verb(target, {$arch_wizard, "rxd", "check_permissions_as"}, {"this", "none", "this"});
      set_verb_code(target, "check_permissions_as", {"return {this, $arch_wizard};"});
      denied = false;
      try
        this:_set_owner_with_cap_as_player(target, $player);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "hostile checker should not authorize ownership change");
      $test_utils:assert_eq(target.owner, original_owner, "hostile checker changed target owner");
      $test_utils:assert_eq(property_info(target, "set_owner_probe")[1], original_owner, "hostile checker changed property owner");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_owner_preserves_owner_and_wizard_access (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Owner and wizard calls still transfer object and clobber-property ownership.";
    target = #-1;
    try
      target = create($thing);
      add_property(target, "set_owner_probe", 1, {target.owner, "rc"});
      target:set_owner($player);
      $test_utils:assert_eq(target.owner, $player, "wizard should change owner");
      $test_utils:assert_eq(property_info(target, "set_owner_probe")[1], $player, "wizard should retitle property");
      this:_set_owner_with_cap_as_player(target, $hacker);
      $test_utils:assert_eq(target.owner, $hacker, "owner should change owner");
      $test_utils:assert_eq(property_info(target, "set_owner_probe")[1], $hacker, "owner should retitle property");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_name_aliases_allows_non_owner_object_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: set_name_aliases capability lets a non-owner rename and update aliases without run_as.";
    target = #-1;
    try
      target = create($thing);
      target:set_name_aliases("unchanged name", {"unchanged-alias"});
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      cap = $root:issue_capability(target, {'set_name_aliases});
      this:_set_name_aliases_with_cap_as_player(cap, "plain cap name", {"plain-cap-alias"});
      $test_utils:assert_eq(target.name, "plain cap name", "set_name_aliases capability should update name");
      $test_utils:assert_eq(target.aliases, {"plain-cap-alias"}, "set_name_aliases capability should update aliases");
      $test_utils:assert_true(target.owner != $player, "capability use should not transfer ownership");
      $test_utils:assert_false($player.wizard, "capability use should not make actor a wizard");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_name_aliases_run_as_allows_non_owner_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Compatibility scenario: set_name_aliases still works for a non-owner actor when the capability carries run_as authority.";
    target = #-1;
    try
      target = create($thing);
      target:set_name_aliases("unchanged name", {"unchanged-alias"});
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      cap = $root:issue_capability(target, {'set_name_aliases}, 0, $arch_wizard);
      this:_set_name_aliases_with_cap_as_player(cap, "run-as cap name", {"run-as-cap-alias"});
      $test_utils:assert_eq(target.name, "run-as cap name", "run_as set_name_aliases capability should update name");
      $test_utils:assert_eq(target.aliases, {"run-as-cap-alias"}, "run_as set_name_aliases capability should update aliases");
      $test_utils:assert_true(target.owner != $player, "capability use should not transfer ownership");
      $test_utils:assert_false($player.wizard, "capability use should not make actor a wizard");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_thumbnail_allows_non_owner_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: set_thumbnail capability lets a non-owner update an object's thumbnail through scoped grants.";
    target = #-1;
    try
      target = create($thing);
      $test_utils:assert_false($player.wizard, "fixture player should not be a wizard");
      $test_utils:assert_true(target.owner != $player, "fixture player should not own the target");
      cap = $root:issue_capability(target, {'set_thumbnail});
      thumbnail = {"image/png", b"iVBORw0KGgo="};
      this:_set_thumbnail_with_cap_as_player(cap, @thumbnail);
      $test_utils:assert_eq(target.thumbnail, thumbnail, "set_thumbnail capability should update thumbnail");
      $test_utils:assert_true(target.owner != $player, "capability use should not transfer ownership");
      $test_utils:assert_false($player.wizard, "capability use should not make actor a wizard");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_set_thumbnail_rejects_invalid_content (this none this) owner: ARCH_WIZARD flags: "rxd"
    "A valid scoped grant reaches thumbnail validation and preserves the prior value on rejection.";
    target = #-1;
    try
      target = create($thing);
      cap = $root:issue_capability(target, {'set_thumbnail});
      thumbnail = {"image/png", b"iVBORw0KGgo="};
      this:_set_thumbnail_with_cap_as_player(cap, @thumbnail);
      rejected_type = false;
      try
        this:_set_thumbnail_with_cap_as_player(cap, "text/plain", b"aGVsbG8=");
      except (E_TYPE)
        rejected_type = true;
      endtry
      $test_utils:assert_true(rejected_type, "non-image content type should be rejected after permission validation");
      rejected_data = false;
      try
        this:_set_thumbnail_with_cap_as_player(cap, "image/png", "not binary");
      except (E_TYPE)
        rejected_data = true;
      endtry
      $test_utils:assert_true(rejected_data, "non-binary thumbnail content should be rejected after permission validation");
      $test_utils:assert_eq(target.thumbnail, thumbnail, "rejected thumbnail update should preserve the prior value");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_root_mutation_wrong_cap_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: an unrelated root mutation capability should not authorize another mutation.";
    target = #-1;
    try
      target = create($thing);
      target:set_description("unchanged");
      move_cap = $root:issue_capability(target, {'move});
      denied = false;
      try
        this:_set_description_with_cap_as_player(move_cap, "wrong cap description");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "move capability should not authorize set_description");
      $test_utils:assert_eq(target.description, "unchanged", "denied wrong-cap mutation should preserve description");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_root_mutation_revoked_bearer_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: revoking a copied root mutation bearer token should deny later runtime-grant installation.";
    target = #-1;
    try
      target = create($thing);
      target:set_description("unchanged");
      cap = $root:issue_capability(target, {'set_description});
      $root:_revoke_capability_token(cap);
      denied = false;
      try
        this:_set_description_with_cap_as_player(cap, "revoked description");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "revoked set_description capability should be denied");
      $test_utils:assert_eq(target.description, "unchanged", "revoked capability should not mutate description");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_runtime_grant_literals_accept_string_and_symbol_specs (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: task capability grant specs accept string and symbol grant/property names.";
    target = #-1;
    symbol_target = #-1;
    try
      target = create($thing);
      this:_set_description_with_string_runtime_grant(target, "string grant description");
      $test_utils:assert_eq(target.description, "string grant description", "string-shaped property_write grant should authorize description write");
      symbol_target = create($thing);
      this:_set_description_with_symbol_runtime_grant(symbol_target, "symbol grant description");
      $test_utils:assert_eq(symbol_target.description, "symbol grant description", "symbol-shaped property_write grant should authorize description write");
    finally
      valid(symbol_target) && symbol_target:destroy();
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_player_mutation_caps_allow_expected_operations (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: player mutation capabilities allow delegated account metadata changes.";
    target = #-1;
    try
      target = create($player);
      password_cap = $root:issue_capability(target, {'set_password}, 0, $arch_wizard);
      programmer_cap = $root:issue_capability(target, {'set_programmer}, 0, $arch_wizard);
      email_cap = $root:issue_capability(target, {'set_email_address}, 0, $arch_wizard);
      oauth_cap = $root:issue_capability(target, {'set_oauth2_identities}, 0, $arch_wizard);
      pronouns_cap = $root:issue_capability(target, {'set_pronouns}, 0, $arch_wizard);
      profile_cap = $root:issue_capability(target, {'set_profile_picture}, 0, $arch_wizard);
      this:_set_password_with_cap_as_player(password_cap, "cap-test-password");
      this:_set_programmer_with_cap_as_player(programmer_cap, true);
      this:_set_email_address_with_cap_as_player(email_cap, "cap-test@example.invalid");
      identities = {["provider" -> "test", "subject" -> "cap-subject"]};
      this:_set_oauth2_identities_with_cap_as_player(oauth_cap, identities);
      this:_set_pronouns_with_cap_as_player(pronouns_cap, "they/them");
      profile = {"image/png", b"iVBORw0KGgo="};
      this:_set_profile_picture_with_cap_as_player(profile_cap, @profile);
      $test_utils:assert_true(target.password:challenge("cap-test-password"), "set_password capability should update password");
      $test_utils:assert_true(target.programmer, "set_programmer capability should update programmer flag");
      $test_utils:assert_eq(target.email_address, "cap-test@example.invalid", "set_email_address capability should update email");
      $test_utils:assert_eq(target.oauth2_identities, identities, "set_oauth2_identities capability should update identities");
      $test_utils:assert_eq(target:pronouns_display(), "they/them", "set_pronouns capability should update pronouns");
      $test_utils:assert_eq(target.profile_picture, profile, "set_profile_picture capability should update profile picture");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_player_mutation_without_caps_denies_expected_operations (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: player mutation helpers deny non-owner calls without matching capabilities.";
    target = #-1;
    try
      target = create($player);
      target:set_password("original-password");
      target:set_programmer(false);
      target:set_email_address("original@example.invalid");
      original_identities = {["provider" -> "test", "subject" -> "original-subject"]};
      target:set_oauth2_identities(original_identities);
      target:set_pronouns("they/them");
      original_pronouns = target.pronouns;
      original_profile = {"image/png", b"iVBORw0KGgo="};
      target:set_profile_picture(@original_profile);
      denied = false;
      try
        this:_set_password_as_player(target, "unauthorized-password");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_password should deny non-owner without capability");
      $test_utils:assert_true(target.password:challenge("original-password"), "denied set_password should preserve password");
      denied = false;
      try
        this:_set_programmer_as_player(target, true);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_programmer should deny non-owner without capability");
      $test_utils:assert_false(target.programmer, "denied set_programmer should preserve programmer flag");
      denied = false;
      try
        this:_set_email_address_as_player(target, "unauthorized@example.invalid");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_email_address should deny non-owner without capability");
      $test_utils:assert_eq(target.email_address, "original@example.invalid", "denied set_email_address should preserve email");
      denied = false;
      try
        this:_set_oauth2_identities_as_player(target, {["provider" -> "test", "subject" -> "unauthorized-subject"]});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_oauth2_identities should deny non-owner without capability");
      $test_utils:assert_eq(target.oauth2_identities, original_identities, "denied set_oauth2_identities should preserve identities");
      denied = false;
      try
        this:_set_pronouns_as_player(target, "she/her");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_pronouns should deny non-owner without capability");
      $test_utils:assert_eq(target.pronouns, original_pronouns, "denied set_pronouns should preserve pronouns");
      denied = false;
      try
        this:_set_profile_picture_as_player(target, "image/png", b"iVBORw0KGgo=");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_profile_picture should deny non-owner without capability");
      $test_utils:assert_eq(target.profile_picture, original_profile, "denied set_profile_picture should preserve profile picture");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_llm_api_key_cap_allows_non_owner_update (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: set_api_key capability lets a non-owner update the LLM client key.";
    original_key = $llm_client.api_key;
    try
      cap = $root:issue_capability($llm_client, {'set_api_key});
      this:_set_api_key_with_cap_as_player(cap, "cap-test-api-key");
      $test_utils:assert_eq($llm_client.api_key, "cap-test-api-key", "set_api_key capability should update the key");
    finally
      $llm_client.api_key = original_key;
    endtry
    return true;
  endverb

  verb test_headless_capability_llm_api_key_without_cap_denies (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: set_api_key denies non-owner calls without a capability.";
    original_key = $llm_client.api_key;
    try
      $llm_client.api_key = "original-api-key";
      denied = false;
      try
        this:_set_api_key_as_player("unauthorized-api-key");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "set_api_key should deny non-owner without capability");
      $test_utils:assert_eq($llm_client.api_key, "original-api-key", "denied set_api_key should preserve the key");
    finally
      $llm_client.api_key = original_key;
    endtry
    return true;
  endverb

  verb test_headless_capability_revoke_denies_stored_grant_operation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: revoking a stored grant prevents later lookup-mediated use by the grantee.";
    key = this:_test_key();
    test_area = #-1;
    created = #-1;
    try
      test_area = create($area);
      $root:grant_capability(test_area, {'add_room}, $player, 'area, key);
      $test_utils:assert_true($root:revoke_capability(test_area, $player, 'area), "revoke should remove the stored area grant");
      denied = false;
      try
        created = this:_make_room_with_area_grant_as_player(test_area);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "revoked stored grant should not authorize area room creation");
      $test_utils:assert_false($player:find_capability_for(test_area, 'area), "revoked grant should not be discoverable");
    finally
      valid(created) && created:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_revoke_denies_copied_bearer_token (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a revoked grant should not leave a copied bearer token usable.";
    key = this:_test_key();
    test_area = #-1;
    try
      test_area = create($area);
      copied_cap = $root:grant_capability(test_area, {'add_room}, $player, 'area, key);
      $test_utils:assert_true($root:revoke_capability(test_area, $player, 'area), "revoke should remove the stored area grant");
      denied = false;
      try
        copied_cap:challenge_for_with_key({'add_room}, key);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "revoked copied bearer token should be denied");
    finally
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_revoke_denies_passage_area_grant (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: revoked create_passage grants should deny copied tokens and passage creation.";
    test_area = #-1;
    room_a = #-1;
    room_b = #-1;
    try
      test_area = create($area);
      room_a = test_area:make_room_in($room);
      room_b = test_area:make_room_in($room);
      area_cap = $root:grant_capability(test_area, {'create_passage}, $player, 'area);
      from_cap = $root:grant_capability(room_a, {'dig_from}, $player, 'room);
      to_cap = $root:grant_capability(room_b, {'dig_into}, $player, 'room);
      passage = $passage:mk(room_a, "north", {"north"}, "", true, room_b, "", {}, "", false, true);
      created = this:_create_passage_with_caps_as_player(area_cap, from_cap, to_cap, passage);
      $test_utils:assert_true(created.connection_id != "", "A valid grant creates an identified connection");
      $test_utils:assert_eq(test_area:passage_for(room_a, room_b), created, "passage should be registered before revocation");
      $test_utils:assert_true(test_area:remove_passage(room_a, room_b), "setup should remove the first passage");
      $test_utils:assert_true($root:revoke_capability(test_area, $player, 'area), "revoke should remove stored create_passage grant");
      denied = false;
      try
        area_cap:challenge_for({'create_passage});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "revoked copied create_passage token should be denied");
      denied = false;
      try
        this:_create_passage_with_caps_as_player(area_cap, from_cap, to_cap, passage);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "revoked create_passage grant should deny passage creation");
      $test_utils:assert_false(test_area:passage_for(room_a, room_b), "revoked create_passage grant should not register passage");
    finally
      valid(test_area) && $root:revoke_capability(test_area, $player, 'area);
      valid(room_b) && $root:revoke_capability(room_b, $player, 'room);
      valid(room_a) && $root:revoke_capability(room_a, $player, 'room);
      valid(room_b) && room_b:destroy();
      valid(room_a) && room_a:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_make_player_requires_capability (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not create players through the wizard-owned make_player wrapper.";
    created = #-1;
    denied = false;
    wrong_error = 0;
    before_players = players();
    try
      try
        created = this:_call_make_player_as_player();
      except e (ANY)
        if (e[1] == E_PERM)
          denied = true;
        else
          wrong_error = e[1];
        endif
      endtry
      $test_utils:assert_true(denied, "non-owner make_player wrapper call should be denied without capability");
      $test_utils:assert_false(wrong_error, "non-owner make_player should deny before reaching setup work, got " + toliteral(wrong_error));
      after_players = players();
      $test_utils:assert_eq(length(after_players), length(before_players), "denied make_player should not change player count");
      for candidate in (before_players)
        $test_utils:assert_true(candidate in after_players, "denied make_player should preserve existing players");
      endfor
    finally
      if (valid(created))
        set_player_flag(created, 0);
        recycle(created);
      endif
    endtry
    return true;
  endverb

  verb test_headless_capability_setup_cap_denies_extra_mutations (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: setup capabilities should not authorize player mutations outside the setup grant list.";
    key = this:_test_key();
    created = #-1;
    try
      setup_cap = $player:make_player(0, key);
      created = setup_cap.delegate;
      original_pronouns = created.pronouns;
      original_profile = created.profile_picture;
      denied = false;
      try
        this:_set_pronouns_with_cap_as_player(setup_cap, "they/them");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "setup capability should not grant set_pronouns");
      $test_utils:assert_eq(created.pronouns, original_pronouns, "denied setup set_pronouns should preserve pronouns");
      denied = false;
      try
        this:_set_profile_picture_with_cap_as_player(setup_cap, "image/png", b"iVBORw0KGgo=");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "setup capability should not grant set_profile_picture");
      $test_utils:assert_eq(created.profile_picture, original_profile, "denied setup set_profile_picture should preserve profile picture");
    finally
      valid(created) && created:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_make_player_setup_can_mark_player (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a player setup capability can complete login's initial player setup operations.";
    key = this:_test_key();
    created = #-1;
    start_room = #-1;
    try
      start_room = create($room);
      setup_cap = $player:_make_player_setup_cap($arch_wizard, key);
      $test_utils:assert_type(setup_cap, TYPE_FLYWEIGHT, "make_player should return a setup capability");
      created = setup_cap.delegate;
      $test_utils:assert_true(valid(created), "setup capability delegate should be a valid player object");
      {target, run_as} = setup_cap:challenge_for_with_key({'set_player_flag}, key);
      $test_utils:assert_eq(target, created, "setup capability should target the new player");
      $test_utils:assert_eq(run_as, $arch_wizard, "setup capability should run as $arch_wizard for player flag setup");
      this:_set_player_flag_with_key(setup_cap, 1, key);
      this:_set_name_aliases_with_key(setup_cap, "setup cap player", {"setup-cap-player"}, key);
      this:_set_password_with_key(setup_cap, "setup-password", key);
      this:_set_email_address_with_key(setup_cap, "setup@example.invalid", key);
      identities = {["provider" -> "setup", "subject" -> "subject-1"]};
      this:_set_oauth2_identities_with_key(setup_cap, identities, key);
      this:_set_home_with_key(setup_cap, $first_room, key);
      this:_move_with_key(setup_cap, start_room, key);
      this:_set_owner_with_key(setup_cap, created, key);
      $test_utils:assert_true(is_player(created), "setup capability should be able to mark the new object as a player");
      $test_utils:assert_eq(created.name, "setup cap player", "setup capability should set player name");
      $test_utils:assert_eq(created.aliases, {"setup-cap-player"}, "setup capability should set player aliases");
      $test_utils:assert_true(created.password:challenge("setup-password"), "setup capability should set password");
      $test_utils:assert_eq(created.email_address, "setup@example.invalid", "setup capability should set email");
      $test_utils:assert_eq(created.oauth2_identities, identities, "setup capability should set oauth identities");
      $test_utils:assert_eq(created.home, $first_room, "setup capability should be able to set the new player's home");
      $test_utils:assert_eq(created.location, start_room, "setup capability should move the player to the start room");
      $test_utils:assert_eq(created.owner, created, "setup capability should transfer ownership to the new player");
    finally
      if (valid(created))
        set_player_flag(created, 0);
        recycle(created);
      endif
      valid(start_room) && start_room:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_merge_rejects_wrong_target (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: merging capabilities for different targets is denied.";
    key = this:_test_key();
    target_a = #-1;
    target_b = #-1;
    try
      target_a = create($thing);
      target_b = create($thing);
      cap_a = target_a:issue_capability(target_a, {'read}, 0, 0, key);
      cap_b = target_b:issue_capability(target_b, {'write}, 0, 0, key);
      wrong_target_denied = false;
      try
        $root:merge_capability(cap_a, cap_b, key);
      except (E_INVARG)
        wrong_target_denied = true;
      endtry
      $test_utils:assert_true(wrong_target_denied, "wrong-target merge should be rejected");
    finally
      valid(target_b) && target_b:destroy();
      valid(target_a) && target_a:destroy();
    endtry
    return true;
  endverb

  verb test_headless_capability_merge_rejects_expired_inputs (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: expired capabilities cannot be laundered through merge.";
    key = this:_test_key();
    target = #-1;
    try
      target = create($thing);
      expired_cap = target:issue_capability(target, {'read}, time() - 1, 0, key);
      live_cap = target:issue_capability(target, {'write}, time() + 3600, 0, key);
      expired_denied = false;
      try
        expired_cap:challenge_for_with_key({'read}, key);
      except (E_PERM)
        expired_denied = true;
      endtry
      $test_utils:assert_true(expired_denied, "expired fixture capability should be denied before merge");
      merge_denied = false;
      try
        $root:merge_capability(expired_cap, live_cap, key);
      except (E_PERM)
        merge_denied = true;
      endtry
      $test_utils:assert_true(merge_denied, "merge should reject expired input instead of reissuing its permissions");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb _make_room_with_area_grant_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use PLAYER's stored area grant to create a room in target_area.";
    {target_area} = args;
    cap = $player:find_capability_for(target_area, 'area);
    typeof(cap) == TYPE_FLYWEIGHT || raise(E_PERM);
    return cap:make_room_in($room);
  endverb

  verb _make_room_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied area capability to create a room as PLAYER.";
    {cap} = args;
    typeof(cap) == TYPE_FLYWEIGHT || raise(E_INVARG);
    return cap:make_room_in($room);
  endverb

  verb _create_passage_with_caps_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use supplied area and room capabilities to create a passage as PLAYER.";
    {area_cap, from_room, to_room, passage} = args;
    return area_cap:create_passage(from_room, to_room, passage);
  endverb

  verb _remove_passage_with_caps_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use supplied area and room capabilities to remove a passage as PLAYER.";
    {area_cap, from_room, to_room} = args;
    return area_cap:remove_passage(from_room, to_room);
  endverb

  verb _update_passage_as_player (this none this) owner: PLAYER flags: "rxd"
    "Update a passage as PLAYER.";
    {area, from_room, to_room, passage} = args;
    return area:update_passage(from_room, to_room, passage);
  endverb

  verb _find_room_named_in_area (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Return the first room in area with the given name.";
    {area, room_name} = args;
    for candidate in (area:contents())
      if (valid(candidate) && candidate.name == room_name)
        return candidate;
      endif
    endfor
    return #-1;
  endverb

  verb _create_child_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied create_child capability as PLAYER.";
    {cap} = args;
    return cap:create();
  endverb

  verb _set_description_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_description capability as PLAYER.";
    {cap, description} = args;
    return cap:set_description(description);
  endverb

  verb _set_description_with_string_runtime_grant (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Install a string-shaped runtime property_write grant for description.";
    {target, description} = args;
    set_task_perms($player, {{"property_write", target, "description"}});
    target.description = description;
    return true;
  endverb

  verb _set_description_with_symbol_runtime_grant (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Install a symbol-shaped runtime property_write grant for description.";
    {target, description} = args;
    set_task_perms($player, {{'property_write, target, 'description}});
    target.description = description;
    return true;
  endverb

  verb _set_thumbnail_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_thumbnail capability as PLAYER.";
    {cap, content_type, picbin} = args;
    return cap:set_thumbnail(content_type, picbin);
  endverb

  verb _set_name_aliases_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_name_aliases capability as PLAYER.";
    {cap, new_name, new_aliases} = args;
    return cap:set_name_aliases(new_name, new_aliases);
  endverb

  verb _set_owner_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_owner capability as PLAYER.";
    {cap, new_owner} = args;
    return cap:set_owner(new_owner);
  endverb

  verb _move_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied move capability as PLAYER.";
    {cap, destination} = args;
    return cap:moveto(destination);
  endverb

  verb _destroy_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied recycle capability as PLAYER.";
    {cap} = args;
    return cap:destroy();
  endverb

  verb _set_password_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_password capability as PLAYER.";
    {cap, new_password} = args;
    return cap:set_password(new_password);
  endverb

  verb _set_programmer_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_programmer capability as PLAYER.";
    {cap, flag_value} = args;
    return cap:set_programmer(flag_value);
  endverb

  verb _set_email_address_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_email_address capability as PLAYER.";
    {cap, email} = args;
    return cap:set_email_address(email);
  endverb

  verb _set_oauth2_identities_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_oauth2_identities capability as PLAYER.";
    {cap, identities} = args;
    return cap:set_oauth2_identities(identities);
  endverb

  verb _set_pronouns_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_pronouns capability as PLAYER.";
    {cap, pronouns_str} = args;
    return cap:set_pronouns(pronouns_str);
  endverb

  verb _set_profile_picture_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_profile_picture capability as PLAYER.";
    {cap, content_type, picbin} = args;
    return cap:set_profile_picture(content_type, picbin);
  endverb

  verb _set_password_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's password as PLAYER without a capability.";
    {target, new_password} = args;
    return target:set_password(new_password);
  endverb

  verb _set_programmer_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's programmer flag as PLAYER without a capability.";
    {target, flag_value} = args;
    return target:set_programmer(flag_value);
  endverb

  verb _set_email_address_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's email address as PLAYER without a capability.";
    {target, email} = args;
    return target:set_email_address(email);
  endverb

  verb _set_oauth2_identities_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's OAuth2 identities as PLAYER without a capability.";
    {target, identities} = args;
    return target:set_oauth2_identities(identities);
  endverb

  verb _set_pronouns_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's pronouns as PLAYER without a capability.";
    {target, pronouns_str} = args;
    return target:set_pronouns(pronouns_str);
  endverb

  verb _set_profile_picture_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set another player's profile picture as PLAYER without a capability.";
    {target, content_type, picbin} = args;
    return target:set_profile_picture(content_type, picbin);
  endverb

  verb _set_api_key_with_cap_as_player (this none this) owner: PLAYER flags: "rxd"
    "Use a supplied set_api_key capability as PLAYER.";
    {cap, new_key} = args;
    return cap:set_api_key(new_key);
  endverb

  verb _set_api_key_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to set the LLM API key as PLAYER without a capability.";
    {new_key} = args;
    return $llm_client:set_api_key(new_key);
  endverb

  verb _validate_area_grant_as_player (this none this) owner: PLAYER flags: "rxd"
    "Validate PLAYER's stored area grant with a test key.";
    {target_area, key} = args;
    cap = $player:find_capability_for(target_area, 'area);
    typeof(cap) == TYPE_FLYWEIGHT || raise(E_PERM);
    return cap:challenge_for_with_key({'add_room}, key);
  endverb

  verb _grant_area_capability_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt to grant capabilities as PLAYER.";
    {target, cap_list, grantee, category, key} = args;
    return $root:grant_capability(target, cap_list, grantee, category, key);
  endverb

  verb _call_make_player_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt player creation through a non-owner player-owned helper.";
    cap = $player:make_player();
    return cap.delegate;
  endverb

  verb _set_player_flag_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set a player flag through a test-key setup capability.";
    {cap, flag_value, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_player_flag}, key);
    set_task_perms(perms);
    set_player_flag(target, flag_value);
  endverb

  verb _set_name_aliases_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set name and aliases through a test-key setup capability.";
    {cap, new_name, new_aliases, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_name_aliases}, key);
    set_task_perms(perms);
    target:set_name_aliases(new_name, new_aliases);
  endverb

  verb _set_password_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set password through a test-key setup capability.";
    {cap, new_password, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_password}, key);
    set_task_perms(perms);
    target:set_password(new_password);
  endverb

  verb _set_email_address_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set email address through a test-key setup capability.";
    {cap, email, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_email_address}, key);
    set_task_perms(perms);
    target:set_email_address(email);
  endverb

  verb _set_oauth2_identities_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set OAuth2 identities through a test-key setup capability.";
    {cap, identities, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_oauth2_identities}, key);
    set_task_perms(perms);
    target:set_oauth2_identities(identities);
  endverb

  verb _set_home_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set a player home through a test-key setup capability.";
    {cap, home, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_home}, key);
    set_task_perms(perms);
    target.home = home;
  endverb

  verb _move_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Move a player through a test-key setup capability.";
    {cap, destination, key} = args;
    {target, perms} = cap:challenge_for_with_key({'move}, key);
    set_task_perms(perms);
    target:moveto(destination);
  endverb

  verb _set_owner_with_key (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Set owner through a test-key setup capability.";
    {cap, new_owner, key} = args;
    {target, perms} = cap:challenge_for_with_key({'set_owner}, key);
    set_task_perms(perms);
    target:set_owner(new_owner);
  endverb
endobject
