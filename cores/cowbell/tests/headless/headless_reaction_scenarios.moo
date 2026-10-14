object HEADLESS_REACTION_SCENARIOS
  name: "Headless Reaction Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for reaction state mutation, thresholds, and trigger chaining.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_reaction_scenarios";

  method test_named_timers_and_declared_state owner: ARCH_WIZARD
    "An owner can replace or cancel a timer; its condition uses current declared state when it wakes.";
    const host = create($thing, #90102, 2);
    try
      $reaction:declare_state(host, 'count, 'integer, 0);
      $reaction:declare_state(host, 'level, 'float, 1.0);
      $reaction:change(host, 'level, 'increment, 1);
      $test_utils:assert_eq(host.level, 2.0, "A decimal counter can use the default whole-number increment");
      $reaction:declare_state(host, 'mode, 'string, "armed", {"armed", "idle"});
      const reaction = $reaction:mk('start, "state.mode == \"armed\"",
        {{'schedule, 'alarm, 1, {'increment, 'count}}}, $reaction:state_types(host));
      $reaction:install(host, "alarm_reaction", reaction);
      this:_timer_as_owner(host, "start");
      this:_timer_as_owner(host, "start");
      $test_utils:assert_eq(length($reaction:timers(host)), 1, "The same timer name replaces the previous task");
      suspend(2);
      $test_utils:assert_eq(host.count, 1, "Only the replacement runs");
      $test_utils:assert_eq($reaction:timers(host), [], "A finished timer removes its record");
      this:_timer_as_owner(host, "start");
      this:_timer_as_owner(host, "cancel");
      $test_utils:assert_eq($reaction:timers(host), [], "Cancellation removes the pending task");
      this:_timer_as_owner(host, "start");
      this:_timer_as_owner(host, "idle");
      suspend(2);
      $test_utils:assert_eq(host.count, 1, "Cancellation and a false condition prevent later effects");
      for invalid in ({"bad_counter", "bad_choice"})
        let rejected = false;
        try
          this:_timer_as_owner(host, invalid);
        except problem (E_INVARG)
          rejected = true;
        endtry
        $test_utils:assert_true(rejected, "Declared state rejects the wrong type or an unknown choice");
      endfor
      $test_utils:assert_eq(host.mode, "idle", "Invalid state changes preserve the old value");
    finally
      $reaction:cancel_timer(host, 'alarm);
      recycle(host);
    endtry
    return true;
  endmethod

  method test_reaction_chain_and_timer_limits owner: ARCH_WIZARD
    "A recursive trigger stops with a useful limit, and an object cannot accumulate unbounded timers.";
    const host = create($thing, $hacker, 2);
    try
      $reaction:declare_state(host, 'count, 'integer, 0);
      $reaction:install(host, "loop_reaction", $reaction:mk('loop, 0,
        {{'increment, 'count}, {'trigger, host, 'loop}}));
      $test_utils:assert_raises(E_QUOTA, host, "fire_trigger", {'loop}, "A reaction loop has a fixed limit");
      $test_utils:assert_eq(host.count, 4, "The chain stops after four completed steps");
      for i in [1..16]
        $reaction:execute_effect({'schedule, tosym("timer_" + tostr(i)), 60, {'increment, 'count}},
          ['Actor -> player, 'This -> host], host);
      endfor
      $test_utils:assert_eq(length($reaction:timers(host)), 16, "The configured pending timer limit is available");
      $test_utils:assert_raises(E_QUOTA, $reaction, "execute_effect",
        {{'schedule, 'overflow, 60, {'increment, 'count}}, ['Actor -> player, 'This -> host], host},
        "Another timer is rejected without scheduling it");
    finally
      for name in (mapkeys($reaction:timers(host)))
        $reaction:cancel_timer(host, name);
      endfor
      recycle(host);
    endtry
    return true;
  endmethod

  method _timer_as_owner owner: RUNTIME_PLAYER
    "Use timer and state services as the non-programmer who owns the object.";
    caller == this || raise(E_PERM);
    const {host, operation} = args;
    operation == "start" && return host:fire_trigger('start, ['Actor -> #90102]);
    operation == "cancel" && return $reaction:cancel_timer(host, 'alarm);
    operation == "idle" && return $reaction:change(host, 'mode, 'set, "idle");
    operation == "bad_counter" && return $reaction:change(host, 'count, 'set, "many");
    operation == "bad_choice" && return $reaction:change(host, 'mode, 'set, "missing");
    raise(E_INVARG);
  endmethod

  method test_reinstall_cancels_timers_and_movement_changes_scope owner: ARCH_WIZARD
    "Replacing an installation cancels its task immediately; moved hosts cannot execute captured effects.";
    const first = create($thing, #90102, 2);
    const second = create($thing, #90102, 2);
    try
      for host in ({first, second})
        $reaction:declare_state(host, 'count, 'integer, 0);
        const reaction = $reaction:mk('start, 0, {{'schedule, 'alarm, 1, {'increment, 'count}}});
        $reaction:install(host, "alarm_reaction", reaction);
        this:_timer_as_owner(host, "start");
      endfor
      let disabled = first.alarm_reaction;
      disabled.enabled = false;
      $reaction:install(first, "alarm_reaction", disabled);
      $test_utils:assert_eq($reaction:timers(first), [], "Disabling cancels immediately");
      disabled.enabled = true;
      $reaction:install(first, "alarm_reaction", disabled);
      move(second, #90103);
      suspend(2);
      $test_utils:assert_eq({first.count, second.count}, {0, 0},
        "Re-enabling does not revive a timer; a changed location does not reuse its old scope");
    finally
      for host in ({first, second})
        $reaction:cancel_timer(host, 'alarm);
        recycle(host);
      endfor
    endtry
    return true;
  endmethod

  verb _fixture (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Create a named thing fixture for reaction state scenarios.";
    {name} = args;
    fixture = create($thing);
    fixture:set_name_aliases(name, {});
    return fixture;
  endverb

  verb test_headless_reaction_trigger_mutates_state (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: fire_trigger runs enabled reaction effects and skips disabled ones.";
    target = #-1;
    actor = #-1;
    try
      target = this:_fixture("headless reaction target");
      actor = this:_fixture("headless reaction actor");
      add_property(target, "headless_count", 0, {this.owner, "r"});
      add_property(target, "headless_state", "idle", {this.owner, "r"});
      enabled = $reaction:mk('on_ping, 0, {{'increment, 'headless_count, 2}, {'set, 'headless_state, "awake"}});
      disabled = $reaction:mk('on_ping, 0, {{'increment, 'headless_count, 100}});
      disabled.enabled = false;
      $reaction:install(target, "enabled_ping_reaction", enabled);
      $reaction:install(target, "disabled_ping_reaction", disabled);
      target:fire_trigger('on_ping, ['Actor -> actor]);
      $test_utils:assert_eq(target.headless_count, 2, "enabled reaction should mutate numeric fixture state");
      $test_utils:assert_eq(target.headless_state, "awake", "enabled reaction should mutate symbolic fixture state");
    finally
      valid(actor) && actor:destroy();
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_reaction_threshold_fires_during_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: mutation effects fire threshold reactions when a property crosses its boundary.";
    target = #-1;
    actor = #-1;
    try
      target = this:_fixture("headless threshold target");
      actor = this:_fixture("headless threshold actor");
      add_property(target, "score", 1, {this.owner, "r"});
      add_property(target, "threshold_seen", false, {this.owner, "r"});
      threshold = $reaction:mk({'when, 'score, 'ge, 5}, 0, {{'set, 'threshold_seen, true}});
      scorer = $reaction:mk('on_score, 0, {{'increment, 'score, 4}, {'decrement, 'score, 1}});
      $reaction:install(target, "score_threshold_reaction", threshold);
      $reaction:install(target, "score_event_reaction", scorer);
      target:fire_trigger('on_score, ['Actor -> actor]);
      $test_utils:assert_eq(target.score, 4, "mutation effects should continue after threshold reactions run");
      $test_utils:assert_true(target.threshold_seen, "threshold reaction should fire at the crossing point");
    finally
      valid(actor) && actor:destroy();
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_reaction_trigger_chains_context (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: trigger effects chain into another object's reactions while preserving actor context.";
    source = #-1;
    receiver = #-1;
    actor = #-1;
    handler = #-1;
    try
      source = this:_fixture("headless reaction source");
      receiver = this:_fixture("headless reaction receiver");
      actor = this:_fixture("headless reaction actor");
      handler = this:_fixture("headless reaction handler");
      add_property(receiver, "chain_count", 0, {this.owner, "r"});
      add_property(receiver, "chain_actor", #-1, {this.owner, "r"});
      add_property(receiver, "chain_this", #-1, {this.owner, "r"});
      add_verb(handler, {$arch_wizard, "rxd", "action_record_context"}, {"this", "none", "this"});
      set_verb_code(handler, "action_record_context", {"set_task_perms(caller_perms());", "const {values, context} = args;", "values['target].chain_actor = values['actor];", "values['target].chain_this = values['target];", "return ['status -> 'applied];"});
      $reaction:register_action('record_context,
        ['description -> "Record the trigger participants.", 'inputs -> ['target -> 'object, 'actor -> 'object]],
        handler, "action_record_context");
      $reaction:install(source, "source_trigger_reaction", $reaction:mk('on_first, 0, {{'trigger, receiver, 'on_second}}));
      $reaction:install(receiver, "receiver_trigger_reaction", $reaction:mk('on_second, 0, {{'increment, 'chain_count, 1}, {'action, 'record_context, ['target -> ['kind -> 'input, 'name -> 'This], 'actor -> ['kind -> 'input, 'name -> 'Actor]]}}));
      source:fire_trigger('on_first, ['Actor -> actor]);
      $test_utils:assert_eq(receiver.chain_count, 1, "trigger effect should execute the receiver reaction");
      $test_utils:assert_eq(receiver.chain_actor, actor, "chained trigger should preserve actor context");
      $test_utils:assert_eq(receiver.chain_this, receiver, "chained trigger should update receiver context");
    finally
      $reaction:unregister_action('record_context);
      valid(handler) && handler:destroy();
      valid(actor) && actor:destroy();
      valid(receiver) && receiver:destroy();
      valid(source) && source:destroy();
    endtry
    return true;
  endverb
  method test_item_entry_and_exit_use_surrounding_room owner: ARCH_WIZARD
    "Movement events on a character identify that character and its surrounding room.";
    const room = create($room, #90100, 2);
    const host = create($actor, #90101, 2);
    const item = create($thing, #90102, 2);
    try
      move(host, room);
      $reaction:declare_state(host, 'arrivals, 'integer, 0);
      $reaction:declare_state(host, 'departures, 'integer, 0);
      const condition = "input.This == " + toliteral(host) + " AND input.Who == "
        + toliteral(item) + " AND input.Location == " + toliteral(room);
      $reaction:install(host, "arrive_reaction", $reaction:mk('on_enter, condition,
        {{'increment, 'arrivals}}, $reaction:state_types(host)));
      $reaction:install(host, "leave_reaction", $reaction:mk('on_exit, condition,
        {{'increment, 'departures}}, $reaction:state_types(host)));
      host:enterfunc(item);
      host:exitfunc(item);
      $test_utils:assert_eq({host.arrivals, host.departures}, {0, 0},
        "Calling movement hooks directly cannot forge events");
      move(item, host);
      move(item, room);
      $test_utils:assert_eq({host.arrivals, host.departures}, {1, 1},
        "Both completed movements identify the surrounding room");
    finally
      recycle(item);
      recycle(host);
      recycle(room);
    endtry
    return true;
  endmethod
endobject
