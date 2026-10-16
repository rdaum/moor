object LLM_JOB [
  import_export_id -> "llm_job",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Job"
  parent: ROOT
  owner: ARCH_WIZARD

  property host (owner: ARCH_WIZARD, flags: "") = #-1;
  property requester (owner: ARCH_WIZARD, flags: "") = #-1;
  property reply_connection (owner: ARCH_WIZARD, flags: "") = 0;
  property query (owner: ARCH_WIZARD, flags: "") = "";
  property context (owner: ARCH_WIZARD, flags: "") = {};
  property presenter (owner: ARCH_WIZARD, flags: "") = #-1;
  property agent (owner: ARCH_WIZARD, flags: "") = #-1;
  property state (owner: ARCH_WIZARD, flags: "") = 'queued;
  property result (owner: ARCH_WIZARD, flags: "") = "";
  property worker_task (owner: ARCH_WIZARD, flags: "") = 0;
  property parent_task (owner: ARCH_WIZARD, flags: "") = 0;
  property timeout_seconds (owner: ARCH_WIZARD, flags: "") = 180;
  property deadline (owner: ARCH_WIZARD, flags: "") = 0;
  property finished_at (owner: ARCH_WIZARD, flags: "") = 0;
  property recovery_required (owner: ARCH_WIZARD, flags: "") = false;
  property history_taken (owner: ARCH_WIZARD, flags: "") = false;
  property tool_state (owner: ARCH_WIZARD, flags: "") = [];

  method new owner: ARCH_WIZARD
    "Consume one signed request and allocate its private anonymous execution state.";
    caller_perms().wizard || raise(E_PERM);
    const {room, task} = args;
    const claims = $agent_room:_consume_task(room, task);
    const destination = maphaskey(claims, "connection") ? claims["connection"] | 0;
    const job = $llm_job:new_run(room, claims["actor"], destination);
    job.query = claims["query"];
    job.context = claims["context"];
    return job;
  endmethod

  method new_run owner: ARCH_WIZARD
    "Allocate anonymous execution state for a trusted host and captured requester.";
    caller_perms().wizard || raise(E_PERM);
    const {host, actor, destination} = args;
    valid(host) && valid(actor) && isa(this, $llm_job) || raise(E_INVARG);
    const job = create(this, $arch_wizard, 1);
    job.host = host;
    job.requester = actor;
    job.reply_connection = destination;
    job.presenter = $llm_presenter:new(actor, destination, host);
    return job;
  endmethod

  method bind_agent owner: ARCH_WIZARD
    "Attach one anonymous agent before execution; the host retains the job until completion.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    this.state == 'queued && !valid(this.agent) || raise(E_INVARG, "Job already configured");
    const {agent} = args;
    valid(agent) && isa(agent, $llm_agent) && is_anonymous(agent)
      && (agent.owner == this.requester || this.requester.wizard) || raise(E_INVARG);
    !agent.running || raise(E_INVARG, "Agent already running.");
    this.agent = agent;
    agent.tool_callback = this;
    agent.compaction_callback = this;
  endmethod

  method run owner: ARCH_WIZARD
    "Run the bound agent with a tracked child and a bounded deadline.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    const {prompt, timeout} = args;
    this.state == 'queued && valid(this.agent) || raise(E_INVARG, "Job is not ready");
    typeof(timeout) == TYPE_INT && timeout >= 1 && timeout <= 3600 || raise(E_INVARG);
    this.state = 'running;
    this.parent_task = task_id();
    const deadline = time() + timeout;
    fork worker (0)
      try
        const result = this:_send(prompt);
        const status = this.agent.execution_status;
        this:_finish(status in {'complete, 'cancelled, 'unknown, 'waiting} ? status | 'failed, result);
      except e (ANY)
        this:_finish('failed, "Error: " + tostr(e[1]) + " - " + tostr(e[2]));
      endtry
      `task_send(this.parent_task, ["type" -> "job_done"]) ! E_INVARG, E_PERM';
    endfork
    this.worker_task = worker;
    while (this.state == 'running)
      task_recv(1);
      if (this.state == 'running && time() >= deadline)
        this:cancel('timed_out);
      endif
    endwhile
    this.worker_task = 0;
    this.parent_task = 0;
    return this.state;
  endmethod

  method _send owner: ARCH_WIZARD
    "Execute model tools under the captured request actor, independently of room display state.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {prompt} = args;
    const actor = this.requester;
    const agent = this.agent;
    this.state == 'running || raise(E_INVARG, "Job no longer running");
    set_task_perms(actor);
    return agent:send_message(prompt, false, actor);
  endmethod

  method _finish owner: ARCH_WIZARD
    "Record one terminal outcome; a late child cannot overwrite cancellation.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    this.state in {'queued, 'running} || return false;
    const {state, result} = args;
    state in {'complete, 'failed, 'cancelled, 'timed_out, 'unknown, 'waiting} || raise(E_INVARG);
    this.state = state;
    this.result = result;
    this.finished_at = time();
    if (valid(this.agent))
      if (this.agent.owner == this.requester || this.requester.wizard)
        this.context = `$llm_agent:context_snapshot(this.agent) ! ANY => {}';
      else
        this.context = {};
      endif
      this.recovery_required = length(this.agent.pending_tools) > 0;
    endif
    return true;
  endmethod

  method cancel owner: ARCH_WIZARD
    "Stop only this job's child. The requester, host owner, or wizard may cancel.";
    const principal = caller_perms();
    valid(principal) && (principal.wizard || principal == this.requester
      || (valid(this.host) && principal == this.host.owner)) || raise(E_PERM);
    const {?state = 'cancelled} = args;
    state in {'cancelled, 'timed_out} || raise(E_INVARG);
    this.state in {'queued, 'running} || return false;
    if (valid(this.agent))
      this.agent.cancel_requested = true;
    endif
    const worker = this.worker_task;
    this:_finish(state, state == 'timed_out ? "Error: Model task timed out." | "Operation cancelled.");
    if (worker > 0 && worker != task_id())
      `kill_task(worker) ! E_INVARG';
    endif
    this.worker_task = 0;
    return true;
  endmethod

  method fail owner: ARCH_WIZARD
    "Record host setup failure through the same terminal-state transition as model failures.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    const {message} = args;
    return this:_finish('failed, message);
  endmethod

  method report owner: ARCH_WIZARD
    "Send the final result to the connection captured for this request.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    this.state in {'complete, 'failed, 'cancelled, 'timed_out, 'unknown, 'waiting} || raise(E_INVARG);
    this:_notify(typeof(this.result) == TYPE_STR ? this.result | toliteral(this.result));
  endmethod

  method take_history owner: ARCH_WIZARD
    "Return a terminal history value once, without retaining the job or its agent in that value.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    this.state in {'complete, 'failed, 'cancelled, 'timed_out, 'unknown, 'waiting} || raise(E_INVARG, "Job is active");
    this.history_taken && return [];
    this.history_taken = true;
    return ['query -> this.query, 'requester -> this.requester, 'result -> this.result,
      'context -> this.context, 'status -> this.state, 'finished_at -> this.finished_at,
      'recovery_required -> this.recovery_required];
  endmethod

  method release owner: ARCH_WIZARD
    "Break agent callback cycles and release captured state after its history value is taken.";
    (caller == this || caller == this.host || caller == $llm_room_runtime) && caller_perms().wizard || raise(E_PERM);
    this.state in {'queued, 'running} && this:cancel();
    if (valid(this.agent))
      this.agent.tool_callback = #-1;
      this.agent.compaction_callback = #-1;
    endif
    if (valid(this.presenter))
      this.presenter:close();
    endif
    this.presenter = #-1;
    this.agent = #-1;
    this.context = {};
    this.result = "";
    this.tool_state = [];
    this.reply_connection = 0;
    this.parent_task = 0;
    this.worker_task = 0;
    this.host = #-1;
    this.requester = #-1;
    this.query = "";
  endmethod

  method _notify owner: ARCH_WIZARD
    "Deliver progress only to the captured connection; a detached session receives nothing.";
    caller == this || raise(E_PERM);
    const {message} = args;
    return $llm_presenter:send(this.presenter, this.requester, message);
  endmethod

  method deliver_callback owner: ARCH_WIZARD
    "Authenticate the source preserved by a canonical agent callback helper before dispatch.";
    const principal = caller_perms();
    caller in {$llm_agent} || raise(E_PERM);
    const {job, source, name, values} = args;
    source == job.agent && principal == job.requester || raise(E_PERM);
    name in {"on_tool_call", "on_tool_complete", "on_tool_error", "on_compaction_start", "on_compaction_end"}
      || raise(E_PERM);
    return job:(name)(@values);
  endmethod

  method on_tool_call owner: ARCH_WIZARD
    "Record tool progress only from this job's bound agent.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    const {name, ?parameters = ""} = args;
    this.tool_state = ['name -> name, 'status -> 'running];
    this:_notify("Running " + name + ".");
  endmethod

  method on_tool_complete owner: ARCH_WIZARD
    "Record successful tool completion without retaining its full payload.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    const {name, ?parameters = "", ?content = ""} = args;
    this.tool_state = ['name -> name, 'status -> 'complete];
  endmethod

  method on_tool_error owner: ARCH_WIZARD
    "Record failed tool completion without publishing arguments or private results.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    const {name, ?parameters = "", ?error = ""} = args;
    this.tool_state = ['name -> name, 'status -> 'failed];
    this:_notify("Tool failed: " + name + ".");
  endmethod

  method on_compaction_start owner: ARCH_WIZARD
    "Report compaction for this job's captured session.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    this:_notify("Compacting context.");
  endmethod

  method on_compaction_end owner: ARCH_WIZARD
    "Report the end of this job's compaction attempt.";
    caller == $llm_job && caller_perms().wizard || raise(E_PERM);
    this:_notify("Context compaction finished.");
  endmethod
endobject
