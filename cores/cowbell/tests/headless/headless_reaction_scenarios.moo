object HEADLESS_REACTION_SCENARIOS
  name: "Headless Reaction Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for reaction state mutation, thresholds, and trigger chaining.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_reaction_scenarios";

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
      add_verb(handler, {this.owner, "rxd", "action_record_context"}, {"this", "none", "this"});
      set_verb_code(handler, "action_record_context", {"{target, context} = args;", "target.chain_actor = context['Actor];", "target.chain_this = context['This];", "return true;"});
      $reaction:install(source, "source_trigger_reaction", $reaction:mk('on_first, 0, {{'trigger, receiver, 'on_second}}));
      $reaction:install(receiver, "receiver_trigger_reaction", $reaction:mk('on_second, 0, {{'increment, 'chain_count, 1}, {'action, 'record_context, handler}}));
      source:fire_trigger('on_first, ['Actor -> actor]);
      $test_utils:assert_eq(receiver.chain_count, 1, "trigger effect should execute the receiver reaction");
      $test_utils:assert_eq(receiver.chain_actor, actor, "chained trigger should preserve actor context");
      $test_utils:assert_eq(receiver.chain_this, receiver, "chained trigger should update receiver context");
    finally
      valid(handler) && handler:destroy();
      valid(actor) && actor:destroy();
      valid(receiver) && receiver:destroy();
      valid(source) && source:destroy();
    endtry
    return true;
  endverb
endobject
