object AGENTIC_CODING_ROOM [
  import_export_id -> "agentic_coding_room",
  import_export_hierarchy -> {"agentic"}
]
  name: "Agentic Coding Room"
  parent: AGENT_ROOM
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property agentic_last_error (owner: ARCH_WIZARD, flags: "rc") = "\"\"";
  property agentic_max_iterations (owner: ARCH_WIZARD, flags: "rc") = 30;
  property agentic_progress_mode (owner: ARCH_WIZARD, flags: "rc") = "\"normal\"";
  property agentic_system_prompt (owner: ARCH_WIZARD, flags: "rc") = "You are a room-based coding agent for MOO development. Use tools to inspect code, propose targeted changes, and program verbs safely. Prefer showing concise reports and code snippets.";
  property agentic_task_timeout_seconds (owner: ARCH_WIZARD, flags: "rc") = 180;
  property agentic_tool_current (owner: ARCH_WIZARD, flags: "rc") = "\"\"";
  property agentic_tool_last (owner: ARCH_WIZARD, flags: "rc") = "\"\"";
  property agentic_tool_note (owner: ARCH_WIZARD, flags: "rc") = "\"\"";
  property agentic_tool_state (owner: ARCH_WIZARD, flags: "rc") = "\"\"";
  property agentic_wait_started (owner: ARCH_WIZARD, flags: "rc") = 0;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Room-based coding agent that uses $agentic.agent for task execution and tool orchestration.";

  method _tool_dump_object owner: ARCH_WIZARD
    "Tool: Dump object summary with properties and verbs.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    valid(actor) || raise(E_PERM);
    const obj_ref = `args_map["object"] ! ANY => ""';
    typeof(obj_ref) == TYPE_STR || raise(E_TYPE, "object must be string");
    const obj = $match:match_object(obj_ref, actor);
    valid(obj) || raise(E_INVARG, "Could not find object: " + obj_ref);
    set_task_perms(actor);
    const name = `obj.name ! ANY => tostr(obj)';
    const parent_obj = parent(obj);
    const props = properties(obj);
    const verbs_list = verbs(obj);
    const lines = {"Object: " + tostr(obj) + " (" + name + ")", "Parent: " + tostr(parent_obj), "Properties (" + tostr(length(props)) + "): " + (length(props) > 0 ? props:join(", ") | "(none)"), "Verbs (" + tostr(length(verbs_list)) + "): " + (length(verbs_list) > 0 ? verbs_list:join(", ") | "(none)")};
    return lines:join("\n");
  endmethod

  method _tool_list_verbs owner: ARCH_WIZARD
    "Tool: List verbs on an object.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    valid(actor) || raise(E_PERM);
    const obj_ref = `args_map["object"] ! ANY => ""';
    typeof(obj_ref) == TYPE_STR || raise(E_TYPE, "object must be string");
    const obj = $match:match_object(obj_ref, actor);
    valid(obj) || raise(E_INVARG, "Could not find object: " + obj_ref);
    set_task_perms(actor);
    const verbs_list = verbs(obj);
    !verbs_list && return "No verbs on " + tostr(obj) + ".";
    return "Verbs on " + tostr(obj) + ":\n- " + verbs_list:join("\n- ");
  endmethod

  method _tool_get_verb_code owner: ARCH_WIZARD
    "Tool: Return full code for a verb.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    valid(actor) || raise(E_PERM);
    const obj_ref = `args_map["object"] ! ANY => ""';
    const verb_name = `args_map["verb"] ! ANY => ""';
    typeof(obj_ref) == TYPE_STR || raise(E_TYPE, "object must be string");
    typeof(verb_name) == TYPE_STR || raise(E_TYPE, "verb must be string");
    const obj = $match:match_object(obj_ref, actor);
    valid(obj) || raise(E_INVARG, "Could not find object: " + obj_ref);
    set_task_perms(actor);
    const code_lines = `verb_code(obj, verb_name) ! E_VERBNF => 0';
    code_lines == 0 && raise(E_VERBNF, "Verb not found: " + verb_name);
    return code_lines:join("\n");
  endmethod

  method _tool_read_property owner: ARCH_WIZARD
    "Tool: Read a property value from an object.";
    let value = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    valid(actor) || raise(E_PERM);
    const obj_ref = `args_map["object"] ! ANY => ""';
    const prop_name = `args_map["property"] ! ANY => ""';
    typeof(obj_ref) == TYPE_STR || raise(E_TYPE, "object must be string");
    typeof(prop_name) == TYPE_STR || raise(E_TYPE, "property must be string");
    const obj = $match:match_object(obj_ref, actor);
    valid(obj) || raise(E_INVARG, "Could not find object: " + obj_ref);
    try
      set_task_perms(actor);
      value = obj.(prop_name);
    except e (E_PROPNF)
      raise(E_PROPNF, "Property not found: " + prop_name);
    endtry
    return toliteral(value);
  endmethod

  method test_agentic_inspection_tools owner: ARCH_WIZARD
    "Inspection tools preserve public metadata access and deny arbitrary private targets.";
    const actor = $test_player;
    const public_output = this:_tool_list_verbs(["object" -> "$agentic.agent"], actor);
    $test_utils:assert_true(index(public_output, "send_message") > 0, "readable metadata remains available");
    const target = create($thing, $hacker, 2);
    try
      target.r = 0;
      add_property(target, "private_probe", "secret", {$hacker, ""});
      add_verb(target, {$hacker, "xd", "private_probe"}, {"this", "none", "this"});
      set_verb_code(target, "private_probe", {"return \"secret\";"});
      for entry in ({{"_tool_read_property", ["object" -> tostr(target), "property" -> "private_probe"]}, {"_tool_get_verb_code", ["object" -> tostr(target), "verb" -> "private_probe"]}, {"_tool_list_verbs", ["object" -> tostr(target)]}, {"_tool_dump_object", ["object" -> tostr(target)]}})
        let denied = false;
        try
          this:(entry[1])(entry[2], actor);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, entry[1] + " must deny foreign private access");
      endfor
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endmethod

  method _get_agentic_tools owner: ARCH_WIZARD
    "Return tool definitions for agentic coding room.";
    const base = this:_get_room_tools();
    const extra = {["name" -> "dump_object", "description" -> "Get object summary including parent, properties, and verbs.", "target_obj" -> this, "target_verb" -> "_tool_dump_object", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference like '$room' or '#123'"]], "required" -> {"object"}]], ["name" -> "list_verbs", "description" -> "List verb names on an object.", "target_obj" -> this, "target_verb" -> "_tool_list_verbs", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference like '$room' or '#123'"]], "required" -> {"object"}]], ["name" -> "get_verb_code", "description" -> "Get full source code for a verb.", "target_obj" -> this, "target_verb" -> "_tool_get_verb_code", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string"], "verb" -> ["type" -> "string"]], "required" -> {"object", "verb"}]], ["name" -> "read_property", "description" -> "Read a property value from an object.", "target_obj" -> this, "target_verb" -> "_tool_read_property", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string"], "property" -> ["type" -> "string"]], "required" -> {"object", "property"}]]};
    let building = {};
    try
      building = $agent_building_tools:get_tools();
    except (ANY)
      building = {};
    endtry
    return {@base, @extra, @building, @{tool:to_mcp_schema() for tool in ($change_tools:get_tools())}};
  endmethod

  method _execute_task owner: ARCH_WIZARD
    "Internal: execute one queued task using $agentic.agent backend.";
    let brief = 0;
    let child_result = 0;
    let child_status = 0;
    let content = 0;
    let description = 0;
    let input_schema = 0;
    let target_obj = 0;
    let target_verb = 0;
    let tool_fw = 0;
    let tool_name = 0;
    caller == this || raise(E_PERM);
    const {task} = args;
    $agent_room:_consume_task(this, task);
    const task_player = task['player];
    this.task_requester = task_player;
    this.current_task = task['query];
    this.agentic_tool_current = "";
    this.agentic_tool_last = "";
    this.agentic_tool_state = "";
    this.agentic_tool_note = "";
    this.agentic_last_error = "";
    this.agentic_wait_started = 0;
    this:_announce(task_player.name + " requested: \"" + task['query] + "\"");
    const agent = create($agentic.agent, task_player, 1);
    this.agent = agent;
    agent.client = valid(this.llm_client) ? this.llm_client | $llm_client;
    agent.token_owner = task_player;
    agent.system_prompt = `this.agentic_system_prompt ! ANY => "You are a room-based coding agent for MOO development."' + "\n\n" + $change_tools.guide;
    agent.tool_callback = this;
    agent.compaction_callback = this;
    agent:reset_context();
    let registered = [];
    for tool_def in (this:_get_agentic_tools())
      if (typeof(tool_def) == TYPE_FLYWEIGHT)
        tool_name = `tool_def.name ! ANY => ""';
        if (typeof(tool_name) == TYPE_STR && length(tool_name) > 0 && !maphaskey(registered, tool_name))
          agent:add_tool(tool_name, tool_def);
          registered[tool_name] = 1;
        endif
      elseif (typeof(tool_def) == TYPE_MAP)
        tool_name = `tool_def["name"] ! ANY => ""';
        if (typeof(tool_name) == TYPE_STR && length(tool_name) > 0 && !maphaskey(registered, tool_name))
          target_obj = `tool_def["target_obj"] ! ANY => #-1';
          target_verb = `tool_def["target_verb"] ! ANY => ""';
          input_schema = `tool_def["input_schema"] ! ANY => ["type" -> "object", "properties" -> [], "required" -> {}]';
          description = `tool_def["description"] ! ANY => ""';
          if (typeof(target_obj) == TYPE_OBJ && typeof(target_verb) == TYPE_STR && length(target_verb) > 0)
            tool_fw = $agentic.tool:mk(tool_name, description, input_schema, target_obj, target_verb);
            agent:add_tool(tool_name, tool_fw);
            registered[tool_name] = 1;
          endif
        endif
      endif
    endfor
    if (maphaskey(task, 'context) && typeof(task['context]) == TYPE_LIST)
      agent.context = task['context];
    endif
    const run_prompt = "User request: " + task['query] + "\n\nProcess the request. Use tools for inspection and edits. Present final results with present_report or present_table when useful.";
    let task_timeout = `this.agentic_task_timeout_seconds ! ANY => 180';
    if (typeof(task_timeout) != TYPE_INT || task_timeout < 15)
      task_timeout = 180;
    endif
    this.agentic_tool_state = "awaiting_model";
    this.agentic_tool_note = "Waiting for model response...";
    this.agentic_wait_started = time();
    this:_tell_requester("waiting for model response...");
    const parent_tid = task_id();
    let run_tid = 0;
    fork child_tid (0)
      try
        child_result = this:_send_as_requester(agent, run_prompt, task_player);
        child_status = typeof(child_result) == TYPE_STR && child_result:starts_with("Error:") ? 'failed | 'complete;
      except child_err (ANY)
        child_result = "Error: " + tostr(child_err[1]) + " - " + tostr(child_err[2]);
        child_status = 'failed;
      endtry
      `task_send(parent_tid, ["type" -> "agent_done", "status" -> child_status, "result" -> child_result, "tid" -> task_id()]) ! ANY';
    endfork
    run_tid = child_tid;
    const deadline = time() + task_timeout;
    let done = 0;
    let status = 'failed;
    let result = "Error: Task terminated unexpectedly.";
    while (!done)
      for msg in (task_recv(1))
        if (typeof(msg) != TYPE_MAP)
          continue;
        endif
        if (`msg["type"] ! ANY => ""' == "agent_done")
          status = `msg["status"] ! ANY => 'failed';
          result = `msg["result"] ! ANY => "Error: Missing model result."';
          done = 1;
          break;
        endif
      endfor
      if (!done && time() >= deadline)
        `kill_task(run_tid) ! ANY';
        status = 'failed;
        result = "Error: Timed out waiting for model response after " + tostr(task_timeout) + "s";
        this.agentic_last_error = result;
        this.agentic_tool_state = "timeout";
        this.agentic_tool_note = "Model call timed out after " + tostr(task_timeout) + "s";
        this:_tell_requester(("model call timed out after " + tostr(task_timeout) + "s"));
        done = 1;
      endif
    endwhile
    if (status == 'complete)
      this.agentic_tool_state = "ok";
      this.agentic_tool_note = "Model response received.";
    elseif (this.agentic_tool_state == "awaiting_model")
      this.agentic_tool_state = "error";
    endif
    this.agentic_wait_started = 0;
    let failure_reason = "";
    if (status == 'failed)
      failure_reason = typeof(result) == TYPE_STR ? result | toliteral(result);
      if (this.agentic_last_error)
        failure_reason = this.agentic_last_error + (failure_reason ? " | " + failure_reason | "");
      endif
      if (length(failure_reason) > 260)
        failure_reason = failure_reason[1..260] + "...";
      endif
    endif
    const entry = ['query -> task['query], 'result -> result, 'status -> status, 'failure_reason -> failure_reason, 'finished_at -> time(), 'requester -> task_player, 'context -> agent.context];
    this.history = {@this.history, entry};
    if (status == 'complete)
      content = typeof(result) == TYPE_STR ? result | toliteral(result);
      this:_announce($format.block:mk($format.title:mk("Task Completed for " + task_player.name), content));
    else
      brief = failure_reason ? failure_reason | typeof(result) == TYPE_STR ? result | toliteral(result);
      if (length(brief) > 220)
        brief = brief[1..220] + "...";
      endif
      task_player:inform_current($event:mk_info(task_player, ("  \u2192 Task failed: " + brief)));
      this:_announce("Task failed for " + task_player.name + ". Reason: " + brief);
    endif
    this.current_task = "";
    this.agent = #-1;
    this.task_requester = #-1;
    this.agentic_tool_current = "";
    this.agentic_tool_state = "";
    this.agentic_tool_note = "";
    this.agentic_wait_started = 0;
    try
      agent:destroy();
    except (ANY)
    endtry
  endmethod

  method _send_as_requester owner: ARCH_WIZARD
    "Confine requester permissions to model execution so the child can report completion to its parent task.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {agent, prompt, requester} = args;
    set_task_perms(requester);
    return agent:send_message(prompt, false, requester);
  endmethod

  verb status (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show current agent status (agentic-safe).";
    let agent = 0;
    let err = 0;
    let iter = 0;
    let mine = 0;
    let note = 0;
    let state = 0;
    let task_desc = 0;
    let timeout_s = 0;
    let wait_started = 0;
    let waited = 0;
    let lines = {};
    let show_details = `player.wizard ! ANY => 0';
    if (!show_details && this.current_task && this.task_requester == player)
      show_details = 1;
    endif
    if (this.current_task)
      if (show_details)
        task_desc = this.current_task;
        if (length(task_desc) > 60)
          task_desc = task_desc[1..60] + "...";
        endif
        lines = {@lines, "Task: \"" + task_desc + "\""};
        if (valid(this.agent))
          agent = this.agent;
          iter = `agent.current_iteration ! ANY => `agent.iteration ! ANY => 0'';
          lines = {@lines, "Status: running  |  Turn: " + tostr(iter)};
          if (this.agentic_tool_current)
            lines = {@lines, "Tool: running " + this.agentic_tool_current};
          elseif (this.agentic_tool_last)
            state = this.agentic_tool_state ? " (" + this.agentic_tool_state + ")" | "";
            lines = {@lines, "Last tool: " + this.agentic_tool_last + state};
          endif
          if (this.agentic_tool_state == "awaiting_model")
            waited = 0;
            wait_started = `this.agentic_wait_started ! ANY => 0';
            if (typeof(wait_started) == TYPE_INT && wait_started > 0)
              waited = time() - wait_started;
              waited < 0 && (waited = 0);
            endif
            timeout_s = `this.agentic_task_timeout_seconds ! ANY => 180';
            lines = {@lines, "Model wait: " + tostr(waited) + "s / " + tostr(timeout_s) + "s timeout"};
          endif
          if (this.agentic_tool_note)
            note = this.agentic_tool_note;
            if (length(note) > 140)
              note = note[1..140] + "...";
            endif
            lines = {@lines, "Tool note: " + note};
          endif
          if (this.agentic_last_error)
            err = this.agentic_last_error;
            if (length(err) > 180)
              err = err[1..180] + "...";
            endif
            lines = {@lines, "Last error: " + err};
          endif
        endif
      else
        lines = {@lines, "A task is currently running for another requester."};
      endif
    else
      lines = {@lines, "No task running."};
      if (this.agentic_last_error)
        err = this.agentic_last_error;
        if (length(err) > 180)
          err = err[1..180] + "...";
        endif
        lines = {@lines, "Last error: " + err};
      endif
    endif
    const active_count = length(this.active_tasks);
    if (active_count > 0)
      lines = {@lines, "Active workers: " + tostr(active_count)};
    endif
    const queue_len = length(this.task_queue);
    if (queue_len > 0)
      if (`player.wizard ! ANY => 0')
        lines = {@lines, "Queue: " + tostr(queue_len) + " pending"};
      else
        mine = 0;
        for task in (this.task_queue)
          if (`task['player] ! ANY => #-1' == player)
            mine = mine + 1;
          endif
        endfor
        lines = {@lines, "Queue: " + tostr(mine) + " yours pending"};
      endif
    endif
    if (this.loop_task > 0)
      lines = {@lines, "Event loop: running"};
    else
      lines = {@lines, "Event loop: stopped"};
    endif
    player:inform_current($event:mk_info(player, lines:join("\n")));
  endverb

  verb "@agentic-selftest selftest" (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Run deterministic agentic integration checks (no LLM calls).";
    let exec_out = 0;
    let first = 0;
    let registered = 0;
    let sz = 0;
    let t = 0;
    let lines = {"Agentic self-test: " + this:name()};
    let passed = 0;
    let failed = 0;
    let check = `valid($agentic) ! ANY => 0' && `valid($agentic.tool) ! ANY => 0' && `valid($agentic.loop) ! ANY => 0' && `valid($agentic.agent) ! ANY => 0' && `valid($agentic.event_queue) ! ANY => 0' && `valid($agentic.runner) ! ANY => 0' && `valid($agentic.room_observer) ! ANY => 0' && `valid($agentic.coding_room) ! ANY => 0';
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] Namespace and component references resolve."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] Namespace/component references missing."};
    endif
    const schema = ["type" -> "object", "properties" -> ["object" -> ["type" -> "string"], "property" -> ["type" -> "string"]], "required" -> {"object", "property"}];
    const test_tool = `$agentic.tool:mk("selftest_read_property", "Read a property", schema, this, "read_property") ! ANY => 0';
    const tool_schema = typeof(test_tool) == TYPE_FLYWEIGHT ? `test_tool:to_schema() ! ANY => 0' | 0;
    check = typeof(test_tool) == TYPE_FLYWEIGHT && typeof(tool_schema) == TYPE_MAP && `tool_schema["function"]["name"] ! ANY => ""' == "selftest_read_property";
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] $agentic.tool mk/to_schema behavior."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] $agentic.tool mk/to_schema behavior."};
    endif
    const verbs_out = `this:_tool_list_verbs(["object" -> "$agentic.agent"], player) ! ANY => 0';
    check = typeof(verbs_out) == TYPE_STR && index(verbs_out, "send_message") > 0;
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] _tool_list_verbs can inspect $agentic.agent."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] _tool_list_verbs could not inspect $agentic.agent."};
    endif
    const prop_out = `this:_tool_read_property(["object" -> "$agentic.agent", "property" -> "max_iterations"], player) ! ANY => 0';
    check = typeof(prop_out) == TYPE_STR && length(prop_out) > 0;
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] _tool_read_property returns max_iterations."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] _tool_read_property failed for max_iterations."};
    endif
    const a = `$agentic.agent:create(true) ! ANY => #-1';
    check = valid(a);
    if (check)
      a.owner = this.owner;
      a.token_owner = player;
      t = `$agentic.tool:mk("read_property", "Read property", schema, this, "read_property") ! ANY => 0';
      `a:add_tool("read_property", t) ! ANY';
      registered = `maphaskey(a.tools, "read_property") ! ANY => 0';
      exec_out = `t:execute(["object" -> "$agentic.agent", "property" -> "max_iterations"], player) ! ANY => 0';
      check = registered && typeof(exec_out) == TYPE_STR && length(exec_out) > 0;
      `a:destroy() ! ANY';
    endif
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] Agent + tool registration and direct execute path."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] Agent + tool registration/execute path."};
    endif
    const q = `$agentic.event_queue:create(true) ! ANY => #-1';
    check = valid(q);
    if (check)
      `q:clear() ! ANY';
      `q:push("x") ! ANY';
      `q:push("y") ! ANY';
      first = `q:pop() ! ANY => 0';
      sz = `q:size() ! ANY => -1';
      check = first == "x" && sz == 1;
      `q:destroy() ! ANY';
    endif
    if (check)
      passed = passed + 1;
      lines = {@lines, "[PASS] Event queue FIFO behavior."};
    else
      failed = failed + 1;
      lines = {@lines, "[FAIL] Event queue FIFO behavior."};
    endif
    const summary = "Summary: " + tostr(passed) + " passed, " + tostr(failed) + " failed.";
    lines = {@lines, summary};
    player:inform_current($event:mk_info(player, lines:join("\n")));
    return ["passed" -> passed, "failed" -> failed, "lines" -> lines];
  endverb

  method _ensure_room_client owner: ARCH_WIZARD
    "Ensure this room has a dedicated client object (not global $llm_client).";
    caller == this || caller == this.owner || caller_perms().wizard || raise(E_PERM);
    if (valid(this.llm_client) && this.llm_client != $llm_client)
      return this.llm_client;
    endif
    const c = $llm_client:create(true);
    c.name = "LLM client for " + this:name();
    this.llm_client = c;
    return c;
  endmethod

  method client_config_status owner: ARCH_WIZARD
    "Return a map describing the room's effective client config.";
    caller == this || caller == this.owner || caller_perms().wizard || raise(E_PERM);
    const client = valid(this.llm_client) ? this.llm_client | $llm_client;
    const endpoint = `client.api_endpoint ! ANY => ""';
    const model = `client.model ! ANY => ""';
    const key = `client.api_key ! ANY => ""';
    const key_set = typeof(key) == TYPE_STR && length(key) > 0;
    let key_preview = "(not set)";
    if (key_set)
      if (length(key) > 10)
        key_preview = key[1..6] + "..." + key[length(key) - 3..length(key)];
      else
        key_preview = "(set)";
      endif
    endif
    return ["client" -> client, "room_client" -> valid(this.llm_client) ? this.llm_client | #-1, "using_global_default" -> !(valid(this.llm_client) && this.llm_client != $llm_client), "endpoint" -> endpoint, "model" -> model, "api_key_set" -> key_set, "api_key_preview" -> key_preview];
  endmethod

  verb "@model model" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Model and provider config for this room.";
    "Usage:";
    "  @model                                   (show status)";
    "  @model <provider/model>                  (set model)";
    "  @model model <provider/model>            (set model)";
    "  @model openrouter [model]                (set OpenRouter endpoint + model)";
    "  @model endpoint <url>                    (set endpoint)";
    "  @model key <api-key>                     (set API key)";
    "  @model list [filters]                    (list models)";
    "  @model global                            (use global $llm_client)";
    "List filters:";
    "  provider <name> | find <text> | limit <n> | offset <n> | all | raw";
    "Examples:";
    "  @model list provider openai limit 20";
    "  @model list find gpt-5";
    "  @model list openai gpt-5 limit 30";
    let client = 0;
    let filtered = 0;
    let finish = 0;
    let hay = 0;
    let hdr = 0;
    let i = 0;
    let id = 0;
    let keep = 0;
    let limit = 0;
    let lines = 0;
    let low = 0;
    let model = 0;
    let models = 0;
    let msg = 0;
    let name = 0;
    let offset = 0;
    let owner = 0;
    let parsed = 0;
    let part = 0;
    let pfx = 0;
    let provider = 0;
    let query = 0;
    let raw_limit = 0;
    let raw_mode = 0;
    let rows = 0;
    let s = 0;
    let show_all = 0;
    let shown = 0;
    let shown_rows = 0;
    let start = 0;
    let tok = 0;
    let tokens = 0;
    let total = 0;
    player == this.owner || player.wizard || return player:inform_current($event:mk_error(player, "You can't configure this room."));
    const raw = argstr:trim();
    if (!raw)
      s = this:client_config_status();
      lines = {"Model config:", "  model: " + tostr(s["model"]), "  endpoint: " + tostr(s["endpoint"]), "  using_global_default: " + toliteral(s["using_global_default"]), "", "Usage:", "  @model <provider/model>", "  @model model <provider/model>", "  @model openrouter [provider/model]", "  @model endpoint <url>", "  @model key <api-key>", "  @model list [provider <name>] [find <text>] [limit <n>] [offset <n>] [all] [raw]", "  @model global"};
      return player:inform_current($event:mk_info(player, lines:join("\n")));
    endif
    const space = " " in raw;
    const cmd = space ? raw[1..space - 1]:lowercase() | raw:lowercase();
    const rest = space ? raw[space + 1..$]:trim() | "";
    if (cmd == "global" || cmd == "reset")
      this.llm_client = #-1;
      return player:inform_current($event:mk_info(player, "This room now uses global $llm_client."));
    endif
    if (cmd == "endpoint")
      if (!rest)
        return player:inform_current($event:mk_error(player, "Usage: @model endpoint <url>"));
      endif
      client = this:_ensure_room_client();
      client.api_endpoint = rest;
      this.llm_client = client;
      return player:inform_current($event:mk_info(player, "Room endpoint set to " + rest + "."));
    endif
    if (cmd == "key")
      if (!rest)
        return player:inform_current($event:mk_error(player, "Usage: @model key <api-key>"));
      endif
      client = this:_ensure_room_client();
      try
        client:set_api_key(rest);
      except (ANY)
        client.api_key = rest;
      endtry
      this.llm_client = client;
      return player:inform_current($event:mk_info(player, "Room API key updated."));
    endif
    if (cmd == "list")
      client = valid(this.llm_client) ? this.llm_client | $llm_client;
      provider = "";
      query = "";
      limit = 80;
      offset = 0;
      show_all = 0;
      raw_mode = 0;
      if (rest)
        tokens = rest:split(" ");
        i = 1;
        while (i <= length(tokens))
          tok = tokens[i]:trim();
          low = tok:lowercase();
          if (low == "raw")
            raw_mode = 1;
          elseif (low == "all")
            show_all = 1;
          elseif (low == "provider" && i < length(tokens))
            provider = tokens[i + 1]:trim():lowercase();
            i = i + 1;
          elseif (low == "find" && i < length(tokens))
            query = "";
            for j in [i + 1..length(tokens)]
              part = tokens[j]:trim();
              if (part)
                query = query ? query + " " + part | part;
              endif
            endfor
            query = query:lowercase();
            i = length(tokens);
          elseif (low == "limit" && i < length(tokens))
            parsed = tonum(tokens[i + 1]);
            parsed > 0 && (limit = parsed);
            i = i + 1;
          elseif (low == "offset" && i < length(tokens))
            parsed = tonum(tokens[i + 1]);
            parsed >= 0 && (offset = parsed);
            i = i + 1;
          else
            if (!provider)
              provider = low;
            else
              query = query ? query + " " + tok | tok;
            endif
          endif
          i = i + 1;
        endwhile
      endif
      query = query:trim():lowercase();
      if (raw_mode)
        try
          raw_limit = show_all ? 0 | limit;
          models = client:list_models("raw", raw_limit);
        except e (ANY)
          msg = "Model list failed: " + tostr(e[1]) + " - " + tostr(e[2]);
          return player:inform_current($event:mk_error(player, msg));
        endtry
        return player:inform_current($event:mk_info(player, toliteral(models)));
      endif
      try
        models = client:list_models(0);
      except e (ANY)
        msg = "Model list failed: " + tostr(e[1]) + " - " + tostr(e[2]);
        return player:inform_current($event:mk_error(player, msg));
      endtry
      if (!(typeof(models) == TYPE_MAP && maphaskey(models, "models") && typeof(models["models"]) == TYPE_LIST))
        return player:inform_current($event:mk_info(player, "Model response:\n" + toliteral(models)));
      endif
      rows = models["models"];
      filtered = {};
      for entry in (rows)
        if (typeof(entry) == TYPE_MAP)
          id = maphaskey(entry, "id") ? tostr(entry["id"]) | "";
          if (id)
            owner = maphaskey(entry, "owned_by") ? tostr(entry["owned_by"]) | "";
            name = maphaskey(entry, "name") ? tostr(entry["name"]) | "";
            keep = 1;
            if (provider)
              pfx = provider + "/";
              if (!(owner:lowercase() == provider || index(id:lowercase(), pfx) == 1))
                keep = 0;
              endif
            endif
            if (keep && query)
              hay = (id + " " + owner + " " + name):lowercase();
              if (!(index(hay, query) > 0))
                keep = 0;
              endif
            endif
            if (keep)
              filtered = {@filtered, entry};
            endif
          endif
        endif
      endfor
      total = length(filtered);
      show_all && (limit = total);
      limit <= 0 && (limit = 80);
      offset < 0 && (offset = 0);
      start = offset + 1;
      if (start > total)
        shown_rows = {};
      else
        finish = start + limit - 1;
        finish > total && (finish = total);
        shown_rows = filtered[start..finish];
      endif
      shown = length(shown_rows);
      hdr = "Models (showing " + tostr(shown) + " of " + tostr(total) + ")";
      provider && (hdr = hdr + " [provider=" + provider + "]");
      query && (hdr = hdr + " [find=" + query + "]");
      offset > 0 && (hdr = hdr + " [offset=" + tostr(offset) + "]");
      lines = {hdr + ":"};
      for i in [1..shown]
        entry = shown_rows[i];
        id = tostr(entry["id"]);
        owner = maphaskey(entry, "owned_by") ? " [" + tostr(entry["owned_by"]) + "]" | "";
        lines = {@lines, "  " + tostr(offset + i) + ". " + id + owner};
      endfor
      if (!shown)
        lines = {@lines, "  (no matches)"};
      endif
      if (maphaskey(models, "endpoint"))
        lines = {@lines, "", "Endpoint: " + tostr(models["endpoint"])};
      endif
      if (total > shown + offset)
        lines = {@lines, "", "Tip: use @model list ... offset " + tostr(offset + shown) + " for next page."};
      endif
      return player:inform_current($event:mk_info(player, lines:join("\n")));
    endif
    if (cmd == "openrouter" || cmd == "or")
      model = rest != "" ? rest | "openrouter/auto";
      client = this:_ensure_room_client();
      client.api_endpoint = "https://openrouter.ai/api/v1/chat/completions";
      client.model = model;
      this.llm_client = client;
      msg = "Room client set to OpenRouter with model " + model + ". Use @model key <OPENROUTER_API_KEY> if needed.";
      return player:inform_current($event:mk_info(player, msg));
    endif
    if (cmd == "model")
      if (!rest)
        return player:inform_current($event:mk_error(player, "Usage: @model model <provider/model>"));
      endif
      client = this:_ensure_room_client();
      client.model = rest;
      this.llm_client = client;
      return player:inform_current($event:mk_info(player, "Room model set to " + rest + "."));
    endif
    client = this:_ensure_room_client();
    client.model = raw;
    this.llm_client = client;
    player:inform_current($event:mk_info(player, "Room model set to " + raw + "."));
  endverb

  method on_tool_call owner: ARCH_WIZARD
    "Callback from $agentic.agent: tool execution started.";
    valid(this.agent) && caller == this.agent || return;
    const {tool_name, ?tool_args = ""} = args;
    this.agentic_tool_current = tool_name;
    this.agentic_tool_last = tool_name;
    this.agentic_tool_state = "running";
    const summary = this:_tool_call_summary(tool_args);
    this.agentic_tool_note = summary;
    const mode = `this.agentic_progress_mode ! ANY => "normal"':lowercase();
    if (mode == "quiet")
      return;
    endif
    const iter = `this.agent.current_iteration ! ANY => 0';
    const prefix = ("[turn " + tostr(iter) + "]");
    const label = "tool" + ": " + tool_name;
    let msg = prefix + " " + label;
    if (summary)
      msg = msg + " " + ("(" + summary + ")");
    endif
    this:_tell_requester(msg);
  endmethod

  method on_tool_error owner: ARCH_WIZARD
    "Callback from $agentic.agent: tool execution failed.";
    valid(this.agent) && caller == this.agent || return;
    const {tool_name, ?tool_args = "", ?error_msg = "ERROR"} = args;
    this.agentic_tool_current = "";
    this.agentic_tool_last = tool_name;
    this.agentic_tool_state = "error";
    const summary = this:_tool_call_summary(tool_args);
    let preview = typeof(error_msg) == TYPE_STR ? error_msg | toliteral(error_msg);
    const nl = "\n" in preview;
    nl && (preview = preview[1..nl - 1]);
    if (length(preview) > 160)
      preview = preview[1..160] + "...";
    endif
    this.agentic_tool_note = preview;
    this.agentic_last_error = tool_name + ": " + preview;
    let msg = "error" + " " + tool_name;
    if (summary)
      msg = msg + " " + ("(" + summary + ")");
    endif
    msg = msg + " - " + preview;
    this:_tell_requester(msg);
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Callback from $agentic.agent: tool execution completed.";
    valid(this.agent) && caller == this.agent || return;
    const {tool_name, ?tool_args = "", ?content_out = ""} = args;
    this.agentic_tool_current = "";
    this.agentic_tool_last = tool_name;
    this.agentic_tool_state = "ok";
    let preview = typeof(content_out) == TYPE_STR ? content_out | toliteral(content_out);
    const nl = "\n" in preview;
    nl && (preview = preview[1..nl - 1]);
    if (length(preview) > 120)
      preview = preview[1..120] + "...";
    endif
    this.agentic_tool_note = preview;
    const mode = `this.agentic_progress_mode ! ANY => "normal"':lowercase();
    if (mode != "verbose")
      return;
    endif
    const summary = this:_tool_call_summary(tool_args);
    let msg = "ok" + " " + tool_name;
    if (summary)
      msg = msg + " " + ("(" + summary + ")");
    endif
    this:_tell_requester(msg);
  endmethod

  method on_compaction_start owner: ARCH_WIZARD
    "Callback from $agentic.agent when context compaction starts.";
    valid(this.agent) && caller == this.agent || return;
    const mode = `this.agentic_progress_mode ! ANY => "normal"':lowercase();
    if (mode == "quiet")
      return;
    endif
    this:_tell_requester("compacting context...");
  endmethod

  method on_compaction_end owner: ARCH_WIZARD
    "Callback from $agentic.agent when context compaction ends.";
    valid(this.agent) && caller == this.agent || return;
    const mode = `this.agentic_progress_mode ! ANY => "normal"':lowercase();
    if (mode == "quiet")
      return;
    endif
    this:_tell_requester("context compaction complete.");
  endmethod

  verb history (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show completed task history with failure reason previews.";
    let entry = 0;
    let i = 0;
    let line = 0;
    let query = 0;
    let query_preview = 0;
    let reason = 0;
    let status_str = 0;
    if (length(this.history) == 0)
      return player:inform_current($event:mk_info(player, "No task history yet."));
    endif
    const show_all = `player.wizard ! ANY => false';
    let lines = {"Task history (most recent first):"};
    const hist_len = length(this.history);
    let shown = 0;
    for offset in [0..hist_len - 1]
      if (shown >= 10)
        break;
      endif
      i = hist_len - offset;
      entry = this.history[i];
      if (!show_all && `entry['requester] ! ANY => #-1' != player)
        continue;
      endif
      status_str = tostr(entry['status]);
      query = `entry['query] ! ANY => ""';
      query_preview = query[1..min(60, length(query))];
      if (length(query) > 60)
        query_preview = query_preview + "...";
      endif
      line = "  [" + status_str + "] \"" + query_preview + "\"";
      if (status_str == "failed")
        reason = `entry['failure_reason] ! ANY => ""';
        if (!reason)
          reason = `entry['result] ! ANY => ""';
        endif
        if (typeof(reason) != TYPE_STR)
          reason = toliteral(reason);
        endif
        if (length(reason) > 180)
          reason = reason[1..180] + "...";
        endif
        reason && (line = line + "  -- " + reason);
      endif
      lines = {@lines, line};
      shown = shown + 1;
    endfor
    if (shown == 0)
      lines = {@lines, "  (No history entries for you.)"};
    endif
    player:inform_current($event:mk_info(player, $format.block:mk(@lines)));
  endverb

  verb description (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Formatted description with command and context status.";
    let content = 0;
    let ctx_len = 0;
    let iter = 0;
    let maybe_ctx = 0;
    let mode = 0;
    let recent = 0;
    let role = 0;
    let tail = 0;
    let tokens = 0;
    const intro = "A virtual workspace where you can interact with an AI agent.";
    const cmd_header = $format.title:mk("Commands", 4);
    const cmds = $format.list:mk({"do <task>" + " - Queue a task for the agent", "resume [notes]" + " - Resume your last failed task from saved context", "status" + " - Check current activity", "halt" + " - Interrupt current task", "queue" + " - Show pending tasks", "history" + " - Show completed tasks", "tool" + " - Show available tools (aliases: tools, @tools)", "context" + " - Context details / compact / reset", "progress" + " - Live tool-update verbosity"}, false);
    const ctx_header = $format.title:mk("Context", 4);
    let ctx_items = {};
    let ctx = {};
    let ctx_source = "none";
    if (valid(this.agent))
      ctx = `this.agent.context ! ANY => {}';
      ctx_source = "active task";
    elseif (length(this.history) > 0)
      recent = this.history[length(this.history)];
      maybe_ctx = `recent['context] ! ANY => {}';
      if (typeof(maybe_ctx) == TYPE_LIST)
        ctx = maybe_ctx;
        ctx_source = "last completed task";
      endif
    endif
    if (typeof(ctx) == TYPE_LIST)
      ctx_len = length(ctx);
      ctx_items = {@ctx_items, "Source: " + ctx_source};
      ctx_items = {@ctx_items, "Messages: " + tostr(ctx_len)};
      mode = `this.agentic_progress_mode ! ANY => "normal"';
      ctx_items = {@ctx_items, "Progress mode: " + mode};
      if (valid(this.agent))
        iter = `this.agent.current_iteration ! ANY => 0';
        tokens = `this.agent.total_tokens_used ! ANY => 0';
        ctx_items = {@ctx_items, "Turn: " + tostr(iter)};
        ctx_items = {@ctx_items, "Tokens used: " + tostr(tokens)};
      endif
      if (ctx_len > 0)
        tail = ctx[ctx_len];
        if (typeof(tail) == TYPE_MAP)
          role = maphaskey(tail, "role") ? tostr(tail["role"]) | "?";
          content = maphaskey(tail, "content") ? typeof(tail["content"]) == TYPE_STR ? tail["content"] | toliteral(tail["content"]) | "";
          content = strsub(content, "\n", " ");
          if (length(content) > 140)
            content = content[1..140] + "...";
          endif
          ctx_items = {@ctx_items, "Tail: " + role + " - " + content};
        endif
      endif
    else
      ctx_items = {@ctx_items, "No context available yet."};
    endif
    const ctx_list = $format.list:mk(ctx_items, false);
    return {intro, cmd_header, cmds, ctx_header, ctx_list};
  endverb

  verb "context ctx" (any any any) owner: ARCH_WIZARD flags: "xd"
    "Context management command. Usage: context, context compact, context reset, context tail";
    let content = 0;
    let ctx = 0;
    let ctx_len = 0;
    let emsg = 0;
    let lines = 0;
    let msg = 0;
    let n = 0;
    let recent = 0;
    let role = 0;
    let start = 0;
    let tail = 0;
    const cmd = argstr:trim():lowercase();
    if (!cmd || cmd == "status")
      lines = {"Context status:"};
      if (valid(this.agent))
        ctx = `this.agent.context ! ANY => {}';
        ctx_len = typeof(ctx) == TYPE_LIST ? length(ctx) | 0;
        lines = {@lines, "  Source: active task"};
        lines = {@lines, "  Messages: " + tostr(ctx_len)};
        lines = {@lines, "  Turn: " + tostr(`this.agent.current_iteration ! ANY => 0')};
        lines = {@lines, "  Tokens used: " + tostr(`this.agent.total_tokens_used ! ANY => 0')};
        if (ctx_len > 0)
          tail = ctx[ctx_len];
          if (typeof(tail) == TYPE_MAP)
            role = maphaskey(tail, "role") ? tostr(tail["role"]) | "?";
            content = maphaskey(tail, "content") ? typeof(tail["content"]) == TYPE_STR ? tail["content"] | toliteral(tail["content"]) | "";
            content = strsub(content, "\n", " ");
            if (length(content) > 140)
              content = content[1..140] + "...";
            endif
            lines = {@lines, "  Tail: " + role + " - " + content};
          endif
        endif
      elseif (length(this.history) > 0)
        recent = this.history[length(this.history)];
        ctx = `recent['context] ! ANY => {}';
        ctx_len = typeof(ctx) == TYPE_LIST ? length(ctx) | 0;
        lines = {@lines, "  Source: last completed task"};
        lines = {@lines, "  Messages: " + tostr(ctx_len)};
      else
        lines = {@lines, "  No context available yet."};
      endif
      lines = {@lines, "", "Commands: context compact | context reset | context tail"};
      return player:inform_current($event:mk_info(player, lines:join("\n")));
    endif
    const can_manage = `player.wizard ! ANY => 0' || player == this.owner || player == this.task_requester;
    if (cmd == "compact")
      !can_manage && return player:inform_current($event:mk_error(player, "Only the task requester/owner can compact context."));
      return this:compact();
    elseif (cmd == "reset")
      !can_manage && return player:inform_current($event:mk_error(player, "Only the task requester/owner can reset context."));
      if (!valid(this.agent))
        return player:inform_current($event:mk_info(player, "No agent currently running."));
      endif
      this:_tell_requester("context reset requested...");
      try
        this.agent:reset_context();
        return player:inform_current($event:mk_info(player, "Agent context reset."));
      except e (ANY)
        emsg = "Context reset failed: " + tostr(e[1]) + " - " + tostr(e[2]);
        this.agentic_last_error = emsg;
        return player:inform_current($event:mk_error(player, emsg));
      endtry
    elseif (cmd == "tail")
      if (valid(this.agent))
        ctx = `this.agent.context ! ANY => {}';
      elseif (length(this.history) > 0)
        recent = this.history[length(this.history)];
        ctx = `recent['context] ! ANY => {}';
      else
        ctx = {};
      endif
      typeof(ctx) != TYPE_LIST && (ctx = {});
      n = length(ctx);
      if (n == 0)
        return player:inform_current($event:mk_info(player, "Context is empty."));
      endif
      start = n > 3 ? n - 2 | 1;
      lines = {"Context tail:"};
      for i in [start..n]
        msg = ctx[i];
        if (typeof(msg) == TYPE_MAP)
          role = maphaskey(msg, "role") ? tostr(msg["role"]) | "?";
          content = maphaskey(msg, "content") ? typeof(msg["content"]) == TYPE_STR ? msg["content"] | toliteral(msg["content"]) | "";
          content = strsub(content, "\n", " ");
          if (length(content) > 180)
            content = content[1..180] + "...";
          endif
          lines = {@lines, "  " + role + ": " + content};
        endif
      endfor
      return player:inform_current($event:mk_info(player, lines:join("\n")));
    else
      return player:inform_current($event:mk_error(player, "Usage: context | context compact | context reset | context tail"));
    endif
  endverb

  verb compact (none none none) owner: ARCH_WIZARD flags: "xd"
    "Compact current agent context with explicit progress messages.";
    let after = 0;
    let emsg = 0;
    let msg = 0;
    const agent = this.agent;
    if (!valid(agent))
      return player:inform_current($event:mk_info(player, "No agent currently running."));
    endif
    const can_manage = `player.wizard ! ANY => 0' || player == this.owner || player == this.task_requester;
    if (!can_manage)
      return player:inform_current($event:mk_error(player, "Only the task requester/owner can compact context."));
    endif
    const before = `length(agent.context) ! ANY => 0';
    this:_tell_requester("context compaction starting...");
    try
      agent:compact_context();
      after = `length(agent.context) ! ANY => before';
      msg = "Context compacted: " + tostr(before) + " -> " + tostr(after) + " messages.";
      player:inform_current($event:mk_info(player, msg));
    except e (ANY)
      emsg = "Context compaction failed: " + tostr(e[1]) + " - " + tostr(e[2]);
      this.agentic_last_error = emsg;
      player:inform_current($event:mk_error(player, emsg));
    endtry
  endverb

  verb "progress @progress" (any any any) owner: ARCH_WIZARD flags: "xd"
    "Set or show live progress verbosity. Usage: progress [quiet|normal|verbose]";
    let lines = 0;
    const mode = `this.agentic_progress_mode ! ANY => "normal"';
    const arg = argstr:trim():lowercase();
    if (!arg)
      lines = {"Progress mode: " + mode, "", "Modes:", "  quiet   - only major failures/completions", "  normal  - tool start + errors", "  verbose - tool start + ok + args + compaction notices"};
      return player:inform_current($event:mk_info(player, lines:join("\n")));
    endif
    player.wizard || player == this.owner || return player:inform_current($event:mk_error(player, "Only room owner/wizard can change progress mode."));
    if (!(arg == "quiet" || arg == "normal" || arg == "verbose"))
      return player:inform_current($event:mk_error(player, "Usage: progress [quiet|normal|verbose]"));
    endif
    this.agentic_progress_mode = arg;
    player:inform_current($event:mk_info(player, "Progress mode set to " + arg + "."));
  endverb

  method _tool_call_summary owner: ARCH_WIZARD
    "Build concise target summary for tool-call args.";
    let parsed = 0;
    let value = 0;
    const {tool_args} = args;
    let args_map = [];
    if (typeof(tool_args) == TYPE_MAP)
      args_map = tool_args;
    elseif (typeof(tool_args) == TYPE_STR && length(tool_args) > 0)
      parsed = `parse_json(tool_args) ! ANY => 0';
      if (typeof(parsed) == TYPE_MAP)
        args_map = parsed;
      endif
    endif
    let parts = {};
    const keys = {"object", "property", "verb", "parent", "name", "target", "target_room", "source_room", "destination", "direction", "area", "topic", "reference", "category", "grantee"};
    for key in (keys)
      if (maphaskey(args_map, key))
        value = args_map[key];
        if (typeof(value) != TYPE_STR)
          value = toliteral(value);
        endif
        value = strsub(value, "\n", " ");
        if (length(value) > 64)
          value = value[1..64] + "...";
        endif
        parts = {@parts, key + "=" + value};
      endif
    endfor
    if (parts)
      return parts:join(", ");
    endif
    let fallback = typeof(tool_args) == TYPE_STR ? tool_args | toliteral(tool_args);
    fallback = strsub(fallback, "\n", " ");
    if (length(fallback) > 96)
      fallback = fallback[1..96] + "...";
    endif
    return fallback;
  endmethod

  method _tell_requester owner: ARCH_WIZARD
    "Send an inset utility message to the active requester.";
    const {message} = args;
    const requester = this.task_requester;
    !valid(requester) && return;
    const content = typeof(message) == TYPE_STR ? message | toliteral(message);
    const evt = $event:mk_info(requester, content):with_audience('utility):with_presentation_hint('inset):with_group('llm, this);
    requester:inform_current(evt);
  endmethod

  verb "tools tool @tools agenttools listtools" (none none none) owner: ARCH_WIZARD flags: "xd"
    "List currently available agent tools for this room.";
    let desc = 0;
    let inserted = 0;
    let name = 0;
    let target = 0;
    let target_obj = 0;
    let target_verb = 0;
    const all = this:_get_agentic_tools();
    let seen = [];
    let rows = {};
    for tool in (all)
      if (typeof(tool) == TYPE_FLYWEIGHT)
        name = `tool.name ! ANY => ""';
        desc = `tool.description ! ANY => ""';
        target_obj = `tool.target_obj ! ANY => #-1';
        target_verb = `tool.target_verb ! ANY => ""';
      elseif (typeof(tool) == TYPE_MAP)
        name = `tool["name"] ! ANY => ""';
        desc = `tool["description"] ! ANY => ""';
        target_obj = `tool["target_obj"] ! ANY => #-1';
        target_verb = `tool["target_verb"] ! ANY => ""';
      else
        continue;
      endif
      if (typeof(name) != TYPE_STR || !name || maphaskey(seen, name))
        continue;
      endif
      seen[name] = 1;
      typeof(desc) != TYPE_STR && (desc = toliteral(desc));
      desc = strsub(desc, "\n", " ");
      if (length(desc) > 120)
        desc = desc[1..120] + "...";
      endif
      target = "";
      if (typeof(target_obj) == TYPE_OBJ && typeof(target_verb) == TYPE_STR && length(target_verb) > 0)
        target = tostr(target_obj) + ":" + target_verb;
      endif
      rows = {@rows, {name, {name, target, desc}}};
    endfor
    if (!rows)
      return player:inform_current($event:mk_info(player, "No tools are currently registered."):with_audience('utility):with_group('tools));
    endif
    let sorted = {};
    for entry in (rows)
      inserted = 0;
      for i in [1..length(sorted)]
        if (entry[1] < sorted[i][1])
          sorted = {@sorted[1..i - 1], entry, @sorted[i..$]};
          inserted = 1;
          break;
        endif
      endfor
      !inserted && (sorted = {@sorted, entry});
    endfor
    let table_rows = {};
    for entry in (sorted)
      table_rows = {@table_rows, entry[2]};
    endfor
    const headers = {"Tool", "Target", "Description"};
    const table_obj = $format.table:mk(headers, table_rows);
    const title_obj = $format.title:mk("Available Tools (" + tostr(length(table_rows)) + ")");
    const content = $format.block:mk(title_obj, table_obj);
    player:inform_current($event:mk_info(player, content):with_audience('utility):with_group('tools));
  endverb

  verb "resume continue retry" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Resume the most recent failed task for this requester using saved context.";
    let h = 0;
    let query = 0;
    let requester = 0;
    if (this.current_task)
      return player:inform_current($event:mk_error(player, "A task is already running. Use `status` or `halt` first."));
    endif
    const extra = argstr:trim();
    const show_all = `player.wizard ! ANY => false';
    let entry = 0;
    for i in [length(this.history)..1]
      h = this.history[i];
      requester = `h['requester] ! ANY => #-1';
      if (!(show_all || requester == player))
        continue;
      endif
      if (`h['status] ! ANY => ""' == 'failed || `h['status] ! ANY => ""' == "failed")
        entry = h;
        break;
      endif
    endfor
    if (!entry)
      return player:inform_current($event:mk_error(player, "No failed task with resumable context was found."));
    endif
    const ctx = `entry['context] ! ANY => {}';
    if (typeof(ctx) != TYPE_LIST || length(ctx) == 0)
      return player:inform_current($event:mk_error(player, "That failed task has no saved context to resume from."));
    endif
    const base_query = `entry['query] ! ANY => "previous task"';
    if (extra)
      query = "Continue from your previous attempt on: " + base_query + "\n\nAdditional direction: " + extra;
    else
      query = "Continue from your previous attempt on: " + base_query + ". Use existing context and avoid repeating completed work.";
    endif
    let max_len = `this.max_query_length ! E_PROPNF => 2000';
    if (max_len < 1)
      max_len = 1;
    endif
    if (length(query) > max_len)
      query = query[1..max_len];
    endif
    let max_pending = `this.max_pending_per_player ! E_PROPNF => 3';
    if (max_pending < 1)
      max_pending = 1;
    endif
    let pending_for_player = 0;
    if (this.current_task && this.task_requester == player)
      pending_for_player = pending_for_player + 1;
    endif
    for task in (this.task_queue)
      if (`task['player] ! ANY => #-1' == player)
        pending_for_player = pending_for_player + 1;
      endif
    endfor
    if (pending_for_player >= max_pending)
      return player:inform_current($event:mk_error(player, "You already have " + tostr(pending_for_player) + " task(s) queued/running here. Please wait for one to finish."));
    endif
    this:_ensure_loop();
    const msg = ["type" -> "task", 'query -> query, 'player -> player, 'queued_at -> time(), 'context -> ctx, 'resumed_query -> base_query];
    try
      task_send(this.loop_task, msg);
    except e (ANY)
      this:_start_loop();
      try
        task_send(this.loop_task, msg);
      except e2 (ANY)
        return player:inform_current($event:mk_error(player, "Failed to queue resumed task."));
      endtry
    endtry
    player:inform_current($event:mk_info(player, "Queued resume from failed task: \"" + base_query + "\""));
  endverb
endobject
