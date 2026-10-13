object CONDITION_SCENARIOS
  name: "Condition Runtime Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method _rule owner: HACKER
    "Parse with a boolean input scope used by the expression truth tables.";
    return $rule_engine:parse_expression(args[1], 'condition_test,
      $rule_engine:scope(['A -> 'boolean, 'B -> 'boolean, 'C -> 'boolean]));
  endmethod

  method test_nested_conditions_and_printing owner: ARCH_WIZARD
    "Mixed nested groups and NOT preserve meaning through printing and reparsing.";
    const expressions = {
      "input.A == true AND (input.B == true OR input.C == true)",
      "NOT (input.A == true AND input.B == true)",
      "NOT (input.A == true OR (input.B == true AND NOT input.C == true))"};
    for text in (expressions)
      const rule = this:_rule(text);
      const printed = $rule_engine:decompile_rule(rule);
      const copy = this:_rule(printed);
      $test_utils:assert_eq($rule_engine:decompile_rule(copy), printed, "Stable editable syntax");
      for a in ({false, true})
        for b in ({false, true})
          for c in ({false, true})
            const inputs = ['A -> a, 'B -> b, 'C -> c];
            let expected;
            if (text == expressions[1])
              expected = a && (b || c);
            elseif (text == expressions[2])
              expected = !(a && b);
            else
              expected = !(a || (b && !c));
            endif
            $test_utils:assert_eq($rule_engine:evaluate(rule, inputs)['success], expected, text);
            $test_utils:assert_eq($rule_engine:evaluate(copy, inputs)['success], expected, printed);
          endfor
        endfor
      endfor
    endfor
    return true;
  endmethod

  method test_literal_roundtrip owner: ARCH_WIZARD
    "Strings remain text, symbols remain literals, and nested collection literals retain value.";
    const examples = {
      "\"a, (b) \\\"quoted\\\"\" == \"a, (b) \\\"quoted\\\"\"",
      "'Actor == 'Actor", "#-1 == #-1", "-3.5 < 0.0",
      "{1, \"two\", {3}} == {1, \"two\", {3}}", "['x -> \"y\"] == ['x -> \"y\"]"};
    for text in (examples)
      const rule = $rule_engine:parse_expression(text);
      const copy = $rule_engine:parse_expression($rule_engine:decompile_rule(rule));
      $test_utils:assert_true($rule_engine:evaluate(rule)['success], "Original literal condition");
      $test_utils:assert_true($rule_engine:evaluate(copy)['success], "Reparsed literal condition");
    endfor
    const different = $rule_engine:parse_expression("\"Actor\" == \"actor\"");
    $test_utils:assert_false($rule_engine:evaluate(different)['success], "String equality preserves case");
    return true;
  endmethod

  method test_namespaces_and_supplied_negation_inputs owner: ARCH_WIZARD
    "Declared inputs satisfy negation; identically named parameters and state remain distinct.";
    const scope = $rule_engine:scope(['Actor -> 'object], ['Actor -> 'object], ['count -> 'integer]);
    const rule = $rule_engine:parse_expression(
      "NOT (input.Actor == parameter.Actor) AND state.count >= 4", 'scoped, scope);
    $test_utils:assert_true($rule_engine:evaluate(rule, ['Actor -> #1], ['Actor -> #2], ['count -> 4])['success],
      "Negation uses supplied inputs");
    $test_utils:assert_false($rule_engine:evaluate(rule, ['Actor -> #1], ['Actor -> #1], ['count -> 4])['success],
      "The parameter cannot be confused with the operation actor");
    $test_utils:assert_raises(E_INVARG, $rule_engine, "evaluate", {rule, [], ['Actor -> #2], ['count -> 4]},
      "Missing inputs remain errors");
    $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression", {"NOT input.Actor == #1"},
      "Undeclared inputs fail validation");
    return true;
  endmethod

  method _register_pairs owner: ARCH_WIZARD
    "Register a bounded test provider with two declared output fields.";
    $rule_engine:register_predicate('test_pairs,
      ['description -> "Two-field test records.", 'inputs -> [],
        'outputs -> ['left -> 'integer, 'right -> 'integer]], this, "query_pairs");
  endmethod

  method query_pairs owner: HACKER
    "Return two distinct records; the evaluator must bind and constrain every selected field.";
    return ['rows -> {['left -> 1, 'right -> 1], ['left -> 1, 'right -> 2]}, 'complete -> true];
  endmethod

  method test_output_modes_and_repeated_bindings owner: ARCH_WIZARD
    "Multiple outputs bind independently, while a repeated binding enforces equality.";
    this:_register_pairs();
    try
      const rule = $rule_engine:parse_expression("test_pairs() -> (left: L, right: R)");
      const result = $rule_engine:evaluate(rule);
      $test_utils:assert_eq(1 + length(result['alternatives]), 2, "Both records are answers");
      const constrained = $rule_engine:parse_expression("test_pairs() -> (left: N, right: N)");
      const one = $rule_engine:evaluate(constrained);
      $test_utils:assert_eq(one['bindings]['N], 1, "Repeated output binding");
      $test_utils:assert_eq(one['alternatives], {}, "Unequal outputs do not match");
      const join = $rule_engine:parse_expression(
        "test_pairs() -> (left: L, right: R) AND binding.R == 2");
      $test_utils:assert_eq($rule_engine:evaluate(join)['bindings]['L], 1, "Later conditions use selected outputs");
      for text in ({"NOT test_pairs() -> (left: N)",
        "(test_pairs() -> (left: N) OR TRUE) AND binding.N == 1"})
        $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression", {text},
          "An output unavailable on every path cannot escape its scope");
      endfor
    finally
      $rule_engine:unregister_predicate('test_pairs);
    endtry
    return true;
  endmethod

  method test_parse_rejects_entire_invalid_input owner: ARCH_WIZARD
    "Trailing garbage, missing delimiters, unknown names, and extra arguments are errors.";
    for text in ({"TRUE garbage", "TRUE)", "(TRUE", "NOT", "TRUE AND", "input.No == 1",
      "unknown_predicate()", "isa(object: #1)", "isa(object: #1, object: #2)",
      "is(left: 1, right: 1, ignored: 2)", "is(left: 1, right: 1,)", "\"unterminated"})
      $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression", {text}, text);
    endfor
    return true;
  endmethod

  method test_incomplete_search_and_provider_errors owner: ARCH_WIZARD
    "Unavailable facts cannot establish absence, and a provider bug remains an error under NOT.";
    const schema = ['description -> "Incomplete or broken provider.", 'inputs -> [], 'outputs -> []];
    $rule_engine:register_predicate('test_partial, schema, this, "query_partial");
    $rule_engine:register_predicate('test_broken, schema, this, "query_broken");
    try
      const partial = $rule_engine:parse_expression("NOT test_partial()");
      $test_utils:assert_raises(E_QUOTA, $rule_engine, "evaluate", {partial}, "Incomplete absence is an error");
      const broken = $rule_engine:parse_expression("NOT test_broken()");
      $test_utils:assert_raises(E_VERBNF, $rule_engine, "evaluate", {broken}, "Errors are not negative facts");
    finally
      $rule_engine:unregister_predicate('test_partial);
      $rule_engine:unregister_predicate('test_broken);
    endtry
    return true;
  endmethod

  method test_truncated_conjunction_still_checks_later_conditions owner: ARCH_WIZARD
    "A full intermediate candidate set cannot skip a later condition that rejects every row.";
    const tree = $condition:parse("test_many() -> (value: N) AND FALSE");
    const environment = ['input -> [], 'parameter -> [], 'state -> [], 'binding -> []];
    const provider = {name, inputs, limit} =>
      ['rows -> {['value -> 1], ['value -> 2], ['value -> 3]}, 'complete -> true];
    const result = $condition:evaluate(tree, environment, provider, 2);
    $test_utils:assert_eq(result['bindings], {}, "A truncated prefix is not a conjunction answer");
    $test_utils:assert_false(result['complete], "The discarded candidates are still reported");
    return true;
  endmethod

  method query_partial owner: HACKER
    "Report an incomplete search without pretending that absence is proven.";
    return ['rows -> {}, 'complete -> false];
  endmethod

  method query_broken owner: HACKER
    "Propagate a real implementation error from a registered predicate.";
    return this:no_such_query_implementation();
  endmethod
endobject
