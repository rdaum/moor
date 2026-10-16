object HEADLESS_LLM_SCENARIOS
  name: "Headless LLM and Tool Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_llm_scenarios";

  method test_rule_tool_uses_supplied_inputs_and_actor owner: ARCH_WIZARD
    "Rule tools preserve literal text, report invalid conditions, and cannot read private facts.";
    const target = create($thing, #90100, 2);
    try
      add_property(target, "private_rule_fact", 1, {#90100, ""});
      const result = $llm_behavior_tools:tool_test_rule([
        "expression" -> "input.Actor == #90102 AND input.Word == \"Actor\"",
        "bindings" -> ["Actor" -> "#90102", "Word" -> "Actor"]], #90102);
      $test_utils:assert_true(index(result, "Condition matched"),
        "An explicit reference is resolved while ordinary text remains text");
      for sample in ({
        {["expression" -> "TRUE ignored", "bindings" -> []], E_INVARG},
        {["expression" -> "NOT property_is(object: input.Target, field: 'private_rule_fact, value: 1)",
          "bindings" -> ["Target" -> tostr(target)]], E_PERM}})
        const {values, expected} = sample;
        let failure = 0;
        try
          $llm_behavior_tools:tool_test_rule(values, #90102);
        except error (E_INVARG, E_PERM)
          failure = error[1];
        endtry
        $test_utils:assert_eq(failure, expected, "Tool errors retain their cause and actor");
      endfor
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method test_rule_tool_input_limits owner: ARCH_WIZARD
    "LLM rule inputs are bounded JSON values with explicit names and object literals.";
    let too_many = [];
    for i in [1..65]
      too_many["input_" + tostr(i)] = i;
    endfor
    for sample in ({{"['Actor -> #90102]", E_TYPE}, {["Actor" -> #90102], E_TYPE},
        {["" -> 1], E_INVARG}, {["Actor" -> "#not_an_object"], E_INVARG}, {too_many, E_QUOTA}})
      const {bindings, expected} = sample;
      let failure = 0;
      try
        $llm_behavior_tools:tool_test_rule(["expression" -> "TRUE", "bindings" -> bindings], #90102);
      except error (E_TYPE, E_INVARG, E_QUOTA)
        failure = error[1];
      endtry
      $test_utils:assert_eq(failure, expected, "Invalid rule inputs fail before evaluation");
    endfor
    return true;
  endmethod

  method test_headless_wearable_token_usage owner: ARCH_WIZARD
    "Token displays accept integer limits and float thresholds for fresh and used contexts.";
    const wearer = create($player, $hacker, 2);
    const wearable = create($llm_wearable, $hacker, 2);
    const agent = create($llm_agent, $hacker, 2);
    try
      add_property(wearer, "captured_events", {}, {$arch_wizard, ""});
      add_verb(wearer, {$arch_wizard, "rxd", "inform_current"}, {"this", "none", "this"});
      set_verb_code(wearer, "inform_current", {
        "this.captured_events = {@this.captured_events, args[1]};"});
      wearer.llm_token_budget = 10000;
      wearer.llm_tokens_used = 2500;
      wearable.agent = agent;
      agent.token_limit = 2000;
      agent.compaction_threshold = 0.75;
      for sample in ({{0, "FRESH"}, {1199, "OK"}, {1200, "NEAR LIMIT"}, {1500, "COMPACTING"}})
        const {prompt_tokens, status} = sample;
        agent.last_token_usage = prompt_tokens
          ? ["prompt_tokens" -> prompt_tokens, "total_tokens" -> prompt_tokens + 10] | [];
        wearer.captured_events = {};
        wearable:_show_token_usage(wearer);
        $test_utils:assert_eq(length(wearer.captured_events), 2,
          "usage display emits both budget and context events");
        const budget = $document:render(wearer.captured_events[1]:resolve(wearer), 'text_plain)['body];
        $test_utils:assert_true(index(budget, "2500/10000 (25% used)"),
          "budget display preserves integer percentages");
        const context = $document:render(wearer.captured_events[2]:resolve(wearer), 'text_plain)['body];
        $test_utils:assert_true(index(context, "Compaction at 1500"),
          "float threshold produces an integer token count");
        $test_utils:assert_true(index(context, status), "context displays the expected status");
        if (prompt_tokens)
          const diagnostic = wearable:debug_status();
          $test_utils:assert_eq(diagnostic["threshold_tokens"], 1500,
            "diagnostic agrees with the displayed compaction threshold");
          $test_utils:assert_eq(diagnostic["status"], status,
            "diagnostic agrees with the displayed status");
        endif
      endfor
      wearer.llm_token_budget = 0;
      wearer.captured_events = {};
      wearable:_show_token_usage(wearer);
      $test_utils:assert_eq(length(wearer.captured_events), 2,
        "A zero token budget still produces a bounded usage display");
    finally
      for temporary in ({wearable, agent, wearer})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

  method test_headless_building_tool_rejects_actor_spoof owner: ARCH_WIZARD
    "A direct non-wizard tool caller cannot claim a wizard actor.";
    const target = create($thing, $hacker, 2);
    try
      const original_name = target.name;
      let denied = false;
      try
        this:_direct_rename_as_player(target);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "foreign actor must be denied");
      $test_utils:assert_eq(target.name, original_name, "denied spoof must preserve target state");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endmethod

  method _direct_rename_as_player owner: RUNTIME_PLAYER
    "Attempt only the direct actor-spoof regression under player permissions.";
    caller == this && this == #90012 || raise(E_PERM);
    const {target} = args;
    return $llm_building_tools:rename_object(["object" -> target, "name" -> "spoofed"], $arch_wizard);
  endmethod

  method test_headless_agent_tool_rejects_hostile_descendant owner: ARCH_WIZARD
    "Agent ancestry and gifted ownership cannot authorize a claimed wizard actor.";
    const target = create($thing, $hacker, 2);
    try
      for specification in ({
        {$llm_agent, $llm_agent_tool},
        {$llm_agent, $llm_agent_tool}})
        const {prototype, delegate} = specification;
        const agent = create(prototype, #90102, 2);
        try
          add_verb(agent, {#90102, "rxd", "fake_model_tool_call"}, {"this", "none", "this"});
          set_verb_code(agent, "fake_model_tool_call", {
            "const {tool, values, actor} = args;",
            "return tool:execute(values, actor);"});
          const tool = delegate:mk("rename", "Regression rename", [], $llm_building_tools, "rename_object");
          for gift in ({false, true})
            gift && this:_gift_agent_as_player(agent);
            const original_name = target.name;
            let denied = false;
            try
              agent:fake_model_tool_call(tool, ["object" -> target, "name" -> "spoofed"], $arch_wizard);
            except (E_PERM)
              denied = true;
            endtry
            $test_utils:assert_true(denied, "a hostile descendant must not impersonate a wizard");
            $test_utils:assert_eq(target.name, original_name, "failed dispatch must preserve target state");
          endfor
        finally
          valid(agent) && agent:destroy();
        endtry
      endfor
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endmethod

  method _gift_agent_as_player owner: RUNTIME_PLAYER
    "Transfer an owned hostile agent to the core owner to test provenance claims.";
    caller == this && this == #90012 || raise(E_PERM);
    const {agent} = args;
    return agent:set_owner($hacker);
  endmethod

  method test_headless_agent_public_context_denies_foreign_principal owner: ARCH_WIZARD
    "A public state wrapper must check the invoking principal before calling its wizard helper.";
    const agent = create($llm_agent, $hacker, 2);
    try
      const original_context = agent:get_context();
      let denied = false;
      try
        this:_append_context_as_player(agent);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "a foreign principal must not append agent context");
      $test_utils:assert_eq(agent:get_context(), original_context, "denied append must preserve context");
    finally
      valid(agent) && agent:destroy();
    endtry
    return true;
  endmethod

  method _append_context_as_player owner: RUNTIME_PLAYER
    "Attempt only the public agent-state regression under player permissions.";
    caller == this && this == #90012 || raise(E_PERM);
    const {agent} = args;
    return agent:add_message("user", "untrusted appended message");
  endmethod
  method test_headless_tools_deny_foreign_private_access owner: ARCH_WIZARD
    "A real non-wizard actor receives no private read or write grants merely by invoking tools.";
    const target = create($thing, $hacker, 2);
    try
      target.r = 0;
      add_property(target, "private_probe", "secret", {$hacker, ""});
      add_verb(target, {$hacker, "xd", "private_probe"}, {"this", "none", "this"});
      set_verb_code(target, "private_probe", {"return \"secret\";"});
      for entry in ({
        {"get_property", ["object" -> target, "property" -> "private_probe"]},
        {"set_property", ["object" -> target, "property" -> "private_probe", "value" -> "stolen"]},
        {"get_verb_code", ["object" -> target, "verb" -> "private_probe"]},
        {"list_verbs", ["object" -> target]},
        {"list_properties", ["object" -> target]}})
        const {endpoint, values} = entry;
        let denied = false;
        try
          this:_tool_as_player(endpoint, values);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, endpoint + " must deny foreign private access");
        $test_utils:assert_eq(target.private_probe, "secret", "denied tools preserve private state");
      endfor
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endmethod

  method test_headless_tools_allow_own_and_public_access owner: ARCH_WIZARD
    "Own private data and public foreign data remain available to ordinary actor tools.";
    const own_target = create($thing, #90102, 2);
    const public_target = create($thing, $hacker, 2);
    try
      add_property(own_target, "probe", "own", {#90102, ""});
      add_property(public_target, "probe", "public", {$hacker, "r"});
      const own_result = this:_tool_as_player("get_property", ["object" -> own_target, "property" -> "probe"]);
      $test_utils:assert_true(index(own_result, "own") > 0, "owned private property is readable");
      const public_result = this:_tool_as_player("get_property", ["object" -> public_target, "property" -> "probe"]);
      $test_utils:assert_true(index(public_result, "public") > 0, "foreign public property is readable");
      this:_tool_as_player("set_property", ["object" -> own_target, "property" -> "probe", "value" -> "updated"]);
      $test_utils:assert_eq(own_target.probe, "updated", "owned private property is writable");
    finally
      valid(own_target) && own_target:destroy();
      valid(public_target) && public_target:destroy();
    endtry
    return true;
  endmethod

  method _tool_as_player owner: RUNTIME_PLAYER
    "Invoke a selected real endpoint under player authority for these fixed regressions.";
    caller == this && this == #90012 || raise(E_PERM);
    const {endpoint, values} = args;
    const provider = endpoint == "set_property" ? $llm_programming_tools | $llm_inspection_tools;
    return provider:(endpoint)(values, #90102);
  endmethod

  method test_headless_fake_model_preserves_requester_across_callbacks owner: ARCH_WIZARD
    "Local model replies exercise the common agent, successful tools, denied tools, and a suspending metadata mutation.";
    for specification in ({{$llm_agent, $llm_agent_tool}})
      const {prototype, delegate} = specification;
      const agent = create(prototype, #90102, 2);
      const client = create($root, #90102, 2);
      const callback = create($root, #90102, 2);
      const own_target = create($thing, #90102, 2);
      const private_target = create($thing, $hacker, 2);
      try
        add_property(private_target, "secret", "private", {$hacker, ""});
        add_property(callback, "agent", agent, {#90102, ""});
        add_property(callback, "seen", {}, {#90102, ""});
        add_verb(callback, {#90102, "rxd", "on_tool_call on_tool_complete on_tool_error"}, {"this", "none", "this"});
        set_verb_code(callback, "on_tool_call", {
          "this.seen = {@this.seen, {verb, caller_perms()}};",
          "if (verb == \"on_tool_call\")",
          "  this.agent.claimed_actor = $arch_wizard;",
          "  suspend(0);",
          "endif"});
        const calls = {
          ["id" -> "success", "function" -> ["name" -> "rename", "arguments" -> ["object" -> own_target, "name" -> "requester rename"]]],
          ["id" -> "denied", "function" -> ["name" -> "read", "arguments" -> ["object" -> private_target, "property" -> "secret"]]]};
        const replies = {
          ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> calls]]}],
          ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "done"]]}]};
        add_property(client, "replies", replies, {#90102, ""});
        add_verb(client, {#90102, "rxd", "chat"}, {"this", "none", "this"});
        set_verb_code(client, "chat", {
          "const reply = this.replies[1];",
          "this.replies = listdelete(this.replies, 1);",
          "return reply;"});
        agent.client = client;
        agent.tool_callback = callback;
        add_property(agent, "claimed_actor", #90102, {#90102, ""});
        agent.tools = [
          "rename" -> delegate:mk("rename", "Rename own target", [], $llm_building_tools, "rename_object"),
          "read" -> delegate:mk("read", "Read target", [], $llm_inspection_tools, "get_property", 'inspection)];
        const result = this:_send_as_player(agent, "local regression");
        $test_utils:assert_eq(result, "done", "local model reaches final response");
        $test_utils:assert_eq(own_target.name, "requester rename", "legitimate requester mutation succeeds");
        $test_utils:assert_eq(private_target.secret, "private", "denied private tool preserves data");
        $test_utils:assert_eq(agent.claimed_actor, $arch_wizard, "callback changed untrusted actor metadata");
        $test_utils:assert_true(length(callback.seen) >= 4, "both tool callbacks were invoked");
        for observation in (callback.seen)
          $test_utils:assert_eq(observation[2], #90102, "external callbacks receive requester authority");
        endfor
        let denied_result = false;
        for message in (agent:get_context())
          if (maphaskey(message, "tool_call_id") && message["tool_call_id"] == "denied")
            const outcome = parse_json(message["content"]);
            denied_result = !outcome["ok"] && outcome["code"] == "E_PERM";
          endif
        endfor
        $test_utils:assert_true(denied_result, "permission failure is an error result, not successful private access");
      finally
        for temporary in ({agent, client, callback, own_target, private_target})
          valid(temporary) && temporary:destroy();
        endfor
      endtry
    endfor
    return true;
  endmethod

  method _send_as_player owner: RUNTIME_PLAYER
    "Exercise the ordinary public agent entry as the real requesting player.";
    caller == this && this == #90012 || raise(E_PERM);
    const {agent, prompt} = args;
    return agent:send_message(prompt);
  endmethod

  method test_headless_room_owner_cannot_forge_wizard_requester owner: ARCH_WIZARD
    "Owning a room and its task state cannot authorize a forged foreign requester.";
    const room = create($agent_room, #90102, 2);
    const client = create($root, #90102, 2);
    const target = create($thing, $hacker, 2);
    try
      const original_name = target.name;
      const code = tostr(target) + ".name = \"room spoof\"; return \"changed\";";
      const calls = {
        ["id" -> "spoof", "function" -> ["name" -> "moo_eval", "arguments" -> ["code" -> code]]],
        ["id" -> "finish", "function" -> ["name" -> "report_finding", "arguments" -> ["subject" -> "finished", "content" -> "done", "final" -> true]] ]};
      add_property(client, "reply", ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> calls]]}], {#90102, ""});
      add_verb(client, {#90102, "rxd", "chat"}, {"this", "none", "this"});
      set_verb_code(client, "chat", {"const result = this.reply;", "this.reply = [\"choices\" -> {[\"message\" -> [\"role\" -> \"assistant\", \"content\" -> \"completed\"]]}];", "return result;"});
      room.llm_client = client;
      add_verb(room, {#90102, "rxd", "_announce"}, {"this", "none", "this"});
      set_verb_code(room, "_announce", {"return 0;"});
      add_verb(room, {#90102, "rxd", "forge_task"}, {"this", "none", "this"});
      set_verb_code(room, "forge_task", {"return this:_execute_task(['player -> $arch_wizard, 'query -> \"forged wizard request\"]);"});
      let denied = false;
      try
        room:forge_task();
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_eq(target.name, original_name, "forged requester must not modify foreign object");
      $test_utils:assert_true(denied, "unsigned owner-created request must be denied before execution");
    finally
      for temporary in ({room, client, target})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

  method test_headless_no_rlm_runtime owner: ARCH_WIZARD
    "The experimental runtime and its system registration are absent from a fresh core.";
    $test_utils:assert_false(valid(#100), "No RLM prototype remains");
    $test_utils:assert_false("rlm_agent" in properties($sysobj), "No RLM system registration remains");
    return true;
  endmethod

  method test_headless_signed_room_visitor_tamper_and_replay owner: ARCH_WIZARD
    "A visitor's bounded signed request executes once; actor, query, context, and room changes fail.";
    const room = create($agent_room, $hacker, 2);
    const other_room = create($agent_room, $hacker, 2);
    const client = create($root, #90102, 2);
    const target = create($thing, #90102, 2);
    try
      const code = toliteral(target) + ".name = \"visitor success\"; return 1;";
      const calls = {
        ["id" -> "own", "function" -> ["name" -> "rename_object", "arguments" -> ["object" -> tostr(target), "name" -> "visitor success"]]]};
      add_property(client, "reply", ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> calls]]}], {#90102, ""});
      add_verb(client, {#90102, "rxd", "chat"}, {"this", "none", "this"});
      set_verb_code(client, "chat", {"const result = this.reply;", "this.reply = [\"choices\" -> {[\"message\" -> [\"role\" -> \"assistant\", \"content\" -> \"completed\"]]}];", "return result;"});
      room.llm_client = client;
      for candidate in ({room, other_room})
        add_verb(candidate, {#90102, "rxd", "execute_signed"}, {"this", "none", "this"});
        set_verb_code(candidate, "execute_signed", {"const {task} = args; return this:_execute_task(task);"});
      endfor
      let task = ['player -> #90102, 'query -> "visit and rename own target"];
      task["authorization"] = this:_sign_as_player(room, task);
      for change in ({
        {'player, $arch_wizard},
        {'query, "a different request"},
        {'context, {["role" -> "user", "content" -> "injected context"]}}})
        let altered = task;
        altered[change[1]] = change[2];
        let denied = false;
        try
          room:execute_signed(altered);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "modified task field must fail authorization");
      endfor
      let wrong_room_denied = false;
      try
        other_room:execute_signed(task);
      except (E_PERM)
        wrong_room_denied = true;
      endtry
      $test_utils:assert_true(wrong_room_denied, "authorization is bound to the exact room");
      room:execute_signed(task);
      $test_utils:assert_eq(target.name, "visitor success", "visitor runs with ordinary requester authority");
      $test_utils:assert_eq(room.history[$]['requester], #90102, "history records authenticated requester");
      let replay_denied = false;
      try
        room:execute_signed(task);
      except (E_PERM)
        replay_denied = true;
      endtry
      $test_utils:assert_true(replay_denied, "consumed task authorization cannot replay");
    finally
      for temporary in ({room, other_room, client, target})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

  method _sign_as_player owner: RUNTIME_PLAYER
    "A visitor can authorize its own bounded request, with no room ownership required.";
    caller == this && this == #90012 || raise(E_PERM);
    const {room, task} = args;
    return $agent_room:_authorize_task(room, task);
  endmethod

  method test_headless_authorization_overrides_cannot_bypass_boundary owner: ARCH_WIZARD
    "Ordinary descendants cannot override the permission decision used by inherited wizard entry points.";
    const tools = create($llm_building_tools, #90102, 2);
    const agent = create($llm_agent, #90102, 2);
    const target = create($thing, $hacker, 2);
    try
      const original_name = target.name;
      add_verb(tools, {#90102, "rxd", "_require_tool_dispatch"}, {"this", "none", "this"});
      set_verb_code(tools, "_require_tool_dispatch", {"return true;"});
      add_verb(tools, {#90102, "rxd", "attempt_rename"}, {"this", "none", "this"});
      set_verb_code(tools, "attempt_rename", {"const {target} = args; return this:rename_object([\"object\" -> target, \"name\" -> \"override spoof\"], $arch_wizard);"});
      let denied = false;
      try
        tools:attempt_rename(target);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "overridden decision helper must not authorize a foreign actor");
      $test_utils:assert_eq(target.name, original_name, "denied override preserves foreign target");
      add_verb(agent, {#90102, "rxd", "_challenge_permissions"}, {"this", "none", "this"});
      set_verb_code(agent, "_challenge_permissions", {"return true;"});
      this:_gift_agent_as_player(agent);
      const original_context = agent:get_context();
      let context_denied = false;
      try
        this:_append_context_as_player(agent);
      except (E_PERM)
        context_denied = true;
      endtry
      $test_utils:assert_true(context_denied, "overridden challenge must not authorize foreign state mutation");
      $test_utils:assert_eq(agent:get_context(), original_context, "denied challenge preserves context");
    finally
      for temporary in ({tools, agent, target})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

  method test_headless_observer_preserves_npc_tool_actor owner: ARCH_WIZARD
    "A verified observer dispatch keeps the NPC actor instead of inheriting its owner's permissions.";
    const observer = create($llm_room_observer, #90102, 2);
    const agent = create($llm_agent, #90102, 1);
    const client = create($root, #90102, 2);
    const callback = create($root, #90102, 2);
    const target = create($thing, observer, 2);
    try
      const call = ["id" -> "npc", "function" -> ["name" -> "rename", "arguments" -> ["object" -> target, "name" -> "npc actor"]]];
      const replies = {
        ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> {call}]]}],
        ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "done"]]}]};
      add_property(client, "replies", replies, {#90102, ""});
      add_verb(client, {#90102, "rxd", "chat"}, {"this", "none", "this"});
      set_verb_code(client, "chat", {"const reply = this.replies[1]; this.replies = listdelete(this.replies, 1); return reply;"});
      add_property(callback, "seen", #-1, {#90102, ""});
      add_verb(callback, {#90102, "rxd", "on_tool_call"}, {"this", "none", "this"});
      set_verb_code(callback, "on_tool_call", {"this.seen = caller_perms();"});
      agent.client = client;
      add_property(observer, "callback_fixture", callback, {#90102, ""});
      add_verb(observer, {#90102, "rxd", "on_tool_call"}, {"this", "none", "this"});
      set_verb_code(observer, "on_tool_call", {"this.callback_fixture.seen = caller_perms();"});
      add_property(agent, "claimed_actor", #90102, {#90102, ""});
      agent.tools = ["rename" -> $llm_agent_tool:mk("rename", "NPC rename", [], $llm_building_tools, "rename_object")];
      observer.agent = agent;
      const job = this:_send_observer_as_player(observer, "local NPC regression");
      for attempt in [1..60]
        if (job.retired)
          break;
        endif
        suspend(0.1);
      endfor
      const result = job.state;
      $test_utils:assert_eq(result, 'complete, "observer fake response completes");
      $test_utils:assert_eq(target.name, "npc actor", "NPC can mutate its own target");
      $test_utils:assert_eq(callback.seen, observer, "callbacks use the NPC principal");
      $test_utils:assert_eq(agent.claimed_actor, #90102, "untrusted actor metadata cannot replace NPC authority");
    finally
      for temporary in ({observer, client, callback, target})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

  method _send_observer_as_player owner: RUNTIME_PLAYER
    "Exercise the observer owner control boundary as an ordinary principal.";
    caller == this && this == #90012 || raise(E_PERM);
    const {observer, prompt} = args;
    return observer:request_response(prompt);
  endmethod

  method test_headless_shared_agent_foreign_configuration_denied owner: ARCH_WIZARD
    "Foreign callers cannot replace tools on the maintained agent.";
    const agent = create($llm_agent, $hacker, 1);
    const original = agent.tools;
    let denied = false;
    try
      this:_configure_agent_as_player(agent);
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Foreign tool registration is denied");
    $test_utils:assert_eq(agent.tools, original, "Denied registration preserves tools");
    return true;
  endmethod

  method _configure_agent_as_player owner: RUNTIME_PLAYER
    "Exercise foreign registration as an ordinary authenticated principal.";
    caller == this && this == #90012 || raise(E_PERM);
    const {agent} = args;
    return agent:add_tool("foreign", $llm_agent_tool:mk("foreign", "No authority", ["type" -> "object"], this, "noop"));
  endmethod

  method test_headless_source_snapshot_cannot_spoof_actor owner: ARCH_WIZARD
    "Shared source snapshots validate the principal before reading private code.";
    const reader = create($root, #90102, 2);
    const foreign = create($thing, $hacker, 2);
    const owned = create($thing, #90102, 2);
    try
      for target in ({foreign, owned})
        target.r = 0;
        add_verb(target, {target.owner, "xd", "payload_probe"}, {"this", "none", "this"});
        set_verb_code(target, "payload_probe", {"return 123;"});
      endfor
      add_verb(reader, {#90102, "rxd", "payload_probe"}, {"this", "none", "this"});
      set_verb_code(reader, "payload_probe", {"const {target, actor} = args; return $llm_verb_source:read(actor, target, \"payload_probe\");"});
      let denied = false;
      try
        reader:payload_probe(foreign, $arch_wizard);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "ordinary caller cannot impersonate a wizard while reading source");
      const payload = reader:payload_probe(owned, #90102);
      $test_utils:assert_type(payload, TYPE_FLYWEIGHT, "ordinary owned code presentation remains available");
      denied = false;
      try
        reader:payload_probe(foreign, #90102);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "ordinary payload cannot inspect foreign private code");
      foreign.r = 1;
      add_verb(reader, {#90102, "rxd", "metadata_probe"}, {"this", "none", "this"});
      set_verb_code(reader, "metadata_probe", {
        "return $llm_verb_source:metadata(#90102, args[1], \"payload_probe\");"});
      denied = false;
      try
        reader:metadata_probe(foreign);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Native metadata permissions remain enforced");
      set_verb_info(foreign, "payload_probe", {$hacker, "rxd", "payload_probe"});
      const metadata = reader:metadata_probe(foreign);
      $test_utils:assert_eq(metadata.owner, $hacker, "Readable verb metadata uses the actual owner");
    finally
      for temporary in ({reader, foreign, owned})
        valid(temporary) && temporary:destroy();
      endfor
    endtry
    return true;
  endmethod

endobject
