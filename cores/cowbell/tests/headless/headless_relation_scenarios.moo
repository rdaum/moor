object HEADLESS_RELATION_SCENARIOS
  name: "Headless Relation Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for relation queries over runtime-created fixtures.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_relation_scenarios";

  verb test_headless_relation_graph_query (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: anonymous relations support indexed query and reachability over object fixtures.";
    rel = $relation:create(true);
    graph = $relation:create(true);
    room_a = $room:create(true);
    room_b = $room:create(true);
    room_c = $room:create(true);
    rel:assert({room_a, room_b, "north"});
    rel:assert({room_b, room_c, "east"});
    rel:assert({room_a, room_c, "shortcut"});
    $test_utils:assert_eq(rel:count(), 3, "relation should have three asserted tuples before query");
    $test_utils:assert_eq(length(rel:tuples()), 3, "relation should enumerate three asserted tuples before query");
    from_a = rel:query({room_a, {'var, 'dest}, {'var, 'label}});
    $test_utils:assert_eq(length(from_a), 2, "query should find two outgoing room_a tuples");
    $test_utils:assert_true(['dest -> room_b, 'label -> "north"] in from_a, "query should bind room_b north");
    $test_utils:assert_true(['dest -> room_c, 'label -> "shortcut"] in from_a, "query should bind room_c shortcut");
    graph:assert({room_a, room_b});
    graph:assert({room_b, room_c});
    reachable = graph:reachable_from(room_a);
    $test_utils:assert_true(room_a in reachable, "reachable set should include start room");
    $test_utils:assert_true(room_b in reachable, "reachable set should include room_b");
    $test_utils:assert_true(room_c in reachable, "reachable set should include room_c");
    rel:clear();
    graph:clear();
    $test_utils:assert_eq(rel:count(), 0, "clear should remove all relation tuples");
    $test_utils:assert_eq(graph:count(), 0, "clear should remove all graph tuples");
    return true;
  endverb

  verb test_headless_relation_retract_updates_indexes (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: retracting fixture tuples updates membership and indexed query results.";
    rel = $relation:create(true);
    actor = $thing:create(true);
    target_a = $thing:create(true);
    target_b = $thing:create(true);
    rel:assert({actor, target_a, "holds"});
    rel:assert({actor, target_b, "wears"});
    $test_utils:assert_eq(length(rel:tuples()), 2, "relation should enumerate two asserted tuples before retract");
    $test_utils:assert_true(rel:member({actor, target_a, "holds"}), "first tuple should be present");
    $test_utils:assert_true(rel:member({actor, target_b, "wears"}), "second tuple should be present");
    $test_utils:assert_true(rel:retract({actor, target_a, "holds"}), "retract should remove existing tuple");
    $test_utils:assert_false(rel:member({actor, target_a, "holds"}), "retracted tuple should be absent");
    remaining = rel:query({actor, {'var, 'target}, {'var, 'label}});
    $test_utils:assert_eq(length(remaining), 1, "query should return only the unretracted tuple");
    $test_utils:assert_true(['target -> target_b, 'label -> "wears"] in remaining, "query should retain indexed second tuple");
    $test_utils:assert_false(rel:retract({actor, target_a, "holds"}), "retracting a missing tuple should return false");
    return true;
  endverb
  method test_headless_relation_repeated_scalar_index owner: ARCH_WIZARD
    "Repeated values in one tuple produce one result per asserted tuple.";
    const relation = create($relation, $hacker, 2);
    try
      relation:assert({1, 1});
      $test_utils:assert_eq(relation:select(1, 1), {{1, 1}}, "one tuple must appear once");
      $test_utils:assert_eq(relation:select_containing(1), {{1, 1}}, "index must not duplicate the tuple");
      relation:assert({1, 1});
      $test_utils:assert_eq(length(relation:select(1, 1)), 1, "repeated assertion preserves one fact");
      relation:retract({1, 1});
      $test_utils:assert_eq(length(relation:select_containing(1)), 0, "one retraction removes the fact");
    finally
      valid(relation) && relation:destroy();
    endtry
    return true;
  endmethod

  method test_headless_relation_private_reads_denied owner: ARCH_WIZARD
    "Read APIs cannot use their verb owner to bypass private tuple and index permissions.";
    const relation = create($relation, $hacker, 2);
    try
      relation:assert({100, 200});
      for prop in (properties(relation))
        const name = tostr(prop);
        if (name:starts_with("tuple_") || name:starts_with("index_"))
          set_property_info(relation, prop, {$hacker, ""});
        endif
      endfor
      relation.r = 0;
      for operation in ({"select", "select_containing", "tuples", "count", "query", "reachable_from"})
        let denied = false;
        try
          this:_relation_call_as_player(relation, operation);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, operation + " must preserve caller read authority");
      endfor
    finally
      valid(relation) && relation:destroy();
    endtry
    return true;
  endmethod

  method test_legacy_relation_repair_and_exact_values owner: ARCH_WIZARD
    "Legacy duplicates collapse without losing case-distinct facts or tuple identities.";
    const relation = create($relation, $hacker, 2);
    try
      add_property(relation, "tuple_first", {1, "Key"}, {$hacker, "r"});
      add_property(relation, "tuple_duplicate", {1, "Key"}, {$hacker, "r"});
      add_property(relation, "tuple_lower", {1, "key"}, {$hacker, "r"});
      add_property(relation, "index_stale", [1 -> {"missing"}], {$hacker, "r"});
      $test_utils:assert_eq(relation:count(), 2, "Legacy reads already return distinct facts");
      $test_utils:assert_eq(relation:repair(), 1, "One duplicate is removed");
      $test_utils:assert_eq(length(relation:select(1, 1)), 2, "Indexes retain both exact values");
      const identity = relation:assert({1, "Key"});
      $test_utils:assert_true(identity in {"first", "duplicate"}, "An original identity survives");
      $test_utils:assert_eq(relation:count(), 2, "Repeated assertion is a no-op");
      $test_utils:assert_true(relation:retract({1, "Key"}), "One retraction removes that fact");
      $test_utils:assert_eq(relation:tuples(), {{1, "key"}}, "Other string case remains distinct");
      $test_utils:assert_false("index_stale" in properties(relation), "Stale indexes are removed");
      const zero = relation:assert({0.0});
      $test_utils:assert_eq(relation:assert({-0.0}), zero, "Floating-point zero signs have one identity");
      relation:assert({{'var, 'literal}});
      $test_utils:assert_eq(relation:query({{'var, 'Value}}, ['Value -> {'var, 'literal}]),
        {['Value -> {'var, 'literal}]}, "Stored variable-shaped data remains data");
    finally
      relation:destroy();
    endtry
    return true;
  endmethod

  method test_relation_query_completeness owner: ARCH_WIZARD
    "A result limit reports incompleteness, while complete-list callers receive an error.";
    const relation = create($relation, $hacker, 2);
    try
      relation:assert({1, "A"});
      relation:assert({1, "a"});
      relation:assert({1, "A"});
      const pattern = {1, {'var, 'Name}};
      const result = relation:query_result(pattern, [], ['max_solutions -> 1]);
      $test_utils:assert_eq(length(result['rows]), 1, "One requested answer");
      $test_utils:assert_false(result['complete], "Another answer exists");
      $test_utils:assert_raises(E_QUOTA, relation, "query", {pattern, [], ['max_solutions -> 1]},
        "Complete-list queries must not return an unchecked prefix");
      const all = relation:query_result(pattern, [], ['max_solutions -> 2]);
      $test_utils:assert_true(all['complete], "Exactly the limit can still be complete");
      $test_utils:assert_eq(length(all['rows]), 2, "Case-distinct answers remain distinct");
      const absent = relation:query_result({2, {'var, 'Name}});
      $test_utils:assert_true(absent['complete], "An absent indexed value has a complete empty result");
      $test_utils:assert_eq(absent['rows], {}, "No matching fact");
      $rule_engine:register_relation('test_names, relation, {'group, 'name},
        ['description -> "Names in one group.", 'inputs -> ['group -> 'integer],
          'outputs -> ['name -> 'string]]);
      const rule = $rule_engine:parse_expression(
        "test_names(group: 1) -> (name: Name) AND test_names(group: 1) -> (name: Name)");
      const matched = $rule_engine:evaluate(rule);
      $test_utils:assert_eq(1 + length(matched['alternatives]), 2,
        "Repeated bindings preserve both exact string values");
      const incomplete = $rule_engine:query('test_names, ['group -> 1], 1);
      $test_utils:assert_false(incomplete['complete], "Named queries retain result completeness");
      relation:assert({1, 2});
      $test_utils:assert_raises(E_INVARG, $rule_engine, "evaluate", {rule},
        "Malformed typed facts remain errors, rather than missing facts");
    finally
      $rule_engine:unregister_predicate('test_names);
      relation:destroy();
    endtry
    return true;
  endmethod

  method test_relation_world_and_reaction_queries_agree owner: ARCH_WIZARD
    "World selectors and named relation facts share bindings, types, and read authority.";
    const relation = create($relation, $hacker, 2);
    const room = create($room, $hacker, 2);
    const visitor = create($thing, $hacker, 2);
    try
      move(visitor, room);
      relation:assert({room, visitor});
      relation:assert({room, visitor});
      $rule_engine:register_relation('test_members, relation, {'location, 'member},
        ['description -> "Members recorded at a location.", 'inputs -> ['location -> 'object],
          'outputs -> ['member -> 'object]]);
      const text = "occupants(location: input.Location) -> (object: Visitor)"
        + " AND test_members(location: input.Location) -> (member: Visitor)";
      const rule = $rule_engine:parse_expression(text, 'members,
        $rule_engine:scope(['Location -> 'object]));
      const result = $rule_engine:evaluate(rule, ['Location -> room]);
      $test_utils:assert_eq(result['bindings], ['Visitor -> visitor], "The world and relation agree");
      $test_utils:assert_eq(result['alternatives], {}, "Repeated facts do not repeat the answer");
      const direct = relation:query({room, {'var, 'Visitor}});
      $test_utils:assert_eq(direct, {result['bindings]}, "Direct relation queries have the same bindings");
      add_property(visitor, "responses", 0, {$hacker, "r"});
      $reaction:install(visitor, "member_reaction", $reaction:mk('probe, text, {{'increment, 'responses}}));
      visitor:fire_trigger('probe, ['Actor -> visitor, 'Location -> room]);
      $test_utils:assert_eq(visitor.responses, 1, "The reaction uses the same matching facts");
      for property in (properties(relation))
        const field_name = tostr(property);
        if (field_name:starts_with("tuple_") || field_name:starts_with("index_"))
          set_property_info(relation, property, {$hacker, ""});
        endif
      endfor
      relation.r = 0;
      let denied = false;
      try
        const negative = $rule_engine:parse_expression("NOT test_members(location: input.Location)",
          'absence, $rule_engine:scope(['Location -> 'object]));
        this:_query_as_player(negative, room);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A registered query does not grant read access");
    finally
      $rule_engine:unregister_predicate('test_members);
      recycle(visitor);
      recycle(room);
      relation:destroy();
    endtry
    return true;
  endmethod

  method test_relation_owner_authoring_permissions owner: ARCH_WIZARD
    "A non-programmer can expose an owned relation but cannot change a foreign relation.";
    const owned = create($relation, #90102, 2);
    const foreign = create($relation, $hacker, 2);
    try
      for method in ({"_index_tuple", "_find_tuple_id", "repair"})
        add_verb(owned, {#90102, "rxd", method}, {"this", "none", "this"});
        set_verb_code(owned, method, {"raise(E_ASSERT, \"An internal storage call reached a target override\");"});
      endfor
      foreign:assert({1});
      this:_register_as_player(owned);
      $test_utils:assert_eq(owned:tuples(), {{1}}, "The owner's fact was stored");
      for operation in ({"register", "assert"})
        let denied = false;
        try
          this:_register_as_player(foreign, operation);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign registration and even idempotent writes require ownership");
      endfor
    finally
      $rule_engine:unregister_predicate('test_owned_relation);
      owned:destroy();
      foreign:destroy();
    endtry
    return true;
  endmethod

  method _register_as_player owner: RUNTIME_PLAYER
    "Use relation authoring services under the non-programmer owner's permissions.";
    caller == this || raise(E_PERM);
    const {relation, ?operation = "both"} = args;
    if (operation in {"both", "assert"})
      relation:assert({1});
    endif
    if (operation in {"both", "register"})
      $rule_engine:register_relation('test_owned_relation, relation, {'value},
        ['description -> "An owner's declared fact.", 'inputs -> [], 'outputs -> ['value -> 'integer]]);
    endif
  endmethod

  method _query_as_player owner: RUNTIME_PLAYER
    "Exercise registered relation reads with ordinary player permissions.";
    caller == this || raise(E_PERM);
    const {rule, room} = args;
    return $rule_engine:evaluate(rule, ['Location -> room]);
  endmethod

  method _relation_call_as_player owner: PLAYER
    "Invoke only private-read regression operations under the player principal.";
    caller == this && this == #90003 || raise(E_PERM);
    const {relation, operation} = args;
    operation == "select" && return relation:select(1, 100);
    operation == "select_containing" && return relation:select_containing(100);
    operation == "tuples" && return relation:tuples();
    operation == "count" && return relation:count();
    operation == "query" && return relation:query({100, {'var, 'target}});
    operation == "reachable_from" && return relation:reachable_from(100);
    raise(E_PERM);
  endmethod

endobject
