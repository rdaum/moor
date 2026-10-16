object AGENT_ROOM [
  import_export_id -> "agent_room",
  import_export_hierarchy -> {"llm"}
]
  name: "Generic Agent Room"
  parent: ROOM
  owner: ARCH_WIZARD
  readable: true

  property max_iterations (owner: ARCH_WIZARD, flags: "rc") = 30;
  property system_prompt (owner: ARCH_WIZARD, flags: "rc") = "You are a room-based coding agent for MOO development. Use tools to inspect code, propose targeted changes, and program verbs safely. Prefer showing concise reports and code snippets.";
  property task_timeout_seconds (owner: ARCH_WIZARD, flags: "rc") = 180;
  property history (owner: ARCH_WIZARD, flags: "") = {};
  property llm_client (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property loop_task (owner: ARCH_WIZARD, flags: "") = 0;
  property max_history_context_items (owner: ARCH_WIZARD, flags: "rc") = 4;
  property max_pending_per_player (owner: ARCH_WIZARD, flags: "rc") = 3;
  property max_query_length (owner: ARCH_WIZARD, flags: "rc") = 2000;
  property max_workers (owner: ARCH_WIZARD, flags: "rc") = 1;
  property max_jobs (owner: ARCH_WIZARD, flags: "rc") = 32;
  property spent_requests (owner: ARCH_WIZARD, flags: "") = [];
  property jobs (owner: ARCH_WIZARD, flags: "") = {};

  override description (owner: ARCH_WIZARD, flags: "rc") = "A virtual workspace for agent interaction.";

  method _require_owner owner: ARCH_WIZARD
    "Authorize lifecycle changes for the captured owner or wizard principal.";
    const principal = caller_perms();
    const {subject, actor} = args;
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    actor == subject.owner || actor.wizard || raise(E_PERM);
    return actor;
  endmethod

  method latest_history owner: ARCH_WIZARD
    "Return the requester's newest entry matching an optional status, or an empty map.";
    const principal = caller_perms();
    const {subject, actor, ?status = false} = args;
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    typeof(subject) == TYPE_OBJ && valid(subject) && isa(subject, $agent_room) || raise(E_TYPE);
    $llm_room_runtime:initialize_state(subject);
    const history = subject.history;
    for offset in [1..length(history)]
      const entry = history[length(history) - offset + 1];
      if (typeof(entry) != TYPE_MAP)
        continue;
      endif
      const requester = entry['requester];
      if (actor != requester)
        continue;
      endif
      if (status && (!maphaskey(entry, 'status) || tostr(entry['status]) != tostr(status)))
        continue;
      endif
      return entry;
    endfor
    return [];
  endmethod

  verb do (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Usage: do <task>. Admit a bounded request for the current connection.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    set_task_perms(actor);
    try
      $llm_room_runtime:submit(this, actor, argstr, {}, `connection() ! E_INVARG => 0');
    except error (E_QUOTA, E_INVARG)
      return actor:inform_current($event:mk_error(actor, tostr(error[2])));
    endtry
    actor:inform_current($event:mk_info(actor, "Queued: " + argstr));
  endverb

  verb halt (none none none) owner: ARCH_WIZARD flags: "xd"
    "Cancel the requester's jobs. Administrators use reset to stop all jobs and clear history.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    set_task_perms(actor);
    const count = $llm_room_runtime:cancel(this, actor);
    actor:inform_current($event:mk_info(actor, "Cancelled " + tostr(count) + " task(s)."));
  endverb

  verb status (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show only the requester's or administrator's status snapshot.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    const text = $llm_room_view:render(this, actor, 'status);
    set_task_perms(actor);
    return actor:inform_current($event:mk_info(actor, text):with_audience('utility));
  endverb

  verb queue (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show only the requester's or administrator's queue snapshot.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    const text = $llm_room_view:render(this, actor, 'queue);
    set_task_perms(actor);
    return actor:inform_current($event:mk_info(actor, text):with_audience('utility));
  endverb

  verb history (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show only the requester's or administrator's history snapshot.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    const text = $llm_room_view:render(this, actor, 'history);
    set_task_perms(actor);
    return actor:inform_current($event:mk_info(actor, text):with_audience('utility));
  endverb

  verb description (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Describe the workspace without exposing requests or transcripts in room output.";
    return {"A workspace for LLM tasks.", $format.title:mk("Commands", 4),
      $format.list:mk({"do <task> - Queue a task", "status - Show your active tasks",
        "queue - Show your queued tasks", "history - Show your recent results",
        "fix <instructions> - Continue from your latest result", "context - Show your context",
        "halt - Cancel your tasks"}, false)};
  endverb

  verb reset (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Stop all jobs and clear history. Only the room owner or a wizard may reset.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    set_task_perms(actor);
    $llm_room_runtime:reset(this, actor);
    actor:inform_current($event:mk_info(actor, "Room jobs and history cleared."));
  endverb

  verb compact (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Compact the requester's last completed conversation; active jobs compact their own context.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    set_task_perms(actor);
    const changed = $llm_room_runtime:compact(this, actor);
    actor:inform_current($event:mk_info(actor, changed ? "Saved context compacted." | "Context was not compacted."));
  endverb

  verb "fix revise" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Request correction/revision to previous work - continues with existing context.";
    "Usage: fix <instructions> - tells agent to revise existing work";
    const incoming = caller_perms();
    const requester = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(requester) == TYPE_OBJ && valid(requester) || raise(E_PERM);
    const command_text = argstr;
    if (!command_text)
      requester:inform_current($event:mk_info(requester, "What needs to be fixed or revised?"));
      return;
    endif
    "Get the most recent history entry for this requester";
    const entry = $agent_room:latest_history(this, requester);
    if (`entry['recovery_required] ! E_RANGE => false')
      return requester:inform_current($event:mk_error(requester, "This task has unresolved tool effects. Inspect the actual changes and submit a new request with explicit recovery instructions."));
    endif
    const prev_context = maphaskey(entry, 'context) && typeof(entry['context]) == TYPE_LIST ? entry['context] | {};
    if (length(prev_context) == 0)
      requester:inform_current($event:mk_info(requester, "No previous task found to continue from. Use 'do' to start a new task."));
      return;
    endif
    "Build the revision query";
    const revision_query = "REVISION REQUEST: Please fix/revise the previous work as follows: " + command_text;
    set_task_perms(requester);
    try
      $llm_room_runtime:submit(this, requester, revision_query, prev_context, `connection() ! E_INVARG => 0');
    except error (E_QUOTA, E_INVARG)
      return requester:inform_current($event:mk_error(requester, tostr(error[2])));
    endtry
    requester:inform_current($event:mk_info(requester, "Revision queued: " + command_text));
  endverb

  method _execute_task owner: ARCH_WIZARD
    "Execute a signed request synchronously through the shared room runtime.";
    caller == this || raise(E_PERM);
    const {task} = args;
    return $llm_room_runtime:execute(this, task);
  endmethod

  method _authorize_task owner: ARCH_WIZARD
    "Sign one bounded room request using the actual incoming principal, without ancestry or owner trust.";
    const principal = caller_perms();
    const {room, task} = args;
    typeof(room) == TYPE_OBJ && valid(room) && isa(room, $agent_room) || raise(E_PERM);
    typeof(task) == TYPE_MAP || raise(E_TYPE);
    const actor = task['player];
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    const query = task['query];
    typeof(query) == TYPE_STR || raise(E_TYPE);
    length(query) <= 16384 || raise(E_QUOTA);
    const context = maphaskey(task, 'context) ? task['context] | {};
    typeof(context) == TYPE_LIST || raise(E_TYPE);
    length(context) <= 256 && value_bytes(context) <= 1048576 || raise(E_QUOTA);
    return paseto_make_local(["purpose" -> "agent_room_task", "room" -> room, "actor" -> actor, "query" -> query, "context" -> context, "connection" -> (maphaskey(task, 'connection) ? task['connection] | 0), "jti" -> uuid(), "exp" -> time() + 60]);
  endmethod

  method _consume_task owner: ARCH_WIZARD
    "Verify and consume the bounded request before callbacks, suspension, or task effects.";
    const principal = caller_perms();
    valid(principal) && principal.wizard || raise(E_PERM);
    const {room, task} = args;
    $llm_room_runtime:initialize_state(room);
    typeof(task) == TYPE_MAP && maphaskey(task, "authorization") || raise(E_PERM);
    let claims = [];
    try
      claims = paseto_verify_local(task["authorization"]);
    except (E_TYPE, E_INVARG)
      raise(E_PERM, "Invalid room task authorization");
    endtry
    typeof(claims) == TYPE_MAP || raise(E_PERM);
    for key in ({"purpose", "room", "actor", "query", "context", "jti", "exp"})
      maphaskey(claims, key) || raise(E_PERM, "Incomplete room task authorization");
    endfor
    const context = maphaskey(task, 'context) ? task['context] | {};
    const destination = maphaskey(task, 'connection) ? task['connection] | 0;
    const signed_destination = maphaskey(claims, "connection") ? claims["connection"] | 0;
    claims["purpose"] == "agent_room_task" && claims["room"] == room && claims["actor"] == task['player]
      && strcmp(toliteral(claims["query"]), toliteral(task['query])) == 0
      && strcmp(toliteral(claims["context"]), toliteral(context)) == 0
      && signed_destination == destination || raise(E_PERM, "Room task authorization does not match request");
    typeof(claims["actor"]) == TYPE_OBJ && valid(claims["actor"]) || raise(E_PERM);
    typeof(claims["jti"]) == TYPE_STR && length(claims["jti"]) > 0 || raise(E_PERM);
    const now = time();
    typeof(claims["exp"]) == TYPE_INT && claims["exp"] > now || raise(E_PERM, "Room task authorization expired");
    let spent = [];
    for id in (mapkeys(room.spent_requests))
      if (room.spent_requests[id] > now)
        spent[id] = room.spent_requests[id];
      endif
    endfor
    !maphaskey(spent, claims["jti"]) || raise(E_PERM, "Room task authorization already consumed");
    length(spent) < 256 || raise(E_QUOTA, "Room request rate exceeded");
    spent[claims["jti"]] = claims["exp"];
    room.spent_requests = spent;
    return claims;
  endmethod
  method _get_tools owner: ARCH_WIZARD
    "Compose shared domain and change tools; jobs supply their own presentation destination.";
    set_task_perms(caller_perms());
    return $llm_tool_catalog:compose($agent_building_tools:get_tools(), $change_tools:get_tools());
  endmethod

  method tools_for owner: ARCH_WIZARD
    "Call room specialization hooks under the authenticated requester before normalizing their descriptors.";
    "Pass a request presenter for execution; the default supplies descriptors for catalog inspection.";
    const principal = caller_perms();
    const {room, actor, ?presenter = $llm_presenter} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    typeof(room) == TYPE_OBJ && valid(room) && isa(room, $agent_room) || raise(E_TYPE);
    set_task_perms(actor);
    return $llm_tool_catalog:compose(room:_get_tools(), presenter:get_tools());
  endmethod

  verb "context ctx" (any any any) owner: ARCH_WIZARD flags: "xd"
    "Show the authenticated requester's current or most recent conversation.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    const text = $llm_room_view:render(this, actor, 'context);
    set_task_perms(actor);
    return actor:inform_current($event:mk_info(actor, text):with_audience('utility));
  endverb
endobject
