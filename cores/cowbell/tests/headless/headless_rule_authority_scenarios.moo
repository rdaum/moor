// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later

object HEADLESS_RULE_AUTHORITY_SCENARIOS
  name: "Rule Authority Scenarios"
  parent: ROOT
  owner: RUNTIME_WIZARD
  readable: true
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_rule_authority_scenarios";

  method test_foreign_reaction_mutation_denied owner: RUNTIME_WIZARD
    "A direct generic effect must preserve the invoking programmer's principal.";
    const victim = create($root, $hacker, 2);
    try
      add_property(victim, "audit_value", 0, {$hacker, ""});
      try
        this:_invoke_as_programmer("direct", victim);
      except (E_PERM)
      endtry
      $test_utils:assert_eq(victim.audit_value, 0, "A generic effect must preserve another owner's protected data");
    finally
      recycle(victim);
    endtry
    return true;
  endmethod

  method test_owned_reaction_and_delay_preserve_caller owner: RUNTIME_WIZARD
    "Generic effects and their delayed continuations keep the owner's invoking principal.";
    const target = create($root, #90101, 2);
    try
      add_property(target, "audit_value", 0, {#90101, ""});
      this:_invoke_as_programmer("owned", target);
      $test_utils:assert_eq(target.audit_value, 3, "Own immediate effect must run");
      suspend(2);
      $test_utils:assert_eq(target.audit_value, 4, "Own delayed effect must retain invoking authority");
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method test_guest_runs_installed_owner_reaction owner: RUNTIME_WIZARD
    "A real guest operation runs approved effects; forged events and transitions do not.";
    const target = create($container, #90101, 2);
    try
      move(target, #90102.location);
      add_property(target, "audit_value", 0, {#90101, ""});
      target.open = false;
      target.locked = false;
      $reaction:install(target, "probe_reaction", $reaction:mk('on_open, "input.Actor == #90102", {{'set, 'audit_value, 7}}));
      $reaction:install(target, "threshold_reaction", $reaction:mk({'when, 'audit_value, 'ge, 5}, 0, {{'set, 'audit_value, 9}}));
      for attack in ({"event", "threshold", "dispatch", "continuation", "actor"})
        $test_utils:assert_raises(E_PERM, this, "_trigger_as_guest", {target, attack}, "Guests cannot forge owner events or internal dispatch");
        $test_utils:assert_eq(target.audit_value, 0, "Rejected requests must leave state unchanged");
      endfor
      $test_utils:assert_raises(E_VERBNF, target, "_check_thresholds", {'audit_value, 0, 5, []}, "There must be no API for fabricated transitions");
      this:_trigger_as_guest(target, "open");
      $test_utils:assert_eq(target.audit_value, 9, "The authenticated guest operation and actual threshold crossing must run");
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method _trigger_as_guest owner: RUNTIME_PLAYER
    "Invoke entries as an ordinary guest, without wizard authority from the test.";
    caller == #90013 || raise(E_PERM);
    const {target, attack} = args;
    attack == "event" && return target:fire_trigger('on_open, ['Actor -> #90100]);
    attack == "threshold" && return $reaction:change(target, 'audit_value, 'set, 5, ['Actor -> #90100]);
    attack == "dispatch" && return $reaction:_dispatch(target, 'on_open, ['Actor -> #90100]);
    attack == "continuation" && return $reaction:_run_installed(target, "probe_reaction", target.probe_reaction, ['Actor -> #90100, 'This -> target]);
    attack == "actor" && return target:action_open(#90100, []);
    attack == "open" && return target:action_open(#90102, []);
    raise(E_INVARG);
  endmethod

  method test_consumption_authenticates_actor_and_emits_threshold owner: RUNTIME_WIZARD
    "A guest can consume food, but cannot substitute another actor or invoke the test bypass.";
    const food = create($food, #90100, 2);
    try
      food.portions = 2;
      food.on_finish = "keep";
      add_property(food, "threshold_count", 0, {#90100, ""});
      $reaction:install(food, "empty_reaction", $reaction:mk({'when, 'portions, 'eq, 0}, 0, {{'increment, 'threshold_count}}));
      move(food, #90102.location);
      $test_utils:assert_raises(E_PERM, this, "_consume_as_guest", {food, "actor"}, "A guest cannot consume on behalf of another player");
      $test_utils:assert_raises(E_PERM, this, "_consume_as_guest", {food, "bypass"}, "The test helper cannot bypass participant checks");
      $test_utils:assert_eq(food.portions, 2, "Denied calls preserve portions");
      $test_utils:assert_true(this:_consume_as_guest(food, "consume"), "An authenticated guest can consume nearby food");
      $test_utils:assert_eq(food.portions, 0, "Consumption updates actual portions");
      $test_utils:assert_eq(food.threshold_count, 1, "Consumption emits the actual threshold crossing");
    finally
      recycle(food);
    endtry
    return true;
  endmethod

  method _consume_as_guest owner: RUNTIME_PLAYER
    caller == #90013 || raise(E_PERM);
    const {food, entry} = args;
    entry == "actor" && return food:action_consume(#90100, [], 2);
    entry == "bypass" && return food:consume_for_test(#90102, 2);
    return food:action_consume(#90102, [], 2);
  endmethod

  method test_reaction_installation_rejects_stale_authority owner: RUNTIME_WIZARD
    "Ownership changes, copied grants, and edited definitions invalidate installation authority.";
    const target = create($container, #90101, 2);
    const other = create($container, #90101, 2);
    try
      for host in ({target, other})
        add_property(host, "audit_value", 0, {#90101, "r"});
        host.open = false;
        host.locked = false;
      endfor
      this:_install_as_programmer(target);
      add_property(other, "probe_reaction", target.probe_reaction, {#90101, "r"});
      other:fire_trigger('on_open);
      $test_utils:assert_eq(other.audit_value, 0, "An installation must not authorize another host");
      const approved = target.probe_reaction;
      add_property(target, "copied_reaction", approved, {#90101, "r"});
      target:fire_trigger('on_open);
      $test_utils:assert_eq(target.audit_value, 7, "A copied grant must not execute from a second property");
      target.audit_value = 0;
      delete_property(target, "copied_reaction");
      let edited = approved;
      edited.effects = {{'set, 'audit_value, 100}};
      target.probe_reaction = edited;
      target:fire_trigger('on_open);
      $test_utils:assert_eq(target.audit_value, 0, "Edited effects require a new installation");
      edited = approved;
      edited.authorization = "forged";
      target.probe_reaction = edited;
      target:fire_trigger('on_open);
      $test_utils:assert_eq(target.audit_value, 0, "A forged signature must not authorize execution");
      target.probe_reaction = approved;
      this:_transfer_as_programmer(target);
      this:_trigger_as_guest(target, "open");
      $test_utils:assert_eq(target.audit_value, 0, "A wizard-owned host must not run the old owner's reaction");
      $test_utils:assert_false(target.wizard, "Ownership transfer must not grant wizard status");
      $test_utils:assert_raises(E_PERM, $reaction, "execute_effect", {{'set, 'wizard, true}, ['This -> target], target}, "Even authorized reactions cannot change security fields");
    finally
      recycle(other);
      recycle(target);
    endtry
    return true;
  endmethod

  method _install_as_programmer owner: RUNTIME_PROGRAMMER
    caller == #90013 || raise(E_PERM);
    const {target} = args;
    return $reaction:install(target, "probe_reaction", $reaction:mk('on_open, 0, {{'increment, 'audit_value, 7}}));
  endmethod

  method _transfer_as_programmer owner: RUNTIME_PROGRAMMER
    caller == #90013 || raise(E_PERM);
    const {target} = args;
    target:set_owner(#90100);
    $test_utils:assert_raises(E_PERM, target, "fire_trigger", {'on_open}, "The former owner cannot request triggers after transfer");
  endmethod

  method test_delayed_reaction_rechecks_installation owner: RUNTIME_WIZARD
    "Revocation and transfer cancel delayed effects; an unchanged installation still runs.";
    let targets = {};
    try
      for mode in ({"unchanged", "disabled", "replaced", "transferred"})
        const target = create($root, #90101, 2);
        targets = {@targets, target};
        add_property(target, "audit_value", 0, {#90101, "r"});
        $reaction:install(target, "probe_reaction", $reaction:mk('later, 0, {{'schedule, 'probe, 1, {'set, 'audit_value, 7}}}));
        target:fire_trigger('later);
        if (mode == "disabled")
          let reaction = target.probe_reaction;
          reaction.enabled = false;
          $reaction:install(target, "probe_reaction", reaction);
        elseif (mode == "replaced")
          $reaction:install(target, "probe_reaction", $reaction:mk('later, 0, {}));
        elseif (mode == "transferred")
          target:set_owner(#90100);
        endif
      endfor
      suspend(2);
      $test_utils:assert_eq(targets[1].audit_value, 7, "An unchanged installation retains its delayed effect");
      for target in (targets[2..$])
        $test_utils:assert_eq(target.audit_value, 0, "A stale delayed effect must not run");
      endfor
    finally
      for target in (targets)
        recycle(target);
      endfor
    endtry
    return true;
  endmethod

  method test_movement_reactions_require_real_callbacks owner: RUNTIME_WIZARD
    "Native movement fires installed room reactions; direct hook calls do not.";
    const room = create($room, #90101, 2);
    const item = create($thing, #90101, 2);
    try
      add_property(room, "arrivals", 0, {#90101, "r"});
      $reaction:install(room, "arrival_reaction", $reaction:mk('on_enter, 0, {{'increment, 'arrivals}}));
      room:enterfunc(item);
      $test_utils:assert_eq(room.arrivals, 0, "A forged entry callback must not emit an event");
      move(item, room);
      $test_utils:assert_eq(room.arrivals, 1, "A native movement must emit its entry event");
    finally
      recycle(item);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_anonymous_installations_preserve_identity owner: RUNTIME_WIZARD
    "Anonymous hosts remain distinct even though their display names omit their identity.";
    const target = create($root, #90101, 1);
    const other = create($root, #90101, 1);
    add_property(target, "audit_value", 0, {#90101, "r"});
    add_property(other, "audit_value", 0, {#90101, "r"});
    const installed = $reaction:install(target, "probe_reaction", $reaction:mk('probe, 0, {{'increment, 'audit_value}}));
    add_property(other, "probe_reaction", installed, {#90101, "r"});
    other:fire_trigger('probe);
    $test_utils:assert_eq(other.audit_value, 0, "An anonymous host must not accept another host's grant");
    target:fire_trigger('probe);
    $test_utils:assert_eq(target.audit_value, 1, "The original anonymous host retains its grant");
    return true;
  endmethod

  method test_rule_predicate_preserves_caller owner: RUNTIME_WIZARD
    "Nested engine and flyweight entries must pass the original caller to predicates.";
    $rule_engine:register_predicate('audit_principal,
      ['description -> "Observe the incoming principal.", 'inputs -> [],
        'outputs -> ['principal -> 'object]], this, "query_principal");
    try
      const rule = $rule_engine:parse_expression("audit_principal() -> (principal: Principal)");
      for entry in ({"engine", "rule"})
        const result = this:_evaluate_as_programmer(entry, rule);
        $test_utils:assert_true(result['success], "Principal predicate must succeed");
        $test_utils:assert_eq(result['bindings]['Principal], #90101,
          "Predicate caller must be the invoking programmer");
      endfor
    finally
      $rule_engine:unregister_predicate('audit_principal);
    endtry
    return true;
  endmethod

  method test_rule_cartesian_alternatives owner: RUNTIME_WIZARD
    "Independent two-valued predicates retain all four conjunction bindings.";
    $rule_engine:register_predicate('audit_numbers,
      ['description -> "Two numbers.", 'inputs -> [], 'outputs -> ['value -> 'integer]],
      this, "query_numbers");
    $rule_engine:register_predicate('audit_letters,
      ['description -> "Two letters.", 'inputs -> [], 'outputs -> ['value -> 'string]],
      this, "query_letters");
    try
      const rule = $rule_engine:parse_expression(
        "audit_numbers() -> (value: N) AND audit_letters() -> (value: L)");
      const result = $rule_engine:evaluate(rule);
      $test_utils:assert_true(result['success], "A conjunction solution exists");
      const rows = {result['bindings], @result['alternatives]};
      $test_utils:assert_eq(length(rows), 4, "All conjunction combinations remain available");
      for expected in ({['N -> 1, 'L -> "a"], ['N -> 1, 'L -> "b"],
        ['N -> 2, 'L -> "a"], ['N -> 2, 'L -> "b"]})
        $test_utils:assert_true(expected in rows, "Each independent combination is present");
      endfor
    finally
      $rule_engine:unregister_predicate('audit_numbers);
      $rule_engine:unregister_predicate('audit_letters);
    endtry
    return true;
  endmethod

  method _invoke_as_programmer owner: RUNTIME_PROGRAMMER
    "Invoke a reaction entry with a verified programmer principal.";
    caller == #90013 || raise(E_PERM);
    const {entry, target} = args;
    entry == "direct" && return $reaction:execute_effect({'set, 'audit_value, 7}, ['Actor -> #90101, 'This -> target], target);
    if (entry == "owned")
      const reaction = $reaction:mk('probe, 0, {{'set, 'audit_value, 3}, {'schedule, 'probe, 1, {'increment, 'audit_value, 1}}});
      return reaction:execute(['Actor -> #90101, 'This -> target]);
    endif
    raise(E_INVARG);
  endmethod

  method _evaluate_as_programmer owner: RUNTIME_PROGRAMMER
    "Evaluate through each public rule entry as the programmer fixture.";
    caller == #90013 || raise(E_PERM);
    const {entry, rule} = args;
    entry == "engine" && return $rule_engine:evaluate(rule);
    entry == "rule" && return rule:evaluate();
    raise(E_INVARG);
  endmethod

  method query_principal owner: RUNTIME_WIZARD
    "Observe the principal passed by the engine's predicate call.";
    return ['rows -> {['principal -> caller_perms()]}, 'complete -> true];
  endmethod

  method query_numbers owner: RUNTIME_WIZARD
    "Enumerate two independent number bindings.";
    return ['rows -> {['value -> 1], ['value -> 2]}, 'complete -> true];
  endmethod

  method query_letters owner: RUNTIME_WIZARD
    "Enumerate two independent letter bindings.";
    return ['rows -> {['value -> "a"], ['value -> "b"]}, 'complete -> true];
  endmethod

  method test_invalid_conditions_fail_closed owner: RUNTIME_WIZARD
    "Unknown predicates fail authoring, and failed queries cannot satisfy negation.";
    $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression", {"NOT audit_missing()"},
      "An unknown predicate cannot become an installed condition");
    $rule_engine:register_predicate('audit_broken,
      ['description -> "An implementation error.", 'inputs -> [], 'outputs -> []],
      this, "query_broken");
    try
      const rule = $rule_engine:parse_expression("NOT audit_broken()");
      $test_utils:assert_raises(E_VERBNF, $rule_engine, "evaluate", {rule},
        "Negation must not turn an evaluation error into success");
    finally
      $rule_engine:unregister_predicate('audit_broken);
    endtry
    const valid_rule = $rule_engine:parse_expression("NOT FALSE");
    $test_utils:assert_true($rule_engine:evaluate(valid_rule)['success], "An ordinary false result supports negation");
    for condition in ({valid_rule, {}, [], false, 1, ""})
      $test_utils:assert_raises(E_INVARG, $reaction, "mk", {'probe, condition, {}},
        "Unsupported conditions must not produce unconditional reactions");
    endfor
    for expression in ({"TRUE garbage", "TRUE)", "TRUE FALSE"})
      $test_utils:assert_raises(E_INVARG, $rule_engine, "parse_expression", {expression},
        "The parser must consume the complete condition");
    endfor
    return true;
  endmethod

  method query_broken owner: RUNTIME_WIZARD
    "A broken predicate must not look like an ordinary negative result.";
    return this:audit_missing_internal_method();
  endmethod
endobject
