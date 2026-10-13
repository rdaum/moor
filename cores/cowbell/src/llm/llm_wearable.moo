object LLM_WEARABLE [
  import_export_id -> "llm_wearable",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM-Powered Wearable"
  parent: WEARABLE
  owner: HACKER
  fertile: true
  readable: true

  property agent (owner: HACKER, flags: "r") = #-1;
  property auto_confirm (owner: HACKER, flags: "rc") = 0;
  property placeholder_text (owner: HACKER, flags: "rc") = "Ask a question...";
  property preferred_model (owner: HACKER, flags: "rc") = "";
  property processing_message (owner: HACKER, flags: "rc") = "Processing request...";
  property progress_steps (owner: HACKER, flags: "rc") = [];
  property prompt_label (owner: HACKER, flags: "rc") = "[TOOL]";
  property prompt_text (owner: HACKER, flags: "rc") = "Enter your query:";
  property requires_wearing_only (owner: HACKER, flags: "rc") = true;
  property tool_name (owner: HACKER, flags: "rc") = "TOOL";

  override description (owner: HACKER, flags: "rc") = "Base prototype for AI-powered wearable tools that use LLM agents for interactive assistance.";

  method configure owner: ARCH_WIZARD
    "Create agent and apply configuration. Children override _setup_agent to customize.";
    caller == this || caller == this.owner || caller.wizard || raise(E_PERM);
    "Reset auto_confirm mode on reconfigure";
    this.auto_confirm = false;
    "Create anonymous agent - GC'd when no longer referenced";
    this.agent = $llm_agent:create(true);
    "Set agent owner to tool owner so they can write to agent properties";
    this.agent.owner = this.owner;
    "Downgrade perms after wiz-level property writes are done";
    set_task_perms(this.owner);
    "Set model if preferred_model is configured";
    if (this.preferred_model)
      this.agent.client.model = this.preferred_model;
    endif
    "Let child class configure it";
    this:_setup_agent(this.agent);
  endmethod

  method _setup_agent owner: HACKER
    "Override in child objects to configure agent with specific system prompt and tools";
    const {agent} = args;
    raise(E_VERBNF, "Child objects must override :_setup_agent to configure their agent");
  endmethod

  method _action_perms_check owner: ARCH_WIZARD
    "Check tool is accessible and return wearer. Caller must set_task_perms(wearer).";
    let candidate = 0;
    let carried = 0;
    let worn = 0;
    const {?actor = #-1} = args;
    let wearer = this:wearer();
    if (!valid(wearer))
      "Use the explicit actor or invoking principal for carried tools.";
      candidate = valid(actor) ? actor | caller_perms();
      if (valid(candidate))
        if (respond_to(candidate, 'is_wearing) && `candidate:is_wearing(this) ! ANY => false')
          wearer = candidate;
        elseif (is_member(this, `candidate.contents ! ANY => {}'))
          wearer = candidate;
        endif
      endif
    endif
    if (!valid(wearer))
      "Fallback to command context player";
      wearer = player;
      valid(wearer) || raise(E_PERM, "Tool must be worn or in inventory to use");
      worn = respond_to(wearer, 'is_wearing) && `wearer:is_wearing(this) ! ANY => false';
      carried = is_member(this, `wearer.contents ! ANY => {}');
      worn || carried || raise(E_PERM, "Tool must be worn or in inventory to use");
    endif
    "Check user eligibility - override _check_user_eligible in children for custom requirements";
    this:_check_user_eligible(wearer);
    return wearer;
  endmethod

  method _require_tool_dispatch owner: ARCH_WIZARD
    "Validate the entry's captured principal before privileged tool effects. Caller ancestry does not grant authority.";
    const {actor, principal} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM, "Tool actor must be valid");
    principal == actor || principal.wizard || raise(E_PERM, "Tool caller cannot impersonate actor");
  endmethod

  method _check_user_eligible owner: HACKER
    "Override in children to customize user eligibility requirements. Default: no restrictions";
    const {wearer} = args;
    return true;
  endmethod

  method do_wear owner: ARCH_WIZARD
    "Require the actual wearer to own this device, then keep the invoking principal through pass.";
    const {who, @rest} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    (who == this.owner || principal.wizard)
      || raise(E_PERM, "That device belongs to someone else.");
    set_task_perms(principal);
    return pass(@args);
  endmethod

  method _check_for_owner owner: HACKER
    "Override parent to enforce owner-only usage";
    let event = 0;
    const action = {args};
    if (player != this.owner && !player.wizard)
      "Announce the violation to the room";
      if (valid(player.location))
        event = $event:mk_info(player, this:name(), " emits a sharp warning tone as ", $sub:n(), " ", $sub:self_alt("attempt", "attempts"), " to ", action, " it, rejecting the unauthorized user."):with_this(player.location);
        player.location:announce(event);
      else
        player:inform_current($event:mk_error(player, this:name(), " refuses to attune to you. The device is bonded to its rightful owner and will not respond to unauthorized use."));
      endif
      return;
    endif
  endmethod

  method _show_token_usage owner: ARCH_WIZARD
    "Display token usage information to the user";
    let context_msg = 0;
    let context_percent = 0;
    let context_tts = 0;
    let last_tokens = 0;
    let num_messages = 0;
    let prompt_tokens = 0;
    let status = 0;
    let threshold_percent = 0;
    let tts_msg = 0;
    let usage_msg = 0;
    const {wearer} = args;
    !valid(this.agent) && return;
    "Don't downgrade perms - need ARCH_WIZARD perms to read llm_token_budget and llm_tokens_used";
    const budget = wearer.llm_token_budget;
    const used = wearer.llm_tokens_used;
    const percent_used = used * 100 / budget;
    "Check if there's been a recent API call";
    const last_usage = this.agent.last_token_usage;
    if (typeof(last_usage) == TYPE_MAP && maphaskey(last_usage, "total_tokens"))
      last_tokens = last_usage["total_tokens"];
      usage_msg = "[TOKENS] Last call: " + (tostr(last_tokens)) + " | Total: " + tostr(used) + "/" + tostr(budget) + " (" + tostr(percent_used) + "% used)";
      tts_msg = "Token usage: Last call used " + tostr(last_tokens) + " tokens. Total " + tostr(used) + " of " + tostr(budget) + ", " + tostr(percent_used) + " percent used.";
    else
      "No API calls yet, just show budget";
      usage_msg = "[TOKENS] Budget: " + tostr(used) + "/" + tostr(budget) + " (" + tostr(percent_used) + "% used)";
      tts_msg = "Token budget: " + tostr(used) + " of " + tostr(budget) + " used, " + tostr(percent_used) + " percent.";
    endif
    wearer:inform_current($event:mk_info(wearer, usage_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_msg));
    "Show context status";
    const token_limit = this.agent.token_limit;
    const compaction_threshold = this.agent.compaction_threshold;
    const threshold_tokens = toint(tofloat(token_limit) * compaction_threshold);
    if (typeof(last_usage) == TYPE_MAP && maphaskey(last_usage, "prompt_tokens"))
      "We have actual prompt token data from a previous call";
      prompt_tokens = last_usage["prompt_tokens"];
      context_percent = prompt_tokens * 100 / token_limit;
      threshold_percent = prompt_tokens * 100 / threshold_tokens;
      "Describe proximity to the compaction threshold";
      if (threshold_percent >= 100)

        status = "COMPACTING";
      elseif (threshold_percent >= 80)

        status = "NEAR LIMIT";
      else

        status = "OK";
      endif
      context_msg = "[CONTEXT] Size: " + (tostr(prompt_tokens)) + "/" + tostr(token_limit) + " (" + tostr(context_percent) + "%) | Compaction at " + tostr(threshold_tokens) + " - " + status;
      context_tts = "Context size: " + tostr(prompt_tokens) + " of " + tostr(token_limit) + " tokens, " + tostr(context_percent) + " percent. Status: " + status + ".";
    else
      "No API call yet - show context is fresh, estimate from message count";
      num_messages = length(this.agent.context);
      if (num_messages == 0)
        status = "FRESH";

        context_msg = "[CONTEXT] " + status + " | Limit: " + tostr(token_limit) + " | Compaction at " + tostr(threshold_tokens);
        context_tts = "Context is fresh. Token limit is " + tostr(token_limit) + ", compaction at " + tostr(threshold_tokens) + ".";
      else
        status = tostr(num_messages) + " messages";

        context_msg = "[CONTEXT] " + status + " | Limit: " + tostr(token_limit) + " | Compaction at " + tostr(threshold_tokens);
        context_tts = "Context has " + tostr(num_messages) + " messages. Token limit is " + tostr(token_limit) + ".";
      endif
    endif
    wearer:inform_current($event:mk_info(wearer, context_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(context_tts));
  endmethod

  method _tool_explain owner: ARCH_WIZARD
    "Tool: Communicate reasoning, progress updates, or error details to user";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const message = args_map["message"];
    typeof(message) == TYPE_STR || raise(E_TYPE("Expected message string"));
    "Message is displayed by on_tool_call callback, no need to display here";
    return "Message delivered to user.";
  endmethod

  method _tool_ask_user owner: ARCH_WIZARD
    "Tool: Ask the wearer a question, supporting choices or free-text responses";
    let metadata = 0;
    let response = 0;
    let change_metadata = 0;
    let change_request = 0;
    let choices = 0;
    let rows = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const question = args_map["question"];
    typeof(question) == TYPE_STR || raise(E_TYPE("Expected question string"));
    const placeholder = `args_map["placeholder"] ! ANY => "Describe your requested changes..."';
    "If explicit choices are provided, present them directly";
    if (maphaskey(args_map, "choices"))
      choices = args_map["choices"];
      typeof(choices) == TYPE_LIST && length(choices) > 0 || raise(E_TYPE("choices must be a non-empty list when provided"));
      "Always include a stop option so user can end the flow";
      if (!("Stop" in choices) && !("stop" in choices))
        choices = {@choices, "Stop"};
      endif
      metadata = {{"input_type", "choice"}, {"prompt", question}, {"choices", choices}};
      response = wearer:read_with_prompt(metadata);
      if (typeof(response) != TYPE_STR || response == "" || response == "Stop")
        this.agent.cancel_requested = true;
        return "User cancelled.";
      endif
      return "User chose: " + response;
    endif
    "Handle explicitly requested input types (text, text_area, yes_no, confirmation, etc.)";
    const requested_type = `args_map["input_type"] ! ANY => ""';
    const allowed_types = {"yes_no", "text", "text_area", "confirmation", "yes_no_alternative"};
    if (is_member(requested_type, allowed_types))
      metadata = {{"input_type", requested_type}, {"prompt", question}};
      if (typeof(placeholder) == TYPE_STR && placeholder != "")
        metadata = {@metadata, {"placeholder", placeholder}};
      endif
      if (requested_type == "text_area")
        rows = `args_map["rows"] ! ANY => 4';
        typeof(rows) == TYPE_INT && (metadata = {@metadata, {"rows", rows}});
      endif
      response = wearer:read_with_prompt(metadata);
      if (!response || response == "@abort")
        this.agent.cancel_requested = true;
        return "User cancelled.";
      endif
      if (requested_type == "yes_no")
        if (response == "yes")
          return "User accepted.";
        else
          this.agent.cancel_requested = true;
          return "User cancelled.";
        endif
      endif
      return "User response: " + response;
    endif
    "Default: Accept / Stop / Request Change flow";
    metadata = {{"input_type", "choice"}, {"prompt", question}, {"choices", {"Accept", "Stop", "Request Change"}}};
    response = wearer:read_with_prompt(metadata);
    if (response == "Stop" || typeof(response) != TYPE_STR || response == "")
      this.agent.cancel_requested = true;
      return "User cancelled.";
    elseif (response == "Request Change")
      "Follow up with multiline text input for the requested change";
      change_metadata = {{"input_type", "text_area"}, {"prompt", "What changes would you like?"}, {"placeholder", placeholder}, {"rows", 4}};
      change_request = wearer:read_with_prompt(change_metadata);
      if (change_request == "@abort" || typeof(change_request) != TYPE_STR || change_request:trim() == "")
        this.agent.cancel_requested = true;
        return "User cancelled.";
      endif
      return "User requested changes: " + change_request:trim();
    else
      return "User accepted.";
    endif
  endmethod

  method log_tool_error owner: ARCH_WIZARD
    "Log tool execution errors to server_log. Called by agent when a tool raises.";
    const {tool_name, tool_args, error_msg} = args;
    caller == this.agent || caller_perms().wizard || raise(E_PERM);
    const safe_args = typeof(tool_args) == TYPE_STR ? tool_args | toliteral(tool_args);
    set_task_perms(this.owner, {{"builtin_call", "server_log"}});
    server_log("LLM tool error [" + tostr(tool_name) + "]: " + tostr(error_msg) + " args=" + safe_args);
    return true;
  endmethod

  method on_tool_call owner: ARCH_WIZARD
    "Callback when agent uses a tool - per-tool rewritable placeholder.";
    caller == this || caller == this.agent || caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    let message = 0;
    let {tool_name, tool_args} = args;
    "Use the wearer or authenticated requester for a carried tool.";
    let wearer = this:wearer();
    if (!valid(wearer) && valid(this.agent))
      wearer = caller_perms();
    endif
    if (!valid(wearer))
      return;
    endif
    "Parse JSON string to map";
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    "Explain is not rewritable; emit once";
    if (tool_name == "explain")
      message = this:_format_hud_message(tool_name, tool_args);
      wearer:inform_current($event:mk_info(wearer, message):with_presentation_hint('inset):with_group('llm, this));
      return;
    endif
    "Create per-tool rewritable placeholder on current connection";
    const rewrite_id = uuid();
    message = this:_format_hud_message(tool_name, tool_args);
    const placeholder = $event:mk_info(wearer, message):with_rewritable(rewrite_id, 300, message):with_presentation_hint('processing):with_group('llm, this);
    wearer:inform_current(placeholder);
    "Track rewrite id by tool name (FIFO queue)";
    let steps = typeof(this.progress_steps) == TYPE_MAP ? this.progress_steps | [];
    let queue = maphaskey(steps, tool_name) ? steps[tool_name] | {};
    queue = {@queue, rewrite_id};
    steps[tool_name] = queue;
    this.progress_steps = steps;
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Callback after tool execution - rewrite per-tool placeholder once.";
    let event = 0;
    let key = 0;
    let message = 0;
    let rewrite_id = 0;
    let subject = 0;
    let tts_msg = 0;
    caller == this || caller == this.agent || caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    let {tool_name, tool_args, result} = args;
    "Skip explain tool - the message IS the result";
    if (tool_name == "explain")
      return;
    endif
    "Use the wearer or authenticated requester for a carried tool.";
    let wearer = this:wearer();
    if (!valid(wearer) && valid(this.agent))
      wearer = caller_perms();
    endif
    if (!valid(wearer))
      return;
    endif
    "Prepare result text (truncated)";
    let result_text = typeof(result) == TYPE_STR ? result | toliteral(result);
    if (length(result_text) > 100)
      result_text = result_text[1..100] + "...";
    endif
    "Check if error";
    const is_error = typeof(result) == TYPE_STR && (result:starts_with("ERROR:") || result:starts_with("TOOL BLOCKED:"));
    "Parse tool_args if JSON string";
    if (typeof(tool_args) == TYPE_STR)
      tool_args = `parse_json(tool_args) ! ANY => []';
    endif
    "Special formatting for task-related tools";
    const task_tools = {"create_task", "record_finding", "get_findings", "task_status"};
    const is_task_tool = tool_name in task_tools;
    "For ask_user, include the question in the completion message";
    let question_text = "";
    if (tool_name == "ask_user" && typeof(tool_args) == TYPE_MAP && maphaskey(tool_args, "question"))
      question_text = tool_args["question"];
      if (length(question_text) > 60)
        question_text = question_text[1..60] + "...";
      endif
    endif
    "For record_finding, show what was recorded";
    let finding_text = "";
    if (tool_name == "record_finding" && typeof(tool_args) == TYPE_MAP)
      subject = `tool_args["subject"] ! ANY => ""';
      key = `tool_args["key"] ! ANY => ""';
      if (subject != "" && key != "")
        finding_text = subject + "/" + key;
      endif
    endif
    "Find rewrite target for this tool";
    let steps = typeof(this.progress_steps) == TYPE_MAP ? this.progress_steps | [];
    if (maphaskey(steps, tool_name) && length(steps[tool_name]) > 0)
      rewrite_id = steps[tool_name][1];
      if (length(steps[tool_name]) > 1)
        steps[tool_name] = steps[tool_name][2..length(steps[tool_name])];
      else
        steps = mapdelete(steps, tool_name);
      endif
      this.progress_steps = steps;
      if (is_error)
        message = "[\u2717] " + tool_name + ": " + result_text;
        tts_msg = "Error from " + tool_name + ": " + result_text;
      elseif (tool_name == "ask_user" && question_text != "")
        message = "[\u2713] " + tool_name + " \"" + question_text + "\": " + result_text;
        tts_msg = "Response to question: " + question_text + ". " + result_text;
      elseif (tool_name == "record_finding" && finding_text != "")
        message = "[\u2713] " + tool_name + " [" + finding_text + "]: " + result_text;
        tts_msg = "Recorded finding for " + finding_text + ": " + result_text;
      elseif (is_task_tool)
        message = "[\u2713] " + tool_name + ": " + result_text;
        tts_msg = tool_name + " completed: " + result_text;
      else
        message = "[\u2713] " + tool_name + ": " + result_text;
        tts_msg = tool_name + " completed: " + result_text;
      endif
      event = $event:mk_info(wearer, message):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_msg):with_audience('utility);
      wearer:rewrite_event(rewrite_id, event);
      return;
    endif
    "Fallback: no rewrite target; emit normal event";
    if (is_error)
      message = "[\u2717] " + tool_name + ": " + result_text;
      tts_msg = "Error from " + tool_name + ": " + result_text;
    elseif (tool_name == "ask_user" && question_text != "")
      message = "[\u2713] " + tool_name + " \"" + question_text + "\": " + result_text;
      tts_msg = "Response to question: " + question_text + ". " + result_text;
    elseif (tool_name == "record_finding" && finding_text != "")
      message = "[\u2713] " + tool_name + " [" + finding_text + "]: " + result_text;
      tts_msg = "Recorded finding for " + finding_text + ": " + result_text;
    elseif (is_task_tool)
      message = "[\u2713] " + tool_name + ": " + result_text;
      tts_msg = tool_name + " completed: " + result_text;
    else
      message = "[\u2713] " + tool_name + ": " + result_text;
      tts_msg = tool_name + " completed: " + result_text;
    endif
    wearer:inform_current($event:mk_info(wearer, message):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_msg));
  endmethod

  method _format_hud_message owner: HACKER
    "Override in child objects for tool-specific HUD message formatting";
    let question = 0;
    let suffix = 0;
    const {tool_name, tool_args} = args;
    "Default formatting for common tools";
    if (tool_name == "explain")
      return "[INFO] " + tool_args["message"];
    elseif (tool_name == "ask_user")
      question = tool_args["question"];
      "Truncate long questions";
      if (length(question) > 60)
        question = question[1..60] + "...";
      endif
      suffix = "";
      if (maphaskey(tool_args, "choices") && typeof(tool_args["choices"]) == TYPE_LIST && length(tool_args["choices"]) > 0)
        suffix = " [options]";
      elseif (maphaskey(tool_args, "input_type") && typeof(tool_args["input_type"]) == TYPE_STR)
        suffix = " [" + tool_args["input_type"] + "]";
      endif
      return "[QUESTION] Asking: " + question + suffix;
    endif
    "Fallback for unknown tools";
    return "[PROCESS] " + tool_name;
  endmethod

  method _format_tts_message owner: HACKER
    "Override in child objects for tool-specific TTS message formatting";
    let question = 0;
    let suffix = 0;
    const {tool_name, tool_args} = args;
    "Default TTS-friendly formatting for common tools";
    if (tool_name == "explain")
      return "Info: " + tool_args["message"];
    elseif (tool_name == "ask_user")
      question = tool_args["question"];
      if (length(question) > 100)
        question = question[1..100] + "...";
      endif
      suffix = "";
      if (maphaskey(tool_args, "choices") && typeof(tool_args["choices"]) == TYPE_LIST && length(tool_args["choices"]) > 0)
        suffix = " with options";
      elseif (maphaskey(tool_args, "input_type") && typeof(tool_args["input_type"]) == TYPE_STR)
        suffix = ", " + tool_args["input_type"] + " input";
      endif
      return "Question: " + question + suffix;
    endif
    "Fallback for unknown tools";
    return "Processing tool: " + tool_name;
  endmethod


  verb stop (this none none) owner: ARCH_WIZARD flags: "rd"
    "Stop the current agent operation without clearing context";
    caller == this || caller == this.owner || (valid(caller_perms()) && caller_perms().wizard) || raise(E_PERM);
    if (!valid(this.agent))
      player:inform_current($event:mk_error(player, "No agent is currently configured."));
      return;
    endif
    "Request cancellation of current operation";
    this.agent.cancel_requested = true;
    set_task_perms(this.owner);
    "Show token usage before stopping";
    this:_show_token_usage(player);
    player:inform_current($event:mk_info(player, "Cancellation requested. Agent will stop at next safe point."));
  endverb

  verb reset (this none none) owner: ARCH_WIZARD flags: "rd"
    "Reset agent context, clearing conversation history";
    caller == this || caller == this.owner || (valid(caller_perms()) && caller_perms().wizard) || raise(E_PERM);
    if (!valid(this.agent))
      player:inform_current($event:mk_error(player, "No agent is currently configured."));
      return;
    endif
    set_task_perms(this.owner);
    "Show token usage before resetting";
    this:_show_token_usage(player);
    "Clear the agent's context";
    this.agent:reset_context();
    player:inform_current($event:mk_info(player, "Agent context reset. Conversation history cleared."));
  endverb

  method debug_status owner: ARCH_WIZARD
    "Debug the status calculation to see what's happening";
    let status = 0;
    if (!valid(this.agent))
      return "No agent configured";
    endif
    const last_usage = this.agent.last_token_usage;
    if (typeof(last_usage) != TYPE_MAP || !maphaskey(last_usage, "prompt_tokens"))
      return "No prompt token data available";
    endif
    const prompt_tokens = last_usage["prompt_tokens"];
    const token_limit = this.agent.token_limit;
    const compaction_threshold = this.agent.compaction_threshold;
    const threshold_tokens = toint(tofloat(token_limit) * compaction_threshold);
    const context_percent = prompt_tokens * 100 / token_limit;
    const threshold_percent = prompt_tokens * 100 / threshold_tokens;
    if (threshold_percent >= 100)
      status = "COMPACTING";
    elseif (threshold_percent >= 80)
      status = "NEAR LIMIT";
    else
      status = "OK";
    endif
    return ["prompt_tokens" -> prompt_tokens, "token_limit" -> token_limit, "compaction_threshold" -> compaction_threshold, "threshold_tokens" -> threshold_tokens, "context_percent" -> context_percent, "threshold_percent" -> threshold_percent, "test_gte_100" -> threshold_percent >= 100, "test_gte_80" -> threshold_percent >= 80, "status" -> status];
  endmethod

  method test_tool_get_message_template_actor_permissions owner: ARCH_WIZARD
    "get_message_template should read a private target property under ordinary actor permissions.";
    let msg_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msg", "secret message", {$test_player, ""});
      msg_result = tool:_tool_get_message_template(["object" -> tostr(target), "property" -> "wearable_private_msg"], $test_player);
      $test_utils:assert_true(index(msg_result, "\"secret message\"") > 0, "get_message_template should read a non-readable property under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_list_messages_actor_permissions owner: ARCH_WIZARD
    "list_messages should enumerate and read private target message properties under ordinary actor permissions.";
    let msg_list = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msg", "secret message", {$test_player, ""});
      msg_list = tool:_tool_list_messages(["object" -> tostr(target)], $test_player);
      $test_utils:assert_true(index(msg_list, "wearable_private_msg") > 0, "list_messages should read private message properties under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_show_rule_actor_permissions owner: ARCH_WIZARD
    "show_rule should read a private target rule under ordinary actor permissions.";
    let rule_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_rule", 0, {$test_player, ""});
      rule_result = tool:_tool_show_rule(["object" -> tostr(target), "property" -> "wearable_private_rule"], $test_player);
      $test_utils:assert_true(index(rule_result, "(not set)") > 0, "show_rule should read a non-readable rule under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_list_rules_actor_permissions owner: ARCH_WIZARD
    "list_rules should enumerate and read private target rule properties under ordinary actor permissions.";
    let rule_list = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_rule", 0, {$test_player, ""});
      rule_list = tool:_tool_list_rules(["object" -> tostr(target)], $test_player);
      $test_utils:assert_true(index(rule_list, "wearable_private_rule") > 0, "list_rules should read private rule properties under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_set_rule_actor_permissions owner: ARCH_WIZARD
    "set_rule should write a private target rule under ordinary actor permissions.";
    let set_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_set_rule", 1, {$test_player, ""});
      set_result = tool:_tool_set_rule(["object" -> tostr(target), "property" -> "wearable_set_rule", "expression" -> "0"], $test_player);
      $test_utils:assert_true(index(set_result, "Cleared rule") > 0, "set_rule should write a non-writable rule through a property_write grant");
      $test_utils:assert_eq(target.wearable_set_rule, 0, "set_rule should update the private rule property");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_set_message_template_actor_permissions owner: ARCH_WIZARD
    "set_message_template should write a private target message property under ordinary actor permissions.";
    let set_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_set_msg", "old", {$test_player, ""});
      set_result = tool:_tool_set_message_template(["object" -> tostr(target), "property" -> "wearable_set_msg", "template" -> "new message"], $test_player);
      $test_utils:assert_true(index(set_result, "Set wearable_set_msg") > 0, "set_message_template should update a non-writable message property through a property_write grant");
      $test_utils:assert_true(index($sub_utils:decompile(target.wearable_set_msg), "new message") > 0, "set_message_template should store the compiled template");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_add_message_template_actor_permissions owner: ARCH_WIZARD
    "add_message_template should append to a private message bag under ordinary actor permissions.";
    let add_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    const bag = $msg_bag:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msgs", bag, {$test_player, ""});
      add_result = tool:_tool_add_message_template(["object" -> tostr(target), "property" -> "wearable_private_msgs", "template" -> "added message"], $test_player);
      $test_utils:assert_true(index(add_result, "Added entry") > 0, "add_message_template should append through a message bag entries grant");
      $test_utils:assert_eq(length(bag.entries), 1, "add_message_template should append one bag entry");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_delete_message_template_actor_permissions owner: ARCH_WIZARD
    "delete_message_template should remove from a private message bag under ordinary actor permissions.";
    let del_result = 0;
    const target = $thing:create(0);
    const tool = this:create(true);
    const bag = $msg_bag:create(true);
    try
      tool:moveto($test_player);
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      bag.entries = {$sub_utils:compile("first"), $sub_utils:compile("second")};
      add_property(target, "wearable_private_msgs", bag, {$test_player, ""});
      del_result = tool:_tool_delete_message_template(["object" -> tostr(target), "property" -> "wearable_private_msgs", "index" -> 1], $test_player);
      $test_utils:assert_true(index(del_result, "Removed entry #1") > 0, "delete_message_template should remove through a message bag entries grant");
      $test_utils:assert_eq(length(bag.entries), 1, "delete_message_template should remove one bag entry");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(tool) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method recycle owner: ARCH_WIZARD
    "Clean up agent when object is destroyed";
    caller == this || caller_perms() == this.owner || (valid(caller_perms()) && caller_perms().wizard) || raise(E_PERM);
    if (valid(this.agent))
      this.agent = #-1;
    endif
  endmethod

  method reconfigure owner: ARCH_WIZARD
    "Reconfigure by clearing old agent ref and creating fresh one";
    caller == this || caller == this.owner || caller_perms().wizard || raise(E_PERM);
    "Clear ref - anonymous agent will be GC'd";
    this.agent = #-1;
    "Create fresh agent with current configuration";
    this:configure();
  endmethod

  method _register_common_tools owner: ARCH_WIZARD
    "Register tools common to all LLM wearables. Called by child configure verbs.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    const {agent} = args;
    "Register explain tool";
    const explain_tool = $llm_agent_tool:mk("explain", "Share your thought process, findings, or reasoning with the user. Use this frequently to narrate what you're investigating, explain what you discovered from tool results, or describe your plan before taking actions.", ["type" -> "object", "properties" -> ["message" -> ["type" -> "string", "description" -> "Your explanation, findings, or thought process to share with the user"]], "required" -> {"message"}], this, "_tool_explain");
    agent:add_tool("explain", explain_tool);
    "Register ask_user tool";
    const ask_user_tool = $llm_agent_tool:mk("ask_user", "Ask the user a question and receive their response. Provide 'choices' for a multiple-choice prompt or set 'input_type' to 'text'/'text_area' with an optional 'placeholder' (and 'rows' for text_area) to gather free-form input. If no options are provided, the prompt defaults to Accept/Stop/Request Change with a follow-up text box for requested changes.", ["type" -> "object", "properties" -> ["question" -> ["type" -> "string", "description" -> "The question or proposal to present to the user"], "choices" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Optional list of explicit choices to show the user"], "input_type" -> ["type" -> "string", "description" -> "Optional input style: 'text', 'text_area', or 'yes_no'"], "placeholder" -> ["type" -> "string", "description" -> "Placeholder to show in free-form prompts"], "rows" -> ["type" -> "integer", "description" -> "Number of rows when using text_area prompts"]], "required" -> {"question"}], this, "_tool_ask_user");
    agent:add_tool("ask_user", ask_user_tool);
    "Register todo list tools";
    const todo_write_tool = $llm_agent_tool:mk("todo_write", "Replace the entire todo list. Use this to track multi-step tasks. Each todo needs 'content' (what to do) and 'status' ('pending', 'in_progress', or 'completed'). Mark tasks in_progress when starting, completed when done.", ["type" -> "object", "properties" -> ["todos" -> ["type" -> "array", "items" -> ["type" -> "object", "properties" -> ["content" -> ["type" -> "string"], "status" -> ["type" -> "string", "enum" -> {"pending", "in_progress", "completed"}]], "required" -> {"content", "status"}], "description" -> "List of todo items"]], "required" -> {"todos"}], this, "_tool_todo_write");
    agent:add_tool("todo_write", todo_write_tool);
    const get_todos_tool = $llm_agent_tool:mk("get_todos", "Get the current todo list to see what tasks are pending, in progress, or completed.", ["type" -> "object", "properties" -> [], "required" -> {}], this, "_tool_get_todos");
    agent:add_tool("get_todos", get_todos_tool);
  endmethod

  method _tool_todo_write owner: ARCH_WIZARD
    "Tool: Replace the entire todo list to track multi-step tasks";
    let content = 0;
    let prefix = 0;
    let status = 0;
    let status_str = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const todos_input = args_map["todos"];
    typeof(todos_input) != TYPE_LIST && raise(E_TYPE, "todos must be a list");
    let todo_items = {};
    for item in (todos_input)
      content = item["content"];
      status_str = item["status"];
      status = status_str == "pending" ? 'pending | status_str == "in_progress" ? 'in_progress | status_str == "completed" ? 'completed | raise(E_INVARG, "Invalid status: " + status_str);
      todo_items = {@todo_items, ["content" -> content, "status" -> status]};
    endfor
    this.agent:set_todos(todo_items);
    let summary = {tostr(length(todo_items)) + " todos set:"};
    for todo in (this.agent:get_todos())
      prefix = todo["status"] == 'completed ? "[x]" | todo["status"] == 'in_progress ? "[>]" | "[ ]";
      summary = {@summary, "  " + prefix + " " + todo["content"]};
    endfor
    return summary:join("\n");
  endmethod

  method _tool_get_todos owner: ARCH_WIZARD
    "Tool: Get the current todo list";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    return this.agent:format_todos();
  endmethod

  method _register_authoring_tools owner: ARCH_WIZARD
    "Register doc/message/rule authoring tools. Called by child configure verbs.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    const {agent} = args;
    "Register doc_lookup tool";
    const doc_tool = $llm_agent_tool:mk("doc_lookup", "Read developer documentation for an object, verb, or property. Use formats: obj, obj:verb, obj.property.", ["type" -> "object", "properties" -> ["target" -> ["type" -> "string", "description" -> "Object/verb/property reference, e.g., '$sub_utils', '#61:drop_msg', '#61.get_msg'"]], "required" -> {"target"}], this, "_tool_doc_lookup");
    agent:add_tool("doc_lookup", doc_tool);
    "Register message tools";
    const list_messages_tool = $llm_agent_tool:mk("list_messages", "List message template properties (*_msg) and message bags (*_msg_bag/_msgs) on an object. Equivalent to @messages.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#62', '$room', 'here')"]], "required" -> {"object"}], this, "_tool_list_messages");
    agent:add_tool("list_messages", list_messages_tool);
    const get_message_tool = $llm_agent_tool:mk("get_message_template", "Show a single message template or list the entries of a message bag. Equivalent to @getm.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"]], "required" -> {"object", "property"}], this, "_tool_get_message_template");
    agent:add_tool("get_message_template", get_message_tool);
    const set_message_tool = $llm_agent_tool:mk("set_message_template", "Set a message template on an object property. For bags (_msgs/_msg_bag), replace all entries with a single template; use add_message_template to append instead.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], this, "_tool_set_message_template");
    agent:add_tool("set_message_template", set_message_tool);
    const add_message_tool = $llm_agent_tool:mk("add_message_template", "Append a message template to a message bag property (_msgs or _msg_bag). Equivalent to @add-message.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], this, "_tool_add_message_template");
    agent:add_tool("add_message_template", add_message_tool);
    const del_message_tool = $llm_agent_tool:mk("delete_message_template", "Remove a message entry by index from a message bag property. Equivalent to @del-message.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "index" -> ["type" -> "integer", "description" -> "1-based index to remove"]], "required" -> {"object", "property", "index"}], this, "_tool_delete_message_template");
    agent:add_tool("delete_message_template", del_message_tool);
    "Register rule tools";
    const list_rules_tool = $llm_agent_tool:mk("list_rules", "List all rule properties (*_rule) on an object and their current expressions. Rules control access to object operations like locking containers. Equivalent to @rules.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#10', '$container', 'chest')"]], "required" -> {"object"}], this, "_tool_list_rules");
    agent:add_tool("list_rules", list_rules_tool);
    const set_rule_tool = $llm_agent_tool:mk("set_rule", "Set an access control rule on an object property. Rules are logical expressions like 'input.Key == #123' or 'NOT is_locked(object: input.This)'. See $rule_engine docs for syntax. Equivalent to @set-rule.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule, e.g., 'lock_rule')"], "expression" -> ["type" -> "string", "description" -> "Condition with AND, OR, NOT, comparisons, and named predicate arguments"]], "required" -> {"object", "property", "expression"}], this, "_tool_set_rule");
    agent:add_tool("set_rule", set_rule_tool);
    const show_rule_tool = $llm_agent_tool:mk("show_rule", "Display the current expression for a specific rule property. Equivalent to @show-rule.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule)"]], "required" -> {"object", "property"}], this, "_tool_show_rule");
    agent:add_tool("show_rule", show_rule_tool);
  endmethod

  method _tool_doc_lookup owner: ARCH_WIZARD
    "Tool: Fetch developer documentation for object/verb/property (like @doc)";
    let alias_name = 0;
    let doc_text = 0;
    let item_name = 0;
    let object_str = 0;
    let parsed = 0;
    let selector = 0;
    let selectors = 0;
    let target_obj = 0;
    let title = 0;
    let type = 0;
    let verb_location = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = actor;
    const target_spec = args_map["target"];
    set_task_perms(wearer);
    "Handle special alias cases";
    let alias_obj = false;
    if (typeof(target_spec) == TYPE_STR)
      alias_name = target_spec:starts_with("$") ? target_spec[2..$] | target_spec;
      alias_name == "sub_utils" && (alias_obj = $sub_utils);
      alias_name == "sub" && (alias_obj = $sub);
    endif
    if (alias_obj)
      type = 'object;
      target_obj = alias_obj;
      item_name = "";
    else
      parsed = $prog_utils:parse_target_spec(target_spec);
      parsed || raise(E_INVARG, "Invalid format. Use object, object:verb, or object.property");
      object_str = parsed['object_str];
      selectors = parsed['selectors];
      "Determine type and item_name from selectors";
      if (length(selectors) > 0)
        selector = selectors[1];
        type = selector['kind];
        item_name = selector['item_name];
      else
        type = 'object;
        item_name = "";
      endif
      target_obj = $match:match_object(object_str, wearer);
      typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
      valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    endif
    "Fetch docs based on type";
    if (type == 'object)
      doc_text = $help_utils:get_object_documentation(target_obj);
      title = "Documentation for " + tostr(target_obj);
    elseif (type == 'verb)
      verb_location = target_obj:find_verb_definer(item_name);
      verb_location == #-1 && raise(E_INVARG, "Verb '" + tostr(item_name) + "' not found on " + tostr(target_obj));
      doc_text = $help_utils:extract_verb_documentation(verb_location, item_name);
      title = "Documentation for " + tostr(target_obj) + ":" + tostr(item_name);
    elseif (type == 'property)
      doc_text = $help_utils:property_documentation(target_obj, item_name);
      title = "Documentation for " + tostr(target_obj) + "." + tostr(item_name);
    else
      raise(E_INVARG, "Unknown target type");
    endif
    const doc_body = typeof(doc_text) == TYPE_LIST ? doc_text:join("\n") | doc_text;
    return title + "\n\n" + (doc_body ? doc_body | "(No documentation available)");
  endmethod

  method _tool_list_messages owner: ARCH_WIZARD
    "Tool: List *_msg properties and message bags on an object (like @messages)";
    let prop_name = 0;
    let prop_value = 0;
    let value_summary = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const msg_props = $obj_utils:message_properties(target_obj, wearer, true);
    !msg_props && return tostr(target_obj) + " has no message properties. (@messages command available)";
    let lines = {"Message properties for " + tostr(target_obj) + ":"};
    for prop_info in (msg_props)
      {prop_name, prop_value} = prop_info;
      value_summary = typeof(prop_value) == TYPE_OBJ && isa(prop_value, $msg_bag) ? "message bag (" + tostr(length(prop_value:entries())) + " entries)" | typeof(prop_value) == TYPE_LIST ? `$sub_utils:decompile(prop_value) ! ANY => toliteral(prop_value)' | toliteral(prop_value);
      lines = {@lines, " - " + tostr(prop_name) + ": " + value_summary};
    endfor
    return lines:join("\n");
  endmethod

  method _tool_get_message_template owner: ARCH_WIZARD
    "Tool: Read a single message template (like @getm)";
    let entries = 0;
    let lines = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    const prop_name = args_map["property"];
    prop_name:ends_with("_msg") || prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msg/_msgs/_msg_bag");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    set_task_perms(wearer);
    const value = target_obj.(prop_name);
    if (typeof(value) == TYPE_OBJ && isa(value, $msg_bag))
      entries = value:entries();
      !entries && return tostr(target_obj) + "." + prop_name + " = (empty message bag)";
      lines = {tostr(target_obj) + "." + prop_name + " (message bag, " + tostr(length(entries)) + " entries):"};
      for i, entry in (entries)
        lines = {@lines, tostr(i) + ". " + (typeof(entry) == TYPE_LIST ? `$sub_utils:decompile(entry) ! ANY => toliteral(entry)' | toliteral(entry))};
      endfor
      return lines:join("\n");
    endif
    const display_value = typeof(value) == TYPE_LIST ? `$sub_utils:decompile(value) ! ANY => toliteral(value)' | toliteral(value);
    return tostr(target_obj) + "." + prop_name + " = " + display_value + " (@getm command available)";
  endmethod

  method _tool_set_message_template owner: ARCH_WIZARD
    "Tool: Set a message template (like @setm). Creates property if missing.";
    let compiled = 0;
    let prop_name = 0;
    let success = 0;
    let template = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    {prop_name, template} = {args_map["property"], args_map["template"]};
    prop_name:ends_with("_msg") || prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msg/_msgs/_msg_bag");
    template || raise(E_INVARG, "Template string required");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    {success, compiled} = $obj_utils:validate_and_compile_template(template);
    success || raise(E_INVARG, "Template compilation failed: " + compiled);
    const obj_name = `target_obj.name ! ANY => tostr(target_obj)';
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    if (prop_key == E_PROPNF)
      "Property doesn't exist - try to create it";
      if (!wearer.wizard && target_obj.owner != wearer)
        raise(E_PERM, "Cannot add property to " + tostr(target_obj) + ": not owner");
      endif
      set_task_perms(wearer);
      add_property(target_obj, prop_name, compiled, {wearer, "rc"});
      return "Created and set " + prop_name + " on \"" + obj_name + "\" (" + tostr(target_obj) + ").";
    endif
    $obj_utils:set_compiled_message(target_obj, prop_key, compiled, wearer);
    return "Set " + prop_name + " on \"" + obj_name + "\" (" + tostr(target_obj) + "). (@setm command available)";
  endmethod

  method _tool_add_message_template owner: ARCH_WIZARD
    "Tool: Append a template to a message bag (like @add-message)";
    let prop_name = 0;
    let template = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    {prop_name, template} = {args_map["property"], args_map["template"]};
    prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msgs/_msg_bag");
    template || raise(E_INVARG, "Template string required");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const compiled = $sub_utils:compile(template);
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    if (prop_key == E_PROPNF)
      if (!wearer.wizard && target_obj.owner != wearer)
        raise(E_PERM, "Cannot add property to " + tostr(target_obj) + ": not owner");
      endif
      $obj_utils:add_message_entry(target_obj, prop_name, compiled, wearer);
      return "Added entry to " + tostr(target_obj) + "." + prop_name + ".";
    endif
    $obj_utils:add_message_entry(target_obj, prop_key, compiled, wearer);
    return "Added entry to " + tostr(target_obj) + "." + prop_name + ".";
  endmethod

  method _tool_delete_message_template owner: ARCH_WIZARD
    "Tool: Delete a template by index from a message bag (like @del-message)";
    let idx = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    {prop_name, idx} = {args_map["property"], args_map["index"]};
    prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msgs/_msg_bag");
    typeof(idx) == TYPE_INT || raise(E_TYPE, "Index must be integer");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && raise(E_INVARG, "Message bag not found on " + tostr(target_obj) + "." + prop_name);
    $obj_utils:remove_message_entry(target_obj, prop_key, idx, wearer, {{"property_write", target_obj, prop_key}});
    return "Removed entry #" + tostr(idx) + " from " + tostr(target_obj) + "." + prop_name + ".";
  endmethod

  method _tool_list_rules owner: ARCH_WIZARD
    "Tool: List *_rule properties on an object (like @rules)";
    let prop_name = 0;
    let prop_value = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const rule_props = $obj_utils:rule_properties(target_obj, wearer, true);
    !rule_props && return tostr(target_obj) + " has no rule properties. (@rules command available)";
    let lines = {"Rule properties for " + tostr(target_obj) + ":"};
    for prop_info in (rule_props)
      {prop_name, prop_value} = prop_info;
      lines = {@lines, " - " + tostr(prop_name) + ": " + (prop_value == 0 ? "(not set)" | $rule_engine:decompile_rule(prop_value))};
    endfor
    return lines:join("\n");
  endmethod

  method _tool_set_rule owner: ARCH_WIZARD
    "Tool: Set a rule on an object property (like @set-rule)";
    let rule = 0;
    let expression = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    {prop_name, expression} = {args_map["property"], args_map["expression"]};
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property must end with _rule");
    expression || raise(E_INVARG, "Rule expression required");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    set_task_perms(wearer);
    "Handle special cases";
    if (expression == "0" || expression == "none" || expression == "always" || expression == "true")
      target_obj.(prop_name) = 0;
      return "Cleared rule on " + tostr(target_obj) + "." + prop_name + " (always passes)";
    endif
    if (expression == "false" || expression == "never" || expression == "locked")
      "Store an explicit condition that never matches.";
      rule = $rule_engine:parse_expression("FALSE", tosym(prop_name));
      target_obj.(prop_name) = rule;
      return "Set " + tostr(target_obj) + "." + prop_name + " to always fail (locked)";
    endif
    rule = $rule_engine:parse_expression(expression, tosym(prop_name), $rule_engine:policy_scope(prop_name));
    const validation = $rule_engine:validate_rule(rule);
    validation['valid] || raise(E_INVARG, "Rule validation failed: " + validation['warnings]:join("; "));
    target_obj.(prop_name) = rule;
    return "Set " + tostr(target_obj) + "." + prop_name + " = " + expression;
  endmethod

  method _tool_show_rule owner: ARCH_WIZARD
    "Tool: Display a rule property expression (like @show-rule)";
    const {args_map, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const wearer = this:_action_perms_check(actor);
    const prop_name = args_map["property"];
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property must end with _rule");
    const target_obj = $match:match_object(args_map["object"], wearer);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    set_task_perms(wearer);
    const rule = target_obj.(prop_name);
    return tostr(target_obj) + "." + prop_name + " = " + (rule == 0 ? "(not set)" | $rule_engine:decompile_rule(rule));
  endmethod

  verb "use inter*act qu*ery" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Use/interact with the wearable - prompts for input";
    let callback_target = 0;
    let error_msg = 0;
    let user_choice = 0;
    const incoming = caller_perms();
    const request_principal = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == this.owner || request_principal.wizard || raise(E_PERM);
    this:_check_for_owner("use");
    "Check if wearing (and optionally carrying)";
    const wearing_check = is_member(this, request_principal.wearing);
    const carrying_check = !this.requires_wearing_only && is_member(this, request_principal.contents);
    if (!wearing_check && !carrying_check)
      error_msg = this.requires_wearing_only ? "You have to wear " + this:name() + " first." | "You have to wear or carry " + this:name() + " first.";
      request_principal:inform_current($event:mk_error(request_principal, error_msg));
      return;
    endif
    "Configure agent if needed, or if inherited/shared agent is bound to another wearable";
    let needs_config = !valid(this.agent);
    if (!needs_config)
      callback_target = `this.agent.tool_callback ! ANY => #-1';
      callback_target != this && (needs_config = true);
    endif
    if (needs_config)
      this:configure();
    endif
    "Prompt for query text using metadata-based read";
    let metadata = {{"input_type", "text"}, {"prompt", this.prompt_label + " " + this.prompt_text}, {"placeholder", this.placeholder_text}, {"tts_prompt", this.prompt_text}};
    const query = request_principal:read_with_prompt(metadata):trim();
    if (!query)
      request_principal:inform_current($event:mk_error(request_principal, "Query cancelled - no input provided."));
      return;
    endif
    "Show query received";
    request_principal:inform_current($event:mk_info(request_principal, this.prompt_label + " Query received: " + query):as_inset():with_group('llm, this):with_tts("Query received: " + query));
    "Set token owner for budget tracking";
    this.agent.token_owner = request_principal;
    "Send to agent with continuation support (max 3 continuations)";
    let response = this.agent:send_message(query, false, request_principal);
    let continuations = 0;
    const max_continuations = 3;
    while (typeof(response) == TYPE_ERR && error_code(response) == E_QUOTA && continuations < max_continuations)
      request_principal:inform_current($event:mk_info(request_principal, ("[" + this.tool_name + "]") + " Agent hit iteration limit (" + tostr(this.agent.max_iterations) + " iterations)."):as_inset():with_group('llm, this):with_tts(this.tool_name + " hit iteration limit after " + tostr(this.agent.max_iterations) + " iterations."));
      metadata = {{"input_type", "yes_no"}, {"prompt", "Continue agent execution?"}};
      user_choice = request_principal:read_with_prompt(metadata);
      if (user_choice != "yes")
        this.agent.current_iteration = 0;
        this:_show_token_usage(request_principal);
        request_principal:inform_current($event:mk_info(request_principal, "Agent stopped at user request after reaching iteration limit."):as_inset():with_group('llm, this));
        return;
      endif
      "Continue the same conversation after the requester agrees";
      continuations = continuations + 1;
      request_principal:inform_current($event:mk_info(request_principal, ("[" + this.tool_name + "]") + " Continuing... (continuation " + tostr(continuations) + "/" + tostr(max_continuations) + ")"):as_inset():with_group('llm, this):with_tts("Continuing, continuation " + tostr(continuations) + " of " + tostr(max_continuations) + "."));
      response = this.agent:send_message("Continue where you left off. Complete any remaining work from the previous request.", false, request_principal);
    endwhile
    "Reset iteration counter";
    this.agent.current_iteration = 0;
    "Show token usage summary";
    this:_show_token_usage(request_principal);
    "Check if we exhausted all continuations";
    if (typeof(response) == TYPE_ERR && error_code(response) == E_QUOTA)
      request_principal:inform_current($event:mk_error(request_principal, "Agent reached maximum iterations even after " + tostr(max_continuations) + " continuations. Task may be too complex."));
      return;
    endif
    "If response is still an ERR but not E_QUOTA, display it as an error";
    if (typeof(response) == TYPE_ERR)
      request_principal:inform_current($event:mk_error(request_principal, "Error: ", $format.code:inline(toliteral(response))));
      return;
    endif
    "Display model prose as literal paragraphs";
    const event = $event:mk_info(request_principal, $format.block:mk(response)):as_inset():with_group('llm, this);
    request_principal:inform_current(event);
  endverb

endobject
