// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_MATCHING_SCENARIOS
  name: "Matching Boundary Scenarios"
  parent: ROOT
  owner: RUNTIME_WIZARD
  method test_integer_fuzzy_threshold owner: RUNTIME_WIZARD
    "Integer tolerance must work for empty scope and real aliases.";
    $test_utils:assert_eq($match:resolve_in_scope("missing", {}, ['fuzzy_threshold -> 1]), $failed_match);
    const scope = {{#90113, "lobby"}};
    $test_utils:assert_eq($match:resolve_in_scope("lobbi", scope, ['fuzzy_threshold -> 1]), #90113);
    $test_utils:assert_eq($match:resolve_in_scope("lobbi", scope, ['fuzzy_threshold -> 0]), $failed_match);
    $test_utils:assert_eq($match:resolve_in_scope("lobbi", scope, ['fuzzy_threshold -> false]), $failed_match);
    $test_utils:assert_eq($match:resolve_in_scope("lobby", scope, ['fuzzy_threshold -> true]), #90113);
  endmethod
  method test_alias_ordinal_and_literal_contract owner: RUNTIME_WIZARD
    "Aliases, ordinals, and explicit object references retain their contracts.";
    const scope = {{#90113, "probe"}, {#90114, "probe", "special probe"}};
    $test_utils:assert_eq($match:resolve_in_scope("second probe", scope), #90114);
    $test_utils:assert_eq($match:resolve_in_scope("third probe", scope), $failed_match);
    $test_utils:assert_eq($match:resolve_in_scope("special probe", scope), #90114);
    $test_utils:assert_eq($match:resolve_in_scope("#90114", {}), $failed_match);
    $test_utils:assert_eq($match:resolve_in_scope("#90114", {}, ['allow_literals -> false]), $failed_match);
    const uuid_object = create($thing, #90100, 2);
    try
      $test_utils:assert_eq($match:match_object(tostr(uuid_object)), uuid_object);
      $test_utils:assert_eq($match:resolve_in_scope(tostr(uuid_object), {uuid_object}), uuid_object);
      $test_utils:assert_eq($match:resolve_in_scope(tostr(uuid_object), {}), $failed_match);
    finally
      recycle(uuid_object);
    endtry
  endmethod
  method test_shared_scope_names owner: RUNTIME_WIZARD
    "Mixed and repeated scope entries keep aliases and one stable candidate per object.";
    const scope = {{#90113, "alpha"}, #90113, {#90113, "other"}, #90114};
    const {objects, names} = $match:normalize_scope(scope);
    $test_utils:assert_eq(objects, {#90113, #90114});
    $test_utils:assert_eq(names[1], {"alpha", "other"});
    const all_scope = {{#90113, "all keys"}, {#90114, "keys"}};
    $test_utils:assert_eq($match:resolve_in_scope("all keys", all_scope), #90113);
    $test_utils:assert_eq(parse_command("touch all keys", all_scope, true, 0.3)['dobj], #90113);
    $test_utils:assert_eq($match:resolve_in_scope("other", scope), #90113);
    $test_utils:assert_eq($match:resolve_in_scope(#90114.name, scope), #90114);
    const parsed = parse_command("touch other", scope, true, 0.3);
    $test_utils:assert_eq(parsed['dobj], #90113);
    $test_utils:assert_eq($match:candidates_in_scope("probe", {#90113, #90113, #90114}), {#90113, #90114});
  endmethod

endobject

object #90113
  name: "matching probe"
  parent: THING
  owner: RUNTIME_WIZARD
  location: RUNTIME_ROOM
  override aliases = {"first probe", "probe"};
endobject

object #90114
  name: "matching probe"
  parent: THING
  owner: RUNTIME_WIZARD
  location: RUNTIME_ROOM
  override aliases = {"second probe", "probe", "special probe"};
  verb "runtime-match-probe" (this none none) owner: RUNTIME_WIZARD flags: "rd"
    "Expose the real object selected by command dispatch.";
    player:inform_current($event:mk_note(player, "MATCHING DISPATCH " + tostr(this)):with_audience('utility));
  endverb
endobject

object #000001-ABCDEF1234
  name: "UUID matching probe"
  parent: THING
  owner: RUNTIME_WIZARD
  location: RUNTIME_ROOM
  verb "runtime-match-probe" (this none none) owner: RUNTIME_WIZARD flags: "rd"
    "Expose dispatch through an actual UUID object reference.";
    player:inform_current($event:mk_note(player, "MATCHING DISPATCH " + tostr(this)):with_audience('utility));
  endverb
endobject
