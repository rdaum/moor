object LLM_SOCIAL_TOOLS [
  import_export_id -> "llm_social_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Social Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  method get_tools owner: ARCH_WIZARD
    "Return shared social inspection and presentation descriptors; each handler authenticates its actor.";
    const text = ["type" -> "string", "maxLength" -> 8192];
    const name = ["type" -> "string", "minLength" -> 1, "maxLength" -> 256];
    return {
      $llm_agent_tool:mk("list_players", "List connected people and their activity.",
        ["type" -> "object", "properties" -> []], this, "list_players", 'inspection),
      $llm_agent_tool:mk("player_info", "Read a person's description and connection status.",
        ["type" -> "object", "properties" -> ["player_name" -> name],
          "required" -> {"player_name"}], this, "player_info", 'inspection),
      $llm_agent_tool:mk("emote", "Express a brief public action for a significant social moment.",
        ["type" -> "object", "properties" -> ["action" -> text],
          "required" -> {"action"}], this, "emote", 'presentation),
      $llm_agent_tool:mk("directed_say", "Address a person in the same room publicly.",
        ["type" -> "object", "properties" -> ["target_name" -> name, "message" -> text],
          "required" -> {"target_name", "message"}], this, "directed_say", 'presentation),
      $llm_agent_tool:mk("think", "Express a brief visible thought when useful for social context.",
        ["type" -> "object", "properties" -> ["thought" -> text],
          "required" -> {"thought"}], this, "think", 'presentation)
    };
  endmethod

  method context owner: ARCH_WIZARD
    "Capture the authenticated actor, room, and current observer job before calling world behavior.";
    const {actor, principal} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    const job = isa(actor, $llm_room_observer)
      ? $llm_observer_job:for_observer(actor, principal) | #-1;
    const room = actor.location;
    valid(room) || raise(E_INVARG, "Actor is not in a room.");
    return ['actor -> actor, 'room -> room, 'job -> job];
  endmethod

  method check_context owner: ARCH_WIZARD
    "Recheck the captured job and room after callbacks, without selecting a replacement job.";
    const {context} = args;
    const actor = context['actor];
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    actor.location == context['room] && valid(context['room]) || raise(E_PERM, "Actor moved.");
    if (isa(actor, $llm_room_observer))
      const job = context['job];
      $llm_observer_job:check(job);
      job.requester == actor && job.room == context['room] && job.state == 'running || raise(E_PERM);
    endif
    return true;
  endmethod

  method _player_record owner: ARCH_WIZARD
    "Read public player information with the requesting actor's permissions; retain bounded inert data.";
    const {target, context, details} = args;
    const actor = context['actor];
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_social_tools:check_context(context);
    set_task_perms(actor);
    const name = target:name();
    const location = target.location;
    let record = ["object" -> tostr(target), "name" -> name,
      "location" -> valid(location) ? location:name() | "(nowhere)"];
    if (details)
      record["description"] = $llm_tool_provider:description_text(target, actor);
      record["wizard"] = target.wizard;
      record["programmer"] = target.programmer;
    endif
    const idle = target in connected_players() ? idle_seconds(target) | E_INVARG;
    record["connected"] = typeof(idle) == TYPE_INT;
    if (typeof(idle) == TYPE_INT)
      record["idle_seconds"] = idle;
      record["connected_seconds"] = connected_seconds(target);
      record["activity"] = idle < 60 ? "active" | (idle < 300 ? "recent" | "idle");
    endif
    $llm_social_tools:check_context(context);
    value_bytes(record) <= 16384 || raise(E_QUOTA, "Player information is too large.");
    return record;
  endmethod

  method list_players owner: ARCH_WIZARD
    "List at most 256 connected players with bounded output and the actor's authority.";
    const {parameters, actor} = args;
    const context = $llm_social_tools:context(actor, caller_perms());
    set_task_perms(actor);
    const people = connected_players();
    length(people) <= 256 || raise(E_QUOTA, "Too many connected players to list.");
    let result = {};
    for target in (people)
      result = {@result, $llm_social_tools:_player_record(target, context, false)};
      value_bytes(result) <= 65536 || raise(E_QUOTA, "Player list is too large.");
    endfor
    return generate_json(result);
  endmethod

  method player_info owner: ARCH_WIZARD
    "Resolve one player with the shared matcher; ambiguous names and read failures remain failures.";
    const {parameters, actor} = args;
    const context = $llm_social_tools:context(actor, caller_perms());
    set_task_perms(actor);
    const target = $match:match_player(parameters["player_name"], actor);
    valid(target) && is_player(target) || raise(E_INVARG, "Person not found.");
    return generate_json($llm_social_tools:_player_record(target, context, true));
  endmethod

  method emote owner: ARCH_WIZARD
    "Publish an action as the authenticated actor in the captured room.";
    const {parameters, actor} = args;
    const context = $llm_social_tools:context(actor, caller_perms());
    set_task_perms(actor);
    return $llm_social_tools:present(context, 'emote, parameters["action"]);
  endmethod

  method think owner: ARCH_WIZARD
    "Publish a visible thought as the authenticated actor in the captured room.";
    const {parameters, actor} = args;
    const context = $llm_social_tools:context(actor, caller_perms());
    set_task_perms(actor);
    return $llm_social_tools:present(context, 'think, parameters["thought"]);
  endmethod

  method directed_say owner: ARCH_WIZARD
    "Resolve the recipient once, then reject movement or revoked work before public delivery.";
    const {parameters, actor} = args;
    const context = $llm_social_tools:context(actor, caller_perms());
    set_task_perms(actor);
    const target = $match:match_object(parameters["target_name"], actor);
    return $llm_social_tools:present(context, 'directed_say, parameters["message"], target);
  endmethod

  method present owner: ARCH_WIZARD
    "Construct social output and recheck the captured context immediately before delivery.";
    const {context, kind, text, ?target = #-1} = args;
    const actor = context['actor];
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_social_tools:check_context(context);
    typeof(text) == TYPE_STR && value_bytes(text) <= 8192 || raise(E_INVARG, "Invalid social output.");
    set_task_perms(actor);
    let event = false;
    if (kind == 'emote)
      event = actor:mk_emote_event(text);
    elseif (kind == 'think)
      event = actor:mk_think_event(text);
    elseif (kind == 'directed_say)
      valid(target) && target.location == context['room] || raise(E_INVARG, "Recipient is not in the room.");
      event = actor:mk_directed_say_event(target, text);
    else
      raise(E_INVARG, "Unsupported social event.");
    endif
    $llm_social_tools:check_context(context);
    if (kind == 'directed_say)
      valid(target) && target.location == context['room] || raise(E_PERM, "Recipient moved.");
    endif
    typeof(event) == TYPE_FLYWEIGHT && event.delegate == $event
      && event.actor == actor && event.location == context['room]
      || raise(E_INVARG, "Social event changed its actor or location.");
    context['room]:announce(event);
    return "Published " + tostr(kind) + ".";
  endmethod
endobject
