object RELATION [
  import_export_id -> "relation",
  import_export_hierarchy -> {"relations"}
]
  name: "Generic Relation"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  override description (owner: HACKER, flags: "rc") = "N-ary persistent data relation.";

  property set_version (owner: HACKER, flags: "rc") = 0;

  method assert owner: ARCH_WIZARD
    "Store a tuple once and return its identity. The first write repairs legacy duplicates.";
    const {tuple, ?relation = this} = args;
    isa(relation, $relation) || raise(E_INVARG, "Expected a relation.");
    const principal = caller_perms();
    (principal == relation.owner || principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    typeof(tuple) == TYPE_LIST || raise(E_TYPE);
    length(tuple) > 0 || raise(E_INVARG);
    relation.set_version != 1 && $relation:repair(relation);
    const existing = $relation:_find_tuple_id(relation, tuple);
    existing != 0 && return existing;
    const identity = uuid();
    add_property(relation, "tuple_" + identity, tuple, {relation.owner, "r"});
    $relation:_index_tuple(relation, identity, tuple);
    return identity;
  endmethod

  method _index_key owner: HACKER
    "Hash an exact scalar value, treating both floating-point zero signs as the same value.";
    let {value} = args;
    if (typeof(value) == TYPE_FLOAT && value == 0.0)
      value = 0.0;
    endif
    return "index_" + value_hash(value);
  endmethod

  method _index_tuple owner: ARCH_WIZARD
    "Add one stored identity to each scalar index under the caller's write permissions.";
    const {relation, identity, tuple} = args;
    const principal = caller_perms();
    (principal == relation.owner || principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    for element in (tuple)
      if (typeof(element) in {TYPE_FLYWEIGHT, TYPE_LIST, TYPE_MAP})
        continue;
      endif
      const name = $relation:_index_key(element);
      const identities = `relation.(name) ! E_PROPNF => 0';
      if (typeof(identities) == TYPE_LIST)
        relation.(name) = setadd(identities, identity);
      else
        add_property(relation, name, {identity}, {relation.owner, "r"});
      endif
    endfor
  endmethod

  method repair owner: ARCH_WIZARD
    "Collapse legacy duplicates and rebuild indexes, retaining one original identity per tuple.";
    "Requires relation ownership or wizard authority. No suspension or partial batch commits.";
    const {?relation = this} = args;
    const principal = caller_perms();
    (principal == relation.owner || principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    const names = properties(relation);
    let kept = [];
    let duplicates = {};
    for property in (names)
      const name = tostr(property);
      if (!name:starts_with("tuple_"))
        continue;
      endif
      const tuple = relation.(property);
      (typeof(tuple) == TYPE_LIST && length(tuple) > 0)
        || raise(E_INVARG, "Invalid stored relation tuple: " + name);
      let duplicate = false;
      for original in (mapvalues(kept))
        if (equal(original, tuple))
          duplicate = true;
          break;
        endif
      endfor
      if (duplicate)
        duplicates = {@duplicates, property};
      else
        kept[name[7..$]] = tuple;
      endif
    endfor
    for property in (names)
      if (tostr(property):starts_with("index_") || property in duplicates)
        delete_property(relation, property);
      endif
    endfor
    for identity in (mapkeys(kept))
      $relation:_index_tuple(relation, identity, kept[identity]);
    endfor
    relation.set_version = 1;
    return length(duplicates);
  endmethod

  method retract owner: ARCH_WIZARD
    "Remove a fact once. Return false when absent; repair legacy duplicates before removal.";
    const {tuple, ?relation = this} = args;
    isa(relation, $relation) || raise(E_INVARG, "Expected a relation.");
    const principal = caller_perms();
    (principal == relation.owner || principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    typeof(tuple) == TYPE_LIST || raise(E_TYPE);
    relation.set_version != 1 && $relation:repair(relation);
    const identity = $relation:_find_tuple_id(relation, tuple);
    identity == 0 && return false;
    for element in (tuple)
      if (typeof(element) in {TYPE_FLYWEIGHT, TYPE_LIST, TYPE_MAP})
        continue;
      endif
      const name = $relation:_index_key(element);
      const previous = `relation.(name) ! E_PROPNF => {}';
      if (length(previous) == 0)
        continue;
      endif
      const identities = setremove(previous, identity);
      if (length(identities) > 0)
        relation.(name) = identities;
      else
        delete_property(relation, name);
      endif
    endfor
    delete_property(relation, "tuple_" + identity);
    return true;
  endmethod

  method member owner: ARCH_WIZARD
    "Check exact tuple membership under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {tuple} = args;
    typeof(tuple) == TYPE_LIST || raise(E_TYPE);
    return $relation:_find_tuple_id(this, tuple) != 0;
  endmethod

  method select owner: ARCH_WIZARD
    "Return distinct tuples with an exact value at a one-based position.";
    set_task_perms(caller_perms());
    const {position, value} = args;
    typeof(position) == TYPE_INT || raise(E_TYPE);
    position > 0 || raise(E_INVARG);
    return {tuple for tuple in ($relation:select_containing(value, this))
      if (length(tuple) >= position && equal(tuple[position], value))};
  endmethod

  method select_containing owner: ARCH_WIZARD
    "Return distinct tuples containing an exact value, using scalar indexes when available.";
    set_task_perms(caller_perms());
    const {value, ?relation = this} = args;
    if (typeof(value) in {TYPE_FLYWEIGHT, TYPE_LIST, TYPE_MAP} || relation.set_version != 1)
      let selected = {};
      for tuple in ($relation:tuples(relation))
        for element in (tuple)
          if (equal(element, value))
            selected = {@selected, tuple};
            break;
          endif
        endfor
      endfor
      return selected;
    endif
    const identities = `relation.($relation:_index_key(value)) ! E_PROPNF => {}';
    let result = {};
    for identity in (identities)
      const tuple = relation.("tuple_" + identity);
      for element in (tuple)
        if (equal(element, value))
          result = {@result, tuple};
          break;
        endif
      endfor
    endfor
    return result;
  endmethod

  method tuples owner: ARCH_WIZARD
    "Return distinct stored facts, including before legacy storage has been repaired.";
    const {?relation = this} = args;
    set_task_perms(caller_perms());
    const tuples = {relation.(name) for name in (properties(relation)) if (tostr(name):starts_with("tuple_"))};
    const matched = term_match({'var, 'Tuple}, tuples, [],
      ['case_sensitive -> true, 'dedupe -> true,
        'max_solutions -> max(1, length(tuples)), 'max_steps -> max(1, length(tuples))]);
    return {row['Tuple] for row in (matched)};
  endmethod

  method clear owner: ARCH_WIZARD
    "Remove all tuples and indexes under the caller's write permissions.";
    const principal = caller_perms();
    (principal == this.owner || principal.wizard) || raise(E_PERM);
    set_task_perms(principal);
    for property in (properties(this))
      const name = tostr(property);
      if (name:starts_with("tuple_") || name:starts_with("index_"))
        delete_property(this, property);
      endif
    endfor
    this.set_version = 1;
    return true;
  endmethod

  method _find_tuple_id owner: ARCH_WIZARD
    "Find an exact tuple's stored identity. Composite-only tuples require a scan.";
    set_task_perms(caller_perms());
    const {relation, tuple} = args;
    let identities = 0;
    if (relation.set_version == 1)
      for element in (tuple)
        if (typeof(element) in {TYPE_FLYWEIGHT, TYPE_LIST, TYPE_MAP})
          continue;
        endif
        identities = `relation.($relation:_index_key(element)) ! E_PROPNF => {}';
        break;
      endfor
    endif
    if (typeof(identities) != TYPE_LIST)
      const names = properties(relation);
      identities = {tostr(name)[7..length(tostr(name))] for name in (names) if (tostr(name):starts_with("tuple_"))};
    endif
    for identity in (identities)
      equal(relation.("tuple_" + identity), tuple) && return identity;
    endfor
    return 0;
  endmethod

  method test_assert_and_member owner: HACKER
    "Test basic assertion and membership checking";
    const rel = $relation:create(true);
    "Test binary relation";
    rel:assert({#1, #2});
    !rel:member({#1, #2}) && raise(E_ASSERT, "Binary tuple not found after assert");
    rel:member({#2, #1}) && raise(E_ASSERT, "Element position within tuple should matter");
    "Test ternary relation";
    rel:assert({#12, #39, "passage1"});
    !rel:member({#12, #39, "passage1"}) && raise(E_ASSERT, "Ternary tuple not found");
    rel:member({#12, #39}) && raise(E_ASSERT, "Partial tuple should not match");
    "Test different arities in same relation";
    rel:assert({#5, #6, #7, #8});
    !rel:member({#5, #6, #7, #8}) && raise(E_ASSERT, "Quaternary tuple not found");
  endmethod

  method test_retract owner: HACKER
    "Test tuple removal";
    const rel = $relation:create(true);
    rel:assert({#1, #2, "edge1"});
    rel:assert({#1, #3, "edge2"});
    rel:assert({#2, #3, "edge3"});
    "Verify all present";
    !rel:member({#1, #2, "edge1"}) && raise(E_ASSERT, "Setup failed");
    "Retract one tuple";
    !rel:retract({#1, #2, "edge1"}) && raise(E_ASSERT, "Retract should return true");
    rel:member({#1, #2, "edge1"}) && raise(E_ASSERT, "Tuple still present after retract");
    "Other tuples should remain";
    !rel:member({#1, #3, "edge2"}) && raise(E_ASSERT, "Other tuple incorrectly removed");
    !rel:member({#2, #3, "edge3"}) && raise(E_ASSERT, "Other tuple incorrectly removed");
    "Retracting non-existent tuple should return false";
    rel:retract({#99, #100, "none"}) && raise(E_ASSERT, "Retract of missing tuple should be false");
  endmethod

  method test_select owner: HACKER
    "Test position-based selection";
    const rel = $relation:create(true);
    "Setup passage-like data";
    rel:assert({#12, #39, "north"});
    rel:assert({#12, #40, "east"});
    rel:assert({#39, #40, "south"});
    "Select by first position";
    let results = rel:select(1, #12);
    length(results) != 2 && raise(E_ASSERT, "Expected 2 results from position 1");
    {#12, #39, "north"} in results || raise(E_ASSERT, "Missing expected tuple");
    {#12, #40, "east"} in results || raise(E_ASSERT, "Missing expected tuple");
    "Select by second position";
    results = rel:select(2, #39);
    length(results) != 1 && raise(E_ASSERT, "Expected 1 result from position 2");
    {#12, #39, "north"} in results || raise(E_ASSERT, "Missing expected tuple");
    "Select by third position";
    results = rel:select(3, "south");
    length(results) != 1 && raise(E_ASSERT, "Expected 1 result from position 3");
    {#39, #40, "south"} in results || raise(E_ASSERT, "Missing expected tuple");
    "Select non-existent value";
    results = rel:select(1, #99);
    length(results) != 0 && raise(E_ASSERT, "Expected empty results for non-existent value");
  endmethod

  method test_tuples owner: HACKER
    "Test retrieving all tuples";
    const rel = $relation:create(true);
    "Empty relation";
    length(rel:tuples()) != 0 && raise(E_ASSERT, "Empty relation should have no tuples");
    "Add some tuples";
    rel:assert({#1, #2});
    rel:assert({#3, #4});
    rel:assert({#5, #6, #7});
    const all_tuples = rel:tuples();
    length(all_tuples) != 3 && raise(E_ASSERT, "Expected 3 tuples");
    {#1, #2} in all_tuples || raise(E_ASSERT, "Missing tuple");
    {#3, #4} in all_tuples || raise(E_ASSERT, "Missing tuple");
    {#5, #6, #7} in all_tuples || raise(E_ASSERT, "Missing tuple");
  endmethod

  method test_clear owner: HACKER
    "Test clearing all tuples";
    const rel = $relation:create(true);
    "Add several tuples";
    rel:assert({#1, #2, "a"});
    rel:assert({#3, #4, "b"});
    rel:assert({#5, #6, "c"});
    length(rel:tuples()) != 3 && raise(E_ASSERT, "Setup failed");
    "Clear the relation";
    !rel:clear() && raise(E_ASSERT, "Clear should return true");
    "Verify empty";
    length(rel:tuples()) != 0 && raise(E_ASSERT, "Relation should be empty after clear");
    rel:member({#1, #2, "a"}) && raise(E_ASSERT, "Tuple still present after clear");
  endmethod

  method test_duplicate_assert owner: HACKER
    "Repeated assertion keeps one identity, one fact, and one query answer.";
    const relation = $relation:create(true);
    const first = relation:assert({#1, #2, "edge"});
    const again = relation:assert({#1, #2, "edge"});
    $test_utils:assert_eq(first, again, "Assertion is idempotent");
    $test_utils:assert_eq(relation:count(), 1, "One stored fact");
    $test_utils:assert_eq(relation:query({#1, {'var, 'To}, {'var, 'Label}}),
      {['To -> #2, 'Label -> "edge"]}, "One binding map");
    $test_utils:assert_true(relation:retract({#1, #2, "edge"}), "One retraction removes the fact");
    $test_utils:assert_false(relation:member({#1, #2, "edge"}), "No duplicate remains");
    return true;
  endmethod

  method test_bidirectional_indexing owner: HACKER
    "Test that a single tuple is indexed under all its values.";
    const rel = $relation:create(true);
    "Assert a passage-like tuple";
    rel:assert({#12, #39, "north"});
    "Find tuples containing #12";
    const results_from_12 = rel:select_containing(#12);
    length(results_from_12) != 1 && raise(E_ASSERT, "Should find 1 tuple via #12");
    {#12, #39, "north"} in results_from_12 || raise(E_ASSERT, "Wrong tuple via #12");
    "Find tuples containing #39";
    const results_from_39 = rel:select_containing(#39);
    length(results_from_39) != 1 && raise(E_ASSERT, "Should find 1 tuple via #39");
    {#12, #39, "north"} in results_from_39 || raise(E_ASSERT, "Wrong tuple via #39");
    "Find tuples containing 'north'";
    const results_from_north = rel:select_containing("north");
    length(results_from_north) != 1 && raise(E_ASSERT, "Should find 1 tuple via 'north'");
    {#12, #39, "north"} in results_from_north || raise(E_ASSERT, "Wrong tuple via 'north'");
    "Verify it's the SAME tuple found three different ways";
    results_from_12 == results_from_39 || raise(E_ASSERT, "Results should be identical");
    results_from_39 == results_from_north || raise(E_ASSERT, "Results should be identical");
    "Test with multiple passages";
    rel:assert({#12, #40, "east"});
    rel:assert({#39, #40, "south"});
    "Find all passages from #12";
    const from_12 = rel:select_containing(#12);
    length(from_12) != 2 && raise(E_ASSERT, "Room #12 should have 2 passages");
    {#12, #39, "north"} in from_12 || raise(E_ASSERT, "Missing north passage");
    {#12, #40, "east"} in from_12 || raise(E_ASSERT, "Missing east passage");
    "Find all passages from #40";
    const from_40 = rel:select_containing(#40);
    length(from_40) != 2 && raise(E_ASSERT, "Room #40 should have 2 passages");
    {#12, #40, "east"} in from_40 || raise(E_ASSERT, "Missing east passage");
    {#39, #40, "south"} in from_40 || raise(E_ASSERT, "Missing south passage");
    "Room #39 appears in different positions but both found";
    const from_39 = rel:select_containing(#39);
    length(from_39) != 2 && raise(E_ASSERT, "Room #39 should have 2 passages");
    {#12, #39, "north"} in from_39 || raise(E_ASSERT, "Missing north passage (pos 2)");
    {#39, #40, "south"} in from_39 || raise(E_ASSERT, "Missing south passage (pos 1)");
  endmethod

  method query_result owner: ARCH_WIZARD
    "Return distinct binding maps and whether the requested scope was searched completely.";
    set_task_perms(caller_perms());
    return $relation:_query_result(this, @args);
  endmethod

  method query_rows owner: ARCH_WIZARD
    "Read named, typed columns from one relation without invoking its overridable verbs.";
    set_task_perms(caller_perms());
    const {relation, columns, schema, values, limit} = args;
    $behavior_schema:bindings(values, schema['inputs], "relation inputs");
    const pattern = {{'var, column} for column in (columns)};
    const types = {maphaskey(schema['inputs], field)
      ? schema['inputs][field] | schema['outputs][field] for field in (columns)};
    const result = $relation:_query_result(relation, pattern, values,
      ['max_solutions -> limit], types);
    let rows = {};
    for binding in (result['rows])
      let row = [];
      for output in (mapkeys(schema['outputs]))
        row[output] = binding[output];
      endfor
      rows = {@rows, row};
    endfor
    return ['rows -> rows, 'complete -> result['complete]];
  endmethod

  method _query_result owner: ARCH_WIZARD
    "Match stored facts with explicit result completeness. This method does not mutate or suspend.";
    set_task_perms(caller_perms());
    let {relation, pattern, ?bindings = [], ?options = [], ?types = {}} = args;
    isa(relation, $relation) || raise(E_INVARG, "Expected a relation");
    typeof(pattern) == TYPE_LIST || raise(E_TYPE);
    typeof(bindings) == TYPE_MAP || raise(E_TYPE);
    typeof(options) == TYPE_MAP || raise(E_TYPE);
    const limit = `options['max_solutions] ! E_RANGE => 256';
    (typeof(limit) == TYPE_INT && limit > 0 && limit <= 4096)
      || raise(E_INVARG, "Relation result limit must be between 1 and 4096");
    options['case_sensitive] = true;
    options['dedupe] = true;
    options['max_solutions] = limit + 1;
    term_variables(pattern);
    let identities = 0;
    if (relation.set_version == 1)
      for value in (pattern)
        let bound = value;
        if (typeof(value) == TYPE_LIST && length(value) == 2 && value[1] == 'var)
          bound = `bindings[value[2]] ! E_RANGE => {}';
        endif
        if (!(typeof(bound) in {TYPE_LIST, TYPE_MAP, TYPE_FLYWEIGHT}))
          identities = `relation.($relation:_index_key(bound)) ! E_PROPNF => {}';
          break;
        endif
      endfor
    endif
    if (typeof(identities) != TYPE_LIST)
      const names = properties(relation);
      identities = {tostr(name)[7..length(tostr(name))] for name in (names) if (tostr(name):starts_with("tuple_"))};
    endif
    length(identities) <= 4096 || raise(E_QUOTA, "Relation query needs a narrower input scope");
    const candidates = {relation.("tuple_" + identity) for identity in (identities)};
    for tuple in (candidates)
      (typeof(tuple) == TYPE_LIST && length(tuple) > 0)
        || raise(E_INVARG, "Invalid stored relation fact");
      if (length(types) > 0)
        length(tuple) == length(types) || raise(E_INVARG, "Stored fact has the wrong number of fields");
        for i in [1..length(types)]
          $behavior_schema:_type(tuple[i], types[i], "relation field " + tostr(i));
        endfor
      endif
    endfor
    const rows = term_match(pattern, candidates, bindings, options);
    return ['rows -> rows[1..min(length(rows), limit)], 'complete -> length(rows) <= limit];
  endmethod

  method query owner: ARCH_WIZARD
    "Return distinct complete query answers. Raise E_QUOTA instead of returning a silent prefix.";
    set_task_perms(caller_perms());
    const result = $relation:_query_result(this, @args);
    result['complete] || raise(E_QUOTA, "Relation query exceeded its result limit");
    return result['rows];
  endmethod

  method reachable_from owner: ARCH_WIZARD
    "Find all values reachable via transitive closure from start value. Assumes binary relation.";
    "Read and mutation authority is the calling principal; this method does not suspend.";
    set_task_perms(caller_perms());
    const {start} = args;
    let visited = [start -> true];
    let frontier = {start};
    while (length(frontier) > 0)
      const current = frontier[1];
      frontier = listdelete(frontier, 1);
      for tuple in ($relation:select_containing(current, this))
        length(tuple) != 2 && raise(E_INVARG, "reachable_from requires binary relation");
        const {val_a, val_b} = tuple;
        const other = val_a == current ? val_b | val_a;
        !maphaskey(visited, other) && (visited[other] = true) && (frontier = {@frontier, other});
      endfor
    endwhile
    return mapkeys(visited);
  endmethod

  method test_query_basic owner: HACKER
    "Test basic pattern matching with variables";
    const rel = $relation:create(true);
    rel:assert({#12, #39, "north"});
    rel:assert({#12, #40, "east"});
    rel:assert({#39, #40, "south"});
    "Query for all passages from #12";
    let results = rel:query({#12, {'var, 'dest}, {'var, 'label}});
    length(results) != 2 && raise(E_ASSERT, "Expected 2 results from #12");
    ['dest -> #39, 'label -> "north"] in results || raise(E_ASSERT, "Missing north binding");
    ['dest -> #40, 'label -> "east"] in results || raise(E_ASSERT, "Missing east binding");
    "Query for passages to #39";
    results = rel:query({{'var, 'src}, #39, {'var, 'label}});
    length(results) != 1 && raise(E_ASSERT, "Expected 1 result to #39");
    ['src -> #12, 'label -> "north"] in results || raise(E_ASSERT, "Missing binding");
    "Query all tuples";
    results = rel:query({{'var, 'a}, {'var, 'b}, {'var, 'c}});
    length(results) != 3 && raise(E_ASSERT, "Expected 3 results for all tuples");
  endmethod

  method test_reachable owner: HACKER
    "Test transitive closure - note: relation is bidirectional";
    const rel = $relation:create(true);
    "Build a chain: 1 <-> 2 <-> 3 <-> 4";
    rel:assert({#1, #2});
    rel:assert({#2, #3});
    rel:assert({#3, #4});
    "Add a branch: 2 <-> 5";
    rel:assert({#2, #5});
    let reachable = rel:reachable_from(#1);
    length(reachable) != 5 && raise(E_ASSERT, "Should reach 5 nodes from #1");
    #1 in reachable || raise(E_ASSERT, "Should include start node");
    #2 in reachable || raise(E_ASSERT, "Should reach #2");
    #3 in reachable || raise(E_ASSERT, "Should reach #3");
    #4 in reachable || raise(E_ASSERT, "Should reach #4");
    #5 in reachable || raise(E_ASSERT, "Should reach #5");
    "From #3 reaches all nodes (bidirectional graph)";
    reachable = rel:reachable_from(#3);
    length(reachable) != 5 && raise(E_ASSERT, "Should reach all 5 nodes from #3");
    #1 in reachable || raise(E_ASSERT, "Should reach #1 via bidirectional edges");
    #2 in reachable || raise(E_ASSERT, "Should reach #2");
    #4 in reachable || raise(E_ASSERT, "Should reach #4");
    #5 in reachable || raise(E_ASSERT, "Should reach #5 via #2");
  endmethod

  method count owner: ARCH_WIZARD
    "Count distinct facts under the caller's read permissions, including legacy storage.";
    set_task_perms(caller_perms());
    return length($relation:tuples(this));
  endmethod
endobject
