object HEADLESS_RULE_SCENARIOS
  name: "Headless Rule Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Rules select and join facts from runtime-created objects.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_rule_scenarios";

  method query_parents owner: ARCH_WIZARD
    "Read the test object's parents with the invoking principal's permissions.";
    set_task_perms(caller_perms());
    const {name, inputs, limit} = args;
    const child = inputs['child];
    return ['rows -> {['parent -> child.father], ['parent -> child.mother]}, 'complete -> true];
  endmethod

  method test_headless_rule_engine_with_fixture owner: ARCH_WIZARD
    "Select parents, join a second generation, and reject a nonmatching parent.";
    const child = create($root, $hacker, 2);
    const parent = create($root, $hacker, 2);
    try
      for object in ({child, parent})
        add_property(object, "father", $root, {$hacker, "r"});
        add_property(object, "mother", $thing, {$hacker, "r"});
      endfor
      child.father = parent;
      $rule_engine:register_predicate('test_parents,
        ['description -> "Parents of the fixture.", 'inputs -> ['child -> 'object],
          'outputs -> ['parent -> 'object]], this, "query_parents");
      const scope = $rule_engine:scope(['Child -> 'object, 'Other -> 'object]);
      const expression = "test_parents(child: input.Child) -> (parent: Parent)";
      const rule = $rule_engine:parse_expression(expression, 'parents, scope);
      const result = $rule_engine:evaluate(rule, ['Child -> child]);
      $test_utils:assert_true(result['success], "The fixture has parents");
      const parents = {row['Parent] for row in ({result['bindings], @result['alternatives]})};
      $test_utils:assert_eq(length(parents), 2, "Both parents are selected");
      $test_utils:assert_true(parent in parents, "The father is selected");
      $test_utils:assert_true($thing in parents, "The mother is selected");
      const missing = $rule_engine:parse_expression(
        expression + " AND binding.Parent == input.Other", 'missing, scope);
      const absent = $rule_engine:evaluate(missing, ['Child -> child, 'Other -> $room]);
      $test_utils:assert_false(absent['success], "An unrelated object does not match");
      $test_utils:assert_eq(absent['bindings], [], "No binding on failure");
      $test_utils:assert_eq(absent['alternatives], {}, "No alternatives on failure");
      const joined = $rule_engine:parse_expression(expression
        + " AND binding.Parent == input.Other"
        + " AND test_parents(child: binding.Parent) -> (parent: Grandparent)", 'joined, scope);
      const ancestors = $rule_engine:evaluate(joined, ['Child -> child, 'Other -> parent]);
      $test_utils:assert_eq(1 + length(ancestors['alternatives]), 2,
        "A later query receives the earlier query's bound parent");
      $test_utils:assert_eq(ancestors['bindings]['Parent], parent, "The join preserves its input");
    finally
      $rule_engine:unregister_predicate('test_parents);
      recycle(child);
      recycle(parent);
    endtry
    return true;
  endmethod
endobject
