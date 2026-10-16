object LLM_ROOM_VIEW [
  import_export_id -> "llm_room_view",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Room View"
  parent: ROOT
  owner: ARCH_WIZARD

  method snapshot owner: ARCH_WIZARD
    "Copy only jobs, queue entries, and history visible to the authenticated requester or room administrator.";
    const principal = caller_perms();
    const {room, actor} = args;
    valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    valid(room) && isa(room, $agent_room) || raise(E_TYPE);
    $llm_room_runtime:initialize_state(room);
    const administrator = actor == room.owner || actor.wizard;
    let jobs = {};
    let queue = {};
    let history = {};
    for job in (room.jobs)
      if (administrator || job.requester == actor)
        if (job.state == 'queued && job.parent_task == 0)
          queue = {@queue, ['query -> job.query, 'requester -> job.requester]};
          continue;
        endif
        jobs = {@jobs, ['query -> job.query, 'requester -> job.requester, 'state -> job.state,
          'iteration -> (valid(job.agent) ? job.agent.current_iteration | 0),
          'context -> (valid(job.agent) ? job.agent:get_context() | job.context)]};
      endif
    endfor
    for offset in [1..length(room.history)]
      const entry = room.history[length(room.history) - offset + 1];
      if (administrator || entry['requester] == actor)
        history = {@history, entry};
        if (length(history) >= 10)
          break;
        endif
      endif
    endfor
    return ['jobs -> jobs, 'queue -> queue, 'history -> history];
  endmethod

  method render owner: ARCH_WIZARD
    "Render a filtered snapshot without reading another requester's transcript.";
    const {room, actor, kind} = args;
    set_task_perms(caller_perms());
    const snapshot = $llm_room_view:snapshot(room, actor);
    if (kind == 'status)
      let lines = {};
      for job in (snapshot['jobs])
        lines = {@lines, tostr(job['state]) + " | turn " + tostr(job['iteration]) + " | " + job['query]};
      endfor
      !lines && (lines = {"No active tasks visible to you."});
      return {@lines, "Queued: " + tostr(length(snapshot['queue]))}:join("\n");
    elseif (kind == 'queue)
      const lines = {entry['query] for entry in (snapshot['queue])};
      return lines ? lines:join("\n") | "No pending tasks visible to you.";
    elseif (kind == 'history)
      const lines = {"[" + tostr(entry['status]) + "] " + entry['query] for entry in (snapshot['history])};
      return lines ? lines:join("\n") | "No task history visible to you.";
    elseif (kind == 'context)
      let context = {};
      if (length(snapshot['jobs]) > 0)
        context = snapshot['jobs][$]['context];
      elseif (length(snapshot['history]) > 0)
        context = snapshot['history][1]['context];
      endif
      return "Context messages: " + tostr(length(context)) + "\n" + toliteral(context);
    endif
    raise(E_INVARG, "Unknown room view");
  endmethod
endobject
