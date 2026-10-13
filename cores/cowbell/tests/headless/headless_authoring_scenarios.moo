object HEADLESS_AUTHORING_SCENARIOS
  name: "Headless Nonprogrammer Authoring Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property preview_calls (owner: ARCH_WIZARD, flags: "") = 0;
  override import_export_id = "headless_authoring_scenarios";
  override import_export_hierarchy = {"tests", "headless"};

  method _author owner: ARCH_WIZARD
    "Exercise the public shared service with the ordinary builder's permissions.";
    caller == this || raise(E_PERM);
    const {command, text} = args;
    set_task_perms(#90102);
    return $builder_features:author(command, text, #90102);
  endmethod

  method test_settings_selection_and_preview owner: ARCH_WIZARD
    "Select two distinct participants, count each once, and preview without changing state.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const room = create($room, #90102);
    const host = create($container, #90102);
    const first = create($actor, #90102);
    const second = create($actor, #90102);
    try
      move(host, room);
      move(first, room);
      move(second, room);
      const ref = tostr(host);
      this:_author('state, ref + " visits number 0");
      this:_author('reaction, ref + " count after open");
      this:_author('reaction, ref + " count with friend object " + tostr(first));
      this:_author('reaction, ref + " count if friend is in place AND find occupant in place as visitor AND visitor is a $actor");
      this:_author('reaction, ref + " count select each by visitor");
      $test_utils:assert_eq(host.count_reaction.selection, ['mode -> 'each, 'order -> {'visitor}, 'limit -> 256], "Installed match selection");
      this:_author('reaction, ref + " count then add visits 1");
      const preview = this:_author('reaction, ref + " count preview"):join("\n");
      $test_utils:assert_true(index(preview, "Selected visitor:") > 0, "Preview shows the selected object");
      $test_utils:assert_eq(host.visits, 0, "Preview cannot increment the counter");
      this:_author('reaction, ref + " count enable");
      host:fire_trigger('on_open, ['Actor -> #90102]);
      $test_utils:assert_eq(host.visits, 2, "Each selected actor contributes exactly one effect");
      move(first, #-1);
      host:fire_trigger('on_open, ['Actor -> #90102]);
      $test_utils:assert_eq(host.visits, 2, "A false setting-dependent condition skips all steps");
      this:_author('reaction, ref + " count with friend object " + tostr(second));
      this:_author('reaction, ref + " count select one");
      host:fire_trigger('on_open, ['Actor -> #90102]);
      $test_utils:assert_eq(host.visits, 3, "Editing a reusable setting changes the chosen context");
      const child = create(host, #90102);
      try
        this:_author('reaction, tostr(child) + " count enable");
        $test_utils:assert_eq(child.visits, 0, "Explicit child installation starts its own declared counter");
        this:_author('state, ref + " visits set 8");
        $test_utils:assert_eq(child.visits, 0, "The child no longer reads changing parent state");
        this:_author('state, ref + " later number 1");
        $test_utils:assert_false(maphaskey($reaction:state_types(child), 'later), "Child declarations do not track later parent edits");
        this:_author('reaction, tostr(child) + " count remove");
        $test_utils:assert_eq(child.count_reaction, 0, "Removing an inherited reaction hides it on this child");
        $test_utils:assert_eq(host.count_reaction.delegate, $reaction, "Removing the child's reaction preserves the parent's definition");
      finally
        recycle(child);
      endtry
      const sorted = $reaction:_order_matches({['name -> "a"], ['name -> "A"]}, {'name});
      $test_utils:assert_true(equal(sorted, {['name -> "A"], ['name -> "a"]}), "Selection sorts text exactly, including case");
    finally
      recycle(first);
      recycle(second);
      recycle(host);
      recycle(room);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod

  method test_preview_rejects_extension_code owner: ARCH_WIZARD
    "A preview rejects an executable extension without calling it, even when the definition is valid.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const host = create($container, #90102);
    try
      this.preview_calls = 0;
      $rule_engine:register_predicate('author_preview_probe,
        ['description -> "Test extension with a visible side effect.", 'inputs -> [], 'outputs -> []], this, "_unsafe_query");
      const ref = tostr(host);
      this:_author('reaction, ref + " probe after open");
      this:_author('reaction, ref + " probe if fact author_preview_probe");
      let refused = false;
      try
        this:_author('reaction, ref + " probe preview");
      except problem (E_INVARG)
        refused = index(problem[2], "Preview cannot call") > 0;
      endtry
      $test_utils:assert_true(refused, "Preview identifies the unsupported extension");
      $test_utils:assert_eq(this.preview_calls, 0, "No extension method was invoked");
    finally
      $rule_engine:unregister_predicate('author_preview_probe);
      recycle(host);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod

  method _unsafe_query owner: ARCH_WIZARD
    "Expose a side effect if a preview mistakenly calls an extension provider.";
    this.preview_calls = this.preview_calls + 1;
    return ['rows -> {[]}, 'complete -> true];
  endmethod

  method test_author_timer_removal_and_failed_edit owner: ARCH_WIZARD
    "A failed edit preserves the signed reaction and timer; removing it cancels the timer immediately.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const host = create($container, #90102);
    try
      const ref = tostr(host);
      this:_author('state, ref + " mode choice idle armed");
      $test_utils:assert_eq(host.reaction_state_fields['mode]['choices], {"idle", "armed"}, "Installed state choices");
      this:_author('reaction, ref + " alarm after open");
      this:_author('reaction, ref + " alarm then schedule reset 30 set mode idle");
      this:_author('reaction, ref + " alarm enable");
      host:fire_trigger('on_open, ['Actor -> #90102]);
      const saved = host.alarm_reaction;
      let rejected = false;
      try
        this:_author('reaction, ref + " alarm then set mode invalid");
      except problem (E_INVARG)
        rejected = true;
      endtry
      $test_utils:assert_true(rejected, "An unknown choice is rejected before installation");
      $test_utils:assert_eq(host.alarm_reaction, saved, "The old definition remains installed");
      $test_utils:assert_eq(length($reaction:timers(host)), 1, "Failed editing preserves the pending timer");
      this:_author('reaction, ref + " alarm remove");
      $test_utils:assert_eq($reaction:timers(host), [], "Removal cancels all work owned by that reaction");
    finally
      $reaction:cancel_timer(host, 'reset);
      recycle(host);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod
  method test_named_fact_authoring_and_effects owner: ARCH_WIZARD
    "Builders define typed facts, reuse them in policies, and record unique rows through reaction steps.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const members = create($relation, #90102);
    const visits = create($relation, #90102);
    const host = create($container, #90102);
    try
      const ref = tostr(host);
      this:_author('facts, tostr(members) + " define author_members person:object group:text");
      this:_author('facts, tostr(members) + " add person=me group=visitors");
      this:_author('facts, tostr(members) + " add person=me group=visitors");
      $test_utils:assert_eq(length($relation:tuples(members)), 1, "Repeated builder commands store one fact");
      this:_author('rule, ref + " open fact author_members person=actor group=visitors");
      const result = $rule_engine:evaluate(host.open_rule, ['This -> host, 'Accessor -> #90102]);
      $test_utils:assert_true(result['success], "Named facts use the same bindings as the policy");
      this:_author('facts, tostr(visits) + " define author_visits person:object box:object");
      this:_author('reaction, ref + " visit after open");
      this:_author('reaction, ref + " visit then record author_visits person=actor box=this");
      this:_author('reaction, ref + " visit preview");
      $test_utils:assert_eq($relation:tuples(visits), {}, "Preview cannot write facts");
      this:_author('reaction, ref + " visit enable");
      host:fire_trigger('on_open, ['Actor -> #90102]);
      host:fire_trigger('on_open, ['Actor -> #90102]);
      $test_utils:assert_eq($relation:tuples(visits), {{#90102, host}}, "Reaction writes retain relation set semantics");
      this:_author('facts, tostr(members) + " remove person=me group=visitors");
      $test_utils:assert_false($rule_engine:evaluate(host.open_rule,
        ['This -> host, 'Accessor -> #90102])['success], "Removing membership revokes access");
    finally
      $rule_engine:unregister_predicate('author_members);
      $rule_engine:unregister_predicate('author_visits);
      recycle(host);
      recycle(members);
      recycle(visits);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod

  method test_trace_reports_denial_and_stops_steps owner: ARCH_WIZARD
    "An owner can inspect the attempted action and denial; a failed action does not run later steps.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const host = create($container, #90102);
    try
      const ref = tostr(host);
      this:_author('state, ref + " visits number 0");
      this:_author('reaction, ref + " deny after open");
      this:_author('reaction, ref + " deny then do open target=this actor=" + tostr(#90101));
      this:_author('reaction, ref + " deny then add visits 1");
      this:_author('reaction, ref + " deny trace on");
      this:_author('reaction, ref + " deny enable");
      const before = host.open;
      this:_author('reaction, ref + " deny preview");
      $test_utils:assert_eq(host.open, before, "An action preview never invokes the operation");
      let denied = false;
      try
        host:fire_trigger('on_open, ['Actor -> #90102]);
      except problem (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A reaction cannot impersonate another player");
      $test_utils:assert_eq(host.visits, 0, "A denied action stops later steps");
      const report = this:_author('reaction, ref + " deny trace"):join("\n");
      $test_utils:assert_true(index(report, "do open") > 0, "The report names the attempted action");
      $test_utils:assert_true(index(report, "Stopped at step 1") > 0, "The report identifies the failing step");
    finally
      recycle(host);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod
  method test_timer_report_rechecks_current_state owner: ARCH_WIZARD
    "A delayed step reports why it skipped after a state edit invalidates its condition.";
    const old_builder = #90102.is_builder;
    #90102.is_builder = true;
    const host = create($container, #90102);
    try
      const ref = tostr(host);
      this:_author('state, ref + " mode choice armed idle");
      this:_author('state, ref + " fired number 0");
      this:_author('reaction, ref + " alarm after open");
      this:_author('reaction, ref + " alarm if mode is armed");
      this:_author('reaction, ref + " alarm then schedule reset 1 add fired 1");
      this:_author('reaction, ref + " alarm trace on");
      this:_author('reaction, ref + " alarm enable");
      host:fire_trigger('on_open, ['Actor -> #90102]);
      this:_author('state, ref + " mode set idle");
      suspend(2);
      $test_utils:assert_eq(host.fired, 0, "The delayed step rechecks current state");
      const report = this:_author('reaction, ref + " alarm trace"):join("\n");
      $test_utils:assert_true(index(report, "Timer reset: skipped") > 0, "The builder can diagnose a skipped timer");
    finally
      $reaction:cancel_timer(host, 'reset);
      recycle(host);
      #90102.is_builder = old_builder;
    endtry
    return true;
  endmethod
endobject
