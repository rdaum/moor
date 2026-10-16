object LLM_WEARABLE_SESSION [
  import_export_id -> "llm_wearable_session",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Wearable Interaction"
  parent: LLM_JOB
  owner: ARCH_WIZARD

  property base_tools (owner: ARCH_WIZARD, flags: "") = [];
  property tools_bound (owner: ARCH_WIZARD, flags: "") = false;
  property tool_providers (owner: ARCH_WIZARD, flags: "") = {};
  property approvals (owner: ARCH_WIZARD, flags: "") = [];
  property actor_builder (owner: ARCH_WIZARD, flags: "") = false;
  property actor_wizard (owner: ARCH_WIZARD, flags: "") = false;
  property actor_programmer (owner: ARCH_WIZARD, flags: "") = false;
  property active (owner: ARCH_WIZARD, flags: "") = false;
  property device_owner (owner: ARCH_WIZARD, flags: "") = #-1;
  property interaction_task (owner: ARCH_WIZARD, flags: "") = 0;
  property monitor_task (owner: ARCH_WIZARD, flags: "") = 0;

  method open owner: ARCH_WIZARD
    "Admit one interaction before configuration or input; descendants get independent state.";
    const principal = caller_perms();
    const {device, actor, destination} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    isa(device, $llm_wearable) && (actor == device.owner || actor.wizard) || raise(E_PERM);
    $llm_wearable_session:_eligible(device, actor);
    if (!is_clear_property(device, "interaction") && valid(device.interaction))
      const previous = device.interaction;
      if (previous.active && valid_task(previous.interaction_task))
        raise(E_INVARG, "This device already has an active interaction.");
      endif
      previous:close();
    endif
    const timeout = device.interaction_timeout;
    typeof(timeout) == TYPE_INT && timeout >= 1 && timeout <= 3600 || raise(E_INVARG);
    const session = $llm_wearable_session:new_run(device, actor, destination);
    session.device_owner = device.owner;
    session.actor_programmer = actor.programmer;
    session.actor_builder = actor.is_builder;
    session.actor_wizard = actor.wizard;
    session.interaction_task = task_id();
    session.deadline = time() + timeout;
    session.active = true;
    device.interaction = session;
    return session:_prepare();
  endmethod

  method _prepare owner: ARCH_WIZARD
    "Configure and bind the admitted interaction, releasing it if setup fails.";
    caller == $llm_wearable_session && caller_perms().wizard || raise(E_PERM);
    const device = this.host;
    const actor = this.requester;
    this:_start_monitor();
    try
      this:_check_frontend();
      if (is_clear_property(device, "agent") || !valid(device.agent)
          || device.agent.owner != device.owner)
        device:configure();
      endif
      $llm_wearable_session:check(this, actor);
      this:bind_agent(device.agent);
      this:_bind_tools();
      this:_register_common_tools(this.agent);
      return this;
    except error (ANY)
      this:close();
      raise(error[1], error[2]);
    endtry
  endmethod

  method _bind_tools owner: ARCH_WIZARD
    "Bind shared domain providers once per interaction and retain the original immutable catalog.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    this.base_tools = this.agent.tools;
    this.tools_bound = true;
    let tools = this.base_tools;
    let providers = [];
    for name in (mapkeys(tools))
      const tool = tools[name];
      const prototype = tool.target_obj;
      if (prototype in {$llm_programming_tools, $llm_building_tools, $llm_behavior_tools})
        if (!maphaskey(providers, prototype))
          const provider = $llm_tool_provider:bind(prototype, this, this.requester);
          providers[prototype] = provider;
          this.tool_providers = {@this.tool_providers, provider};
        endif
        tools[name] = $llm_agent_tool:mk(tool.name, tool.description, tool.parameters,
          providers[prototype], tool.target_verb, tool.effect);
      endif
    endfor
    this.agent.tools = tools;
  endmethod

  method _eligible owner: ARCH_WIZARD
    "Check current possession without invoking frontend behavior.";
    caller_perms().wizard || raise(E_PERM);
    const {device, actor} = args;
    const worn = device in actor.wearing;
    const carried = !device.requires_wearing_only && device in actor.contents;
    worn || carried || raise(E_PERM, "Wear the device before using it.");
  endmethod

  method _check_frontend owner: ARCH_WIZARD
    "Run frontend eligibility inside the monitored interaction task.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const actor = this.requester;
    const device = this.host;
    set_task_perms(actor);
    device:_check_user_eligible(actor);
  endmethod

  method check owner: ARCH_WIZARD
    "Recheck captured authority, device binding, deadline, and connection after suspension.";
    const {session, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    isa(session, $llm_wearable_session) && session.active || raise(E_INVARG, "Interaction closed.");
    actor == session.requester && actor.programmer == session.actor_programmer
      && actor.is_builder == session.actor_builder && actor.wizard == session.actor_wizard || raise(E_PERM);
    const device = session.host;
    $llm_wearable_session:_eligible(device, actor);
    valid(device) && device.owner == session.device_owner || raise(E_PERM);
    device.interaction == session || raise(E_PERM);
    actor == device.owner || actor.wizard || raise(E_PERM);
    if (valid(session.agent))
      device.agent == session.agent && session.agent.owner == session.device_owner || raise(E_PERM);
    endif
    time() < session.deadline || raise(E_QUOTA, "Interaction timed out.");
    if (typeof(session.reply_connection) == TYPE_OBJ)
      const attached = {entry[1] for entry in (connections(actor))};
      session.reply_connection in attached || raise(E_PERM, "Connection is no longer attached.");
    endif
    return true;
  endmethod

  method for_device owner: ARCH_WIZARD
    "Return only the live interaction bound to this device and actor.";
    const {device, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    !is_clear_property(device, "interaction") && valid(device.interaction)
      || raise(E_INVARG, "No active interaction.");
    const session = device.interaction;
    session.host == device || raise(E_PERM);
    $llm_wearable_session:check(session, actor);
    return session;
  endmethod

  method _start_monitor owner: ARCH_WIZARD
    "Fork outside cleanup scopes so input, model work, and abandoned tasks share a deadline.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    fork monitor (0)
      while (this.active)
        const usable = `$llm_wearable_session:check(this, this.requester) ! ANY => false';
        if (!valid_task(this.interaction_task) || !usable)
          this:close();
          return;
        endif
        suspend(1);
      endwhile
    endfork
    this.monitor_task = monitor;
  endmethod

  method read_input owner: ARCH_WIZARD
    "Read bounded input from the captured connection and recheck the interaction on return.";
    const {actor, metadata} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    typeof(this.reply_connection) == TYPE_OBJ || raise(E_INVARG, "Input requires a connection.");
    value_bytes(metadata) <= 16384 || raise(E_QUOTA);
    const response = read(this.reply_connection, metadata);
    $llm_wearable_session:check(this, actor);
    typeof(response) == TYPE_STR && length(response) <= 16384 || raise(E_INVARG);
    return response;
  endmethod

  method emit owner: ARCH_WIZARD
    "Send one bounded structured event to the captured connection.";
    const {actor, event} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    value_bytes(event) <= 65536 || raise(E_QUOTA);
    const destination = this.reply_connection;
    typeof(destination) == TYPE_OBJ || return false;
    set_task_perms(actor);
    actor:inform_connection(destination, event);
    $llm_wearable_session:check(this, actor);
    return true;
  endmethod

  method emit_for_device owner: ARCH_WIZARD
    "Route frontend tool output through its captured interaction.";
    const {device, actor, event} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    const session = $llm_wearable_session:for_device(device, actor);
    return session:emit(actor, event);
  endmethod

  method confirm owner: ARCH_WIZARD
    "Read one scoped approval; providers own preview validation and native effect snapshots.";
    const {actor, effect, target, prompt, ?allow_all = true} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    typeof(effect) == TYPE_SYM && length(tostr(effect)) <= 128 || raise(E_INVARG);
    valid(target) && typeof(allow_all) == TYPE_BOOL || raise(E_INVARG);
    const owner = target.owner;
    let scopes = maphaskey(this.approvals, target) ? this.approvals[target] | [];
    if (allow_all && maphaskey(scopes, effect) && scopes[effect] == owner)
      return true;
    endif
    const input_type = allow_all ? "yes_no_alternative_all" | "yes_no_alternative";
    const suffix = allow_all
      ? " Yes to All applies only to this operation on this object, for this request." | "";
    const response = this:read_input(actor, {{"input_type", input_type},
      {"prompt", prompt + suffix}, {"alternative_label", "Suggest an alternative:"},
      {"alternative_placeholder", "Describe the change you want."}});
    $llm_wearable_session:check(this, actor);
    if (!valid(target) || target.owner != owner)
      return $llm_tool_result:mk('waiting, "Target ownership changed while awaiting confirmation.", "stale_confirmation");
    endif
    if (response == "yes")
      return true;
    elseif (response == "yes_all" && allow_all)
      maphaskey(this.approvals, target) || length(this.approvals) < 128
        || raise(E_QUOTA, "Too many approval targets.");
      scopes = maphaskey(this.approvals, target) ? this.approvals[target] | [];
      maphaskey(scopes, effect) || length(scopes) < 32
        || raise(E_QUOTA, "Too many approval operations on this target.");
      scopes[effect] = owner;
      this.approvals[target] = scopes;
      return true;
    elseif (index(response, "alternative: ") == 1)
      return $llm_tool_result:mk('waiting, response[14..$], "user_alternative");
    endif
    this.agent.cancel_requested = true;
    return $llm_tool_result:mk('cancelled, "User cancelled.", "user_cancelled");
  endmethod

  method forget_approval owner: ARCH_WIZARD
    "Discard an approval when the provider detects a changed preview.";
    const {actor, effect, target} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    if (maphaskey(this.approvals, target))
      this.approvals[target] = `mapdelete(this.approvals[target], effect) ! E_RANGE => this.approvals[target]';
    endif
  endmethod

  method interact owner: ARCH_WIZARD
    "Run a prompted request and at most three explicit continuations on the shared job executor.";
    caller == this.host && caller_perms().wizard || raise(E_PERM);
    const actor = this.requester;
    const device = this.host;
    try
      let prompt = this:read_input(actor, {{"input_type", "text"},
        {"prompt", device.prompt_label + " " + device.prompt_text},
        {"placeholder", device.placeholder_text}, {"tts_prompt", device.prompt_text}}):trim();
      if (!prompt || prompt == "@abort")
        return;
      endif
      this.query = prompt;
      this:_notify("Query received: " + prompt);
      let continuations = 0;
      while (true)
        $llm_wearable_session:check(this, actor);
        this:run(prompt, this.deadline - time());
        if (typeof(this.result) != TYPE_ERR || error_code(this.result) != E_QUOTA)
          break;
        endif
        if (continuations >= 3 || length(this.agent.pending_tools))
          break;
        endif
        const answer = this:read_input(actor, {{"input_type", "yes_no"},
          {"prompt", "Iteration limit reached. Continue this request?"}});
        if (answer != "yes")
          break;
        endif
        continuations = continuations + 1;
        this.state = 'queued;
        prompt = "Continue where you left off. Complete the remaining work from this request.";
      endwhile
      this:report();
      device:_show_token_usage(actor, this);
    finally
      this:close();
    endtry
  endmethod

  method stop owner: ARCH_WIZARD
    "Cancel this device's interaction without acting on an inherited prototype's state.";
    const principal = caller_perms();
    const {device, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    isa(device, $llm_wearable) && (actor == device.owner || actor.wizard) || raise(E_PERM);
    if (!is_clear_property(device, "interaction") && valid(device.interaction))
      device.interaction.host == device || raise(E_PERM);
      device.interaction:close();
    endif
  endmethod

  method close owner: ARCH_WIZARD
    "Cancel owned tasks and drop references while retaining the device's conversation.";
    const principal = caller_perms();
    (caller == this || principal == this.requester || principal.wizard) || raise(E_PERM);
    this.active || return;
    this.active = false;
    const device = this.host;
    const agent = this.agent;
    const interaction_task = this.interaction_task;
    const monitor_task = this.monitor_task;
    this.state in {'queued, 'running} && this:cancel();
    if (interaction_task > 0 && interaction_task != task_id())
      `kill_task(interaction_task) ! E_INVARG';
    endif
    if (monitor_task > 0 && monitor_task != task_id())
      `kill_task(monitor_task) ! E_INVARG';
    endif
    if (valid(agent))
      agent.running = false;
      agent.current_iteration = 0;
      if (this.tools_bound)
        agent.tools = this.base_tools;
      endif
    endif
    if (valid(device) && !is_clear_property(device, "interaction") && device.interaction == this)
      device.interaction = #-1;
    endif
    for provider in (this.tool_providers)
      $llm_tool_provider:release(provider);
    endfor
    this.tool_providers = {};
    this.base_tools = [];
    this.tools_bound = false;
    this:release();
    this.approvals = [];
    this.device_owner = #-1;
    this.interaction_task = 0;
    this.monitor_task = 0;
  endmethod

  method _notify owner: ARCH_WIZARD
    "Send progress only while the captured actor, device, and connection remain valid.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {message} = args;
    $llm_wearable_session:check(this, this.requester);
    return $llm_presenter:send(this.presenter, this.requester, message);
  endmethod

  method on_tool_call owner: ARCH_WIZARD
    "Render frontend-specific progress for the captured requester.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    let {name, parameters} = args;
    name == "explain" && return;
    $llm_wearable_session:check(this, this.requester);
    typeof(parameters) == TYPE_STR && (parameters = parse_json(parameters));
    const actor = this.requester;
    const device = this.host;
    set_task_perms(actor);
    const message = device:_format_hud_message(name, parameters);
    const speech = device:_format_tts_message(name, parameters);
    this:emit(actor, $event:mk_info(actor, message):as_inset():with_tts(speech)
      :with_group('llm, device));
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Report typed successful completion without retaining arguments or tool output.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    const {name, parameters, content} = args;
    name == "explain" && return;
    const text = typeof(content) == TYPE_STR ? content | toliteral(content);
    const preview = length(text) > 100 ? text[1..100] + "..." | text;
    this:_notify(name + ": " + preview);
  endmethod

  method _tool_explain owner: ARCH_WIZARD
    "Tool: Communicate reasoning, progress updates, or error details to user";
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    const device = this.host;
    set_task_perms(actor);
    const message = args_map["message"];
    typeof(message) == TYPE_STR || raise(E_TYPE("Expected message string"));
    this:emit(actor, $event:mk_info(actor, message):as_inset():with_group('llm, device));
    return "Message delivered to user.";
  endmethod

  method _tool_ask_user owner: ARCH_WIZARD
    "Read one answer on the captured connection. Unrecognized choices never count as approval.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_wearable_session:check(this, actor);
    const agent = this.agent;
    let kind = `parameters["input_type"] ! E_RANGE => "choice"';
    let choices = `parameters["choices"] ! E_RANGE => {"Accept", "Stop", "Request Change"}';
    if (maphaskey(parameters, "choices"))
      kind = "choice";
      choices = setadd(choices, "Stop");
    endif
    let metadata = {{"input_type", kind}, {"prompt", parameters["question"]}};
    if (kind == "choice")
      metadata = {@metadata, {"choices", choices}};
    elseif (kind == "text_area")
      metadata = {@metadata, {"rows", `parameters["rows"] ! E_RANGE => 4'}};
    endif
    if (maphaskey(parameters, "placeholder"))
      metadata = {@metadata, {"placeholder", parameters["placeholder"]}};
    endif
    const response = this:read_input(actor, metadata);
    if (!response || response == "@abort" || (kind == "choice" && response == "Stop"))
      agent.cancel_requested = true;
      return $llm_tool_result:mk('cancelled, "User cancelled.", "user_cancelled");
    endif
    if (kind == "choice")
      response in choices || raise(E_INVARG, "Answer is not an offered choice.");
      if (response == "Request Change" && !maphaskey(parameters, "choices"))
        const change = this:read_input(actor, {{"input_type", "text_area"},
          {"prompt", "What changes would you like?"}, {"rows", 4}}):trim();
        if (!change || change == "@abort")
          agent.cancel_requested = true;
          return $llm_tool_result:mk('cancelled, "User cancelled.", "user_cancelled");
        endif
        return "User requested changes: " + change;
      endif
      return "User chose: " + response;
    elseif (kind in {"yes_no", "confirmation", "yes_no_alternative"})
      if (response == "yes")
        return "User accepted.";
      elseif (kind == "yes_no_alternative" && index(response, "alternative: ") == 1)
        return "User requested changes: " + response[14..$];
      endif
      agent.cancel_requested = true;
      return $llm_tool_result:mk('cancelled, "User cancelled.", "user_cancelled");
    endif
    return "User response: " + response;
  endmethod

  method _register_common_tools owner: ARCH_WIZARD
    "Bind bounded interaction tools to this session, independently of the device's domain catalog.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {agent} = args;
    const text = ["type" -> "string", "maxLength" -> 4096];
    const tools = {
      $llm_agent_tool:mk("explain", "Give the user a concise progress update or finding.",
        ["type" -> "object", "properties" -> ["message" -> text],
          "required" -> {"message"}, "additionalProperties" -> false], this, "_tool_explain", 'presentation),
      $llm_agent_tool:mk("ask_user", "Ask a question. Without choices or an input type, offer Accept, Stop, or Request Change.",
        ["type" -> "object", "properties" -> ["question" -> text,
          "choices" -> ["type" -> "array", "items" -> text, "minItems" -> 1, "maxItems" -> 16],
          "input_type" -> ["type" -> "string", "enum" -> {"text", "text_area", "yes_no", "confirmation", "yes_no_alternative"}],
          "placeholder" -> text, "rows" -> ["type" -> "integer", "minimum" -> 1, "maximum" -> 20]],
          "required" -> {"question"}, "additionalProperties" -> false], this, "_tool_ask_user", 'input)
    };
    for tool in (tools)
      agent:add_tool(tool.name, tool);
    endfor
    const project_tools = $llm_tool_provider:bind($llm_project_tools, this, this.requester);
    this.tool_providers = {@this.tool_providers, project_tools};
    for tool in (project_tools:get_tools())
      agent:add_tool(tool.name, tool);
    endfor
  endmethod

endobject
