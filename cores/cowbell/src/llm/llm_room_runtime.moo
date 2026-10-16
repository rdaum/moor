object LLM_ROOM_RUNTIME [
  import_export_id -> "llm_room_runtime",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Room Runtime"
  parent: ROOT
  owner: ARCH_WIZARD

  method authenticate owner: ARCH_WIZARD
    "Validate the incoming principal, explicit actor, and room before reading private runtime state.";
    const principal = caller_perms();
    const {room, actor} = args;
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    typeof(room) == TYPE_OBJ && valid(room) && isa(room, $agent_room) || raise(E_TYPE);
    $llm_room_runtime:initialize_state(room);
    return actor;
  endmethod

  method initialize_state owner: ARCH_WIZARD
    "Materialize private runtime fields so a room never inherits its prototype's live requests.";
    caller_perms().wizard || raise(E_PERM);
    const {room} = args;
    typeof(room) == TYPE_OBJ && valid(room) && isa(room, $agent_room) || raise(E_TYPE);
    for field in ({"jobs", "history", "loop_task", "spent_requests"})
      if (is_clear_property(room, field))
        room.(field) = field == "loop_task" ? 0 | field == "spent_requests" ? [] | {};
      endif
    endfor
  endmethod

  method limits owner: ARCH_WIZARD
    "Validate configurable limits against hard bounds before admission or dispatch.";
    const {room} = args;
    for setting in ({
      {room.max_workers, 1, 8}, {room.max_jobs, 1, 32},
      {room.max_pending_per_player, 1, 8}, {room.max_query_length, 1, 16384},
      {room.max_history_context_items, 0, 4}, {room.task_timeout_seconds, 1, 3600},
      {room.max_iterations, 1, 100}})
      typeof(setting[1]) == TYPE_INT && setting[1] >= setting[2] && setting[1] <= setting[3]
        || raise(E_INVARG, "Invalid room execution limit");
    endfor
    return true;
  endmethod

  method submit owner: ARCH_WIZARD
    "Admit a request before starting or waking the monitor; queued jobs count toward every limit.";
    const principal = caller_perms();
    const {room, actor, query, ?context = {}, ?destination = 0} = args;
    set_task_perms(principal);
    $llm_room_runtime:authenticate(room, actor);
    let request = ['player -> actor, 'query -> query, 'context -> context, 'connection -> destination];
    request["authorization"] = $agent_room:_authorize_task(room, request);
    return $llm_room_runtime:_enqueue(room, request);
  endmethod

  method _enqueue owner: ARCH_WIZARD
    "Consume the signed request and store it before the fork commits admission.";
    this == $llm_room_runtime && caller == this || raise(E_PERM);
    const {room, request} = args;
    this:_admit(room, request);
    this:_ensure_monitor(room);
    return true;
  endmethod

  method _admit owner: ARCH_WIZARD
    "Allocate one anonymous job after query, context, global, and requester limits pass.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room, request} = args;
    this:initialize_state(room);
    this:limits(room);
    const query = request['query];
    const context = maphaskey(request, 'context) ? request['context] | {};
    typeof(query) == TYPE_STR && length(query) > 0 && length(query) <= room.max_query_length
      || raise(E_INVARG, "Task text is empty or too long");
    typeof(context) == TYPE_LIST && length(context) <= 256 && value_bytes(context) <= 1048576
      || raise(E_QUOTA, "Task context exceeds the room limit");
    length(room.jobs) < room.max_jobs || raise(E_QUOTA, "Room task queue is full");
    const own = {candidate for candidate in (room.jobs) if (candidate.requester == request['player])};
    length(own) < room.max_pending_per_player || raise(E_QUOTA, "Requester task limit reached");
    const job = $llm_job:new(room, request);
    job.timeout_seconds = room.task_timeout_seconds;
    room.jobs = {@room.jobs, job};
    return job;
  endmethod

  method execute owner: ARCH_WIZARD
    "Submit a signed request and wait for retirement through the same bounded worker pool.";
    caller_perms().wizard || raise(E_PERM);
    const {room, request} = args;
    const job = this:_admit(room, request);
    this:_ensure_monitor(room);
    while (job in room.jobs)
      task_recv(1);
      this:_ensure_monitor(room);
    endwhile
    return true;
  endmethod

  method _ensure_monitor owner: ARCH_WIZARD
    "Keep one monitor for admitted jobs. Forking commits the preceding admission transaction.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room} = args;
    length(room.jobs) > 0 || return;
    if (room.loop_task > 0 && valid_task(room.loop_task))
      return;
    endif
    fork monitor (0)
      $llm_room_runtime:_monitor(room);
    endfork
    room.loop_task = monitor;
  endmethod

  method _spawn owner: ARCH_WIZARD
    "Fork from a separate activation so execution does not inherit an enclosing finally scope.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room, job} = args;
    job.deadline = time() + job.timeout_seconds;
    fork worker (0)
      $llm_room_runtime:_work(room, job);
    endfork
    if (job in room.jobs)
      job.parent_task = worker;
    endif
  endmethod

  method _monitor owner: ARCH_WIZARD
    "Reconcile dead workers, enforce deadlines including setup, and dispatch up to max_workers.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room} = args;
    while (true)
      task_recv(1);
      room.loop_task == task_id() || return;
      for job in (room.jobs)
        if (job.parent_task > 0)
          if (!valid_task(job.parent_task) || time() >= job.deadline)
            job:cancel(time() >= job.deadline ? 'timed_out | 'cancelled);
            `kill_task(job.parent_task) ! E_INVARG';
            this:_retire(room, job);
          endif
        elseif (job.state != 'queued)
          this:_retire(room, job);
        endif
      endfor
      const configured = room.max_workers;
      const workers = typeof(configured) == TYPE_INT && configured >= 1 && configured <= 8 ? configured | 1;
      let active = length({running for running in (room.jobs) if (running.parent_task > 0)});
      for pending in (room.jobs)
        room.loop_task == task_id() || return;
        if (pending in room.jobs && pending.state == 'queued && pending.parent_task == 0 && active < workers)
          this:_spawn(room, pending);
          active = active + 1;
        endif
      endfor
      if (length(room.jobs) == 0)
        room.loop_task = 0;
        return;
      endif
    endwhile
  endmethod

  method _work owner: ARCH_WIZARD
    "Configure and run a job; room tool specialization executes under the captured actor.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room, job} = args;
    job in room.jobs && job.state == 'queued || return;
    job.parent_task = task_id();
    job.deadline = time() + job.timeout_seconds;
    const actor = job.requester;
    try
      const agent = create($llm_agent, actor, 1);
      job:bind_agent(agent);
      agent.client = valid(room.llm_client) ? room.llm_client | $llm_client;
      agent.max_iterations = room.max_iterations;
      agent.system_prompt = room.system_prompt + "\n\n" + $change_tools.guide;
      agent:reset_context();
      for tool in ($agent_room:tools_for(room, actor, job.presenter))
        agent:add_tool(tool.name, tool);
      endfor
      if (length(job.context) > 0)
        agent:set_context(job.context);
      endif
      job:run("User request: " + job.query + "\n\nUse tools for inspection and edits. Report the result concisely.", job.timeout_seconds);
    except e (ANY)
      job:fail("Error: " + tostr(e[1]) + " - " + tostr(e[2]));
    endtry
    "Keep output callbacks in the bounded worker, outside the monitor's cleanup transaction.";
    job.parent_task = task_id();
    try
      job:report();
    except delivery_error (ANY)
      set_task_perms(this.owner, {{"builtin_call", "server_log"}});
      server_log("LLM job result delivery failed: " + tostr(delivery_error[1]));
    endtry
    job.parent_task = 0;
  endmethod

  method _retire owner: ARCH_WIZARD
    "Record terminal history once and release references without invoking output callbacks.";
    this == $llm_room_runtime && caller == this && caller_perms().wizard || raise(E_PERM);
    const {room, job} = args;
    job in room.jobs || return;
    job:cancel();
    let entry = job:take_history();
    if (length(entry) > 0)
      if (typeof(entry['result]) == TYPE_STR && length(entry['result]) > 16384)
        entry['result] = entry['result][1..16384];
      endif
      let history = {@room.history, entry};
      if (length(history) > 20)
        history = history[$ - 19..$];
      endif
      const configured = room.max_history_context_items;
      const retained = typeof(configured) == TYPE_INT && configured >= 0 && configured <= 4 ? configured | 4;
      for index in [1..length(history)]
        if (index <= length(history) - retained || value_bytes(history[index]['context]) > 1048576)
          history[index]['context] = {};
        endif
      endfor
      room.history = history;
    endif
    room.jobs = setremove(room.jobs, job);
    job:release();
  endmethod

  method cancel owner: ARCH_WIZARD
    "Cancel only the authenticated requester's queued and running jobs, including their model children.";
    const principal = caller_perms();
    const {room, actor} = args;
    set_task_perms(principal);
    $llm_room_runtime:authenticate(room, actor);
    return $llm_room_runtime:_cancel(room, actor, false);
  endmethod

  method reset owner: ARCH_WIZARD
    "The room owner or wizard may cancel all jobs and clear retained history.";
    const principal = caller_perms();
    const {room, actor} = args;
    set_task_perms(principal);
    $llm_room_runtime:authenticate(room, actor);
    $agent_room:_require_owner(room, actor);
    return $llm_room_runtime:_cancel(room, actor, true);
  endmethod

  method _cancel owner: ARCH_WIZARD
    "Kill parents after cancelling their model children, then retire each selected job exactly once.";
    this == $llm_room_runtime && caller == this || raise(E_PERM);
    const {room, actor, all} = args;
    let count = 0;
    for job in (room.jobs)
      if (all || job.requester == actor)
        job:cancel();
        if (job.parent_task > 0 && job.parent_task != task_id())
          `kill_task(job.parent_task) ! E_INVARG';
        endif
        this:_retire(room, job);
        count = count + 1;
      endif
    endfor
    if (all)
      if (room.loop_task > 0)
        `kill_task(room.loop_task) ! E_INVARG';
      endif
      room.loop_task = 0;
      room.history = {};
    endif
    return count;
  endmethod

  method compact owner: ARCH_WIZARD
    "Compact a saved conversation under its requester; reject stale history after the model call.";
    const principal = caller_perms();
    const {room, actor} = args;
    set_task_perms(principal);
    $llm_room_runtime:authenticate(room, actor);
    return $llm_room_runtime:_compact(room, actor);
  endmethod

  method _compact owner: ARCH_WIZARD
    "Use a temporary anonymous agent; active job context is never edited by a separate task.";
    this == $llm_room_runtime && caller == this || raise(E_PERM);
    const {room, actor} = args;
    const history = room.history;
    let selected = 0;
    for index in [1..length(history)]
      if (history[index]['requester] == actor)
        selected = index;
      endif
    endfor
    selected > 0 || return false;
    const entry = history[selected];
    entry['recovery_required] && raise(E_INVARG, "Cannot compact unresolved tool effects");
    length(entry['context]) > 0 || return false;
    const agent = create($llm_agent, actor, 1);
    agent.client = valid(room.llm_client) ? room.llm_client | $llm_client;
    agent:set_context(entry['context]);
    agent:compact_context(actor);
    strcmp(toliteral(agent:get_context()), toliteral(entry['context])) != 0 || return false;
    strcmp(toliteral(room.history), toliteral(history)) == 0 || raise(E_INVARG, "Room history changed during compaction");
    room.history[selected]['context] = agent:get_context();
    return true;
  endmethod
endobject
