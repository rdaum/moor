object LLM_ASSIST_CONTEXT [
  import_export_id -> "llm_assist_context",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Assist Context"
  parent: ROOT
  owner: ARCH_WIZARD

  method offer owner: ARCH_WIZARD
    "Retain one immutable opt-in request, with eight requests and 32 KiB at most per player.";
    const {actor, destination, kind, input} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    is_player(actor) && kind in {'command, 'help} || raise(E_INVARG);
    const client = $player.suggestions_llm_client;
    typeof(client) == TYPE_OBJ && valid(client) || return "";
    typeof(destination) == TYPE_OBJ && destination in {entry[1] for entry in (connections(actor))}
      || raise(E_PERM, "Assist connection is not attached to the requester.");
    let value = input;
    if (kind == 'command)
      typeof(input) == TYPE_MAP || raise(E_TYPE);
      value = [];
      for field in ({'verb, 'dobjstr, 'prepstr, 'iobjstr})
        const text = input[field];
        typeof(text) == TYPE_STR && length(text) <= 1024 || raise(E_INVARG);
        value[field] = text;
      endfor
      length(value['verb]) > 0 || raise(E_INVARG);
    else
      typeof(value) == TYPE_STR && length(value) > 0 && length(value) <= 1024 || raise(E_INVARG);
    endif
    value_bytes(value) <= 4096 || raise(E_QUOTA, "Assist input exceeds 4 KiB.");
    const token = uuid();
    const request = <$llm_assist_context, .actor = actor, .destination = destination,
      .kind = kind, .input = value, .token = token, .expires = time() + 120,
      .actor_owner = actor.owner, .programmer = actor.programmer, .wizard = actor.wizard,
      .builder = actor.is_builder, .room = actor.location, .client = client>;
    let retained = $llm_assist_context:prune(actor);
    retained = {@retained, request};
    while (length(retained) > 8 || value_bytes(retained) > 32768)
      retained = retained[2..$];
    endwhile
    actor.llm_assist_contexts = retained;
    return token;
  endmethod

  method current owner: ARCH_WIZARD
    "Validate an opt-in value using native identity, permission, connection, and expiry checks.";
    const {actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    typeof(this) == TYPE_FLYWEIGHT && this.delegate == $llm_assist_context || raise(E_INVARG);
    return this.actor == actor && time() < this.expires && actor.owner == this.actor_owner
      && actor.programmer == this.programmer && actor.wizard == this.wizard
      && actor.is_builder == this.builder && actor.location == this.room
      && valid(this.client) && $player.suggestions_llm_client == this.client
      && this.destination in {entry[1] for entry in (connections(actor))};
  endmethod

  method prune owner: ARCH_WIZARD
    "Discard expired or revoked opt-in requests without exposing another player's saved input.";
    const {actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    is_player(actor) || raise(E_INVARG);
    const retained = {request for request in (actor.llm_assist_contexts) if (request:current(actor))};
    actor.llm_assist_contexts = retained;
    return retained;
  endmethod

  method take owner: ARCH_WIZARD
    "Consume a token once on its originating connection, or choose that connection's newest request.";
    const {actor, destination, ?token = ""} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    typeof(destination) == TYPE_OBJ && typeof(token) == TYPE_STR || raise(E_INVARG);
    const retained = $llm_assist_context:prune(actor);
    let chosen = 0;
    for index in [1..length(retained)]
      const request = retained[index];
      if (request.destination == destination && (token == "" || request.token == token))
        chosen = index;
      endif
    endfor
    chosen > 0 || return false;
    const result = retained[chosen];
    actor.llm_assist_contexts = listdelete(retained, chosen);
    return result;
  endmethod
endobject
