object LLM_SUGGESTION [
  import_export_id -> "llm_suggestion",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Suggestion Job"
  parent: LLM_JOB
  owner: ARCH_WIZARD

  property client (owner: ARCH_WIZARD, flags: "") = #-1;
  property request (owner: ARCH_WIZARD, flags: "") = [];
  property input_snapshot (owner: ARCH_WIZARD, flags: "") = [];
  property actor_owner (owner: ARCH_WIZARD, flags: "") = #-1;
  property actor_programmer (owner: ARCH_WIZARD, flags: "") = false;
  property actor_builder (owner: ARCH_WIZARD, flags: "") = false;
  property actor_wizard (owner: ARCH_WIZARD, flags: "") = false;
  property monitor_task (owner: ARCH_WIZARD, flags: "") = 0;
  property rewrite_id (owner: ARCH_WIZARD, flags: "") = "";
  property fallback (owner: ARCH_WIZARD, flags: "") = "";
  property failure (owner: ARCH_WIZARD, flags: "") = "";
  property closed (owner: ARCH_WIZARD, flags: "") = false;

  method offer owner: ARCH_WIZARD
    "Start a suggestion or display its quota error on the captured connection; return whether handled.";
    const {actor, destination, request} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    this in {$llm_doc_suggestion, $llm_help_suggestion, $llm_command_suggestion} || raise(E_INVARG);
    set_task_perms(actor);
    try
      return typeof(this:start(actor, destination, request)) == TYPE_OBJ;
    except error (E_QUOTA)
      actor:inform_connection(destination,
        $event:mk_error(actor, error[2]):with_audience('utility));
      return true;
    endtry
  endmethod

  method start owner: ARCH_WIZARD
    "Admit at most two anonymous suggestions per player, bound to an attached connection for 30 seconds.";
    const principal = caller_perms();
    const {actor, destination, request} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    this in {$llm_doc_suggestion, $llm_help_suggestion, $llm_command_suggestion} && is_player(actor) || raise(E_INVARG);
    this:validate_request(request, actor);
    typeof(destination) == TYPE_OBJ && destination in {entry[1] for entry in (connections(actor))}
      || raise(E_PERM, "Suggestion connection is not attached to the requester.");
    const client = $player.suggestions_llm_client;
    typeof(client) == TYPE_OBJ && valid(client) || return false;
    if (is_clear_property(actor, "llm_suggestions"))
      actor.llm_suggestions = {};
    endif
    for previous in (actor.llm_suggestions)
      if (previous.closed || !valid_task(previous.worker_task) || time() >= previous.deadline)
        previous:close('cancelled);
      endif
    endfor
    length(actor.llm_suggestions) < 2 || raise(E_QUOTA, "Two LLM suggestions are active. Use stop or wait.");
    const job = this:new_run(this, actor, destination);
    job.client = client;
    job.request = request;
    job.actor_owner = actor.owner;
    job.actor_programmer = actor.programmer;
    job.actor_wizard = actor.wizard;
    job.actor_builder = actor.is_builder;
    job.deadline = time() + 30;
    job.rewrite_id = uuid();
    actor.llm_suggestions = {@actor.llm_suggestions, job};
    job:_launch();
    return job;
  endmethod

  method check owner: ARCH_WIZARD
    "Check the original actor, role, client, destination, and deadline without invoking user behavior.";
    const {job} = args;
    isa(job, $llm_suggestion) && !job.closed || raise(E_INVARG, "Suggestion has closed.");
    const actor = job.requester;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    !is_clear_property(actor, "llm_suggestions") && job in actor.llm_suggestions || raise(E_PERM);
    actor.owner == job.actor_owner && actor.programmer == job.actor_programmer
      && actor.wizard == job.actor_wizard && actor.is_builder == job.actor_builder || raise(E_PERM, "Suggestion authority changed.");
    valid(job.client) && $player.suggestions_llm_client == job.client || raise(E_PERM, "Suggestion client changed.");
    job.reply_connection in {entry[1] for entry in (connections(actor))} || raise(E_PERM, "Suggestion connection detached.");
    time() < job.deadline || raise(E_QUOTA, "Suggestion timed out.");
    return true;
  endmethod

  method _launch owner: ARCH_WIZARD
    "Own both execution and monitoring; account for forks that run before their IDs are stored.";
    caller == this.host && caller_perms().wizard || raise(E_PERM);
    this.state = 'running;
    fork worker (0)
      this:_work();
    endfork
    if (this.closed)
      `kill_task(worker) ! E_INVARG';
      return;
    endif
    this.worker_task = worker;
    fork monitor (0)
      this:_monitor();
    endfork
    if (this.closed)
      `kill_task(monitor) ! E_INVARG';
    else
      this.monitor_task = monitor;
    endif
  endmethod

  method _monitor owner: ARCH_WIZARD
    "Cancel abandoned, revoked, or overdue work without entering presentation callbacks.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    for attempt in [1..20]
      this.closed && return;
      if (this.monitor_task == task_id())
        break;
      endif
      suspend(0.1);
    endfor
    while (!this.closed && this.monitor_task == task_id())
      if (time() >= this.deadline)
        this:close('timed_out);
        return;
      endif
      try
        $llm_suggestion:check(this);
        valid_task(this.worker_task) || raise(E_INVARG);
      except (ANY)
        this:close('cancelled);
        return;
      endtry
      suspend(0.1);
    endwhile
  endmethod

  method _work owner: ARCH_WIZARD
    "Collect, request, validate, and render inside one owned worker; failures retain the plain fallback.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    let outcome = 'failed;
    try
      try
        $llm_suggestion:check(this);
        this.fallback = this:_as_requester("fallback_text", {});
        this:_show(this:_as_requester("initial_text", {}), true);
        this.input_snapshot = this:_as_requester("collect_context", {});
        $llm_suggestion:check(this);
        value_bytes(this.input_snapshot) <= 65536 || raise(E_QUOTA, "Suggestion context is too large.");
        const response = this:_query();
        $llm_suggestion:check(this);
        const current = this:_as_requester("collect_context", {});
        current == this.input_snapshot && strcmp(toliteral(current), toliteral(this.input_snapshot)) == 0
          || raise(E_PERM, "LLM suggestion context changed.");
        typeof(response) == TYPE_STR && value_bytes(response) <= 8192 || raise(E_INVARG, "Invalid suggestion response.");
        const content = this:_as_requester("render_response", {response});
        this:_show(content, false);
        outcome = 'complete;
      except error (ANY)
        const diagnostic = tostr(error[1]) + ": " + tostr(error[2]);
        this.failure = diagnostic[1..min(length(diagnostic), 1024)];
        try
          $llm_suggestion:check(this);
          `this:_show(this.fallback, false) ! ANY';
        except (ANY)
          outcome = time() >= this.deadline ? 'timed_out | 'cancelled;
        endtry
      endtry
    finally
      this:close(outcome);
    endtry
  endmethod

  method _as_requester owner: ARCH_WIZARD
    "Run the concrete suggestion's context and presentation policy under the captured requester.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {name, values} = args;
    name in {"fallback_text", "initial_text", "collect_context", "render_response"} || raise(E_INVARG);
    $llm_suggestion:check(this);
    set_task_perms(this.requester);
    return this:(name)(@values);
  endmethod

  method _query owner: ARCH_WIZARD
    "Use the shared provider and accounting boundary with the original suggestion actor.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    $llm_suggestion:check(this);
    const actor = this.requester;
    const client = this.client;
    const prompt = this.input_snapshot['prompt];
    set_task_perms(actor);
    return $llm_provider:simple_as(client, actor, 'suggestion, prompt);
  endmethod

  method _show owner: ARCH_WIZARD
    "Deliver a bounded placeholder or replacement only while its captured job remains valid.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {content, initial} = args;
    $llm_suggestion:check(this);
    const actor = this.requester;
    const presenter = this.presenter;
    const rewrite_id = this.rewrite_id;
    const remaining = max(1, this.deadline - time());
    const fallback = this.fallback;
    set_task_perms(actor);
    let event = $event:mk_info(actor, content);
    if (initial)
      event = event:with_rewritable(rewrite_id, remaining, fallback):with_presentation_hint('processing);
    else
      event = event:with_metadata('rewrite_target, rewrite_id):as_inset();
    endif
    $llm_suggestion:check(this);
    return $llm_presenter:send_event(presenter, actor, event);
  endmethod

  method cancel_all owner: ARCH_WIZARD
    "Cancel only the authenticated player's outstanding LLM suggestions; return their count.";
    const {actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    is_player(actor) || raise(E_INVARG);
    is_clear_property(actor, "llm_suggestions") && return 0;
    const jobs = actor.llm_suggestions;
    for job in (jobs)
      job:close('cancelled);
    endfor
    return length(jobs);
  endmethod

  method close owner: ARCH_WIZARD
    "Finish once, kill owned tasks, and release request, client, connection, and actor references.";
    (caller == this || caller == $llm_suggestion || caller == this.host) && caller_perms().wizard || raise(E_PERM);
    this.closed && return false;
    const {state} = args;
    state in {'complete, 'failed, 'cancelled, 'timed_out} || raise(E_INVARG);
    this.closed = true;
    const worker = this.worker_task;
    const monitor = this.monitor_task;
    this:_finish(state, "");
    if (valid(this.requester))
      this.requester.llm_suggestions = setremove(this.requester.llm_suggestions, this);
    endif
    for task in ({worker, monitor})
      task > 0 && task != task_id() && `kill_task(task) ! E_INVARG';
    endfor
    this.client = #-1;
    this.request = [];
    this.input_snapshot = [];
    this.actor_owner = #-1;
    this.monitor_task = 0;
    this.rewrite_id = "";
    this.fallback = "";
    this:release();
    worker == task_id() && this:_end_caller(worker);
    return true;
  endmethod

  method _end_caller owner: ARCH_WIZARD
    "Terminate a self-cancelling worker after cleanup commits, using a separate fork activation.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {task} = args;
    fork (0)
      `kill_task(task) ! E_INVARG';
    endfork
  endmethod
endobject
