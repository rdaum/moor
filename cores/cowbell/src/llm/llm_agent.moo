object LLM_AGENT [
  import_export_id -> "llm_agent",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Agent"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  property all_failed_iterations (owner: HACKER, flags: "rc") = 0;
  property cancel_requested (owner: HACKER, flags: "rc") = false;
  property chat_opts (owner: HACKER, flags: "rc") = false;
  property client (owner: HACKER, flags: "rc") = #-1;
  property compaction_callback (owner: HACKER, flags: "rc") = #-1;
  property compaction_threshold (owner: ARCH_WIZARD, flags: "r") = 0.7;
  property consecutive_tool_failures (owner: HACKER, flags: "rc") = [];
  property project_state (owner: ARCH_WIZARD, flags: "") = #-1;
  property conversation (owner: ARCH_WIZARD, flags: "") = #-1;
  property current_continuation (owner: HACKER, flags: "rc") = 0;
  property current_iteration (owner: HACKER, flags: "rc") = 0;
  property last_token_usage (owner: HACKER, flags: "c") = [];
  property max_consecutive_failures (owner: ARCH_WIZARD, flags: "r") = 3;
  property max_iterations (owner: ARCH_WIZARD, flags: "r") = 50;
  property min_messages_to_keep (owner: HACKER, flags: "c") = 15;
  property system_prompt (owner: HACKER, flags: "rc") = "";
  property token_limit (owner: ARCH_WIZARD, flags: "r") = 128000;
  property model_override (owner: HACKER, flags: "rc") = false;
  property tool_callback (owner: HACKER, flags: "rc") = #-1;
  property execution_status (owner: ARCH_WIZARD, flags: "c") = 'idle;
  property running (owner: ARCH_WIZARD, flags: "c") = false;
  property pending_tools (owner: ARCH_WIZARD, flags: "c") = [];
  property tool_outcomes (owner: ARCH_WIZARD, flags: "c") = [];
  property max_tools_per_turn (owner: ARCH_WIZARD, flags: "rc") = 32;
  property tools (owner: HACKER, flags: "c") = [];
  property total_tokens_used (owner: HACKER, flags: "rc") = 0;

  override description (owner: HACKER, flags: "rc") = "Prototype for LLM-powered agents. Maintains conversation context and executes tool calls.";

  method initialize owner: ARCH_WIZARD
    "Create independent conversation state once, under the new agent's owner's authority.";
    $llm_agent:_challenge_permissions(this, caller_perms());
    is_clear_property(this, "conversation") || raise(E_INVARG, "Agent is already initialized");
    pass();
    this.client = $llm_client;
    this.conversation = $llm_conversation:new(this);
    this.project_state = $llm_project:new(this);
  endmethod

  method add_tool owner: ARCH_WIZARD
    "Register one canonical descriptor; replacing a different definition requires explicit removal.";
    const {tool_name, definition} = args;
    const principal = caller_perms();
    $llm_agent:_challenge_permissions(this, principal);
    set_task_perms(principal);
    const tool = $llm_tool_catalog:normalize(definition);
    typeof(tool_name) == TYPE_STR && strcmp(tool_name, tool.name) == 0 || raise(E_INVARG, "Tool name must match its descriptor");
    if (maphaskey(this.tools, tool_name))
      strcmp(toliteral(this.tools[tool_name]), toliteral(tool)) == 0 || raise(E_INVARG, "Conflicting tool: " + tool_name);
    endif
    this.tools[tool_name] = tool;
  endmethod

  method remove_tool owner: ARCH_WIZARD
    "Unregister a tool and release its consecutive-failure counter.";
    $llm_agent:_challenge_permissions(this, caller_perms());
    const {tool_name} = args;
    typeof(tool_name) != TYPE_STR && raise(E_TYPE, "tool_name must be string");
    this.tools = mapdelete(this.tools, tool_name);
    this.consecutive_tool_failures = mapdelete(this.consecutive_tool_failures, tool_name);
  endmethod

  method get_context owner: ARCH_WIZARD
    "Return the conversation snapshot under the caller's current authority.";
    set_task_perms(caller_perms());
    return $llm_agent:context_snapshot(this);
  endmethod

  method context_snapshot owner: ARCH_WIZARD
    "Read canonical conversation state without running agent hooks during job cancellation.";
    const {agent} = args;
    $llm_agent:_challenge_permissions(agent, caller_perms());
    return agent.conversation:snapshot();
  endmethod

  method set_context owner: ARCH_WIZARD
    "Install a validated conversation while idle; active or unresolved work cannot be replaced.";
    $llm_agent:_challenge_permissions(this, caller_perms());
    !this.running && length(this.pending_tools) == 0 || raise(E_INVARG, "Conversation is active or needs recovery");
    const {messages} = args;
    return this.conversation:replace(messages);
  endmethod

  method add_message owner: ARCH_WIZARD
    "Append a validated message to this agent's conversation.";
    $llm_agent:_challenge_permissions(this, caller_perms());
    return this.conversation:append(@args);
  endmethod

  method _get_tool_schemas owner: ARCH_WIZARD
    "Get OpenAI-format tool schemas from registered tools. Internal only.";
    caller == this || raise(E_PERM);
    return {$llm_tool_catalog:normalize(this.tools[k]):to_schema() for k in (mapkeys(this.tools))};
  endmethod

  method _find_tool owner: ARCH_WIZARD
    "Find a registered tool by name. Returns flyweight or #-1 if not found. Internal only.";
    caller == this || raise(E_PERM);
    const {tool_name} = args;
    if (maphaskey(this.tools, tool_name) && strcmp(this.tools[tool_name].name, tool_name) == 0)
      return this.tools[tool_name];
    endif
    return #-1;
  endmethod

  method _request_model owner: ARCH_WIZARD
    "Request one captured conversation snapshot; the provider owns bounded retries and accounting.";
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {schemas, opts, actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    const options = typeof(opts) == TYPE_FLYWEIGHT ? opts | this.chat_opts;
    return $llm_response:mk($llm_provider:chat_as(this.client, actor, 'conversation,
      this.conversation:snapshot(), options, this.model_override, false, schemas));
  endmethod

  method _track_token_usage owner: ARCH_WIZARD
    "Keep conversation diagnostics. Only the provider boundary charges the actor's account.";
    caller == this || raise(E_PERM);
    const {response, @rest} = args;
    const usage = $llm_account:usage(response:usage());
    if (typeof(usage) != TYPE_MAP)
      this.last_token_usage = [];
      return;
    endif
    this.last_token_usage = usage;
    this.total_tokens_used = this.total_tokens_used + usage["total_tokens"];
  endmethod

  method _execute_tool_call owner: ARCH_WIZARD
    "Execute a declared call and retain its typed outcome separately from provider text.";
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {call, actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    const name = call["function"]["name"];
    const parameters = call["function"]["arguments"];
    const id = call["id"];
    const tool = this:_find_tool(name);
    const registered = typeof(tool) == TYPE_FLYWEIGHT;
    const failures = registered && maphaskey(this.consecutive_tool_failures, name)
      ? this.consecutive_tool_failures[name] | 0;
    let outcome = false;
    if (this.cancel_requested)
      outcome = $llm_tool_result:mk('cancelled, "Operation cancelled.", "cancelled");
    elseif (failures >= this.max_consecutive_failures)
      outcome = $llm_tool_result:mk('failure, "Repeated tool failure; inspect the error before retrying.", "tool_blocked");
    else
      if (!registered)
        outcome = $llm_tool_result:mk('failure, "Tool not found.", "unknown_tool");
      else
        let started = false;
        try
          const validated = $llm_tool_schema:arguments(parameters, tool.parameters);
          if (valid(this.tool_callback) && respond_to(this.tool_callback, 'on_tool_call))
            $llm_agent:_invoke_request_callback(this, actor, "on_tool_call", {name, parameters});
          endif
          if (this.cancel_requested)
            outcome = $llm_tool_result:mk('cancelled, "Operation cancelled.", "cancelled");
          else
            started = true;
            outcome = $llm_tool_result:normalize($llm_agent:_dispatch_tool(this, tool, validated, actor));
          endif
        except e (ANY)
          "A throwing handler may already have committed effects across a suspension.";
          const effect = `tool.effect ! E_PROPNF => 'unknown';
          const detail = tostr(e[2]);
          const code = tostr(e[1]);
          outcome = $llm_tool_result:mk(started && effect != 'inspection ? 'unknown | 'failure,
            detail[1..min(length(detail), 2048)], code[1..min(length(code), 256)]);
        endtry
      endif
    endif
    this.tool_outcomes[id] = outcome;
    const index = this.pending_tools[id];
    this.conversation:record_tool(index, id, outcome:message(id, name));
    if (!(outcome.status in {'unknown, 'waiting}))
      this.pending_tools = mapdelete(this.pending_tools, id);
    endif
    if (outcome:is_success())
      if (maphaskey(this.consecutive_tool_failures, name))
        this.consecutive_tool_failures = mapdelete(this.consecutive_tool_failures, name);
      endif
    elseif (registered && outcome.status in {'failure, 'unknown} && this:_find_tool(name) == tool)
      this.consecutive_tool_failures[name] = min(failures + 1, this.max_consecutive_failures);
    elseif (outcome.status == 'cancelled)
      this.cancel_requested = true;
    endif
    const message = outcome:message(id, name);
    const callback_name = outcome:is_success() ? "on_tool_complete" | "on_tool_error";
    if (valid(this.tool_callback) && respond_to(this.tool_callback, callback_name))
      try
        $llm_agent:_invoke_request_callback(this, actor, callback_name, {name, parameters, message["content"]});
      except (ANY)
        this:_log("Tool result callback failed: " + callback_name);
      endtry
    endif
    return message;
  endmethod

  method run_turn owner: ARCH_WIZARD
    "Expose one conversation turn while preventing concurrent conversation execution.";
    const principal = caller_perms();
    const {?opts = false, ?actor = principal} = args;
    $llm_agent:_challenge_permissions(this, principal);
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    const external = caller != this;
    !external || !this.running || raise(E_INVARG, "Conversation already running");
    if (external)
      length(this.pending_tools) == 0 || raise(E_INVARG, "Unresolved tool outcomes require recovery");
      this.running = true;
    endif
    try
      return this:_run_turn(opts, actor);
    finally
      if (external)
        this.running = false;
      endif
    endtry
  endmethod

  method _run_turn owner: ARCH_WIZARD
    "Record the assistant call and pending outcomes before invoking any tool that can commit.";
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {opts, actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    const response = this:_request_model(this:_get_tool_schemas(), opts, actor);
    this:_track_token_usage(response, actor);
    this.cancel_requested && return ["status" -> "cancelled"];
    response:is_valid() || return ["status" -> "invalid"];
    if (!response:has_tool_calls())
      const content = response:content();
      typeof(content) == TYPE_STR && length(content:trim()) > 0 || return ["status" -> "invalid"];
      this:add_message("assistant", content);
      return ["status" -> "complete", "content" -> content];
    endif
    const calls = response:tool_calls();
    length(calls) <= this.max_tools_per_turn || raise(E_QUOTA, "Too many tools in one turn");
    this.tool_outcomes = [];
    const pending = this.conversation:begin_exchange(response:message());
    this.pending_tools = pending;
    let results = {};
    let all_failed = true;
    let waiting = false;
    let halted = false;
    for call in (calls)
      let message = [];
      if (halted)
        const outcome = $llm_tool_result:mk('cancelled, "Not executed because an earlier tool requires recovery.", "recovery_required");
        this.tool_outcomes[call["id"]] = outcome;
        this.pending_tools = mapdelete(this.pending_tools, call["id"]);
        message = outcome:message(call["id"], call["function"]["name"]);
      else
        message = this:_execute_tool_call(call, actor);
        halted = this.tool_outcomes[call["id"]].status in {'unknown, 'waiting};
      endif
      if (this.tool_outcomes[call["id"]].status == 'waiting)
        waiting = true;
      endif
      const index = pending[call["id"]];
      this.conversation:record_tool(index, call["id"], message);
      results = {@results, message};
      if (this.tool_outcomes[call["id"]]:is_success())
        all_failed = false;
      endif
    endfor
    this.cancel_requested && return ["status" -> "cancelled", "tool_results" -> results];
    waiting && return ["status" -> "waiting", "tool_results" -> results];
    length(this.pending_tools) > 0 && return ["status" -> "unknown", "tool_results" -> results];
    return ["status" -> all_failed ? "all_failed" | "tool_calls", "tool_results" -> results];
  endmethod

  method send_message owner: ARCH_WIZARD
    "Run the common bounded turn protocol and always clear execution status on exit.";
    const principal = caller_perms();
    const {input, ?opts = false, ?actor = principal} = args;
    $llm_agent:_challenge_permissions(this, principal);
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    typeof(input) == TYPE_STR || raise(E_TYPE);
    !this.running || raise(E_INVARG, "Conversation already running");
    length(this.pending_tools) == 0 || raise(E_INVARG, "Unresolved tool outcomes require recovery");
    const limit = this.max_iterations;
    typeof(limit) == TYPE_INT && limit > 0 && limit <= 1000 || raise(E_INVARG, "max_iterations must be between 1 and 1000");
    this.running = true;
    this.execution_status = 'running;
    this.cancel_requested = false;
    this.all_failed_iterations = 0;
    try
      this:needs_compaction() && this:compact_context(actor);
      this:add_message("user", input);
      for iteration in [1..limit]
        this.current_iteration = iteration;
        if (this.cancel_requested)
          this.execution_status = 'cancelled;
          return "Operation cancelled.";
        endif
        const budget = this:_check_token_budget(actor);
        if (typeof(budget) == TYPE_STR)
          this.execution_status = 'budget_exhausted;
          return budget;
        endif
        if (iteration > 1 && this:needs_compaction())
          this:compact_context(actor);
        endif
        const turn = this:run_turn(opts, actor);
        const status = turn["status"];
        if (status == "complete")
          this.execution_status = 'complete;
          return turn["content"];
        elseif (status == "cancelled")
          this.execution_status = 'cancelled;
          return "Operation cancelled.";
        elseif (status == "invalid")
          this.execution_status = 'failed;
          return "Error: Invalid model response.";
        elseif (status == "unknown")
          this.execution_status = 'unknown;
          return "Error: Tool outcome is unknown. Inspect committed work before retrying.";
        elseif (status == "waiting")
          this.execution_status = 'waiting;
          return "Operation is waiting. Inspect its status before continuing.";
        endif
        if (status == "all_failed")
          this.all_failed_iterations = this.all_failed_iterations + 1;
          if (this.all_failed_iterations >= 3)
            this.execution_status = 'failed;
            return "Error: Three consecutive turns failed.";
          endif
          this:add_message("system", "The tools failed. Inspect their errors and explain the blocker; do not repeat an unchanged failing call.");
        else
          this.all_failed_iterations = 0;
        endif
      endfor
      this.execution_status = 'limit_exceeded;
      return E_QUOTA("Maximum iterations exceeded");
    finally
      if (this.execution_status == 'running)
        this.execution_status = length(this.pending_tools) > 0 ? 'unknown | 'failed;
      endif
      this.current_iteration = 0;
      this.running = false;
      this.cancel_requested = false;
    endtry
  endmethod

  method reset_context owner: ARCH_WIZARD
    "Clear context and rebuild from system prompt";
    $llm_agent:_challenge_permissions(this, caller_perms());
    !this.running || raise(E_INVARG, "Cancel execution before resetting context");
    this.conversation:replace(this.system_prompt ? {["role" -> "system", "content" -> this.system_prompt]} | {});
    this.pending_tools = [];
    this.tool_outcomes = [];
    this.last_token_usage = [];
    this.total_tokens_used = 0;
    this.consecutive_tool_failures = [];
    this.all_failed_iterations = 0;
  endmethod

  method needs_compaction owner: ARCH_WIZARD
    "Check if context needs compaction based on token usage";
    $llm_agent:_challenge_permissions(this, caller_perms());
    this.conversation:needs_compaction() && return true;
    typeof(this.last_token_usage) != TYPE_MAP && return false;
    !maphaskey(this.last_token_usage, "prompt_tokens") && return false;
    return tofloat(this.last_token_usage["prompt_tokens"]) > tofloat(this.token_limit) * this.compaction_threshold;
  endmethod

  method compact_context owner: ARCH_WIZARD
    "Summarize complete turns without replacing a conversation changed during suspension.";
    const principal = caller_perms();
    $llm_agent:_challenge_permissions(this, principal);
    const {?request_principal = principal} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    length(this.pending_tools) == 0 || raise(E_INVARG, "Cannot compact unresolved tool outcomes");
    const conversation = this.conversation;
    const plan = conversation:prepare_compaction(this.min_messages_to_keep);
    typeof(plan) == TYPE_MAP || return false;
    const callback = this.compaction_callback;
    if (valid(callback) && respond_to(callback, 'on_compaction_start))
      `$llm_agent:_invoke_request_callback(this, request_principal,
        "on_compaction_start", {}, callback) ! ANY';
    endif
    try
      const prompt = "Summarize this conversation, preserving decisions, completed actions, "
        + "unresolved work, and relevant references:\n\n" + toliteral(plan['old]);
      const raw = $llm_provider:chat_as(this.client, request_principal, 'compaction,
        {@plan['prefix], ["role" -> "user", "content" -> prompt]}, this.chat_opts, this.model_override, false, false);
      const response = $llm_response:mk(raw);
      this:_track_token_usage(response);
      if (!response:is_valid() || !response:content())
        this:_log("Context summary was invalid; original conversation retained");
        return;
      endif
      $llm_agent:_challenge_permissions(this, principal);
      this.conversation == conversation || return false;
      const applied = conversation:apply_summary(plan, response:content());
      !applied && this:_log("Conversation changed during compaction; stale summary discarded");
      return applied;
    finally
      if (valid(callback) && respond_to(callback, 'on_compaction_end))
        `$llm_agent:_invoke_request_callback(this, request_principal,
          "on_compaction_end", {}, callback) ! ANY';
      endif
    endtry
  endmethod

  method _challenge_permissions owner: ARCH_WIZARD
    "Check the public entry's captured principal. Object names, ancestry, and gifted ownership grant no caller authority.";
    const {subject, principal} = args;
    valid(principal) || raise(E_PERM);
    principal == subject.owner || principal.wizard || raise(E_PERM);
    return principal;
  endmethod

  method _log owner: ARCH_WIZARD
    "Log message to server log. Internal method - only callable by agent itself.";
    caller == this || raise(E_PERM);
    const {message} = args;
    set_task_perms(this.owner, {{"builtin_call", "server_log"}});
    server_log(message);
  endmethod

  method _check_token_budget owner: ARCH_WIZARD
    "Check the captured actor's remaining capacity, including NPC accounts and reservations.";
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    $llm_account:available(actor) > 0 && return true;
    return "Error: LLM token budget exceeded.";
  endmethod

  method get_project owner: ARCH_WIZARD
    "Return this agent's private project collaborator under current owner authority.";
    $llm_agent:_challenge_permissions(this, caller_perms());
    !is_clear_property(this, "project_state") && valid(this.project_state) || raise(E_INVARG, "Agent has no project state.");
    return this.project_state;
  endmethod

  method reset_tool_failures owner: ARCH_WIZARD
    "Reset consecutive failure counts, optionally for a specific tool.";
    let failures = 0;
    caller == this || caller_perms().wizard || caller_perms() == this.owner || raise(E_PERM);
    const {?tool_name = ""} = args;
    if (tool_name == "")
      this.consecutive_tool_failures = [];
    else
      failures = this.consecutive_tool_failures;
      maphaskey(failures, tool_name) && (this.consecutive_tool_failures = mapdelete(failures, tool_name));
    endif
  endmethod

  method send_message_no_tools owner: ARCH_WIZARD
    "Run a text-only request with the same actor, accounting, and conversation exclusion rules.";
    const actor = caller_perms();
    $llm_agent:_challenge_permissions(this, actor);
    !this.running && length(this.pending_tools) == 0 || raise(E_INVARG, "Conversation is active or needs recovery.");
    const {input, ?opts = false} = args;
    this.running = true;
    try
      this:needs_compaction() && this:compact_context(actor);
      this:add_message("user", input);
      const options = typeof(opts) == TYPE_FLYWEIGHT ? opts | this.chat_opts;
      const response = $llm_response:mk($llm_provider:chat_as(this.client, actor, 'no_tools,
        this.conversation:snapshot(), options, this.model_override, false, false));
      this:_track_token_usage(response);
      response:is_valid() && !response:has_tool_calls() || raise(E_INVARG, "Invalid text-only LLM response.");
      this:add_message("assistant", response:content());
      return response:content();
    finally
      this.running = false;
    endtry
  endmethod

  verb test_internal_permissions (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Test that internal methods reject external callers.";
    let test_args = 0;
    let verb_name = 0;
    const agent = $llm_agent:create(true);
    agent.owner = $hacker;
    "List of internal methods to test with their args";
    const llm_resp = $llm_response;
    const tests = {{"_log", {"test"}}, {"_get_tool_schemas", {}}, {"_find_tool", {"nonexistent"}}, {"_check_token_budget", {$hacker}}, {"_request_model", {{}, false, $hacker}}, {"_execute_tool_call", {["id" -> "x", "function" -> ["name" -> "x", "arguments" -> []]]}}, {"_track_token_usage", {llm_resp:mk([])}}};
    for test in (tests)
      {verb_name, test_args} = test;
      try
        agent:(verb_name)(@test_args);
        raise(E_ASSERT, verb_name + " should have rejected external caller");
      except e (E_PERM)
        "Expected - permission denied";
      except e (ANY)
        raise(E_ASSERT, verb_name + " raised wrong error: " + toliteral(e));
      endtry
    endfor
    return true;
  endverb

  verb test_public_method_wizard_access (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Wizard callers can read guarded LLM project state and reset tool failures.";
    const agent = $llm_agent:create(true);
    agent.owner = $hacker;
    const status = agent:get_project():get_task_status();
    typeof(status) != TYPE_LIST && raise(E_ASSERT, "get_task_status should return list");
    const todos = agent:get_project():get_todos();
    typeof(todos) != TYPE_LIST && raise(E_ASSERT, "get_todos should return list");
    agent.consecutive_tool_failures = ["test_tool" -> 1];
    agent:reset_tool_failures();
    agent.consecutive_tool_failures == [] || raise(E_ASSERT, "reset_tool_failures should clear counts");
    return true;
  endverb

  method _dispatch_tool owner: ARCH_WIZARD
    "Dispatch under the authenticated requester. Billing metadata cannot grant actor authority.";
    const {subject, tool, tool_args, principal} = args;
    caller == subject || raise(E_PERM);
    const incoming = caller_perms();
    incoming == principal || incoming.wizard || raise(E_PERM);
    set_task_perms(principal);
    return tool:execute(tool_args, principal);
  endmethod

  method _invoke_request_callback owner: ARCH_WIZARD
    "External callbacks receive requester permissions; the parent keeps internal state authority.";
    const {subject, principal, callback_verb, callback_args, ?callback = subject.tool_callback} = args;
    caller == subject || raise(E_PERM);
    const incoming = caller_perms();
    incoming == principal || incoming.wizard || raise(E_PERM);
    set_task_perms(principal);
    if (typeof(callback) == TYPE_OBJ && isa(callback, $llm_job))
      return $llm_job:deliver_callback(callback, subject, callback_verb, callback_args);
    endif
    return callback:(callback_verb)(@callback_args);
  endmethod
endobject
