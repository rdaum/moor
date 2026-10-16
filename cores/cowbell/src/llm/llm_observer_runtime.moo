object LLM_OBSERVER_RUNTIME [
  import_export_id -> "llm_observer_runtime",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Observer Runtime"
  parent: ROOT
  owner: ARCH_WIZARD

  property subject (owner: ARCH_WIZARD, flags: "") = #-1;
  property controller (owner: ARCH_WIZARD, flags: "") = #-1;
  property agent (owner: ARCH_WIZARD, flags: "") = #-1;
  property agent_ready (owner: ARCH_WIZARD, flags: "") = false;
  property queue (owner: ARCH_WIZARD, flags: "") = {};
  property current (owner: ARCH_WIZARD, flags: "") = #-1;
  property monitor_task (owner: ARCH_WIZARD, flags: "") = 0;
  property closed (owner: ARCH_WIZARD, flags: "") = false;
  property history (owner: ARCH_WIZARD, flags: "") = {};
  property last_spoke_at (owner: ARCH_WIZARD, flags: "") = 0;
  property dropped_events (owner: ARCH_WIZARD, flags: "") = 0;

  method for_observer owner: ARCH_WIZARD
    "Create private state for this observer; never adopt a prototype's live runtime or agent.";
    const principal = caller_perms();
    const {observer} = args;
    valid(observer) && isa(observer, $llm_room_observer) || raise(E_TYPE);
    principal == observer.owner || principal == observer || principal.wizard || raise(E_PERM);
    if (!is_clear_property(observer, "runtime") && valid(observer.runtime))
      const previous = observer.runtime;
      if (!previous.closed && previous.subject == observer && previous.controller == observer.owner)
        return previous;
      endif
      previous:close();
    endif
    const runtime = create($llm_observer_runtime, $arch_wizard, 1);
    runtime.subject = observer;
    runtime.controller = observer.owner;
    if (!is_clear_property(observer, "agent") && valid(observer.agent))
      const agent = observer.agent;
      isa(agent, $llm_agent) && is_anonymous(agent) && agent.owner == observer.owner
        && !agent.running || raise(E_PERM, "Observer agent must be private and idle.");
      runtime.agent = agent;
      runtime.agent_ready = true;
    else
      observer.agent = #-1;
    endif
    observer.runtime = runtime;
    return runtime;
  endmethod

  method check owner: ARCH_WIZARD
    "Validate captured ownership and bindings without invoking observer overrides.";
    caller_perms().wizard || raise(E_PERM);
    const {runtime, ?allow_disabled = false} = args;
    isa(runtime, $llm_observer_runtime) && !runtime.closed || raise(E_INVARG, "Observer runtime is closed.");
    const observer = runtime.subject;
    valid(observer) && observer.owner == runtime.controller && observer.runtime == runtime
      && (observer.enabled || allow_disabled) || raise(E_PERM, "Observer binding changed.");
    if (valid(runtime.agent))
      observer.agent == runtime.agent && runtime.agent.owner == runtime.controller || raise(E_PERM, "Observer agent changed.");
    else
      !valid(observer.agent) || raise(E_PERM, "Observer agent changed.");
    endif
    return true;
  endmethod

  method submit owner: ARCH_WIZARD
    "Bound queue admission and coalesce adjacent events without extending their original deadline.";
    caller_perms().wizard || raise(E_PERM);
    const {kind, payload, ?questioner = #-1, ?destination = 0} = args;
    $llm_observer_runtime:check(this, kind == 'configure);
    kind in {'observation, 'ask, 'poke, 'nudge, 'configure} || raise(E_INVARG);
    value_bytes(payload) <= 16384 || raise(E_QUOTA, "Observer input exceeds 16 KiB.");
    if (kind == 'observation)
      typeof(payload) == TYPE_FLYWEIGHT && payload.delegate == $llm_observation
        && payload.observer == this.subject && payload:current() || raise(E_TYPE);
      if (length(this.queue) > 0)
        const last = this.queue[$];
        if (last.kind == 'observation && last.room == this.subject.location)
          if (length(last.observations) < 6 && value_bytes({@last.observations, payload}) <= 32768)
            last.observations = {@last.observations, payload};
            this:_ensure_monitor();
            return last;
          endif
          this.dropped_events = min(this.dropped_events + 1, 1000000);
          return false;
        endif
      endif
    elseif (kind == 'ask)
      typeof(payload) == TYPE_STR && length(payload) > 0 && valid(questioner) || raise(E_INVARG);
    endif
    if (length(this.queue) + (valid(this.current) ? 1 | 0) >= 16)
      if (kind in {'observation, 'nudge})
        this.dropped_events = min(this.dropped_events + 1, 1000000);
        return false;
      endif
      raise(E_QUOTA, "Observer queue is full.");
    endif
    if (kind in {'ask, 'poke})
      let pending = 0;
      for job in (this.queue)
        job.questioner == questioner && (pending = pending + 1);
      endfor
      if (valid(this.current) && this.current.questioner == questioner)
        pending = pending + 1;
      endif
      pending < 2 || raise(E_QUOTA, "You already have two pending observer requests.");
    endif
    const timeout = this.subject.response_timeout;
    typeof(timeout) == TYPE_INT && timeout >= 1 && timeout <= 3600 || raise(E_INVARG);
    const delay = this.subject.speak_delay;
    typeof(delay) == TYPE_INT && delay >= 0 && delay <= 30 || raise(E_INVARG);
    const job = $llm_observer_job:new_run(this, this.subject, 0);
    job.actor_programmer = this.subject.programmer;
    job.actor_wizard = this.subject.wizard;
    job.actor_builder = `this.subject.is_builder ! E_PROPNF => false';
    job.kind = kind;
    job.room = this.subject.location;
    job.questioner = questioner;
    job.reply_connection = destination;
    job.deadline = time() + timeout;
    job.ready_at = time() + (kind == 'observation ? delay | 0);
    if (kind == 'observation)
      job.observations = {payload};
    elseif (kind == 'ask)
      job.query = payload;
    endif
    if (valid(questioner))
      job.presenter:close();
      job.presenter = $llm_presenter:new(questioner, destination, this.subject);
    endif
    this.queue = {@this.queue, job};
    this:_ensure_monitor();
    return job;
  endmethod

  method _ensure_monitor owner: ARCH_WIZARD
    "Start one monitor in a separate activation so callers can use finally safely.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    valid_task(this.monitor_task) && return;
    fork monitor (0)
      this:_monitor();
    endfork
    if (this.closed || valid_task(this.monitor_task))
      `kill_task(monitor) ! E_INVARG';
    else
      this.monitor_task = monitor;
    endif
  endmethod

  method _spawn owner: ARCH_WIZARD
    "Own setup, triage, model execution, and output as one bounded worker.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {job} = args;
    this.current = job;
    fork worker (0)
      this:_work(job);
    endfork
    job.interaction_task = worker;
  endmethod

  method _work owner: ARCH_WIZARD
    "Finish or record failure; the monitor retires the worker without calling frontend code.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {job} = args;
    "A fork can start before the spawning transaction records its task ID.";
    for attempt in [1..20]
      if (job.interaction_task == task_id())
        break;
      endif
      (job.retired || this.closed) && return;
      suspend(0.1);
    endfor
    job.interaction_task == task_id() || return;
    try
      job:perform();
    except error (ANY)
      job:fail("Observer request failed: " + tostr(error[1]) + " - " + tostr(error[2]));
      `job:report_error() ! ANY';
    endtry
    job.done = true;
  endmethod

  method _monitor owner: ARCH_WIZARD
    "Reconcile dead tasks and admission deadlines; never replay interrupted tool effects.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    for attempt in [1..20]
      if (this.monitor_task == task_id())
        break;
      endif
      this.closed || valid_task(this.monitor_task) && return;
      suspend(0.1);
    endfor
    while (!this.closed && this.monitor_task == task_id())
      const configuring = valid(this.current) ? this.current.kind == 'configure
        | length(this.queue) > 0 && this.queue[1].kind == 'configure;
      if (!`$llm_observer_runtime:check(this, configuring) ! ANY => false')
        this:close();
        return;
      endif
      if (valid(this.current))
        const job = this.current;
        if (job.done)
          this:_retire(job);
        elseif (time() >= job.deadline)
          this:_retire(job, 'timed_out);
        elseif (!valid_task(job.interaction_task)
            || !`$llm_observer_job:check(job) ! ANY => false')
          this:_retire(job, 'cancelled);
        endif
      endif
      for job in (this.queue)
        if (time() >= job.deadline)
          this.queue = setremove(this.queue, job);
          this:_retire(job, 'timed_out);
        endif
      endfor
      if (!valid(this.current) && length(this.queue) > 0)
        const job = this.queue[1];
        if (time() >= job.ready_at)
          this.queue = listdelete(this.queue, 1);
          this:_spawn(job);
        endif
      endif
      if (!valid(this.current) && length(this.queue) == 0)
        this.monitor_task = 0;
        return;
      endif
      suspend(0.1);
    endwhile
  endmethod

  method _retire owner: ARCH_WIZARD
    "Stop owned tasks and retain only twenty small outcome records.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {job, ?stop = false} = args;
    const owned_tasks = {job.worker_task, job.interaction_task, job.thinking_task};
    if (stop != false)
      job:cancel(stop);
      job.state = stop;
    endif
    for task in (owned_tasks)
      task > 0 && task != task_id() && `kill_task(task) ! E_INVARG';
    endfor
    if (valid(job.agent))
      if (!(task_id() in owned_tasks))
        job.agent.running = false;
        job.agent.current_iteration = 0;
      endif
      job.recovery_required = length(job.agent.pending_tools) > 0;
    endif
    if (!this.agent_ready && valid(this.agent))
      if (valid(this.subject) && this.subject.agent == this.agent)
        this.subject.agent = #-1;
      endif
      this.agent = #-1;
    endif
    this.history = {@this.history, ['kind -> job.kind, 'status -> job.state,
      'finished_at -> time(), 'recovery_required -> job.recovery_required,
      'error -> job.state == 'failed && typeof(job.result) == TYPE_STR ? job.result[1..min(length(job.result), 1024)] | ""]};
    length(this.history) > 20 && (this.history = listdelete(this.history, 1));
    this.current == job && (this.current = #-1);
    job:release();
    return task_id() in owned_tasks;
  endmethod

  method close owner: ARCH_WIZARD
    "Cancel this runtime's work and release its references without touching a replacement runtime.";
    caller_perms().wizard || raise(E_PERM);
    this.closed && return;
    this.closed = true;
    const monitor = this.monitor_task;
    const agent = this.agent;
    this.monitor_task = 0;
    monitor > 0 && monitor != task_id() && `kill_task(monitor) ! E_INVARG';
    let end_current = false;
    if (valid(this.current))
      end_current = this:_retire(this.current, 'cancelled);
    endif
    for job in (this.queue)
      this:_retire(job, 'cancelled);
    endfor
    this.queue = {};
    this.agent = #-1;
    this.subject = #-1;
    this.controller = #-1;
    if (end_current)
      this:_end_caller(task_id(), agent);
    endif
  endmethod

  method _end_caller owner: ARCH_WIZARD
    "Commit cleanup before a one-shot task terminates the worker that requested its own stop.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {task, agent} = args;
    task == task_id() || raise(E_PERM);
    fork (0)
      `kill_task(task) ! E_INVARG';
      if (valid(agent) && !valid(agent.tool_callback))
        agent.running = false;
        agent.current_iteration = 0;
      endif
    endfork
  endmethod

  method stop owner: ARCH_WIZARD
    "Stop only the observer's local runtime; descendants cannot stop their prototype's tasks.";
    const principal = caller_perms();
    const {observer} = args;
    valid(observer) && isa(observer, $llm_room_observer) || raise(E_TYPE);
    principal == observer.owner || principal.wizard || raise(E_PERM);
    if (!is_clear_property(observer, "runtime") && valid(observer.runtime))
      observer.runtime:close();
    endif
    observer.runtime = #-1;
  endmethod

  method snapshot owner: ARCH_WIZARD
    "Expose bounded diagnostics only to the observer owner or a wizard.";
    const principal = caller_perms();
    const {observer} = args;
    valid(observer) && isa(observer, $llm_room_observer) || raise(E_TYPE);
    principal == observer.owner || principal.wizard || raise(E_PERM);
    const failures = is_clear_property(observer, "intake_failures") ? 0 | observer.intake_failures;
    const last_error = is_clear_property(observer, "last_intake_error") ? [] | observer.last_intake_error;
    const runtime = is_clear_property(observer, "runtime") ? #-1 | observer.runtime;
    if (!valid(runtime) || runtime.closed)
      return ['active -> false, 'queued -> 0, 'monitor_alive -> false, 'dropped_events -> 0, 'history -> {},
        'intake_failures -> failures, 'last_intake_error -> last_error];
    endif
    return ['active -> valid(runtime.current), 'queued -> length(runtime.queue),
      'monitor_alive -> valid_task(runtime.monitor_task), 'dropped_events -> runtime.dropped_events,
      'history -> runtime.history, 'intake_failures -> failures,
      'last_intake_error -> last_error];
  endmethod
endobject
