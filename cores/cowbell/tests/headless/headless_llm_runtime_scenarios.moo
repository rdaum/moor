object #90031
  name: "LLM Runtime Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_llm_runtime_scenarios";

  method _client owner: ARCH_WIZARD
    "Create a deterministic provider fixture; the caller must recycle it.";
    caller == this && this == #90031 || raise(E_PERM);
    const {replies} = args;
    const client = create($root, #90102, 2);
    add_property(client, "replies", replies, {#90102, ""});
    add_property(client, "requests", {}, {#90102, ""});
    add_property(client, "append_to", #-1, {#90102, ""});
    add_verb(client, {#90102, "rxd", "chat"}, {"this", "none", "this"});
    set_verb_code(client, "chat", {
      "this.requests = {@this.requests, args[1]};",
      "const reply = this.replies[1];",
      "if (length(this.replies) > 1)",
      "  this.replies = listdelete(this.replies, 1);",
      "endif",
      "if (valid(this.append_to))",
      "  this.append_to:add_message(\"user\", \"arrived during compaction\");",
      "  suspend(0);",
      "endif",
      "return reply;"});
    return client;
  endmethod

  method _tool_reply owner: ARCH_WIZARD
    "Build one provider reply from ordered tool names.";
    const {names} = args;
    let calls = {};
    for i in [1..length(names)]
      calls = {@calls, ["id" -> tostr(i), "type" -> "function",
        "function" -> ["name" -> names[i], "arguments" -> []]]};
    endfor
    return ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> calls]]}];
  endmethod

  method _sink owner: ARCH_WIZARD
    "Create an actor-owned effect recorder; the caller must recycle it.";
    caller == this && this == #90031 || raise(E_PERM);
    const {agent} = args;
    const sink = create($root, #90102, 2);
    add_property(sink, "agent", agent, {#90102, ""});
    add_property(sink, "effects", 0, {#90102, ""});
    add_verb(sink, {#90102, "rxd", "mark"}, {"this", "none", "this"});
    set_verb_code(sink, "mark", {"this.effects = this.effects + 1;", "return \"recorded\";"});
    add_verb(sink, {#90102, "rxd", "stop"}, {"this", "none", "this"});
    set_verb_code(sink, "stop", {"this.agent.cancel_requested = true;", "return \"User cancelled.\";"});
    return sink;
  endmethod

  method _as_player owner: RUNTIME_PLAYER
    "Invoke the fixture target with ordinary player authority.";
    caller == this && this == #90031 || raise(E_PERM);
    const {target, name, values} = args;
    return target:(name)(@values);
  endmethod

  method _as_programmer owner: RUNTIME_PROGRAMMER
    "Invoke a fixture with a different ordinary principal for cross-actor checks.";
    caller == this && this == #90031 || raise(E_PERM);
    const {target, name, values} = args;
    return target:(name)(@values);
  endmethod

  method test_iteration_limit owner: ARCH_WIZARD
    "The common agent stop before requesting more than their configured turn limit.";
    const tool_reply = this:_tool_reply({"mark"});
    const final_reply = ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "done"]]}];
    for spec in ({{$llm_agent, $llm_agent_tool}})
      const {prototype, delegate} = spec;
      const agent = create(prototype, #90102, 2);
      const client = this:_client({tool_reply, tool_reply, tool_reply, final_reply});
      const sink = this:_sink(agent);
      try
        agent.client = client;
        agent.max_iterations = 2;
        agent.tools = ["mark" -> delegate:mk("mark", "Record an effect", [], sink, "mark")];
        const result = this:_as_player(agent, "send_message", {"bounded task"});
        $test_utils:assert_eq(length(client.requests), 2, "The provider call count obeys the limit");
        $test_utils:assert_eq(sink.effects, 2, "Only the permitted turns execute tools");
        $test_utils:assert_eq(typeof(result), TYPE_ERR, "Exhausting the turn limit returns a quota error");
        $test_utils:assert_eq(agent.current_iteration, 0, "The stopped agent is no longer active");
      finally
        for temporary in ({sink, client, agent})
          valid(temporary) && recycle(temporary);
        endfor
      endtry
    endfor
    return true;
  endmethod

  method test_exhausted_budget owner: ARCH_WIZARD
    "An exhausted budget prevents the provider request and clears active status.";
    const previous_budget = #90102.llm_token_budget;
    const previous_used = #90102.llm_tokens_used;
    try
      #90102.llm_token_budget = 1;
      #90102.llm_tokens_used = 1;
      for prototype in ({$llm_agent})
        const agent = create(prototype, #90102, 2);
        const client = this:_client({["choices" -> {["message" -> ["role" -> "assistant", "content" -> "done"]]}]});
        try
          agent.client = client;
          const result = this:_as_player(agent, "send_message", {"over budget"});
          $test_utils:assert_eq(length(client.requests), 0, "No provider request spends an exhausted budget");
          $test_utils:assert_true(index(result, "budget exceeded"), "The result identifies the limit");
          $test_utils:assert_eq(agent.current_iteration, 0, "Budget exhaustion clears active status");
        finally
          recycle(client);
          recycle(agent);
        endtry
      endfor
    finally
      #90102.llm_token_budget = previous_budget;
      #90102.llm_tokens_used = previous_used;
    endtry
    return true;
  endmethod

  method test_invalid_response owner: ARCH_WIZARD
    "Malformed provider replies cannot become empty successful assistant messages.";
    for prototype in ({$llm_agent})
      const agent = create(prototype, #90102, 2);
      const client = this:_client({["error" -> "invalid reply"]});
      try
        agent.client = client;
        const result = this:_as_player(agent, "send_message", {"invalid response"});
        $test_utils:assert_true(result:starts_with("Error:"), "The caller receives an explicit failure");
        $test_utils:assert_eq(length(agent:get_context()), 1, "No invalid assistant message enters context");
        $test_utils:assert_eq(agent.current_iteration, 0, "Failure clears active status");
      finally
        recycle(client);
        recycle(agent);
      endtry
    endfor
    return true;
  endmethod

  method test_cancellation_between_tools owner: ARCH_WIZARD
    "A cancelled interaction prevents later mutations in the same provider batch.";
    for spec in ({{$llm_agent, $llm_agent_tool}})
      const {prototype, delegate} = spec;
      const agent = create(prototype, #90102, 2);
      const client = this:_client({this:_tool_reply({"stop", "mark"})});
      const sink = this:_sink(agent);
      try
        agent.client = client;
        agent.tools = [
          "stop" -> delegate:mk("stop", "Cancel", [], sink, "stop"),
          "mark" -> delegate:mk("mark", "Record an effect", [], sink, "mark")];
        const result = this:_as_player(agent, "send_message", {"cancelled batch"});
        $test_utils:assert_eq(sink.effects, 0, "Cancellation precedes the next mutation");
        $test_utils:assert_eq(length(client.requests), 1, "Cancellation prevents another provider call");
        $test_utils:assert_eq(result, "Operation cancelled.", "The caller sees cancellation");
      finally
        for temporary in ({sink, client, agent})
          valid(temporary) && recycle(temporary);
        endfor
      endtry
    endfor
    return true;
  endmethod

  method test_compaction_preserves_tool_exchange owner: ARCH_WIZARD
    "Compaction retains the assistant call for every retained tool response.";
    for prototype in ({$llm_agent})
      const agent = create(prototype, #90102, 2);
      const client = this:_client({["choices" -> {["message" -> ["role" -> "assistant", "content" -> "summary"]]}]});
      try
        agent.client = client;
        agent.min_messages_to_keep = 4;
        agent:set_context({
          ["role" -> "system", "content" -> "instructions"],
          ["role" -> "user", "content" -> "inspect"],
          this:_tool_reply({"mark"})["choices"][1]["message"],
          ["role" -> "tool", "tool_call_id" -> "1", "content" -> "recorded"],
          ["role" -> "user", "content" -> "follow up"],
          ["role" -> "assistant", "content" -> "answer"],
          ["role" -> "user", "content" -> "next"]});
        this:_as_player(agent, "compact_context", {});
        let seen = {};
        for message in (agent:get_context())
          if (maphaskey(message, "tool_calls"))
            for call in (message["tool_calls"])
              seen = {@seen, call["id"]};
            endfor
          endif
          if (message["role"] == "tool")
            $test_utils:assert_true(message["tool_call_id"] in seen,
              "A retained tool response has its preceding assistant call");
          endif
        endfor
      finally
        recycle(client);
        recycle(agent);
      endtry
    endfor
    return true;
  endmethod

  method test_compaction_preserves_concurrent_append owner: ARCH_WIZARD
    "A summary based on an older conversation cannot overwrite a newly appended message.";
    for prototype in ({$llm_agent})
      const agent = create(prototype, #90102, 2);
      const client = this:_client({["choices" -> {["message" -> ["role" -> "assistant", "content" -> "summary"]]}]});
      try
        agent.client = client;
        client.append_to = agent;
        agent.min_messages_to_keep = 2;
        agent:set_context({["role" -> "system", "content" -> "instructions"]});
        for i in [1..6]
          agent:add_message("user", tostr(i));
        endfor
        this:_as_player(agent, "compact_context", {});
        $test_utils:assert_eq(agent:get_context()[$]["content"], "arrived during compaction",
          "The intervening message remains after the stale summary returns");
      finally
        recycle(client);
        recycle(agent);
      endtry
    endfor
    return true;
  endmethod

  method test_task_initialization_authority owner: ARCH_WIZARD
    "Foreign callers cannot create tasks in another agent's project or alter its task status.";
    const agent = create($llm_agent, #90101, 2);
    try
      const project = agent:get_project();
      const task = project:create_task("original");
      for invocation in ({{project, "create_task", {"foreign"}}, {task, "mark_complete", {"foreign"}},
                         {task, "get_status", {}}, {task, "add_finding", {"private", "key", "value"}}})
        let denied = false;
        try
          this:_as_player(invocation[1], invocation[2], invocation[3]);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign task access is denied");
      endfor
      $test_utils:assert_eq(task:get_status()["status"], 'pending, "Denied writes preserve task state");
      $test_utils:assert_eq(length(project:get_task_status()), 1, "Denied creation adds no task");
    finally
      recycle(agent);
    endtry
    return true;
  endmethod

  method test_fact_compaction_rejects_invalid_proposals owner: ARCH_WIZARD
    "A malformed provider fact cannot erase memory, including through the complete model path.";
    const observer = create($llm_room_observer, #90102, 2);
    const memory = $llm_memory:new(observer);
    const kb = memory.facts;
    const agent = create($llm_agent, #90102, 2);
    const client = this:_client({["choices" -> {["message" -> ["role" -> "assistant", "content" -> "[{}]"]]}]});
    try
      observer.memory = memory;
      observer.agent = agent;
      agent.client = client;
      kb:assert({"person", "first fact", 100});
      kb:assert({"person", "second fact", 200});
      const snapshot = kb:tuples();
      let rejected = false;
      try
        this:_as_player(observer, "compact_facts", {});
      except (E_INVARG)
        rejected = true;
      endtry
      $test_utils:assert_true(rejected, "Reject malformed facts before deleting originals");
      $test_utils:assert_eq(kb:tuples(), snapshot, "Invalid output retains every original fact");
    finally
      for temporary in ({client, agent, observer})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method test_project_ownership_and_descendants owner: ARCH_WIZARD
    "Project state is private, isolated for descendants, and follows current subject ownership.";
    const agent = create($llm_agent, #90102, 2);
    try
      const project = agent:get_project();
      const task = project:start_task("private work");
      project:add_todo("private plan");
      const child = create(agent, #90102, 1);
      $test_utils:assert_eq(child:get_project():get_todos(), {}, "Descendant projects start independently");
      $test_utils:assert_eq(child:get_project():get_task_status(), {}, "Descendants retain no parent tasks");
      add_verb(agent, {#90102, "rxd", "read_private_project"}, {"this", "none", "this"});
      set_verb_code(agent, "read_private_project", {"return this.project_state;"});
      let hidden = false;
      try
        this:_as_player(agent, "read_private_project", {});
      except (E_PERM)
        hidden = true;
      endtry
      $test_utils:assert_true(hidden, "Project storage is private even to the subject owner");
      agent.owner = #90101;
      for invocation in ({{project, "get_todos", {}}, {task, "mark_complete", {"done"}},
                         {project, "clear", {}}})
        let denied = false;
        try
          this:_as_player(invocation[1], invocation[2], invocation[3]);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Retained handles do not retain previous owner authority");
      endfor
    finally
      recycle(agent);
    endtry
    return true;
  endmethod

  method test_project_task_tree_release owner: ARCH_WIZARD
    "Removing a task releases descendants and findings and revokes retained handles.";
    const agent = create($llm_agent, #90102, 1);
    const project = agent:get_project();
    const parent = project:start_task("root");
    const child = parent:add_subtask("child");
    const grandchild = child:add_subtask("grandchild");
    child:add_finding("subject", "key", "private finding");
    const survivor = project:create_task("independent");
    $test_utils:assert_eq(parent:get_status()["subtask_count"], 1, "Direct child count is accurate");
    $test_utils:assert_eq(child:get_status()["parent_task_id"], parent:get_status()["task_id"], "Parent identity is retained");
    project:remove_task(parent:get_status()["task_id"]);
    $test_utils:assert_eq(length(project:get_task_status()), 1, "Only independent work remains");
    $test_utils:assert_eq(project:active_task(), #-1, "Removing selected work clears selection");
    for removed in ({parent, child, grandchild})
      let denied = false;
      try
        removed:mark_in_progress();
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Removed task handles are revoked");
      $test_utils:assert_eq(removed.findings, {}, "Removed findings release references");
    endfor
    survivor:mark_complete("done");
    let rejected = false;
    try
      survivor:mark_in_progress();
    except (E_INVARG)
      rejected = true;
    endtry
    $test_utils:assert_true(rejected, "Finished tasks cannot reopen");
    project:clear();
    $test_utils:assert_eq(project:get_task_status(), {}, "Clear releases all remaining tasks");
    return true;
  endmethod

  method test_stored_rule_frontend_tools owner: ARCH_WIZARD
    "Both LLM wearables evaluate stored rules with optional JSON inputs under the captured actor.";
    const actor = #90101;
    const previous = actor.wearing;
    const was_builder = actor.is_builder;
    const target = create($thing, #90100, 2);
    try
      add_property(target, "constant_rule", $rule_engine:parse_expression("TRUE", 'constant_rule), {#90100, "r"});
      target.r = 0;
      add_verb(target, {#90100, "rxd", "all_properties"}, {"this", "none", "this"});
      set_verb_code(target, "all_properties", {"raise(E_ASSERT, \"Do not enumerate through target callbacks\");"});
      add_property(target, "input_rule", $rule_engine:parse_expression("input.Actor == #90101",
        'input_rule, $rule_engine:scope(['Actor -> 'object])), {#90100, "r"});
      add_property(target, "private_rule", target.constant_rule, {#90100, ""});
      add_property(target, "secret", 1, {#90100, ""});
      add_property(target, "query_rule", $rule_engine:parse_expression(
        "property_is(object: " + tostr(target) + ", field: 'secret, value: 1)", 'query_rule), {#90100, "r"});
      for prototype in ({$data_visor, $architects_compass})
        const device = create(prototype, actor, 2);
        const client = this:_client({});
        let session = #-1;
        try
          device:configure();
          const agent = device.agent;
          agent.client = client;
          actor.wearing = {@previous, device};
          actor.is_builder = true;
          session = $llm_wearable_session:open(device, actor, 0);
          agent.tool_callback = #-1;
          const requests = {
            {"evaluate_rule", ["object" -> tostr(target), "property" -> "constant_rule"]},
            {"evaluate_rule", ["object" -> tostr(target), "property" -> "private_rule"]},
            {"evaluate_rule", ["object" -> tostr(target), "property" -> "input_rule",
              "bindings" -> ["Actor" -> "#90101"]]},
            {"test_rule", ["expression" -> "input.Actor == #90101", "bindings" -> ["Actor" -> "#90101"]]},
            {"evaluate_rule", ["object" -> tostr(target), "property" -> "query_rule"]}};
          let calls = {};
          for i in [1..length(requests)]
            calls = {@calls, ["id" -> tostr(i), "type" -> "function",
              "function" -> ["name" -> requests[i][1], "arguments" -> requests[i][2]]]};
          endfor
          client.replies = {["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> calls]]}]};
          this:_as_programmer(agent, "run_turn", {});
          $test_utils:assert_eq(agent.tool_outcomes["1"].status, 'success,
            "A stored constant rule needs no bindings argument: " + agent.tool_outcomes["1"].code);
          $test_utils:assert_true(index(agent.tool_outcomes["1"].value, "Success: true") > 0,
            "The stored condition was evaluated");
          $test_utils:assert_eq(agent.tool_outcomes["2"].code, "E_PERM", "Private rule reads use actor authority");
          $test_utils:assert_eq(agent.tool_outcomes["2"].status, 'failure, "A denied read is a known failure");
          $test_utils:assert_eq(agent.tool_outcomes["3"].status, 'success, "JSON bindings support object references");
          $test_utils:assert_eq(agent.tool_outcomes["4"].status, 'success, "Expression tests share input conversion");
          $test_utils:assert_eq(agent.tool_outcomes["5"].code, "E_PERM", "Predicates cannot read foreign private facts");
          $test_utils:assert_eq(length(agent.pending_tools), 0, "A denied read does not block later inspection");
          const provider = agent.tools["evaluate_rule"].target_obj;
          $test_utils:assert_true(is_anonymous(provider), "Stored rules use the session's shared provider");
          session:close();
          let denied = false;
          try
            this:_as_programmer(provider, "evaluate_rule", {requests[1][2], actor});
          except (E_INVARG)
            denied = true;
          endtry
          $test_utils:assert_true(denied, "Retained providers reject closed interactions");
        finally
          valid(session) && session:close();
          actor.wearing = previous;
          actor.is_builder = was_builder;
          recycle(client);
          recycle(device);
        endtry
      endfor
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method test_wearable_missing_tasks owner: ARCH_WIZARD
    "Wearable task selection follows project removal and agent replacement without stale IDs.";
    for spec in ({{$data_visor, "complete_investigation"}, {$architects_compass, "complete_building"}})
      const {prototype, operation} = spec;
      const device = create(prototype, #90102, 2);
      const agent = create($llm_agent, #90102, 2);
      try
        device.agent = agent;
        $test_utils:assert_true(this:_as_player(device, operation, {}):starts_with("No active"), "No task is selected");
        const project = agent:get_project();
        const task = project:start_task("Pending investigation");
        project:remove_task(task:get_status()["task_id"]);
        $test_utils:assert_true(this:_as_player(device, operation, {}):starts_with("No active"), "Removal clears selection");
        project:start_task("Another investigation");
        device.agent = create($llm_agent, #90102, 1);
        $test_utils:assert_true(this:_as_player(device, operation, {}):starts_with("No active"), "A new agent has no stale selection");
        device.owner = #90101;
        let denied = false;
        try
          this:_as_player(device, operation, {});
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign callers cannot complete device tasks");
      finally
        recycle(agent);
        recycle(device);
      endtry
    endfor
    return true;
  endmethod

  method _job_room owner: ARCH_WIZARD
    "Create an explicit room caller for job lifecycle tests; caller must recycle it.";
    caller == this && this == #90031 || raise(E_PERM);
    const room = create($agent_room, #90100, 2);
    add_verb(room, {#90100, "rxd", "job_call"}, {"this", "none", "this"});
    set_verb_code(room, "job_call", {"const {job, name, values} = args;", "return job:(name)(@values);"});
    return room;
  endmethod

  method _job owner: ARCH_WIZARD
    "Create a job through the same signed request contract as room execution.";
    caller == this && this == #90031 || raise(E_PERM);
    const {room, actor, query} = args;
    let task = ['player -> actor, 'query -> query];
    task["authorization"] = $agent_room:_authorize_task(room, task);
    return $llm_job:new(room, task);
  endmethod

  method test_jobs_isolate_requesters owner: ARCH_WIZARD
    "Anonymous jobs capture separate requesters and cannot cancel or expose each other's state.";
    const room = this:_job_room();
    const first = this:_job(room, #90102, "first query");
    const second = this:_job(room, #90101, "second query");
    try
      $test_utils:assert_true(is_anonymous(first) && is_anonymous(second), "Mutable jobs use anonymous objects");
      let denied = false;
      try
        this:_as_player(second, "cancel", {});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "One requester cannot cancel another's job");
      this:_as_player(first, "cancel", {});
      $test_utils:assert_eq(second.state, 'queued, "Cancelling one job preserves the other");
      const entry = room:job_call(first, "take_history", {});
      $test_utils:assert_eq(entry['requester], #90102, "History retains the captured requester");
      $test_utils:assert_eq(entry['query], "first query", "History retains the captured query");
      $test_utils:assert_eq(room:job_call(first, "take_history", {}), [], "History is recorded only once");
      const presenter = first.presenter;
      room:job_call(first, "release", {});
      $test_utils:assert_false(presenter.active, "Release closes the job's presenter");
      $test_utils:assert_eq(first.presenter, #-1, "Release drops the presenter reference");
      $test_utils:assert_eq(first.host, #-1, "Release drops the room reference");
      $test_utils:assert_eq(first.context, {}, "Release drops conversation state");
    finally
      if (first.host == room)
        room:job_call(first, "release", {});
      endif
      room:job_call(second, "release", {});
      recycle(room);
    endtry
    return true;
  endmethod

  method _run_job_background owner: ARCH_WIZARD
    "Start the room task from a separate activation without an enclosing finally scope.";
    caller == this && this == #90031 || raise(E_PERM);
    const {room, job} = args;
    fork tid (0)
      room:job_call(job, "run", {"delayed effect", 5});
    endfork
    return tid;
  endmethod

  method test_job_cancellation_owns_model_child owner: ARCH_WIZARD
    "Cancelling a running job stops its recorded child before the delayed effect.";
    const room = this:_job_room();
    const job = this:_job(room, #90102, "delayed effect");
    const agent = create($llm_agent, #90102, 1);
    add_property(agent, "effects", 0, {#90102, ""});
    agent:set_context({["role" -> "user", "content" -> "retained"]});
    add_verb(agent, {#90102, "rxd", "send_message"}, {"this", "none", "this"});
    set_verb_code(agent, "send_message", {"suspend(30);", "this.effects = 1;", "return \"late result\";"});
    let parent_task = 0;
    try
      room:job_call(job, "bind_agent", {agent});
      parent_task = this:_run_job_background(room, job);
      const deadline = time() + 3;
      while (job.worker_task == 0)
        time() < deadline || raise(E_ASSERT, "Model child did not start");
        suspend(0.01);
      endwhile
      const worker = job.worker_task;
      this:_as_player(job, "cancel", {});
      while (job.parent_task != 0)
        time() < deadline || raise(E_ASSERT, "Cancelled job did not finish");
        suspend(0.01);
      endwhile
      $test_utils:assert_eq(job.state, 'cancelled, "The terminal state is cancellation");
      $test_utils:assert_eq(agent.effects, 0, "The cancelled child cannot perform its delayed effect");
      $test_utils:assert_false(worker in {task[1] for task in (queued_tasks())}, "No model child remains queued");
      const entry = room:job_call(job, "take_history", {});
      $test_utils:assert_eq(entry['context], agent:get_context(), "Cancellation retains resumable context");
      room:job_call(job, "release", {});
      $test_utils:assert_eq(agent.tool_callback, #-1, "Release breaks the callback cycle");
    finally
      if (job.host == room)
        room:job_call(job, "release", {});
      endif
      if (parent_task > 0)
        `kill_task(parent_task) ! E_INVARG';
      endif
      recycle(room);
    endtry
    return true;
  endmethod

  method test_typed_tool_outcomes owner: ARCH_WIZARD
    "Error-looking success text is data; explicit envelopes and cancelled outcomes drive control flow.";
    const literal = $llm_tool_result:normalize("ERROR: is part of this quoted example");
    $test_utils:assert_true(literal:is_success(), "Do not infer failure from prose");
    const failed = $llm_tool_result:normalize(["ok" -> false, "error" -> ["code" -> "denied"]]);
    $test_utils:assert_false(failed:is_success(), "An explicit negative envelope is a failure");
    const cancelled = $llm_tool_result:mk('cancelled, "Stopped", "user_cancelled");
    $test_utils:assert_eq(parse_json(cancelled:content())["status"], "cancelled", "Cancellation has an explicit wire status");
    $test_utils:assert_eq(literal.status, 'success, "Constructing another outcome preserves the first value");
    return true;
  endmethod

  method test_tool_results_cannot_retain_live_values owner: ARCH_WIZARD
    "Invalid LLM tool results become bounded outcomes without retaining object graphs or losing mutation recovery.";
    const actor = #90102;
    const retained = create($root, actor, 1);
    let large = "x";
    for power in [1..17]
      large = large + large;
    endfor
    for effect in ({'inspection, 'mutation})
      for payload in ({retained, <$root, .reference = retained>, large,
          <$llm_tool_result, .status = 'success, .value = "small", .code = "", .wire = ["object" -> retained]>})
        const agent = create($llm_agent, actor, 2);
        const client = this:_client({this:_tool_reply({"result", "mark"})});
        const sink = this:_sink(agent);
        try
          add_property(sink, "result", payload, {actor, ""});
          add_verb(sink, {actor, "rxd", "result"}, {"this", "none", "this"});
          set_verb_code(sink, "result", {"return this.result;"});
          agent.client = client;
          agent.tools = ["result" -> $llm_agent_tool:mk("result", "Return fixture data", [], sink, "result", effect),
            "mark" -> $llm_agent_tool:mk("mark", "Record later work", [], sink, "mark", 'mutation)];
          const turn = this:_as_player(agent, "run_turn", {});
          const outcome = agent.tool_outcomes["1"];
          $test_utils:assert_eq(outcome.status, effect == 'inspection ? 'failure | 'unknown,
            "Rejected results preserve the handler's effect classification");
          $llm_value:validate_json(outcome.value);
          $test_utils:assert_true(value_bytes(outcome) <= 65536, "A failed result cannot retain an oversized payload");
          $test_utils:assert_eq(length(agent.pending_tools), effect == 'inspection ? 0 | 1,
            "Only a potentially committed effect requires recovery");
          $test_utils:assert_eq(sink.effects, effect == 'inspection ? 1 | 0,
            "Later tools continue after a known read failure and stop after an uncertain mutation");
        finally
          recycle(sink);
          recycle(client);
          recycle(agent);
        endtry
      endfor
    endfor
    return true;
  endmethod

  method test_tool_intent_precedes_committed_mutation owner: ARCH_WIZARD
    "A handler that commits then throws leaves an intent, unknown outcome, and a recovery barrier.";
    const agent = create($llm_agent, #90102, 2);
    const client = this:_client({this:_tool_reply({"mutate", "mark"})});
    const sink = this:_sink(agent);
    try
      add_property(sink, "intent_seen", false, {#90102, ""});
      add_verb(sink, {#90102, "rxd", "mutate"}, {"this", "none", "this"});
      set_verb_code(sink, "mutate", {
        "this.intent_seen = length(this.agent:get_context()) == 4 && this.agent:get_context()[2][\"role\"] == \"assistant\" && this.agent:get_context()[3][\"role\"] == \"tool\";",
        "this.effects = 1;", "suspend(0);", "raise(E_INVARG, \"failure after commit\");"});
      agent.client = client;
      agent.tools = ["mutate" -> $llm_agent_tool:mk("mutate", "Commit then fail", [], sink, "mutate", 'mutation), "mark" -> $llm_agent_tool:mk("mark", "A later effect", [], sink, "mark", 'mutation)];
      const result = this:_as_player(agent, "send_message", {"run mutation"});
      $test_utils:assert_true(sink.intent_seen, "The transcript contains intent before the handler runs");
      $test_utils:assert_eq(sink.effects, 1, "Only the first committed effect remains; later tools stop");
      $test_utils:assert_true(index(result, "unknown"), "The caller receives an uncertain-outcome failure");
      $test_utils:assert_eq(agent.tool_outcomes["1"].status, 'unknown, "The internal outcome is typed");
      $test_utils:assert_eq(length(agent.pending_tools), 1, "Unresolved intent remains available for recovery");
      $test_utils:assert_eq(parse_json(agent:get_context()[3]["content"])["status"], "unknown", "History preserves the unresolved operation");
      let denied = false;
      try
        this:_as_player(agent, "send_message", {"retry"});
      except (E_INVARG)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A fresh request cannot blindly replay unresolved work");
      $test_utils:assert_eq(length(client.requests), 1, "The recovery barrier precedes another provider request");
      $test_utils:assert_false(agent.running, "Failure clears the execution lock");
    finally
      for temporary in ({sink, client, agent})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method test_provider_content_and_call_validation owner: ARCH_WIZARD
    "Malformed later calls reject the complete batch before any tool can mutate state.";
    $test_utils:assert_true($llm_response:test_flyweight(), "Literal provider content survives normalization");
    const agent = create($llm_agent, #90102, 2);
    let reply = this:_tool_reply({"mark"});
    reply["choices"][1]["message"]["tool_calls"] = {@reply["choices"][1]["message"]["tool_calls"], ["id" -> "malformed"]};
    const client = this:_client({reply});
    const sink = this:_sink(agent);
    try
      agent.client = client;
      agent.tools = ["mark" -> $llm_agent_tool:mk("mark", "Record effect", [], sink, "mark")];
      const result = this:_as_player(agent, "send_message", {"invalid batch"});
      $test_utils:assert_true(result:starts_with("Error:"), "Reject a malformed batch");
      $test_utils:assert_eq(sink.effects, 0, "No earlier tool runs from a malformed batch");
      $test_utils:assert_eq(length(agent:get_context()), 1, "Invalid assistant data does not enter the transcript");
    finally
      for temporary in ({sink, client, agent})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method test_room_lifecycle_authority owner: ARCH_WIZARD
    "Foreign callers cannot stop, restart, or clear another owner's agent room.";
    const room = create($agent_room, #90101, 2);
    try
      room.history = {['query -> "retained"]};
      for name in ({"reset"})
        let denied = false;
        try
          this:_as_player(room, name, {});
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, name + " rejects a foreign principal");
        $test_utils:assert_eq(length(room.history), 1, "Denied lifecycle calls preserve history");
        $test_utils:assert_eq(room.loop_task, 0, "Denied lifecycle calls do not start tasks");
      endfor
    finally
      recycle(room);
    endtry
    return true;
  endmethod

  method test_latest_room_history owner: ARCH_WIZARD
    "History selection uses newest-first order and the actual request actor.";
    const room = create($agent_room, #90101, 2);
    try
      room.history = {
        ['requester -> #90102, 'query -> "old", 'status -> 'failed],
        ['requester -> #90102, 'query -> "recent failure", 'status -> 'failed],
        ['requester -> #90102, 'query -> "recent success", 'status -> 'completed],
        ['requester -> #90101, 'query -> "foreign failure", 'status -> 'failed]};
      $test_utils:assert_eq($agent_room:latest_history(room, #90100), [], "A wizard continuation cannot select another actor's conversation");
      const latest = this:_as_player($agent_room, "latest_history", {room, #90102});
      $test_utils:assert_eq(latest['query], "recent success", "Select the newest own entry");
      const failed = this:_as_player($agent_room, "latest_history", {room, #90102, 'failed});
      $test_utils:assert_eq(failed['query], "recent failure", "Filter status before selecting the newest entry");
      let denied = false;
      try
        this:_as_player($agent_room, "latest_history", {room, #90100});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "An explicit actor cannot impersonate a wizard");
      room.history = {};
      $test_utils:assert_eq(this:_as_player($agent_room, "latest_history", {room, #90102}), [], "Empty history has no result");
    finally
      recycle(room);
    endtry
    return true;
  endmethod

  method test_verb_inspection_argspec owner: ARCH_WIZARD
    "Code inspection reports argument specifications independently from verb names.";
    const target = create($thing, #90102, 2);
    try
      add_verb(target, {#90102, "rxd", "inspect_probe alias"}, {"this", "none", "this"});
      set_verb_code(target, "inspect_probe", {"return 1;"});
      const result = $llm_inspection_tools:get_verb_code(
        ["object" -> target, "verb" -> "inspect_probe"], #90102);
      $test_utils:assert_true(index(result, "Argspec: this none this"),
        "Argument specification comes from verb_args");
    finally
      recycle(target);
    endtry
    return true;
  endmethod
  method test_catalog_collision owner: ARCH_WIZARD
    "A conflicting definition cannot replace a registered tool; exact repeats are harmless.";
    const schema = ["type" -> "object", "properties" -> []];
    const first = $llm_agent_tool:mk("probe", "Inspect", schema, this, "probe", 'inspection);
    const second = $llm_agent_tool:mk("probe", "Mutate", schema, this, "different", 'mutation);
    $test_utils:assert_eq(length($llm_tool_catalog:compose({first}, {first})), 1, "Identical registration is idempotent");
    let denied = false;
    try
      $llm_tool_catalog:compose({first}, {second});
    except (E_INVARG)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Conflicting catalogs are rejected");
    const agent = create($llm_agent, #90102, 1);
    agent:add_tool("probe", first);
    denied = false;
    try
      agent:add_tool("probe", second);
    except (E_INVARG)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Agent registration cannot silently replace a tool");
    $test_utils:assert_eq(agent.tools["probe"].target_verb, "probe", "Original descriptor survives rejection");
    return true;
  endmethod
  method test_room_views_filter_private_state owner: ARCH_WIZARD
    "Room snapshots disclose only the authenticated requester's work unless the actor administers the room.";
    const room = create($agent_room, #90101, 2);
    const own = this:_job(room, #90102, "own active request");
    const foreign = this:_job(room, #90100, "private foreign request");
    try
      room.jobs = {own, foreign};
      own.parent_task = task_id();
      foreign.parent_task = task_id();
      const own_queued = this:_job(room, #90102, "own queued request");
      const foreign_queued = this:_job(room, #90100, "private foreign queue");
      room.jobs = {@room.jobs, own_queued, foreign_queued};
      room.history = {['requester -> #90102, 'query -> "own history", 'status -> 'complete, 'context -> {}],
        ['requester -> #90100, 'query -> "private foreign history", 'status -> 'complete, 'context -> {["role" -> "assistant", "content" -> "private foreign answer"]}]};
      const snapshot = this:_as_player($llm_room_view, "snapshot", {room, #90102});
      $test_utils:assert_eq(length(snapshot['jobs]), 1, "Only the requester's active job is visible");
      $test_utils:assert_eq(length(snapshot['queue]), 1, "Only the requester's queue entry is visible");
      $test_utils:assert_eq(length(snapshot['history]), 1, "Only the requester's history is visible");
      $test_utils:assert_false(index(toliteral(snapshot), "private foreign"), "Foreign content does not enter the snapshot");
      const description = this:_as_player(room, "description", {});
      $test_utils:assert_false(index(toliteral(description), "private foreign"), "Public description contains no transcript");
      let denied = false;
      try
        this:_as_player($llm_room_view, "snapshot", {room, #90100});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "An explicit actor cannot impersonate an administrator");
      denied = false;
      try
        this:_as_player($llm_room_view, "render", {room, #90100, 'context});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "The renderer preserves the incoming principal");
      add_verb(room, {#90102, "rxd", "read_history"}, {"this", "none", "this"});
      set_verb_code(room, "read_history", {"return this.history;"});
      denied = false;
      try
        this:_as_player(room, "read_history", {});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "The underlying history property is private");
    finally
      $llm_room_runtime:reset(room, #90100);
      recycle(room);
    endtry
    return true;
  endmethod
  method test_tool_schema_constraints owner: ARCH_WIZARD
    "Shared validation checks nested fields, exact names, enums, and bounds without coercion.";
    const schema = ["type" -> "object", "properties" -> [
      "items" -> ["type" -> "array", "minItems" -> 1, "maxItems" -> 2, "items" -> [
        "type" -> "object", "properties" -> ["name" -> ["type" -> "string", "enum" -> {"Keep"}],
          "count" -> ["type" -> "number", "minimum" -> 1, "maximum" -> 3]],
        "required" -> {"name", "count"}, "additionalProperties" -> false]]],
      "required" -> {"items"}, "additionalProperties" -> false];
    const good = ["items" -> {["name" -> "Keep", "count" -> 2.5]}];
    $test_utils:assert_eq($llm_tool_schema:arguments(generate_json(good), schema), good, "Valid values are preserved");
    for value in ({[], ["items" -> {}], ["items" -> {["name" -> "keep", "count" -> 2]}],
        ["items" -> {["name" -> "Keep", "count" -> "2"]}],
        ["items" -> {["name" -> "Keep", "count" -> 4]}],
        ["items" -> {["name" -> "Keep", "count" -> true]}],
        ["Items" -> {["name" -> "Keep", "count" -> 2]}],
        ["items" -> {["name" -> "Keep", "count" -> 2, "extra" -> 0]}]})
      let denied = false;
      try
        $llm_tool_schema:arguments(value, schema);
      except (E_TYPE, E_INVARG, E_QUOTA)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Invalid nested arguments are rejected");
    endfor
    let deep = [];
    for depth in [1..18]
      deep = ["nested" -> deep];
    endfor
    let denied = false;
    try
      $llm_tool_schema:arguments(deep, ["type" -> "object"]);
    except (E_QUOTA)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Unconstrained nested values still obey the depth limit");
    return true;
  endmethod

  method test_invalid_tool_arguments_have_no_effects owner: ARCH_WIZARD
    "Invalid mutation arguments fail before callbacks or dispatch and leave no uncertain outcome.";
    const agent = create($llm_agent, #90102, 1);
    const client = this:_client({this:_tool_reply({"mark"})});
    const sink = this:_sink(agent);
    try
      agent.client = client;
      add_property(sink, "callbacks", 0, {#90102, ""});
      add_verb(sink, {#90102, "rxd", "on_tool_call"}, {"this", "none", "this"});
      set_verb_code(sink, "on_tool_call", {"this.callbacks = this.callbacks + 1;"});
      agent.tool_callback = sink;
      const schema = ["type" -> "object", "properties" -> ["count" -> ["type" -> "integer"]],
        "required" -> {"count"}, "additionalProperties" -> false];
      agent:add_tool("mark", $llm_agent_tool:mk("mark", "Count a mutation", schema, sink, "mark", 'mutation));
      const result = agent:run_turn(false, #90102);
      $test_utils:assert_eq(result["status"], "all_failed", "Invalid arguments are a known failure");
      $test_utils:assert_eq(sink.effects, 0, "The handler did not run");
      $test_utils:assert_eq(sink.callbacks, 0, "Invalid arguments did not reach the callback");
      $test_utils:assert_eq(length(agent.pending_tools), 0, "No uncertain mutation remains");
      $test_utils:assert_eq(agent.tool_outcomes["1"].status, 'failure, "Argument failure is not an unknown effect");
      let reply = this:_tool_reply({"mark"});
      reply["choices"][1]["message"]["tool_calls"][1]["function"]["arguments"] = ["count" -> 1];
      client.replies = {reply};
      agent:run_turn(false, #90102);
      $test_utils:assert_eq(sink.effects, 1, "Valid corrected arguments execute once");
      $test_utils:assert_eq(sink.callbacks, 1, "Only validated arguments reach the callback");
    finally
      recycle(client);
      recycle(sink);
    endtry
    return true;
  endmethod

  method test_provider_credentials_and_setter_authority owner: ARCH_WIZARD
    "Keys do not follow prototype inheritance, and key setters never trust target authorization overrides.";
    const parent = create($llm_client, #90100, 2);
    const child = create(parent, #90102, 2);
    try
      parent:set_api_key("fixture-only-key");
      parent.api_endpoint = "http://127.0.0.1:1/chat/completions";
      parent.model = "fixture";
      parent:set_access(true);
      $test_utils:assert_true(parent:is_configured(), "A local key configures its client");
      $test_utils:assert_false(child:is_configured(), "A descendant cannot use an ancestor's key");
      let denied = false;
      try
        this:_as_player(child, "chat", {"probe"});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Inherited credential rejected before network I/O");
      parent:set_access(false);
      denied = false;
      try
        this:_as_player(parent, "chat", {"probe"});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Private client rejects a foreign actor");
      add_verb(parent, {#90102, "rxd", "check_permissions_with_grants_as"}, {"this", "none", "this"});
      set_verb_code(parent, "check_permissions_with_grants_as", {"return {this, $arch_wizard, {}};"});
      denied = false;
      try
        this:_as_player(parent, "set_api_key", {"forged"});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Overridden authorization cannot replace a key");
      $test_utils:assert_eq(parent.api_key, "fixture-only-key", "Credential remains unchanged");
      parent:set_access(false, {#90102});
      let unsupported = false;
      try
        this:_as_player(parent, "chat", {"probe", false, false, true});
      except (E_INVARG)
        unsupported = true;
      endtry
      $test_utils:assert_true(unsupported, "An approved actor reaches validation, which rejects streaming");
    finally
      recycle(child);
      recycle(parent);
    endtry
    return true;
  endmethod

  method test_provider_options_are_inert owner: ARCH_WIZARD
    "Options are validated slots, not an opportunity to invoke attacker-supplied code as wizard.";
    const hostile = create($llm_chat_opts, #90102, 2);
    try
      add_property(hostile, "invoked", false, {#90102, ""});
      add_verb(hostile, {#90102, "rxd", "to_body_params"}, {"this", "none", "this"});
      set_verb_code(hostile, "to_body_params", {"this.invoked = true;", "return [\"model\" -> \"forged\"];"});
      const result = $llm_provider:options(<hostile, .temperature = 0.2, .max_tokens = 50>);
      $test_utils:assert_eq(result["max_tokens"], 50, "Validated options retain their values");
      $test_utils:assert_false(hostile.invoked, "No supplied behavior was called");
      const named = ["type" -> "function", "function" -> ["name" -> "inspect"]];
      const choice = $llm_provider:options($llm_chat_opts:mk():with_tool_choice(named));
      $test_utils:assert_eq(choice["tool_choice"], named, "A named tool choice retains its wire shape");
      for invalid in ({<hostile, .model = "forged">, <hostile, .max_tokens = -1>, <hostile, .temperature = 7>,
          $llm_chat_opts:mk():with_tool_choice(["type" -> "function", "function" -> ["name" -> ""]])})
        let denied = false;
        try
          $llm_provider:options(invalid);
        except (E_TYPE, E_INVARG)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Invalid or transport-changing options are rejected");
      endfor
    finally
      recycle(hostile);
    endtry
    return true;
  endmethod

  method test_provider_account_reservations owner: ARCH_WIZARD
    "Reservations prevent concurrent overspend, settle once, and retain unknown charges after expiry.";
    const actor = create($thing, #90102, 2);
    add_verb($llm_provider, {$arch_wizard, "rxd", "fixture_account"}, {"this", "none", "this"});
    set_verb_code($llm_provider, "fixture_account", {"const {name, @values} = args;", "return $llm_account:(name)(@values);"});
    try
      actor.llm_token_budget = 10000;
      const first = $llm_provider:fixture_account("reserve", actor, 6000, time() + 60, 'conversation, "fixture");
      let denied = false;
      try
        $llm_provider:fixture_account("reserve", actor, 6000, time() + 60, 'compaction, "fixture");
      except (E_QUOTA)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Held capacity excludes a concurrent request");
      $test_utils:assert_eq($llm_account:available(actor), 4000, "Reservations reduce available capacity");
      $test_utils:assert_true($llm_provider:fixture_account("finish", actor, first, ["total_tokens" -> 100], 'complete), "First settlement succeeds");
      $test_utils:assert_false($llm_provider:fixture_account("finish", actor, first, ["total_tokens" -> 100], 'complete), "Duplicate settlement does not charge twice");
      $test_utils:assert_eq(actor.llm_tokens_used, 100, "Only reported usage is charged on completion");
      const expired = $llm_provider:fixture_account("reserve", actor, 500, time() - 1, 'triage, "fixture");
      $llm_account:reconcile(actor);
      $test_utils:assert_eq(actor.llm_tokens_used, 600, "An abandoned request retains an uncertain charge");
      $test_utils:assert_false(actor.llm_usage_log[$]["reported"], "Unknown usage is explicit");
      $test_utils:assert_false($llm_provider:fixture_account("finish", actor, expired, ["total_tokens" -> 100], 'complete), "Late delivery cannot settle an expired request twice");
      const malformed = $llm_provider:fixture_account("reserve", actor, 700, time() + 60, 'facts, "fixture");
      $llm_provider:fixture_account("finish", actor, malformed, ["total_tokens" -> -1], 'complete);
      $test_utils:assert_eq(actor.llm_tokens_used, 1300, "Malformed usage cannot reduce the bill");
      $test_utils:assert_eq(length(actor.llm_reservations), 0, "All reservations have settled");
      denied = false;
      try
        this:_as_player($llm_account, "finish", {actor, first, ["total_tokens" -> 0], 'complete});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Callers cannot forge provider settlements");
      const child = create(actor, #90102, 1);
      $test_utils:assert_eq($llm_account:available(child), 10000, "An actor does not inherit a parent's runtime charges");
    finally
      delete_verb($llm_provider, "fixture_account");
      recycle(actor);
    endtry
    return true;
  endmethod
  method test_wearable_progress_uses_current_arguments owner: ARCH_WIZARD
    "LLM wearable progress uses canonical object arguments and accepts partial source ranges.";
    for prototype in ({$data_visor, $architects_compass})
      const device = create(prototype, #90101, 2);
      try
        for formatter in ({"_format_hud_message", "_format_tts_message"})
          const inspected = this:_as_programmer(device, formatter,
            {"inspect_object", ["object" -> "#90101"]});
          $test_utils:assert_true(index(inspected, "#90101") > 0,
            "Object inspection progress uses the published object argument");
          for bounds in ({["start_line" -> 2], ["end_line" -> 9]})
            let parameters = ["object" -> "#90101", "verb" -> "example"];
            for field in (mapkeys(bounds))
              parameters[field] = bounds[field];
            endfor
            const ranged = this:_as_programmer(device, formatter, {"get_verb_code_range", parameters});
            $test_utils:assert_true(index(ranged, "#90101:example") > 0,
              "Range progress retains its source reference");
            $test_utils:assert_true(index(ranged, tostr(mapvalues(bounds)[1])) > 0,
              "A supplied boundary remains visible without requiring the omitted boundary");
          endfor
        endfor
      finally
        recycle(device);
      endtry
    endfor
    return true;
  endmethod

  method test_inspection_structured_descriptions owner: ARCH_WIZARD
    "LLM object and social inspection resolve structured descriptions as the requester before JSON output.";
    const actor = #90102;
    const target = create($thing, #90101, 2);
    const person = #90101;
    const original = person.description;
    const original_kind = person.description_kind;
    try
      add_property(target, "description_principal", #-1, {#90101, "r"});
      add_verb(target, {#90101, "rxd", "description"}, {"this", "none", "this"});
      set_verb_code(target, "description", {
        "this.description_principal = caller_perms();", "return this.description;"});
      for content in ({"literal *stars* {braces}",
          $format.paragraph:mk("structured description"), {"first paragraph", "second paragraph"},
          $format:authored('djot_blocks, "*formatted description*")})
        target.description = content;
        person.description = content;
        person.description_kind = 'literal;
        const inspected = this:_as_player($llm_inspection_tools, "inspect_object",
          {["object" -> tostr(target)], actor});
        const social = parse_json(this:_as_player($llm_social_tools, "player_info",
          {["player_name" -> tostr(person)], actor}));
        const text = social["description"];
        $test_utils:assert_eq(typeof(text), TYPE_STR, "Social inspection contains plain text, not a flyweight encoding");
        $test_utils:assert_true(index(inspected, text) > 0, "Object and social inspection use the same rendering");
        $test_utils:assert_eq(target.description_principal, actor, "Description callbacks receive requester authority");
        if (typeof(content) == TYPE_STR)
          $test_utils:assert_eq(text, content, "Literal punctuation is never parsed as markup");
        elseif (typeof(content) == TYPE_LIST)
          $test_utils:assert_true(index(text, "first paragraph") > 0 && index(text, "second paragraph") > 0,
            "All structured paragraphs survive rendering");
        else
          $test_utils:assert_true(index(text, "description") > 0, "Authored content becomes readable text");
        endif
      endfor
    finally
      person.description = original;
      person.description_kind = original_kind;
      recycle(target);
    endtry
    return true;
  endmethod

  method test_code_search_pages owner: ARCH_WIZARD
    "LLM source pages preserve alias definitions, literal matching, and every match across cursors.";
    const actor = #90102;
    const target = create($root, actor, 2);
    try
      add_verb(target, {actor, "rxd", "se*arch alias"}, {"this", "none", "this"});
      set_verb_code(target, "alias", {"\"Needle.* one\";", "\"Needle.* two\";", "return \"Needle.* three\";"});
      add_verb(target, {actor, "rxd", "second"}, {"this", "none", "this"});
      set_verb_code(target, "second", {"return \"Needle.* four\";"});
      let request = ["object" -> tostr(target), "pattern" -> "needle.*", "limit" -> 2];
      let found = {};
      let finished = false;
      for page_number in [1..4]
        const page = this:_as_player($llm_code_search, "search", {request, actor});
        $test_utils:assert_true(length(page["matches"]) <= 2, "The requested match limit applies to each page");
        $test_utils:assert_true(value_bytes(page) <= 65536, "Each page is bounded JSON data");
        found = {@found, @page["matches"]};
        if (length(page["next_cursor"]) == 0)
          finished = true;
          break;
        endif
        $test_utils:assert_false(page["next_cursor"] == `request["cursor"] ! E_RANGE => {}',
          "Every continuation makes progress");
        request["cursor"] = page["next_cursor"];
      endfor
      $test_utils:assert_true(finished, "The bounded traversal finishes");
      $test_utils:assert_eq(length(found), 4, "Every literal match appears once");
      $test_utils:assert_eq({entry["line"] for entry in (found)}, {1, 2, 3, 1}, "Line origins survive pagination");
      $test_utils:assert_eq({match["verb_index"] for match in (found)}, {1, 1, 1, 2}, "Aliases use native numeric indexes");
      $test_utils:assert_eq(found[1]["names"], "se*arch alias", "The result retains declared aliases");
      $test_utils:assert_eq(found[4]["text"], "return \"Needle.* four\";", "Literal punctuation remains in the snippet");
      const absent = this:_as_player($llm_code_search, "search",
        {["object" -> tostr(target), "pattern" -> "^Needle"], actor});
      $test_utils:assert_eq(absent["matches"], {}, "Patterns are literals, not regular expressions");
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method test_code_search_authority owner: ARCH_WIZARD
    "LLM search rejects forged actors and private explicit targets and reports unreadable verb omissions.";
    const actor = #90102;
    const target = create($root, #90101, 2);
    const remote = create($root, #90101, 2);
    const room = create($room, actor, 2);
    const previous = actor.location;
    try
      target.r = 1;
      remote.r = 1;
      for candidate in ({target, remote})
        add_verb(candidate, {#90101, "rxd", "visible"}, {"this", "none", "this"});
        set_verb_code(candidate, "visible", {"return \"search-authority-marker\";"});
      endfor
      add_verb(target, {#90101, "xd", "private"}, {"this", "none", "this"});
      set_verb_code(target, "private", {"return \"hidden-search-authority-marker\";"});
      const request = ["object" -> tostr(target), "pattern" -> "search-authority-marker"];
      const page = this:_as_player($llm_code_search, "search", {request, actor});
      $test_utils:assert_eq(length(page["matches"]), 1, "A public verb remains searchable");
      $test_utils:assert_eq(page["unreadable_verbs"], 1, "A private verb is an explicit omission");
      $test_utils:assert_eq(index(toliteral(page), "hidden-search"), 0, "Private source never enters a result");
      let denied = false;
      try
        this:_as_player($llm_code_search, "search", {request, #90101});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A caller cannot use another actor's read authority");
      target.r = 0;
      denied = false;
      try
        this:_as_player($llm_code_search, "search", {request, actor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "An explicitly unreadable object fails instead of returning no matches");
      move(actor, room);
      move(target, room);
      const local = this:_as_player($llm_code_search, "search", {["pattern" -> "search-authority-marker"], actor});
      $test_utils:assert_eq(local["scope"], "local", "An ordinary actor gets local default scope");
      $test_utils:assert_true(local["unreadable_objects"] > 0, "Local scope reports unreadable objects");
      $test_utils:assert_eq(local["matches"], {}, "Remote readable objects are outside local scope");
      $test_utils:assert_eq($llm_code_search:_scope($arch_wizard, "")[2], "global", "Wizard default scope is global");
    finally
      move(actor, previous);
      recycle(target);
      recycle(remote);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_shared_verb_source_snapshots owner: ARCH_WIZARD
    "LLM source snapshots share native lookup, immutable ranges, rendering, and bounded selection.";
    const actor = #90102;
    const parent = create($thing, actor, 2);
    const child = create(parent, actor, 2);
    try
      add_verb(parent, {actor, "rxd", "so*urce alias"}, {"this", "none", "this"});
      set_verb_code(parent, "alias", {"\"source marker\";", "let count = 1;", "count = count + 2;", "return count;"});
      add_verb(child, {actor, "rxd", "find_verb_definer"}, {"this", "none", "this"});
      set_verb_code(child, "find_verb_definer", {"raise(E_ASSERT, \"Do not call target metadata hooks\");"});
      const original = verb_code(parent, "alias");
      const source = this:_as_player($llm_verb_source, "read", {actor, child, "source"});
      $test_utils:assert_eq(source.definer, parent, "Native lookup follows aliases and inheritance");
      $test_utils:assert_eq(source.lines, original, "Source remains separate from line-number formatting");
      const selected = this:_as_player($llm_verb_source, "read", {actor, child, "alias", 2, 2, 1});
      $test_utils:assert_eq(selected.lines, original[1..3], "Context expands the requested range");
      const range = this:_as_player($llm_verb_source, "read", {actor, child, "alias", 2, 3});
      $test_utils:assert_eq(range.first, 2, "Selected source keeps its original line number");
      $test_utils:assert_true(range:numbered_text():starts_with("2: "), "LLM text ranges retain line numbers");
      const document = $event:mk_eval_result(actor, "", range:document()):resolve(actor);
      $test_utils:assert_true($document:validate(document), "Presentation uses canonical document nodes");
      for format in ({'text_plain, 'text_html})
        const rendered = $document:render(document, format)['body];
        $test_utils:assert_true(index(rendered, "count") > 0, "Both renderers receive the selected code");
      endfor
      set_verb_code(parent, "alias", {"return 99;"});
      $test_utils:assert_eq(source.lines, original, "Later edits cannot change captured source values");
      const current = this:_as_player($llm_verb_source, "read", {actor, child, "alias"});
      $test_utils:assert_true(current.lines != source.lines, "A new read sees the edited source");
      let large = "x";
      for i in [1..16]
        large = large + large;
      endfor
      set_verb_code(parent, "alias", {toliteral(large) + ";", "return 1;"});
      let limited = false;
      try
        this:_as_player($llm_verb_source, "read", {actor, child, "alias"});
      except (E_QUOTA)
        limited = true;
      endtry
      $test_utils:assert_true(limited, "Full source snapshots have a byte limit");
      const small = this:_as_player($llm_verb_source, "read", {actor, child, "alias", 2, 2});
      $test_utils:assert_eq(small.lines, {"return 1;"}, "A small range from a large verb remains available");
      $test_utils:assert_eq(small.total, 2, "Range metadata records the complete verb length");
      add_verb(parent, {actor, "rxd", "empty_source"}, {"this", "none", "this"});
      const empty = this:_as_player($llm_verb_source, "read", {actor, child, "empty_source"});
      $test_utils:assert_eq(empty.lines, {}, "An empty verb remains a valid source snapshot");
      const empty_document = $event:mk_info(actor, empty:document()):resolve(actor);
      $test_utils:assert_true(index($document:render(empty_document, 'text_plain)['body], "0 lines") > 0,
        "Empty source has a useful range label");
    finally
      recycle(child);
      recycle(parent);
    endtry
    return true;
  endmethod

  method presentation_case owner: ARCH_WIZARD
    "Run real room job presentation with deterministic model replies and a supplied session endpoint.";
    caller_perms().wizard || raise(E_PERM);
    const {actor, destination} = args;
    const room = this:_job_room();
    const reply = ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "",
      "tool_calls" -> {["id" -> "report", "type" -> "function", "function" -> ["name" -> "present_report",
        "arguments" -> generate_json(["content" -> "private report"])]]}]]}];
    const client = this:_client({reply, ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "done"]]}]});
    try
      room.llm_client = client;
      let task = ['player -> actor, 'query -> "present a report", 'connection -> destination];
      task["authorization"] = $agent_room:_authorize_task(room, task);
      room:job_call(room, "_execute_task", {task});
      $test_utils:assert_eq(room.history[$]['status], 'complete, "Presentation completes through the shared loop");
    finally
      $llm_room_runtime:reset(room, #90100);
      recycle(client);
      recycle(room);
    endtry
    return true;
  endmethod
  method _delayed_room owner: ARCH_WIZARD
    "Create a room with a model barrier; tests must reset the room and recycle both objects.";
    caller == this && this == #90031 || raise(E_PERM);
    const room = this:_job_room();
    const client = this:_client({});
    add_property(client, "effects", 0, {#90102, ""});
    set_verb_code(client, "chat", {
      "this.requests = {@this.requests, caller_perms()};",
      "task_recv(30);",
      "this.effects = this.effects + 1;",
      "return [\"choices\" -> {[\"message\" -> [\"role\" -> \"assistant\", \"content\" -> \"done\"]]}];"});
    room.llm_client = client;
    return {room, client};
  endmethod

  method test_room_concurrent_admission_and_cancellation owner: ARCH_WIZARD
    "Queued work counts immediately; two actors run independently and cancellation kills only selected children.";
    const {room, client} = this:_delayed_room();
    room.max_workers = 2;
    room.max_jobs = 4;
    room.max_pending_per_player = 2;
    try
      this:_as_player($llm_room_runtime, "submit", {room, #90102, "first"});
      $llm_room_runtime:submit(room, #90101, "second");
      this:_as_player($llm_room_runtime, "submit", {room, #90102, "third"});
      $test_utils:assert_eq(length(room.jobs), 3, "Admission records jobs before mailbox or worker dispatch");
      let denied = false;
      try
        this:_as_player($llm_room_runtime, "submit", {room, #90102, "over requester limit"});
      except (E_QUOTA)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Queued requests count toward the requester limit");
      room.max_jobs = 3;
      denied = false;
      try
        $llm_room_runtime:submit(room, #90101, "over global limit");
      except (E_QUOTA)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "All actors share the bounded room capacity");
      const first = room.jobs[1];
      const second = room.jobs[2];
      const third = room.jobs[3];
      const deadline = time() + 8;
      while (length(client.requests) < 2)
        time() < deadline || raise(E_ASSERT, "Concurrent model children did not start");
        suspend(0.01);
      endwhile
      $test_utils:assert_eq(third.state, 'queued, "max_workers bounds concurrent execution");
      $test_utils:assert_eq(third.parent_task, 0, "Queued work has no worker yet");
      const first_child = first.worker_task;
      const second_child = second.worker_task;
      const presenter = first.presenter;
      const cancelled = this:_as_player($llm_room_runtime, "cancel", {room, #90102});
      $test_utils:assert_eq(cancelled, 2, "Cancellation includes queued and running own jobs");
      $test_utils:assert_eq(room.jobs, {second}, "Foreign work remains admitted");
      $test_utils:assert_false(valid_task(first_child), "The cancelled model child is gone");
      $test_utils:assert_true(valid_task(second_child), "The other requester's model child remains live");
      $test_utils:assert_false(presenter.active, "Cancellation closes presentation");
      $test_utils:assert_eq(length(room.history), 2, "Each cancelled job records one outcome");
      task_send(second_child, true);
      while (length(room.jobs) > 0)
        time() < deadline || raise(E_ASSERT, "Completed work did not retire");
        suspend(0.01);
      endwhile
      $test_utils:assert_eq(length(room.history), 3, "Completion records one further outcome");
      $test_utils:assert_eq(room.history[$]['requester], #90101, "The foreign job retains its actor");
      $test_utils:assert_eq(room.history[$]['status], 'complete, "The remaining job completes");
      $test_utils:assert_eq(client.effects, 1, "Cancelled requests cannot perform their delayed effects");
      $test_utils:assert_eq(room.loop_task, 0, "The idle monitor exits");
    finally
      $llm_room_runtime:reset(room, #90100);
      recycle(client);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_room_worker_loss_and_monitor_recovery owner: ARCH_WIZARD
    "A replacement monitor finds admitted jobs and cancels a dead parent's model child without replay.";
    const {room, client} = this:_delayed_room();
    try
      $llm_room_runtime:submit(room, #90102, "orphan probe");
      const first = room.jobs[1];
      const deadline = time() + 8;
      while (length(client.requests) < 1)
        time() < deadline || raise(E_ASSERT, "Initial worker did not start");
        suspend(0.01);
      endwhile
      const child = first.worker_task;
      kill_task(room.loop_task);
      kill_task(first.parent_task);
      $llm_room_runtime:submit(room, #90101, "next request");
      while (length(client.requests) < 2)
        time() < deadline || raise(E_ASSERT, "Replacement monitor did not dispatch queued work");
        suspend(0.01);
      endwhile
      $test_utils:assert_false(valid_task(child), "The dead parent's model child is cancelled");
      $test_utils:assert_eq(room.history[1]['status], 'cancelled, "Worker loss has a terminal record");
      $test_utils:assert_eq(room.history[1]['query], "orphan probe", "Lost work is not replayed");
      $test_utils:assert_eq(client.effects, 0, "Restarting the monitor does not run delayed effects");
    finally
      $llm_room_runtime:reset(room, #90100);
      recycle(client);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_room_prototypes_do_not_share_live_state owner: ARCH_WIZARD
    "A room inherits configuration and behavior, but never its parent's jobs, history, or task IDs.";
    const {parent_room, client} = this:_delayed_room();
    let child = #-1;
    try
      $llm_room_runtime:submit(parent_room, #90102, "parent request");
      parent_room.history = {['query -> "parent history", 'requester -> #90102, 'context -> {}]};
      child = create(parent_room, #90102, 2);
      const snapshot = this:_as_player($llm_room_view, "snapshot", {child, #90102});
      $test_utils:assert_eq(snapshot['jobs], {}, "Child has no inherited active jobs");
      $test_utils:assert_eq(snapshot['queue], {}, "Child has no inherited queued jobs");
      $test_utils:assert_eq(snapshot['history], {}, "Child has no inherited transcript");
      this:_as_player($llm_room_runtime, "reset", {child, #90102});
      $test_utils:assert_eq(length(parent_room.jobs), 1, "Resetting a child preserves the parent's request");
      $test_utils:assert_eq(length(parent_room.history), 1, "Resetting a child preserves the parent's history");
      this:_as_player($llm_room_runtime, "submit", {child, #90102, "child request"});
      $test_utils:assert_eq(child.jobs[1].host, child, "New child jobs capture the child room");
      $test_utils:assert_true(child.loop_task != parent_room.loop_task, "Monitors are room-local");
    finally
      if (valid(child))
        $llm_room_runtime:reset(child, #90100);
        recycle(child);
      endif
      $llm_room_runtime:reset(parent_room, #90100);
      recycle(client);
      recycle(parent_room);
    endtry
    return true;
  endmethod

  method test_memory_isolation_and_authority owner: ARCH_WIZARD
    "Observer descendants receive independent memory; copied references grant no authority.";
    const observer = create($llm_room_observer, #90102, 2);
    let child = #-1;
    try
      const memory = this:_as_player($llm_room_observer, "_memory", {observer, #90102});
      this:_as_player(memory, "remember", {"person", "private fact"});
      child = create(observer, #90101, 2);
      const independent = $llm_room_observer:_memory(child, #90101);
      $test_utils:assert_true(memory != independent, "Descendant does not inherit live memory");
      $test_utils:assert_eq(independent:snapshot(), {}, "Descendant starts with empty memory");
      $test_utils:assert_true(is_anonymous(memory) && is_anonymous(memory.facts), "Memory and relation are anonymous");
      for operation in ({"snapshot", "summary", "get_tools"})
        let denied = false;
        try
          this:_as_player(independent, operation, {});
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign memory rejects " + operation);
      endfor
      let spoofed = false;
      try
        this:_as_player(independent, "remember_fact", {["subject" -> "forged", "fact" -> "forged fact"], #90101});
      except (E_PERM)
        spoofed = true;
      endtry
      $test_utils:assert_true(spoofed, "Tool arguments cannot replace the caller principal");
      const snapshot = memory:snapshot();
      let omitted = false;
      try
        this:_as_player(memory, "apply_compaction", {snapshot, {["subject" -> "person", "fact" -> "new fact", "sources" -> {}]}});
      except (E_TYPE)
        omitted = true;
      endtry
      $test_utils:assert_true(omitted, "Compaction requires supporting sources");
      $test_utils:assert_eq(memory:snapshot(), snapshot, "Invalid provenance preserves memory");
      this:_as_player(memory, "remember", {"another", "another private fact"});
      const complete = memory:snapshot();
      omitted = false;
      try
        this:_as_player(memory, "apply_compaction", {complete,
          {["subject" -> "person", "fact" -> "first fact only", "sources" -> {1}]}});
      except (E_INVARG)
        omitted = true;
      endtry
      $test_utils:assert_true(omitted, "Every source must appear in the compacted proposal");
      $test_utils:assert_eq(memory:snapshot(), complete, "Omitted sources leave all facts unchanged");
    finally
      valid(child) && recycle(child);
      recycle(observer);
    endtry
    return true;
  endmethod

  method _compact_memory_background owner: ARCH_WIZARD
    "Start compaction outside the test's finally scope and record the public outcome.";
    caller == this && this == #90031 || raise(E_PERM);
    const {memory, client, result} = args;
    fork tid (0)
      try
        result.outcome = this:_as_player(memory, "compact", {client});
      except error (ANY)
        result.outcome = error[1];
      endtry
    endfork
    return tid;
  endmethod

  method test_memory_compaction_rechecks_after_io owner: ARCH_WIZARD
    "Intervening facts and ownership changes during a model wait cannot be overwritten.";
    const observer = create($llm_room_observer, #90102, 2);
    const memory = $llm_memory:new(observer);
    const client = this:_client({});
    const result = create($root, $arch_wizard, 2);
    let worker = 0;
    try
      add_property(result, "outcome", 'waiting, {$arch_wizard, ""});
      set_verb_code(client, "chat", {
        "this.requests = {@this.requests, args[1]};",
        "task_recv(20);",
        "return [\"choices\" -> {[\"message\" -> [\"role\" -> \"assistant\",",
        "  \"content\" -> generate_json({[\"subject\" -> \"person\", \"fact\" -> \"combined\", \"sources\" -> {1, 2}]})]]}];"});
      memory:remember("person", "first fact");
      memory:remember("person", "second fact");
      for change in ({'append, 'owner})
        const deadline = time() + 8;
        client.requests = {};
        result.outcome = 'waiting;
        worker = this:_compact_memory_background(memory, client, result);
        while (length(client.requests) == 0)
          time() < deadline || raise(E_ASSERT, "Memory compaction did not start");
          suspend(0.01);
        endwhile
        if (change == 'append)
          this:_as_player(memory, "remember", {"person", "intervening fact"});
        else
          observer.owner = #90101;
        endif
        const snapshot = memory:snapshot();
        task_send(worker, true);
        while (valid_task(worker))
          time() < deadline || raise(E_ASSERT, "Memory compaction did not finish");
          suspend(0.01);
        endwhile
        $test_utils:assert_eq(result.outcome, change == 'append ? false | E_PERM, "Compaction rechecks state and authority after I/O");
        $test_utils:assert_eq(memory:snapshot(), snapshot, "The rejected response preserves all current facts");
      endfor
    finally
      worker != 0 && valid_task(worker) && kill_task(worker);
      recycle(client);
      recycle(result);
      recycle(observer);
    endtry
    return true;
  endmethod

  method _wearable owner: ARCH_WIZARD
    "Make an anonymous device with a minimal frontend for interaction lifecycle tests.";
    const {actor, ?kind = 1} = args;
    const device = create($llm_wearable, actor, kind);
    add_verb(device, {actor, "rxd", "_setup_agent"}, {"this", "none", "this"});
    set_verb_code(device, "_setup_agent", {"return true;"});
    return device;
  endmethod

  method wearable_fixture owner: ARCH_WIZARD
    "Create a deterministic connected wearable; the session scenario recycles the device and client.";
    caller_perms().wizard || raise(E_PERM);
    const {actor} = args;
    const device = this:_wearable(actor, 2);
    device.name = "interaction probe";
    move(device, actor);
    actor.wearing = {@actor.wearing, device};
    device:configure();
    device.agent.client = this:_client({["choices" -> {["message" ->
      ["role" -> "assistant", "content" -> "private interaction result"]]}]});
    add_verb(device, {$arch_wizard, "rxd", "start_probe"}, {"this", "none", "this"});
    set_verb_code(device, "start_probe", {
      "caller_perms().wizard || raise(E_PERM);",
      "const {destination} = args;",
      "fork worker (0)",
      "  const session = $llm_wearable_session:open(this, this.owner, destination);",
      "  session:interact();",
      "endfork",
      "return worker;"});
    add_verb(device, {actor, "rxd", "remove_probe"}, {"this", "none", "this"});
    set_verb_code(device, "remove_probe", {"return this:do_remove(this.owner, true);"});
    return device;
  endmethod

  method test_wearable_interaction_admission owner: ARCH_WIZARD
    "Admission precedes input, active configuration is immutable, and descendants isolate collaborators.";
    const actor = #90102;
    const previous = actor.wearing;
    const device = this:_wearable(actor);
    const child = create(device, actor, 1);
    let first = #-1;
    let second = #-1;
    try
      actor.wearing = {@previous, device, child};
      first = $llm_wearable_session:open(device, actor, 0);
      const agent = device.agent;
      const descriptor = agent.tools["ask_user"];
      const project_tools = agent.tools["todo_write"].target_obj;
      const written = this:_as_player(project_tools, "todo_write", {["todos" -> {
        ["content" -> "captured task", "status" -> "pending"]}], actor});
      $test_utils:assert_true(index(written, "captured task"), "Todo writes use the captured agent under actor authority");
      const todos = this:_as_player(project_tools, "get_todos", {[], actor});
      $test_utils:assert_true(index(todos, "captured task"), "Todo reads use the same captured agent");
      for spec in ({{first, "_tool_explain", ["message" -> "foreign output"]},
          {first, "_tool_ask_user", ["question" -> "foreign prompt"]},
          {project_tools, "todo_write", ["todos" -> {}]}, {project_tools, "get_todos", []}})
        const {target, name, parameters} = spec;
        let denied = false;
        try
          this:_as_programmer(target, name, {parameters, actor});
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "A foreign caller cannot claim the captured actor for " + name);
      endfor
      const forged = create($root, #90101, 1);
      add_property(forged, "interaction", first, {#90101, ""});
      let forged_stop = false;
      try
        this:_as_programmer($llm_wearable_session, "stop", {forged, #90101});
      except (E_PERM)
        forged_stop = true;
      endtry
      $test_utils:assert_true(forged_stop && first.active, "A forged host cannot cancel another actor's interaction");
      $test_utils:assert_true(is_anonymous(first), "Interaction state is anonymous");
      for action in ({"open", "configure", "reconfigure"})
        let denied = false;
        try
          if (action == "open")
            $llm_wearable_session:open(device, actor, 0);
          else
            device:(action)();
          endif
        except (E_INVARG)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Active interaction rejects " + action);
      endfor
      second = $llm_wearable_session:open(child, actor, 0);
      $test_utils:assert_true(child.agent != agent && child.interaction != first,
        "Inherited agents and interaction fields are not reused");
      second:close();
      $test_utils:assert_true(first.active && device.agent == agent,
        "Closing a descendant preserves its parent's interaction");
      first:close();
      $test_utils:assert_eq(device.interaction, #-1, "The device releases its interaction");
      $test_utils:assert_eq(first.host, #-1, "The interaction releases its device");
      $test_utils:assert_eq(agent.tool_callback, #-1, "Callback cycles are broken");
      $test_utils:assert_false(maphaskey(agent.tools, "ask_user"), "Session tools are unregistered");
      let denied = false;
      try
        this:_as_player(descriptor.target_obj, descriptor.target_verb, {["question" -> "stale input"], actor});
      except (E_INVARG)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A retained descriptor cannot revive a closed session");
      second = $llm_wearable_session:open(device, actor, 0);
      $test_utils:assert_eq(device.agent, agent, "The same conversation survives sequential interactions");
    finally
      valid(first) && first:close();
      valid(second) && second:close();
      actor.wearing = previous;
    endtry
    return true;
  endmethod

  method test_wearable_confirmation_scope owner: ARCH_WIZARD
    "Approvals are per effect, target, and interaction; stale previews and alternatives stop effects.";
    const actor = #90102;
    const previous = actor.wearing;
    const device = this:_wearable(actor);
    const target = create($thing, actor, 1);
    let session = #-1;
    try
      actor.wearing = {@previous, device};
      add_property(target, "value", "original", {actor, ""});
      device:configure();
      for tool in ($llm_programming_tools:get_tools())
        device.agent:add_tool(tool.name, tool);
      endfor
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["set_property"].target_obj;
      add_property(session, "answer", "yes_all", {$arch_wizard, ""});
      add_property(session, "reads", 0, {$arch_wizard, ""});
      add_property(session, "mutate", #-1, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {
        "this.reads = this.reads + 1;",
        "if (valid(this.mutate)) this.mutate.value = \"changed\"; endif",
        "return this.answer;"});
      for iteration in [1..2]
        $test_utils:assert_eq(provider:_approve(actor, 'set_property,
          target, 'property, "value", ["value" -> "proposed"]), true, "Scoped approval authorizes its effect");
      endfor
      $test_utils:assert_eq(session.reads, 1, "The same effect and target reuse approval");
      provider:_approve(actor, 'delete_property, target, 'property, "value", ["value" -> "proposed"]);
      $test_utils:assert_eq(session.reads, 2, "Another effect needs separate approval");
      const other = create($thing, actor, 1);
      add_property(other, "value", "other", {actor, ""});
      provider:_approve(actor, 'set_property, other, 'property, "value", ["value" -> "proposed"]);
      $test_utils:assert_eq(session.reads, 3, "Anonymous targets keep distinct approval scopes");
      session.approvals = [];
      session.answer = "yes";
      session.mutate = target;
      const stale = provider:_approve(actor, 'set_property,
        target, 'property, "value", ["value" -> "proposed"]);
      $test_utils:assert_eq(stale.status, 'waiting, "Changed previews require another decision");
      session.mutate = #-1;
      session.answer = "alternative: keep the current value";
      const alternative = provider:_approve(actor, 'set_property,
        target, 'property, "value", ["value" -> "proposed"]);
      $test_utils:assert_eq(alternative.status, 'waiting, "Alternatives are typed waiting outcomes");
      session.answer = "unrecognized answer";
      const cancelled = provider:_approve(actor, 'set_property,
        target, 'property, "value", ["value" -> "proposed"]);
      $test_utils:assert_eq(cancelled.status, 'cancelled, "Unrecognized answers never authorize an effect");
      session:close();
      session = $llm_wearable_session:open(device, actor, 0);
      $test_utils:assert_eq(session.approvals, [], "A new interaction starts without approvals");
    finally
      valid(session) && session:close();
      actor.wearing = previous;
    endtry
    return true;
  endmethod

  method test_wearable_programming_provider owner: ARCH_WIZARD
    "Shared mutations require captured approvals and reject stale targets, foreign actors, and released sessions.";
    const actor = #90101;
    const previous = actor.wearing;
    const device = this:_wearable(actor);
    const target = create($thing, actor, 1);
    const other = create($thing, actor, 1);
    let session = #-1;
    try
      actor.wearing = {@previous, device};
      device:configure();
      for tool in ($llm_programming_tools:get_tools())
        device.agent:add_tool(tool.name, tool);
      endfor
      const original = device.agent.tools;
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["set_property"].target_obj;
      $test_utils:assert_true(is_anonymous(provider), "The bound collaborator is anonymous");
      $test_utils:assert_true(provider != $llm_programming_tools, "The shared prototype stores no interaction state");
      $test_utils:assert_eq(device.agent.tools["program_verb"].target_obj, provider, "One provider serves the domain catalog");
      add_property(target, "probe", "original", {actor, ""});
      add_property(session, "answer", "no", {$arch_wizard, ""});
      add_property(session, "reads", 0, {$arch_wizard, ""});
      add_property(session, "mutate", false, {$arch_wizard, ""});
      add_property(session, "probe_target", target, {$arch_wizard, ""});
      add_property(session, "replacement", other, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {
        "this.reads = this.reads + 1;",
        "if (this.mutate) this.probe_target.probe = this.replacement; endif",
        "return this.answer;"});
      const change = ["object" -> target, "property" -> "probe", "value" -> "new"];
      const cancelled = this:_as_programmer(provider, "set_property", {change, actor});
      $test_utils:assert_eq(cancelled.status, 'cancelled, "A declined mutation has a typed outcome");
      $test_utils:assert_eq(target.probe, "original", "Declining does not mutate the object");
      session.answer = "yes_all";
      const first = this:_as_programmer(provider, "set_property", {change, actor});
      $test_utils:assert_true(typeof(first) == TYPE_STR, "An accepted mutation succeeds");
      $test_utils:assert_eq(target.probe, "new", "Accepted JSON values are used directly");
      this:_as_programmer(provider, "set_property", {change, actor});
      $test_utils:assert_eq(session.reads, 2, "Repeated effect and target reuse only this interaction's approval");
      session.approvals = [];
      session.answer = "yes";
      session.mutate = true;
      target.probe = create($thing, actor, 1);
      const stale = this:_as_programmer(provider, "set_property", {change, actor});
      $test_utils:assert_eq(stale.status, 'waiting, "Changing anonymous object identity invalidates the preview");
      $test_utils:assert_eq(target.probe, other, "A stale approval cannot overwrite the intervening change");
      session.mutate = false;
      let denied = false;
      try
        this:_as_player(provider, "set_property", {change, actor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Another caller cannot impersonate the session actor");
      session:close();
      $test_utils:assert_eq(device.agent.tools, original, "Closing restores the original immutable catalog");
      $test_utils:assert_eq(provider.bound_session, #-1, "Closing drops the provider's session reference");
      let closed = false;
      try
        this:_as_programmer(provider, "set_property", {change, actor});
      except (E_INVARG)
        closed = true;
      endtry
      $test_utils:assert_true(closed, "A retained provider cannot become an unbound mutation path");
      this:_as_programmer($llm_programming_tools, "set_property", {change, actor});
      $test_utils:assert_eq(target.probe, "new", "Direct shared tools retain native actor authority without interactive input");
      device.agent.running = true;
      const callback = device.agent.tool_callback;
      let busy = false;
      try
        session = $llm_wearable_session:open(device, actor, 0);
      except (E_INVARG)
        busy = true;
      endtry
      $test_utils:assert_true(busy && device.agent.running, "Admission cannot take over a running agent");
      $test_utils:assert_eq(device.agent.tool_callback, callback, "Rejected admission preserves existing callbacks");
      $test_utils:assert_eq(device.interaction, #-1, "Rejected admission releases its own interaction");
      device.agent.running = false;
    finally
      valid(session) && session:close();
      actor.wearing = previous;
    endtry
    return true;
  endmethod

  method test_wearable_building_provider owner: ARCH_WIZARD
    "Every compass mutation waits for approval; accepted operations share the ordinary building services.";
    const actor = #90101;
    const previous = actor.wearing;
    const builder = actor.is_builder;
    const device = create($architects_compass, actor, 1);
    const area = create($area, actor, 2);
    const first = area:make_room_in($room, "approval source");
    const second = area:make_room_in($room, "approval destination");
    const third = area:make_room_in($room, "approval third");
    const target = create($thing, actor, 2);
    let session = #-1;
    let created = #-1;
    let built = #-1;
    try
      actor.is_builder = true;
      for configured_room in ({first, second, third})
        configured_room:set_owner(actor);
      endfor
      target.name = "approval object";
      actor.wearing = {@previous, device};
      $llm_building_tools:dig_passage(["source_room" -> first, "target_room" -> second,
        "direction" -> "north", "oneway" -> true], actor);
      const passages = $room:passages(first);
      const contents = area.contents;
      const inventory = actor.contents;
      const description = target.description;
      const integrated = target.integrated_description;
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["build_room"].target_obj;
      $test_utils:assert_true(is_anonymous(provider), "Compass building tools bind an anonymous provider");
      for tool in ($llm_building_tools:get_tools())
        $test_utils:assert_eq(device.agent.tools[tool.name].target_obj, provider, "Every building tool shares the bound provider");
      endfor
      add_property(session, "answer", "no", {$arch_wizard, ""});
      add_property(session, "reads", 0, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {"this.reads = this.reads + 1; return this.answer;"});
      const operations = {
        {"build_room", ["name" -> "approved room", "area" -> area]},
        {"dig_passage", ["source_room" -> first, "target_room" -> third, "direction" -> "east", "oneway" -> true]},
        {"remove_passage", ["source_room" -> first, "target_room" -> second]},
        {"set_passage_description", ["source_room" -> first, "direction" -> "north", "description" -> "approved passage"]},
        {"create_object", ["parent" -> $thing, "name" -> "approved creation"]},
        {"recycle_object", ["object" -> target]},
        {"rename_object", ["object" -> target, "name" -> "approved object"]},
        {"describe_object", ["object" -> target, "description" -> "approved description"]},
        {"move_object", ["object" -> target, "destination" -> second]},
        {"set_integrated_description", ["object" -> target, "integrated_description" -> "approved integration"]}
      };
      for operation in (operations)
        const result = this:_as_programmer(provider, operation[1], {operation[2], actor});
        $test_utils:assert_eq(result.status, 'cancelled, "Declined " + operation[1] + " returns a cancelled outcome");
      endfor
      $test_utils:assert_eq(session.reads, 10, "Every advertised building mutation asks for approval");
      $test_utils:assert_true(length(area.contents) == length(contents)
        && length({room for room in (area.contents) if !(room in contents)}) == 0, "Declined construction creates no room");
      $test_utils:assert_true(length(actor.contents) == length(inventory)
        && length({item for item in (actor.contents) if !(item in inventory)}) == 0, "Declined creation adds no object");
      $test_utils:assert_eq($room:passages(first), passages, "Declined passage changes preserve the connection");
      $test_utils:assert_eq({target.name, target.description, target.integrated_description, target.location},
        {"approval object", description, integrated, #-1}, "Declined object changes preserve native state");
      session.answer = "yes";
      for index in ({1, 2, 4, 5, 7, 8, 9, 10})
        const accepted = operations[index];
        const result = this:_as_programmer(provider, accepted[1], {accepted[2], actor});
        $test_utils:assert_true(typeof(result) == TYPE_STR, "Accepted " + accepted[1] + " succeeds");
      endfor
      const rooms = {built_room for built_room in (area.contents) if built_room.name == "approved room"};
      $test_utils:assert_eq(length(rooms), 1, "One approved room is created");
      built = rooms[1];
      const objects = {object for object in (actor.contents) if object.name == "approved creation"};
      $test_utils:assert_eq(length(objects), 1, "One approved object enters the actor's inventory");
      created = objects[1];
      $test_utils:assert_eq({target.name, target.description, target.integrated_description, target.location},
        {"approved object", "approved description", "approved integration", second}, "Approved object changes take effect");
      $test_utils:assert_eq(length($room:passages(first)), 2, "Approved digging creates a distinct connection");
      provider:remove_passage(operations[3][2], actor);
      $test_utils:assert_eq(length($room:passages(first)), 1, "Approved removal removes only the selected connection");
      provider:recycle_object(["object" -> created], actor);
      $test_utils:assert_false(valid(created), "Approved recycling destroys the selected object");
    finally
      valid(session) && session:close();
      actor.wearing = previous;
      actor.is_builder = builder;
      valid(created) && recycle(created);
      valid(target) && recycle(target);
      valid(built) && recycle(built);
      recycle(first);
      recycle(second);
      recycle(third);
      recycle(area);
    endtry
    return true;
  endmethod

  method test_wearable_building_stale_preview owner: ARCH_WIZARD
    "A changed connection cannot be overwritten or removed through a stale approval.";
    const actor = #90101;
    const previous = actor.wearing;
    const builder = actor.is_builder;
    const device = create($architects_compass, actor, 1);
    const area = create($area, actor, 2);
    const first = area:make_room_in($room, "stale source");
    const second = area:make_room_in($room, "stale destination");
    let session = #-1;
    try
      actor.is_builder = true;
      first:set_owner(actor);
      second:set_owner(actor);
      actor.wearing = {@previous, device};
      $llm_building_tools:dig_passage(["source_room" -> first, "target_room" -> second,
        "direction" -> "north", "oneway" -> true], actor);
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["set_passage_description"].target_obj;
      add_property(session, "source_room", first, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {
        "$llm_building_tools:set_passage_description([\"source_room\" -> this.source_room, \"direction\" -> \"north\", \"description\" -> \"intervening edit\"], this.requester);",
        "return \"yes_all\";"});
      const result = provider:set_passage_description(["source_room" -> first, "direction" -> "north", "description" -> "stale replacement"], actor);
      $test_utils:assert_eq(result.status, 'waiting, "Changed connection requires another decision");
      $test_utils:assert_true(index(toliteral($room:find_passage(first, "north")), "intervening edit"), "The intervening edit survives");
      $test_utils:assert_false(maphaskey(session.approvals[first], 'set_passage_description), "Stale yes-to-all approval is discarded");
      actor.is_builder = false;
      $test_utils:assert_raises(E_PERM, provider, "rename_object", {["object" -> first, "name" -> "revoked builder"], actor},
        "Builder revocation prevents another wearable mutation");
      session:close();
      let denied = false;
      try
        provider:remove_passage(["source_room" -> first, "target_room" -> second], actor);
      except (E_INVARG)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A retained building provider cannot mutate after close");
    finally
      valid(session) && session:close();
      actor.wearing = previous;
      actor.is_builder = builder;
      recycle(first);
      recycle(second);
      recycle(area);
    endtry
    return true;
  endmethod

  method test_building_creation_cleanup owner: ARCH_WIZARD
    "Failed object placement releases its allocation; a no-op destroy callback cannot report success.";
    const actor = #90101;
    const prototype = create($thing, actor, 1);
    const stubborn = create($thing, actor, 2);
    try
      add_verb(prototype, {actor, "rxd", "moveto"}, {"this", "none", "this"});
      set_verb_code(prototype, "moveto", {"return E_PERM;"});
      let denied = false;
      try
        this:_as_programmer($llm_building_tools, "create_object",
          {["parent" -> prototype, "name" -> "failed placement"], actor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Placement errors propagate");
      $test_utils:assert_eq(children(prototype), {}, "The newly allocated ordinary object is recycled on failure");
      add_verb(stubborn, {actor, "rxd", "destroy"}, {"this", "none", "this"});
      set_verb_code(stubborn, "destroy", {"return true;"});
      const result = this:_as_programmer($llm_building_tools, "recycle_object", {["object" -> stubborn], actor});
      $test_utils:assert_eq(result.status, 'unknown, "A callback that leaves the target alive cannot claim successful recycling");
      $test_utils:assert_true(valid(stubborn), "The postcondition detects the surviving object");
      const area = create($area, actor, 1);
      add_property(area, "started", false, {actor, ""});
      add_verb(area, {actor, "rxd", "make_room_in"}, {"this", "none", "this"});
      set_verb_code(area, "make_room_in", {
        "this.started = true; suspend(0); raise(E_PERM, \"Constructor failed after committing.\");"});
      denied = false;
      try
        this:_as_programmer($llm_building_tools, "build_room",
          {["name" -> "partial construction", "area" -> area], actor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(area.started && denied,
        "A constructor error propagates after effects; it is not a preflight denial result");
    finally
      valid(stubborn) && recycle(stubborn);
    endtry
    return true;
  endmethod

  method test_wearable_explicit_eval owner: ARCH_WIZARD
    "Raw eval is opt-in, programmer-only, and requires a fresh confirmation for every execution.";
    const actor = #90101;
    const previous = actor.wearing;
    const device = this:_wearable(actor);
    let session = #-1;
    try
      actor.wearing = {@previous, device};
      device:configure();
      const tool = $llm_programming_tools:get_eval_tool();
      device.agent:add_tool(tool.name, tool);
      $test_utils:assert_false("eval" in {entry.name for entry in ($llm_programming_tools:get_tools())}, "The default programming catalog excludes eval");
      $test_utils:assert_raises(E_PERM, $llm_programming_tools, "evaluate", {["code" -> "return 1;"], actor}, "Unbound eval cannot bypass interactive confirmation");
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["eval"].target_obj;
      add_property(session, "answer", "yes_all", {$arch_wizard, ""});
      add_property(session, "reads", 0, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {"this.reads = this.reads + 1; return this.answer;"});
      const request = ["code" -> "return {player, caller_perms()};", "rationale" -> "Check evaluation identity"];
      const cancelled = provider:evaluate(request, actor);
      $test_utils:assert_eq(cancelled.status, 'cancelled, "Eval never accepts yes to all");
      session.answer = "yes";
      for iteration in [1..2]
        $test_utils:assert_eq(provider:evaluate(request, actor), "Result: {#90101, #90101}", "Eval executes as the captured programmer");
      endfor
      $test_utils:assert_eq(session.reads, 3, "Every eval call reads a new decision");
      const failed = provider:evaluate(["code" -> "raise(E_INVARG);", "rationale" -> "Check failure outcome"], actor);
      $test_utils:assert_eq(failed.status, 'unknown, "An eval runtime failure cannot establish that no effects occurred");
    finally
      valid(session) && session:close();
      actor.wearing = previous;
    endtry
    return true;
  endmethod

  method test_message_edit_authority owner: ARCH_WIZARD
    "Message helpers authenticate actors and cannot turn a reference into write authority.";
    const actor = #90101;
    const target = create($thing, actor, 1);
    const bag = create($msg_bag, $arch_wizard, 1);
    bag.entries = {$sub_utils:compile("original")};
    add_property(target, "shared_msgs", bag, {actor, ""});
    for operation in ({
        {"set_message_template", ["object" -> target, "property" -> "shared_msgs", "template" -> "changed"]},
        {"add_message_template", ["object" -> target, "property" -> "shared_msgs", "template" -> "added"]},
        {"delete_message_template", ["object" -> target, "property" -> "shared_msgs", "index" -> 1]}})
      let denied = false;
      try
        this:_as_programmer($llm_behavior_tools, operation[1], {operation[2], actor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Owning a reference does not authorize changing a foreign message bag");
    endfor
    $test_utils:assert_eq($sub_utils:decompile(bag.entries[1]), "original", "Denied edits preserve the bag");
    const foreign = create($thing, #90102, 1);
    foreign.r = 1;
    add_property(foreign, "protected_msgs", $msg_bag:mk("original"), {#90102, "r"});
    const proxy = create($llm_wearable, actor, 1);
    add_verb(proxy, {actor, "rxd", "forward"}, {"this", "none", "this"});
    set_verb_code(proxy, "forward", {"const {name, values} = args; return $obj_utils:(name)(@values);"});
    for operation in ({
        {"set_compiled_message", {foreign, "protected_msgs", {"changed"}, actor}},
        {"add_message_entry", {foreign, "protected_msgs", {"added"}, actor}},
        {"add_message_entry", {foreign, "new_msgs", {"added"}, actor}},
        {"remove_message_entry", {foreign, "protected_msgs", 1, actor}},
        {"set_compiled_message", {foreign, "protected_msgs", {"forged"}, #90102}}})
      let denied = false;
      try
        this:_as_programmer(proxy, "forward", operation);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Wearable ancestry cannot grant foreign writes or actor impersonation");
    endfor
    $test_utils:assert_eq(foreign.protected_msgs:entries(), {"original"}, "Foreign target remains unchanged");
    $test_utils:assert_eq($obj_utils:property_key(foreign, "new_msgs"), E_PROPNF, "No foreign property is defined");
    set_property_info(bag, "entries", {actor, "r"});
    this:_as_programmer($llm_behavior_tools, "add_message_template",
      {["object" -> target, "property" -> "shared_msgs", "template" -> "authorized"], actor});
    $test_utils:assert_eq(length(bag.entries), 2, "The actual property owner can edit a referenced bag");
    set_property_info(foreign, "protected_msgs", {#90102, "rw"});
    this:_as_programmer(proxy, "forward", {"add_message_entry",
      {foreign, "protected_msgs", {"public entry"}, actor}});
    $test_utils:assert_eq(length(foreign.protected_msgs:entries()), 2,
      "An explicitly writable property remains editable without owning its object");
    const late = create($thing, actor, 1);
    late.r = 1;
    add_property(late, "late_msg", {"original"}, {actor, "r"});
    add_verb(late, {$arch_wizard, "rxd", "message_bindings"}, {"this", "none", "this"});
    set_verb_code(late, "message_bindings", {
      "this.owner = #90102; set_property_info(this, \"late_msg\", {#90102, \"r\"});",
      "suspend(0); return false;"});
    let denied = false;
    try
      this:_as_programmer($llm_behavior_tools, "set_message_template",
        {["object" -> late, "property" -> "late_msg", "template" -> "stale write"], actor});
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Compilation callbacks cannot retain revoked write authority");
    $test_utils:assert_eq(late.late_msg, {"original"}, "The denied stale write leaves the value intact");
    return true;
  endmethod

  method test_wearable_behavior_stale_preview owner: ARCH_WIZARD
    "Message contents and authoring state remain stable across compilation and confirmation.";
    const actor = #90101;
    const previous = actor.wearing;
    const builder = actor.is_builder;
    const device = create($architects_compass, actor, 1);
    const host = create($thing, actor, 2);
    const bag = create($msg_bag, actor, 1);
    let session = #-1;
    try
      actor.is_builder = true;
      actor.wearing = {@previous, device};
      bag.entries = {$sub_utils:compile("original")};
      add_property(host, "replies_msgs", bag, {actor, ""});
      session = $llm_wearable_session:open(device, actor, 0);
      const provider = device.agent.tools["set_rule"].target_obj;
      add_property(session, "subject", host, {$arch_wizard, ""});
      add_property(session, "reads", 0, {$arch_wizard, ""});
      add_property(session, "mode", 'bag, {$arch_wizard, ""});
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {
        "this.reads = this.reads + 1;",
        "if (this.mode == 'bag)",
        "  this.subject.replies_msgs.entries = {$sub_utils:compile(\"intervening\")};",
        "elseif (this.mode == 'state)",
        "  $reaction:declare_state(this.subject, 'intervening, 'integer, 1);",
        "endif",
        "suspend(0); return this.mode == 'bag ? \"yes_all\" | \"yes\";"});
      const request = ["object" -> host, "property" -> "replies_msgs", "template" -> "added"];
      const stale = this:_as_programmer(provider, "add_message_template", {request, actor});
      $test_utils:assert_eq(stale.status, 'waiting, "Changed object-bag entries invalidate approval");
      $test_utils:assert_eq($sub_utils:decompile(bag.entries[1]), "intervening", "The intervening edit survives");
      session.mode = 'yes;
      this:_as_programmer(provider, "add_message_template", {request, actor});
      $test_utils:assert_eq(session.reads, 2, "Stale yes-to-all approval is discarded");
      $test_utils:assert_eq(length(bag.entries), 2, "Fresh approval appends to the current bag");
      add_verb(host, {actor, "rxd", "message_bindings"}, {"this", "none", "this"});
      set_verb_code(host, "message_bindings", {
        "this.replies_msgs.entries = {$sub_utils:compile(\"compile changed\")}; suspend(0); return false;"});
      const compiled = this:_as_programmer(provider, "set_message_template", {request, actor});
      $test_utils:assert_eq(compiled.status, 'waiting, "Compilation changes invalidate the precomputed proposal");
      $test_utils:assert_eq(session.reads, 2, "An already-stale proposal does not ask for approval");
      session.mode = 'state;
      const authored = this:_as_programmer(provider, "author_reaction",
        {["command" -> "state", "text" -> tostr(host) + " visits number 0"], actor});
      $test_utils:assert_eq(session.reads, 3, "Authoring invokes its confirmation callback");
      $test_utils:assert_true(maphaskey(host.reaction_state_fields, 'intervening), "The concurrent state edit actually ran");
      $test_utils:assert_eq(authored.status, 'waiting, "Changed authoring state invalidates approval");
      $test_utils:assert_false(maphaskey(host.reaction_state_fields, 'visits), "The stale state declaration is absent");
    finally
      valid(session) && session:close();
      actor.wearing = previous;
      actor.is_builder = builder;
      recycle(host);
    endtry
    return true;
  endmethod

  method test_observer_triage_provider_and_decision owner: ARCH_WIZARD
    "Triage uses the dedicated provider, visible decisions, current authority, and an unchanged snapshot.";
    const observer = create($llm_room_observer, #90102, 2);
    const agent = create($llm_agent, #90102, 2);
    const engage = ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "ENGAGE"]]}];
    const client = this:_client({engage});
    try
      add_property(client, "actor", #-1, {#90102, ""});
      add_property(client, "purpose", 'none, {#90102, ""});
      set_verb_code(client, "chat", {"this.actor = caller_perms(); this.purpose = args[6];", @verb_code(client, "chat")});
      observer.agent = agent;
      agent.client = client;
      agent:add_message("system", "Test observer.");
      agent:add_message("user", "OBSERVATION: A visitor asks for directions.");
      $test_utils:assert_true(this:_as_player(observer, "triage", {}), "Dedicated provider works without a global key");
      $test_utils:assert_eq(length(client.requests), 1, "Triage makes one accounted call");
      $test_utils:assert_eq(client.actor, observer, "The provider receives the NPC actor");
      $test_utils:assert_eq(client.purpose, 'triage, "The provider receives the triage purpose");
      for text in ({"IGNORE", "IGNORE then ENGAGE", "ENGAGEMENT", "<think>ENGAGE</think>", ""})
        const reply = ["choices" -> {["message" -> ["role" -> "assistant", "content" -> text,
          "reasoning_content" -> "ENGAGE"]]}];
        $test_utils:assert_false($llm_observer_policy:decision(reply), "Only an explicit visible decision engages");
      endfor
      $test_utils:assert_false($llm_observer_policy:decision([]), "Malformed replies decline");
      $test_utils:assert_false($llm_observer_policy:decision(this:_tool_reply({"emote"})), "Triage cannot request tools");
      const original_context = agent:get_context();
      let large = "x";
      for width in [1..16]
        large = large + large;
      endfor
      agent:set_context({["role" -> "user", "content" -> "OBSERVATION: " + large]});
      let oversized = false;
      try
        this:_as_player(observer, "triage", {});
      except (E_QUOTA)
        oversized = true;
      endtry
      $test_utils:assert_true(oversized, "Oversized observations fail before provider work");
      $test_utils:assert_eq(length(client.requests), 1, "Oversized triage makes no calls");
      agent:set_context(original_context);
      client.append_to = agent;
      $test_utils:assert_false(this:_as_player(observer, "triage", {}), "New observations invalidate the decision");
      let denied = false;
      try
        this:_as_programmer(observer, "triage", {});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A foreign owner cannot spend the NPC account");
      observer.enabled = false;
      const count = length(client.requests);
      $test_utils:assert_false(this:_as_player(observer, "triage", {}), "Disabled observers decline");
      $test_utils:assert_eq(length(client.requests), count, "Disabled observers make no calls");
    finally
      for temporary in ({client, agent, observer})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method test_observer_speech_policy owner: ARCH_WIZARD
    "Speech preserves ordinary text, suppresses tool markup, and remains in its captured room.";
    const observer = create($llm_room_observer, #90102, 2);
    const destination = create($room, #90102, 2);
    const elsewhere = create($room, #90102, 2);
    try
      add_property(destination, "heard", {}, {#90102, ""});
      add_property(destination, "principal", #-1, {#90102, ""});
      add_verb(destination, {#90102, "rxd", "announce"}, {"this", "none", "this"});
      set_verb_code(destination, "announce", {"this.heard = {@this.heard, args[1]}; this.principal = caller_perms();"});
      move(observer, destination);
      destination.heard = {};
      $test_utils:assert_true(this:_as_player(observer, "_process_and_announce", {"Done building your room."}), "Done can begin legitimate speech");
      $test_utils:assert_eq(destination.principal, observer, "Speech uses the NPC principal");
      $test_utils:assert_true(this:_as_player(observer, "_process_and_announce", {"As a guide, I can explain DSML."}), "Ordinary words are preserved");
      const emitted = length(destination.heard);
      $test_utils:assert_false(this:_as_player(observer, "_process_and_announce", {"<｜DSML｜invoke name=\"directed_say\"><parameter name=\"message\">Injected speech</parameter></｜DSML｜invoke>"}), "Leaked markup cannot produce speech");
      $test_utils:assert_eq(length(destination.heard), emitted, "Leaked markup creates no event");
      $test_utils:assert_eq($llm_observer_speech:mk("<think>private reasoning</think> Hello.").text, "Hello.", "Reasoning blocks are excluded");
      $test_utils:assert_eq($llm_observer_speech:mk("<think>unfinished").text, "", "Unfinished reasoning is silent");
      $test_utils:assert_eq($llm_observer_speech:mk("SILENT").text, "", "Explicit silence is preserved");
      $test_utils:assert_eq($llm_observer_speech:mk("\"Le Spleen it is.\"").text, "Le Spleen it is.",
        "A reply wrapped in quotes is not quoted twice");
      $test_utils:assert_eq($llm_observer_speech:mk("\u201CGood evening.\u201D").text, "Good evening.",
        "Curly wrapping quotes are removed too");
      $test_utils:assert_eq($llm_observer_speech:mk("\"Yes,\" he said, \"go.\"").text, "\"Yes,\" he said, \"go.\"",
        "Quotes inside the reply are kept");
      let large = "x";
      for width in [1..14]
        large = large + large;
      endfor
      let oversized = false;
      try
        $llm_observer_speech:mk(large);
      except (E_QUOTA)
        oversized = true;
      endtry
      $test_utils:assert_true(oversized, "Oversized speech fails before publication");
      const speech = $llm_observer_speech:mk("I will wait with you.");
      $test_utils:assert_eq(speech.text, "I will wait with you.", "Helpful speech is not filtered by phrases");
      move(observer, elsewhere);
      $test_utils:assert_false(speech:announce(observer, destination), "Moving the observer invalidates the captured destination");
      $test_utils:assert_eq(length(destination.heard), emitted, "No late speech reaches the old room");
      move(observer, destination);
      observer.enabled = false;
      $test_utils:assert_false(speech:announce(observer, destination), "A disabled observer stays silent");
      let denied = false;
      try
        this:_as_programmer(observer, "_process_and_announce", {"forged speech"});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Foreign callers cannot impersonate the NPC");
    finally
      for temporary in ({observer, destination, elsewhere})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method test_observer_event_admission owner: ARCH_WIZARD
    "Capture canonical events and render attributed input only in an owned observer job.";
    const {observer, room, client, agent} = this:_observer_fixture();
    const sibling = create($llm_room_observer, #90102, 2);
    const speaker = create($actor, #90102, 2);
    try
      observer.name = "FirstGuide";
      sibling.name = "SecondGuide";
      move(sibling, room);
      move(speaker, room);
      $test_utils:assert_true($llm_observer_policy:ready(observer), "Dedicated clients work without global configuration");
      const event = $event:mk_say(speaker, "FirstGuide, where is the exit?");
      const captured = $llm_observation:event(observer, event);
      let denied = false;
      try
        captured:render();
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Rendering requires the owned worker even for a wizard");
      const job = captured:submit();
      this:_await_observer(job);
      $test_utils:assert_eq(job.state, 'complete, "Name-addressed input reaches the model");
      $test_utils:assert_eq(length(client.requests), 1, "Addressing bypasses triage");
      let record = [];
      for message in (client.requests[1])
        if (message["role"] == "user" && message["content"]:starts_with("OBSERVATION: "))
          record = parse_json(message["content"][14..$]);
        endif
      endfor
      $test_utils:assert_eq(record["actor"], tostr(speaker), "The captured speaker remains attributed data");
      $test_utils:assert_eq(record["action"], "say", "The action survives rendering");
      $test_utils:assert_true(index(record["text"], "where is the exit?") > 0, "Rendered text becomes event data");
      $test_utils:assert_false($llm_observation:event(observer, $event:mk_say(observer, "my own speech")), "Self-events are ignored");
      $test_utils:assert_false($llm_observation:event(observer, $event:mk_say(sibling, "other NPC speech")), "Paired NPCs do not trigger each other");
      const other = $llm_observation:event(observer, $event:mk_say(speaker, "SecondGuide, hello")):submit();
      this:_await_observer(other);
      $test_utils:assert_eq(length(client.requests), 1, "Speech naming another NPC is ignored before a model call");
      $test_utils:assert_false($llm_observation:event(observer, event:with_iobj(sibling)), "Explicit conversations with someone else are ignored");
      $test_utils:assert_false($llm_observation:event(observer, event:with_location(#-1)), "Events from elsewhere are ignored");
      $test_utils:assert_false($llm_observation:event(observer, []), "Unstructured event input is ignored");
      let large = "x";
      for width in [1..14]
        large = large + large;
      endfor
      $test_utils:assert_false($llm_observation:event(observer, $event:mk_say(speaker, large)), "Oversized events are rejected before rendering");
      for target in ({$llm_observer_policy, $llm_observation})
        denied = false;
        try
          this:_as_programmer(target, target == $llm_observer_policy ? "ready" | "event",
            target == $llm_observer_policy ? {observer} | {observer, event});
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "Foreign callers cannot use the NPC as a client or rendering principal");
      endfor
      observer.enabled = false;
      $test_utils:assert_false($llm_observer_policy:ready(observer), "Disabled observers do not select providers");
      $test_utils:assert_false($llm_observation:event(observer, event), "Disabled observers admit no events");
      $test_utils:assert_true(is_clear_property($mr_welcome, "agent"), "Mr. Welcome starts without a dumped agent");
    finally
      $llm_observer_runtime:stop(observer);
      for temporary in ({client, observer, sibling, speaker, room})
        recycle(temporary);
      endfor
    endtry
    return true;
  endmethod

  method _observer_fixture owner: ARCH_WIZARD
    "Create an isolated observer with a blocking provider and a captured public event sink.";
    caller == this && this == #90031 || raise(E_PERM);
    const {?prototype = $llm_room_observer} = args;
    const observer = create(prototype, #90102, 2);
    const room = create($root, #90102, 2);
    const agent = create($llm_agent, #90102, 1);
    const client = this:_client({["choices" -> {["message" -> ["role" -> "assistant", "content" -> "Hello."]]}]});
    observer.agent = agent;
    observer.speak_delay = 0;
    observer.thinking_delay = 30;
    agent.client = client;
    add_property(client, "blocked", false, {#90102, ""});
    add_property(client, "actors", {}, {#90102, ""});
    set_verb_code(client, "chat", {
      "this.requests = {@this.requests, args[1]};",
      "this.actors = {@this.actors, caller_perms()};",
      "while (this.blocked) suspend(0.1); endwhile",
      "return this.replies[1];"});
    add_property(room, "events", {}, {#90102, ""});
    add_property(room, "blocked", false, {#90102, ""});
    add_property(room, "entered", false, {#90102, ""});
    add_verb(room, {#90102, "rxd", "announce"}, {"this", "none", "this"});
    set_verb_code(room, "announce", {
      "this.entered = true;",
      "while (this.blocked) suspend(0.1); endwhile",
      "this.events = {@this.events, args[1]};"});
    move(observer, room);
    return {observer, room, client, agent};
  endmethod

  method _await_observer owner: ARCH_WIZARD
    "Wait for bounded runtime retirement and fail if the lifecycle stalls.";
    caller == this && this == #90031 || raise(E_PERM);
    const {job} = args;
    for attempt in [1..60]
      job.retired && return;
      suspend(0.1);
    endfor
    raise(E_ASSERT, "Observer job did not retire.");
  endmethod

  method test_observer_serializes_asks_and_events owner: ARCH_WIZARD
    "Room events and direct questions share one execution order and the NPC principal.";
    const {observer, room, client, agent} = this:_observer_fixture();
    try
      client.blocked = true;
      const first = this:_as_player(observer, "request_response", {"first question"});
      for attempt in [1..30]
        if (length(client.requests) == 1)
          break;
        endif
        suspend(0.1);
      endfor
      $test_utils:assert_eq(length(client.requests), 1, "First request reaches the provider: " + toliteral(observer.runtime.history));
      const runtime = observer.runtime;
      const event_job = $llm_observation:event(observer, $event:mk_say(#90102, "visitor asks a question"):with_location(room):with_iobj(observer)):submit();
      const second = this:_as_player(observer, "request_response", {"second question"});
      suspend(0.2);
      $test_utils:assert_eq(length(client.requests), 1, "Queued input cannot race the active conversation");
      client.blocked = false;
      this:_await_observer(second);
      $test_utils:assert_true(first.retired && event_job.retired, "FIFO predecessors retire first");
      $test_utils:assert_eq(client.actors, {observer, observer, observer}, "All three calls bill and act as the NPC");
      $test_utils:assert_eq(length(room.events), 3, "Each completed turn emits one public reply");
      $test_utils:assert_eq(second.state, 'complete, "Direct ask completes through the shared job");
      $test_utils:assert_false(valid(agent.tool_callback) || valid(agent.compaction_callback), "Idle agent retains no job callbacks");
      $test_utils:assert_false(valid(runtime.current) || valid_task(runtime.monitor_task), "Idle runtime releases active work");
      $test_utils:assert_false(valid(second.host) || valid(second.agent) || valid(second.presenter), "Retired job releases its object graph");
    finally
      recycle(observer);
      recycle(room);
      recycle(client);
    endtry
    return true;
  endmethod

  method test_observer_cancel_rebind_and_dead_worker owner: ARCH_WIZARD
    "Cancellation and invalidated bindings kill owned model tasks without retrying effects.";
    for change in ({'stop, 'move, 'owner, 'agent, 'client, 'model, 'role, 'recycle, 'worker})
      const {observer, room, client, agent} = this:_observer_fixture();
      try
        client.blocked = true;
        const job = this:_as_player(observer, "request_response", {"waiting request"});
        const runtime = observer.runtime;
        for attempt in [1..30]
          if (length(client.requests) == 1)
            break;
          endif
          suspend(0.1);
        endfor
        $test_utils:assert_eq(length(client.requests), 1, "Model request reached the barrier");
        const worker = job.worker_task;
        const parent_task = job.interaction_task;
        const thinking = job.thinking_task;
        if (change == 'stop)
          this:_as_player($llm_observer_runtime, "stop", {observer});
        elseif (change == 'move)
          move(observer, #-1);
        elseif (change == 'owner)
          observer.owner = #90101;
        elseif (change == 'agent)
          observer.agent = create($llm_agent, #90102, 1);
        elseif (change == 'client)
          agent.client = $llm_client;
        elseif (change == 'model)
          agent.model_override = "changed";
        elseif (change == 'role)
          observer.programmer = !observer.programmer;
        elseif (change == 'recycle)
          valid(observer) && recycle(observer);
        else
          kill_task(parent_task);
        endif
        this:_await_observer(job);
        $test_utils:assert_eq(job.state, 'cancelled, "Invalidated work is cancelled: " + tostr(change));
        $test_utils:assert_false(valid_task(worker) || valid_task(parent_task) || valid_task(thinking), "All owned workers stop");
        $test_utils:assert_false(agent.running, "Killed model releases the conversation lock");
        client.blocked = false;
        suspend(0.1);
        $test_utils:assert_eq(length(room.events), 0, "Cancelled work cannot publish a late reply");
        $test_utils:assert_eq(length(client.requests), 1, "Cancellation does not replay the request");
      finally
        valid(observer) && recycle(observer);
        recycle(room);
        recycle(client);
      endtry
    endfor
    return true;
  endmethod

  method test_observer_configuration owner: ARCH_WIZARD
    "Observer configuration preserves its owner and selects the client model unless explicitly overridden.";
    const {observer, room, client, agent} = this:_observer_fixture();
    let persona = #-1;
    try
      this:_await_observer(this:_as_player(observer, "request_response", {"question"}));
      const runtime = observer.runtime;
      observer.enabled = false;
      this:_as_player(observer, "reconfigure", {});
      $test_utils:assert_true(runtime.closed, "Reconfiguration closes the previous runtime");
      $test_utils:assert_false(valid(runtime.subject) || valid(runtime.agent), "Old runtime releases its configuration");
      $test_utils:assert_true(observer.agent != agent, "Reconfiguration creates a fresh conversation");
      $test_utils:assert_eq(observer.agent.owner, #90102, "Agent is allocated under the controller's ownership");
      $test_utils:assert_eq(observer.agent.model_override, false, "An unset observer preference uses the client model");
      observer.preferred_model = "explicit-observer-model";
      this:_as_player(observer, "reconfigure", {});
      $test_utils:assert_eq(observer.agent.model_override, "explicit-observer-model", "An explicit observer model remains local to its agent");
      persona = create($mr_welcome, #90102, 2);
      this:_as_player(persona, "configure", {});
      $test_utils:assert_eq(persona.agent.owner, #90102, "Persona hooks configure an ordinary owner's agent");
      $test_utils:assert_true(maphaskey(persona.agent.tools, "remember_fact"), "Persona retains its useful catalog");
    finally
      valid(persona) && recycle(persona);
      recycle(observer);
      recycle(room);
      recycle(client);
    endtry
    return true;
  endmethod
  method test_observer_partial_effect_requires_recovery owner: ARCH_WIZARD
    "Cancelling a suspended mutation retains its unknown outcome and prevents automatic replay.";
    const {observer, room, client, agent} = this:_observer_fixture();
    try
      add_property(room, "effects", 0, {#90102, ""});
      add_verb(room, {#90102, "rxd", "mutate"}, {"this", "none", "this"});
      set_verb_code(room, "mutate", {"this.effects = this.effects + 1; suspend(30); return true;"});
      agent:add_tool("mutate", $llm_agent_tool:mk("mutate", "Test mutation", [], room, "mutate", 'mutation));
      client.replies = {this:_tool_reply({"mutate"})};
      const job = this:_as_player(observer, "request_response", {"one mutation"});
      for attempt in [1..30]
        if (room.effects == 1)
          break;
        endif
        suspend(0.1);
      endfor
      $test_utils:assert_eq(room.effects, 1, "Mutation committed before cancellation");
      this:_as_player($llm_observer_runtime, "stop", {observer});
      $test_utils:assert_true(job.recovery_required, "Cancellation preserves unknown tool effects");
      $test_utils:assert_eq(length(agent.pending_tools), 1, "Unresolved outcome stays on the conversation");
      const next = this:_as_player(observer, "request_response", {"try again"});
      this:_await_observer(next);
      $test_utils:assert_eq(next.state, 'failed, "A new request cannot resume unresolved tools");
      $test_utils:assert_eq(length(client.requests), 1, "Recovery never automatically replays the provider request");
      $test_utils:assert_eq(room.effects, 1, "The committed effect is not repeated");
    finally
      recycle(observer);
      recycle(room);
      recycle(client);
    endtry
    return true;
  endmethod

  method test_observer_shared_social_model_loop owner: ARCH_WIZARD
    "The real observer tool loop dispatches shared social and memory tools as the NPC.";
    const {observer, room, client, agent} = this:_observer_fixture($mr_welcome);
    try
      observer:_setup_agent(agent);
      client.replies = {
        ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> {
          ["id" -> "social", "type" -> "function", "function" -> ["name" -> "emote",
            "arguments" -> ["action" -> "waves in greeting."]]],
          ["id" -> "memory", "type" -> "function", "function" -> ["name" -> "remember_fact",
            "arguments" -> ["subject" -> "visitor", "fact" -> "likes the garden"]]]
        }]]}],
        ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "SILENT"]]}]
      };
      set_verb_code(client, "chat", {
        "this.requests = {@this.requests, args[1]};",
        "this.actors = {@this.actors, caller_perms()};",
        "const reply = this.replies[1];",
        "if (length(this.replies) > 1) this.replies = listdelete(this.replies, 1); endif",
        "return reply;"});
      const job = this:_as_player(observer, "request_response", {"greet and remember"});
      this:_await_observer(job);
      $test_utils:assert_eq(job.state, 'complete, "Shared social tools finish through the actual model loop");
      $test_utils:assert_eq(client.actors, {observer, observer}, "Both calls keep the NPC actor");
      $test_utils:assert_eq(length(room.events), 3, "Emote, thinking, and notebook gestures remain useful output");
      for event in (room.events)
        $test_utils:assert_eq(event.actor, observer, "All persona output attributes the NPC");
        $test_utils:assert_eq(event.location, room, "All persona output keeps the captured room");
      endfor
      $test_utils:assert_eq(length(observer.memory:recall("visitor")), 1, "The memory tool records its effect");
    finally
      recycle(observer);
      recycle(room);
      recycle(client);
    endtry
    return true;
  endmethod
endobject
