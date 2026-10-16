object LLM_WEARABLE [
  import_export_id -> "llm_wearable",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM-Powered Wearable"
  parent: WEARABLE
  owner: HACKER
  fertile: true
  readable: true

  property interaction (owner: ARCH_WIZARD, flags: "") = #-1;
  property interaction_timeout (owner: HACKER, flags: "rc") = 300;
  property agent (owner: HACKER, flags: "r") = #-1;
  property placeholder_text (owner: HACKER, flags: "rc") = "Ask a question...";
  property preferred_model (owner: HACKER, flags: "rc") = "";
  property processing_message (owner: HACKER, flags: "rc") = "Processing request...";
  property prompt_label (owner: HACKER, flags: "rc") = "[TOOL]";
  property prompt_text (owner: HACKER, flags: "rc") = "Enter your query:";
  property requires_wearing_only (owner: HACKER, flags: "rc") = true;
  property tool_name (owner: HACKER, flags: "rc") = "TOOL";

  override description (owner: HACKER, flags: "rc") = "Base prototype for AI-powered wearable tools that use LLM agents for interactive assistance.";

  method configure owner: ARCH_WIZARD
    "Create agent and apply configuration. Children override _setup_agent to customize.";
    const principal = caller_perms();
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    principal == this.owner || principal.wizard || raise(E_PERM);
    if (!is_clear_property(this, "interaction") && valid(this.interaction))
      this.interaction.interaction_task == task_id() && !valid(this.interaction.agent)
        || raise(E_INVARG, "Stop the current interaction before configuring.");
    endif
    if (!is_clear_property(this, "agent") && valid(this.agent))
      !this.agent.running || raise(E_INVARG, "Agent is still running.");
    endif
    "Create anonymous state with the actor as owner from the start.";
    const agent = create($llm_agent, this.owner, 1);
    const actor = this.owner;
    this:_configure_agent(agent, actor);
    this.owner == actor || raise(E_PERM, "Device ownership changed during configuration.");
    this.agent = agent;
  endmethod

  method get_project owner: ARCH_WIZARD
    "Return the configured agent's project, or #-1 when this device has no agent.";
    const principal = caller_perms();
    $llm_agent:_challenge_permissions(this, principal);
    is_clear_property(this, "agent") && return #-1;
    const agent = this.agent;
    valid(agent) || return #-1;
    set_task_perms(principal);
    return agent:get_project();
  endmethod

  method _configure_agent owner: ARCH_WIZARD
    "Invoke frontend configuration under the intended owner before publishing the new agent.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {agent, actor} = args;
    set_task_perms(actor);
    if (this.preferred_model)
      agent.model_override = this.preferred_model;
    endif
    this:_setup_agent(agent);
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

  method do_remove owner: ARCH_WIZARD
    "End the captured interaction before removing this device, even if it is worn again immediately.";
    const principal = caller_perms();
    const {who, @rest} = args;
    $root:check_actor(who, principal);
    if (!is_clear_property(this, "interaction") && valid(this.interaction)
        && this.interaction.requester == who)
      this.interaction:close();
    endif
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
        event = $event:mk_info(player, $sub:capitalize($sub:reference(this)), " ", $sub:agree('dobj, 'name, "emit", "emits"), " a sharp warning tone as ", $sub:the('actor), " ", $sub:agree('actor, 'name, "attempt", "attempts"), " to ", action, " it, rejecting the unauthorized user."):with_this(player.location):with_dobj(this);
        player.location:announce(event);
      else
        player:inform_current($event:mk_error(player, $sub:capitalize($sub:reference(this)), " ", $sub:agree('dobj, 'name, "refuse", "refuses"), " to attune to you. The device is bonded to its rightful owner and will not respond to unauthorized use."):with_dobj(this));
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
    const {wearer, ?session = #-1} = args;
    $llm_tool_provider:_require_tool_dispatch(wearer, caller_perms());
    !valid(this.agent) && return;
    "Don't downgrade perms - need ARCH_WIZARD perms to read llm_token_budget and llm_tokens_used";
    const budget = wearer.llm_token_budget;
    const used = wearer.llm_tokens_used;
    const percent_used = budget > 0 ? used * 100 / budget | 100;
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
    if (valid(session))
      session:emit(wearer, $event:mk_info(wearer, usage_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_msg));
    else
    wearer:inform_current($event:mk_info(wearer, usage_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(tts_msg));
    endif
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
      num_messages = length(this.agent:get_context());
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
    if (valid(session))
      session:emit(wearer, $event:mk_info(wearer, context_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(context_tts));
    else
    wearer:inform_current($event:mk_info(wearer, context_msg):with_presentation_hint('inset):with_group('llm, this):with_tts(context_tts));
    endif
  endmethod

  method _format_hud_message owner: ARCH_WIZARD
    "Render shared LLM tool progress; descendants may specialize this frontend hook.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    return $llm_tool_progress:mk(this.tool_name, @args):hud();
  endmethod

  method _format_tts_message owner: ARCH_WIZARD
    "Render shared LLM tool progress; descendants may specialize this frontend hook.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    return $llm_tool_progress:mk(this.tool_name, @args):speech();
  endmethod

  method _command_actor owner: ARCH_WIZARD
    "Resolve the command principal without treating ambient player as delegated authority.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {principal, source} = args;
    const actor = principal == #-1 && source == player && valid(player) && is_player(player)
      ? player | principal;
    valid(actor) && (actor == this.owner || actor.wizard) || raise(E_PERM);
    return actor;
  endmethod

  verb stop (this none none) owner: ARCH_WIZARD flags: "rd"
    "Stop this wearable's interaction, including a pending input or model request.";
    const actor = this:_command_actor(caller_perms(), caller);
    $llm_wearable_session:stop(this, actor);
    set_task_perms(actor);
    actor:inform_current($event:mk_info(actor, "Interaction stopped."));
  endverb

  verb reset (this none none) owner: ARCH_WIZARD flags: "rd"
    "Stop the interaction and clear this device's conversation.";
    const actor = this:_command_actor(caller_perms(), caller);
    $llm_wearable_session:stop(this, actor);
    if (!is_clear_property(this, "agent") && valid(this.agent))
      this.agent:reset_context();
    endif
    set_task_perms(actor);
    actor:inform_current($event:mk_info(actor, "Conversation history cleared."));
  endverb

  verb clear_project (this none none) owner: ARCH_WIZARD flags: "rd"
    "Usage: clear_project DEVICE. Stop its interaction and release all project tasks, findings, and todos.";
    const actor = this:_command_actor(caller_perms(), caller);
    $llm_wearable_session:stop(this, actor);
    set_task_perms(actor);
    const project = this:get_project();
    valid(project) && project:clear();
    actor:inform_current($event:mk_info(actor, "LLM project cleared."):with_group('llm, this));
  endverb

  method debug_status owner: ARCH_WIZARD
    "Return context usage to the device owner or wizard.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
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

  method recycle owner: ARCH_WIZARD
    "Stop active work and release this device's anonymous collaborators.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    $llm_wearable_session:stop(this, principal);
    this.agent = #-1;
  endmethod

  method reconfigure owner: ARCH_WIZARD
    "Replace idle configuration; active interactions must be stopped first.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    return this:configure();
  endmethod

  verb "use inter*act qu*ery" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Start one bounded interaction on the invoking connection.";
    const actor = this:_command_actor(caller_perms(), caller);
    const session = $llm_wearable_session:open(this, actor, `connection() ! E_INVARG => 0');
    session:interact();
  endverb

  method _register_authoring_tools owner: ARCH_WIZARD
    "Compose shared behavior and documentation catalogs for this frontend's agent.";
    const principal = caller_perms();
    const {agent} = args;
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    for tool in ($llm_tool_catalog:compose($llm_behavior_tools:get_tools(), $llm_help_tools:get_tools()))
      agent:add_tool(tool.name, tool);
    endfor
  endmethod

endobject
