object LLM_OBSERVER_JOB [
  import_export_id -> "llm_observer_job",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Observer Job"
  parent: LLM_JOB
  owner: ARCH_WIZARD

  property actor_programmer (owner: ARCH_WIZARD, flags: "") = false;
  property actor_wizard (owner: ARCH_WIZARD, flags: "") = false;
  property actor_builder (owner: ARCH_WIZARD, flags: "") = false;
  property captured_client (owner: ARCH_WIZARD, flags: "") = #-1;
  property captured_model (owner: ARCH_WIZARD, flags: "") = false;
  property captured_options (owner: ARCH_WIZARD, flags: "") = false;
  property kind (owner: ARCH_WIZARD, flags: "") = 'observation;
  property room (owner: ARCH_WIZARD, flags: "") = #-1;
  property questioner (owner: ARCH_WIZARD, flags: "") = #-1;
  property observations (owner: ARCH_WIZARD, flags: "") = {};
  property ready_at (owner: ARCH_WIZARD, flags: "") = 0;
  property interaction_task (owner: ARCH_WIZARD, flags: "") = 0;
  property thinking_task (owner: ARCH_WIZARD, flags: "") = 0;
  property done (owner: ARCH_WIZARD, flags: "") = false;
  property retired (owner: ARCH_WIZARD, flags: "") = false;

  method check owner: ARCH_WIZARD
    "Check the captured NPC, room, requester connection, and lifetime before effects.";
    caller_perms().wizard || raise(E_PERM);
    const {job} = args;
    isa(job, $llm_observer_job) && !job.retired || raise(E_INVARG);
    const runtime = job.host;
    $llm_observer_runtime:check(runtime, job.kind == 'configure);
    runtime.current == job && runtime.subject == job.requester || raise(E_PERM);
    job.requester.programmer == job.actor_programmer && job.requester.wizard == job.actor_wizard
      && `job.requester.is_builder ! E_PROPNF => false' == job.actor_builder || raise(E_PERM, "NPC permissions changed.");
    job.requester.location == job.room || raise(E_PERM, "Observer moved.");
    time() < job.deadline || raise(E_QUOTA, "Observer request timed out.");
    if (valid(job.agent))
      runtime.agent == job.agent && job.agent.client == job.captured_client
        && job.agent.model_override == job.captured_model && job.agent.chat_opts == job.captured_options
        || raise(E_PERM, "Observer model configuration changed.");
    endif
    if (typeof(job.reply_connection) == TYPE_OBJ)
      valid(job.questioner) && job.reply_connection in {entry[1] for entry in (connections(job.questioner))}
        || raise(E_PERM, "Connection is no longer attached.");
    endif
    return true;
  endmethod

  method for_observer owner: ARCH_WIZARD
    "Resolve the active job for an authenticated NPC callback or tool; owners cannot impersonate it.";
    const {observer, principal} = args;
    $llm_tool_provider:_require_tool_dispatch(observer, caller_perms());
    $llm_tool_provider:_require_tool_dispatch(observer, principal);
    isa(observer, $llm_room_observer) && !is_clear_property(observer, "runtime") || raise(E_PERM);
    const runtime = observer.runtime;
    valid(runtime) && valid(runtime.current) || raise(E_INVARG, "No active observer job.");
    const job = runtime.current;
    $llm_observer_job:check(job);
    job.requester == observer && job.state == 'running || raise(E_PERM);
    return job;
  endmethod

  method bind_agent owner: ARCH_WIZARD
    "Bind the controller's agent while preserving the NPC as tool actor and billing principal.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    $llm_observer_job:check(this);
    const {agent} = args;
    this.state == 'queued && !valid(this.agent) || raise(E_INVARG);
    agent == this.host.agent && isa(agent, $llm_agent) && is_anonymous(agent)
      && agent.owner == this.host.controller && !agent.running || raise(E_PERM);
    this.captured_client = agent.client;
    this.captured_model = agent.model_override;
    this.captured_options = agent.chat_opts;
    this.agent = agent;
    agent.tool_callback = this;
    agent.compaction_callback = this;
  endmethod

  method perform owner: ARCH_WIZARD
    "Prepare one turn and use the shared job execution loop; frontend hooks stay in this owned worker.";
    caller == this.host && caller_perms().wizard || raise(E_PERM);
    $llm_observer_job:check(this);
    const runtime = this.host;
    const observer = this.requester;
    if (!valid(runtime.agent))
      const agent = create($llm_agent, runtime.controller, 1);
      runtime.agent = agent;
      observer.agent = agent;
      agent.model_override = observer.preferred_model == "" ? false | observer.preferred_model;
      this:_setup(observer, agent, runtime.controller);
      $llm_observer_job:check(this);
      runtime.agent_ready = true;
    endif
    if (this.kind == 'configure)
      this:_finish('complete, "Configured.");
      return;
    endif
    this:bind_agent(runtime.agent);
    length(this.agent.pending_tools) == 0 || raise(E_INVARG, "Unresolved tool outcomes require an explicit reset after inspection.");
    if (this.kind in {'observation, 'nudge})
      const ready = $llm_observer_policy:ready(observer);
      $llm_observer_job:check(this);
      if (!ready)
        this:_finish('complete, "SILENT");
        return;
      endif
    endif
    let observations = {};
    for input in (this.observations)
      try
        const observation = input:render();
        if (typeof(observation) == TYPE_MAP)
          const pending = {@observations, observation};
          value_bytes(pending) <= 32768 || raise(E_QUOTA, "Rendered observations exceed 32 KiB.");
          observations = pending;
        endif
      except error (ANY)
        $llm_observation:record_error(observer, error[1]);
      endtry
      $llm_observer_job:check(this);
    endfor
    if (this.kind == 'observation && length(observations) == 0)
      this:_finish('complete, "SILENT");
      return;
    endif
    let addressed = false;
    for observation in (observations)
      this.agent:add_message("user", "OBSERVATION: " + observation["content"]);
      addressed = addressed || observation["addressed"];
    endfor
    this:_trim_observations(false);
    let engage = this.kind in {'ask, 'poke} || addressed;
    if (!engage)
      const cooldown = observer.speak_cooldown;
      typeof(cooldown) == TYPE_INT && cooldown >= 0 && cooldown <= 3600 || raise(E_INVARG);
      if (time() - runtime.last_spoke_at >= cooldown)
        engage = $llm_observer_policy:triage(observer);
      endif
    endif
    $llm_observer_job:check(this);
    if (!engage)
      this:_finish('complete, "SILENT");
      return;
    endif
    let prompt = observer.response_prompt;
    if (this.kind == 'ask)
      prompt = "Answer this direct question from the room: " + generate_json([
        "actor" -> tostr(this.questioner), "question" -> this.query]);
    endif
    value_bytes(prompt) <= 32768 || raise(E_QUOTA);
    this:_start_thinking();
    this:run(prompt, max(1, this.deadline - time()));
    $llm_observer_job:check(this);
    this:_stop_thinking();
    if (this.state == 'complete)
      const speech = $llm_observer_speech:mk(this.result);
      "Models often say a line with a tool and then return it as the reply; the room heard it once.";
      const presented = {$llm_observer_speech:mk(text).text for text in (this.agent.presented_text)};
      if (length(speech.text) && speech.text in presented)
        runtime.last_spoke_at = time();
      elseif (speech:announce(observer, this.room))
        runtime.last_spoke_at = time();
      endif
      this:_trim_observations(true);
      if (valid(this.questioner))
        const usage = this.agent.last_token_usage;
        if (typeof(usage) == TYPE_MAP && maphaskey(usage, "total_tokens"))
          this:_notify("[TOKENS] Last call: " + tostr(usage["total_tokens"]) + ". Charged to " + tostr(observer) + ".");
        endif
      endif
    else
      this:report_error();
    endif
  endmethod

  method _setup owner: ARCH_WIZARD
    "Call the persona configuration hook under the captured controller's permissions.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {observer, agent, controller} = args;
    set_task_perms(controller);
    observer:_setup_agent(agent);
  endmethod

  method _send owner: ARCH_WIZARD
    "Delegate exactly this checked agent request to the NPC, distinct from its controller.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    $llm_observer_job:check(this);
    const {prompt} = args;
    return this.agent:send_message(prompt, false, this.requester);
  endmethod

  method _trim_observations owner: ARCH_WIZARD
    "Keep at most six recent observation messages and 32 KiB across declined batches.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {clear} = args;
    const context = this.agent:get_context();
    let kept = {};
    let recent = {};
    for message in (context)
      if (message["role"] == "user" && typeof(`message["content"] ! E_RANGE => false') == TYPE_STR
          && message["content"]:starts_with("OBSERVATION:"))
        if (!clear)
          recent = {@recent, message};
          while (length(recent) > 6 || value_bytes(recent) > 32768)
            recent = listdelete(recent, 1);
          endwhile
        endif
      else
        kept = {@kept, message};
      endif
    endfor
    this.agent:set_context({@kept, @recent});
  endmethod

  method _start_thinking owner: ARCH_WIZARD
    "Own a separate presentation task so a slow room cannot block lifecycle monitoring.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const delay = this.requester.thinking_delay;
    typeof(delay) == TYPE_INT && delay >= 0 && delay <= 3600 || raise(E_INVARG);
    fork thinking (delay)
      this:_think();
    endfork
    this.thinking_task = thinking;
  endmethod

  method _think owner: ARCH_WIZARD
    "Publish persona thinking messages only while the captured job remains current.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    for attempt in [1..20]
      if (this.thinking_task == task_id())
        break;
      endif
      this.retired && return;
      suspend(0.1);
    endfor
    let index = 1;
    while (!this.retired && this.thinking_task == task_id())
      $llm_observer_job:check(this);
      const observer = this.requester;
      const messages = observer.thinking_messages;
      const interval = observer.thinking_interval;
      typeof(interval) == TYPE_INT && interval >= 1 && interval <= 60 || return;
      typeof(messages) == TYPE_LIST && length(messages) > 0 && value_bytes(messages) <= 16384 || return;
      const text = messages[(index - 1) % length(messages) + 1];
      this:_emote(text);
      index = index + 1;
      suspend(interval);
    endwhile
  endmethod

  method _emote owner: ARCH_WIZARD
    "Use NPC authority for persona event construction and room delivery.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {text} = args;
    const observer = this.requester;
    const room = this.room;
    $llm_observer_job:check(this);
    set_task_perms(observer);
    const event = observer:mk_emote_event(text);
    $llm_observer_job:check_callback(this, observer);
    room:announce(event);
  endmethod

  method _stop_thinking owner: ARCH_WIZARD
    "Stop only the task allocated for this job.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const thinking = this.thinking_task;
    this.thinking_task = 0;
    thinking > 0 && thinking != task_id() && `kill_task(thinking) ! E_INVARG';
  endmethod

  method check_callback owner: ARCH_WIZARD
    "Recheck a canonical job after a persona callback returns with NPC permissions.";
    const {job, actor} = args;
    caller == job && caller_perms() == actor && job.requester == actor || raise(E_PERM);
    return $llm_observer_job:check(job);
  endmethod

  method _forward owner: ARCH_WIZARD
    "Validate before and after each persona hook; revoked work stops before tool dispatch.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {name, values} = args;
    try
      $llm_observer_job:check(this);
      this:_call_persona(name, values);
      $llm_observer_job:check(this);
    except error (ANY)
      valid(this.agent) && (this.agent.cancel_requested = true);
      raise(error[1], error[2]);
    endtry
  endmethod

  method _call_persona owner: ARCH_WIZARD
    "Invoke optional persona behavior with the captured NPC principal.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {name, values} = args;
    const observer = this.requester;
    set_task_perms(observer);
    if (respond_to(observer, name))
      observer:(name)(@values);
    endif
  endmethod

  method on_tool_call owner: ARCH_WIZARD
    "Forward checked tool progress to the persona.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    return this:_forward("on_tool_call", args);
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Forward a recorded tool outcome to the persona.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    return this:_forward("on_tool_complete", args);
  endmethod

  method on_tool_error owner: ARCH_WIZARD
    "Forward a recorded tool failure to the persona.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    return this:_forward("on_tool_error", args);
  endmethod

  method on_compaction_start owner: ARCH_WIZARD
    "Forward compaction progress to the persona.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    return this:_forward("on_compaction_start", args);
  endmethod

  method on_compaction_end owner: ARCH_WIZARD
    "Forward memory restoration and persona output after compaction.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    return this:_forward("on_compaction_end", args);
  endmethod

  method fail owner: ARCH_WIZARD
    "Record an output failure even after the model completes; cancellation remains terminal.";
    (caller == this || caller == this.host) && caller_perms().wizard || raise(E_PERM);
    if (this.state == 'complete)
      const {message} = args;
      this.state = 'failed;
      this.result = message;
      this.finished_at = time();
      return true;
    endif
    return pass(@args);
  endmethod

  method report_error owner: ARCH_WIZARD
    "Report failures to the captured questioner without publishing diagnostics to the room.";
    (caller == this || caller == this.host) && caller_perms().wizard || raise(E_PERM);
    $llm_observer_job:check(this);
    valid(this.questioner) && this:_notify(this.result);
  endmethod

  method _notify owner: ARCH_WIZARD
    "Deliver usage and errors to the original questioner's connection.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {text} = args;
    $llm_observer_job:check(this);
    valid(this.questioner) || return false;
    return $llm_presenter:send(this.presenter, this.questioner, text);
  endmethod

  method release owner: ARCH_WIZARD
    "Release queued input, callbacks, tasks, and destinations after runtime retirement.";
    caller == this.host && caller_perms().wizard || raise(E_PERM);
    this:_stop_thinking();
    pass();
    this.questioner = #-1;
    this.room = #-1;
    this.observations = {};
    this.captured_client = #-1;
    this.captured_model = false;
    this.captured_options = false;
    this.interaction_task = 0;
    this.retired = true;
  endmethod
endobject
