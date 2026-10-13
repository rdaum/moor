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
      const fields = ['group -> 'integer, 'name -> 'string];
      $relation:define_fields(relation, {'group, 'name}, fields);
      const rule = $rule_engine:parse_expression(
        "relation(" + tostr(relation) + ", group: 1) -> (name: Name) AND relation("
          + tostr(relation) + ", group: 1) -> (name: Name)");
      const matched = $rule_engine:evaluate(rule);
      $test_utils:assert_eq(1 + length(matched['alternatives]), 2,
        "Repeated bindings preserve both exact string values");
      const incomplete = $relation:query_rows(relation, fields, ['group -> 1], 1);
      $test_utils:assert_false(incomplete['complete], "Named queries retain result completeness");
      $test_utils:assert_raises(E_INVARG, relation, "assert", {{1, 2}},
        "Typed relations reject malformed facts at insertion");
      add_property(relation, "tuple_bad", {1, 2}, {$hacker, "r"});
      $relation:_index_tuple(relation, "bad", {1, 2});
      $test_utils:assert_raises(E_INVARG, $rule_engine, "evaluate", {rule},
        "Malformed typed facts remain errors, rather than missing facts");
    finally
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
      $relation:define_fields(relation, {'location, 'member}, ['location -> 'object, 'member -> 'object]);
      const text = "occupants(location: input.Location) -> (object: Visitor)"
        + " AND relation(" + tostr(relation) + ", location: input.Location) -> (member: Visitor)";
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
        const negative = $rule_engine:parse_expression("NOT relation(" + tostr(relation) + ", location: input.Location)",
          'absence, $rule_engine:scope(['Location -> 'object]));
        this:_query_as_player(negative, room);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "An explicit relation query does not grant read access");
    finally
      recycle(visitor);
      recycle(room);
      relation:destroy();
    endtry
    return true;
  endmethod

  method test_relation_owner_authoring_permissions owner: ARCH_WIZARD
    "A non-programmer can expose an owned relation but cannot change a foreign relation.";
    const owned = create($relation, #90102, 2);
    const other_owner = create($relation, #90101, 2);
    const foreign = create($relation, $hacker, 2);
    try
      for method in ({"_index_tuple", "_find_tuple_id", "repair"})
        add_verb(owned, {#90102, "rxd", method}, {"this", "none", "this"});
        set_verb_code(owned, method, {"raise(E_ASSERT, \"An internal storage call reached a target override\");"});
      endfor
      foreign:assert({1});
      this:_define_as_player(owned);
      this:_define_as_programmer(other_owner);
      $test_utils:assert_eq(owned.fact_schema, other_owner.fact_schema,
        "Different owners can define identical fields without registering a name");
      $test_utils:assert_eq(other_owner:tuples(), {{2}}, "Each owner has independent storage");
      $test_utils:assert_eq(owned:tuples(), {{1}}, "The owner's fact was stored");
      for operation in ({"define", "assert"})
        let denied = false;
        try
          this:_define_as_player(foreign, operation);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign field definitions and even idempotent writes require ownership");
      endfor
    finally
      owned:destroy();
      other_owner:destroy();
      foreign:destroy();
    endtry
    return true;
  endmethod

  method _define_as_player owner: RUNTIME_PLAYER
    "Use relation authoring services under the non-programmer owner's permissions.";
    caller == this || raise(E_PERM);
    const {relation, ?operation = "both"} = args;
    if (operation in {"both", "assert"})
      relation:assert({1});
    endif
    if (operation in {"both", "define"})
      $relation:define_fields(relation, {'value}, ['value -> 'integer]);
    endif
  endmethod

  method _define_as_programmer owner: RUNTIME_PROGRAMMER
    "Define the same local fields as another owner and store a different fact.";
    caller == this || raise(E_PERM);
    const {relation} = args;
    $relation:define_fields(relation, {'value}, ['value -> 'integer]);
    $relation:assert({2}, relation);
  endmethod

  method _query_as_player owner: RUNTIME_PLAYER
    "Exercise explicit relation reads with ordinary player permissions.";
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

  method test_relation_directions_joins_and_absence owner: ARCH_WIZARD
    "One field definition supports either query direction, joins, and correlated absence checks.";
    const relation = create($relation, $hacker);
    try
      $relation:define_fields(relation, {'parent, 'child}, ['parent -> 'integer, 'child -> 'integer]);
      relation:assert({1, 2});
      relation:assert({1, 3});
      relation:assert({2, 4});
      const call = "relation(" + tostr(relation);
      const forward = $rule_engine:parse_expression(call + ", parent: 1) -> (child: Child)");
      const children = $rule_engine:evaluate(forward);
      $test_utils:assert_eq(1 + length(children['alternatives]), 2, "Both children are selected");
      const parameters = ['family -> relation];
      const scope = $rule_engine:scope([], ['family -> 'object]);
      const reusable = $rule_engine:parse_expression(
        "relation(parameter.family, child: 4) -> (parent: Parent)", 'parents, scope, parameters);
      $test_utils:assert_eq($rule_engine:evaluate(reusable, [], parameters)['bindings], ['Parent -> 2],
        "Programmers can compile a query through an explicit setting");
      const restored = $rule_engine:parse_expression($rule_engine:decompile_rule(reusable),
        'parents, scope, parameters);
      $test_utils:assert_eq($rule_engine:evaluate(restored, [], parameters)['bindings], ['Parent -> 2],
        "Printed settings remain references when reparsed");
      const reverse = $rule_engine:parse_expression(call + ", child: 4) -> (parent: Parent)");
      $test_utils:assert_eq($rule_engine:evaluate(reverse)['bindings], ['Parent -> 2], "Either column can be bound");
      const join = $rule_engine:parse_expression(call + ", parent: 1) -> (child: Middle) AND "
        + call + ", parent: binding.Middle) -> (child: End)");
      const joined = $rule_engine:evaluate(join);
      $test_utils:assert_eq(joined['bindings], ['Middle -> 2, 'End -> 4], "The first query feeds the next");
      const leaf = $rule_engine:parse_expression(call + ", parent: 1) -> (child: Leaf) AND NOT "
        + call + ", parent: binding.Leaf)");
      $test_utils:assert_eq($rule_engine:evaluate(leaf)['bindings], ['Leaf -> 3], "Absence is correlated with each match");
      const printed = $rule_engine:decompile_rule(join);
      const reparsed = $rule_engine:parse_expression(printed);
      $test_utils:assert_eq($rule_engine:evaluate(reparsed), joined, "Printing and parsing preserve relation joins");
      $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression",
        {"NOT " + call + ") -> (child: Unbound)"}, "Negation cannot introduce a selected value");
      $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression",
        {call + ", missing: 1)"}, "Unknown fields fail at authoring time");
      $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression",
        {call + ", parent: \"one\")"}, "Mistyped inputs fail at authoring time");
      const saved = relation.fact_schema;
      $test_utils:assert_raises(E_INVARG, $relation, "define_fields",
        {relation, {'parent, 'child}, ['parent -> 'string, 'child -> 'integer]},
        "A field change must validate existing tuples before changing anything");
      $test_utils:assert_eq(relation.fact_schema, saved, "A failed field change preserves the definition");
      $relation:define_fields(relation, {'from, 'to}, ['from -> 'integer, 'to -> 'integer]);
      $test_utils:assert_raises(E_INVARG, $rule_engine, "evaluate", {forward},
        "Existing conditions detect renamed fields instead of silently changing meaning");
    finally
      recycle(relation);
    endtry
    return true;
  endmethod
endobject
