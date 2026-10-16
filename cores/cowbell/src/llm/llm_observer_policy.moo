object LLM_OBSERVER_POLICY [
  import_export_id -> "llm_observer_policy",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Observer Policy"
  parent: ROOT
  owner: ARCH_WIZARD

  method ready owner: ARCH_WIZARD
    "Check the observer's selected client without invoking client code with owner authority.";
    const principal = caller_perms();
    const {observer} = args;
    typeof(observer) == TYPE_OBJ && valid(observer) && isa(observer, $llm_room_observer)
      || raise(E_TYPE);
    valid(principal) && (principal == observer || principal == observer.owner || principal.wizard)
      || raise(E_PERM);
    observer.enabled || return false;
    let client = $llm_client;
    if (!is_clear_property(observer, "agent") && valid(observer.agent))
      const agent = observer.agent;
      isa(agent, $llm_agent) && agent.owner == observer.owner || raise(E_PERM);
      client = agent.client;
    endif
    typeof(client) == TYPE_OBJ && valid(client) || return false;
    set_task_perms(observer);
    respond_to(client, 'is_configured) && return client:is_configured();
    return respond_to(client, 'chat) ? true | false;
  endmethod

  method triage owner: ARCH_WIZARD
    "Ask the observer's provider for an explicit decision; stale, invalid, and reasoning-only replies decline.";
    const principal = caller_perms();
    const {observer} = args;
    typeof(observer) == TYPE_OBJ && valid(observer) && isa(observer, $llm_room_observer)
      || raise(E_TYPE);
    valid(principal) && (principal == observer.owner || principal.wizard) || raise(E_PERM);
    observer.enabled || return false;
    const owner = observer.owner;
    const agent = observer.agent;
    typeof(agent) == TYPE_OBJ && valid(agent) || return false;
    isa(agent, $llm_agent) && agent.owner == owner || raise(E_PERM, "Foreign observer agent.");
    const client = agent.client;
    const destination = observer.location;
    const context = agent:get_context();
    let recent = {};
    for message in (context)
      if (message["role"] == "user" && maphaskey(message, "content")
          && typeof(message["content"]) == TYPE_STR
          && message["content"]:starts_with("OBSERVATION:"))
        recent = {@recent, message["content"]};
        length(recent) > 6 && (recent = listdelete(recent, 1));
      endif
    endfor
    length(recent) > 0 || return false;
    const events = recent:join("\n");
    value_bytes(events) <= 32768 || raise(E_QUOTA, "Observer triage input exceeds 32 KiB.");
    const template = observer.triage_prompt;
    typeof(template) == TYPE_STR && length(template) <= 8192 || raise(E_INVARG);
    const prompt = strsub(strsub(template, "{name}", observer.name), "{events}", events);
    const selected = observer.triage_model;
    typeof(selected) == TYPE_STR || raise(E_TYPE);
    const model = length(selected) > 0 ? selected | agent.model_override;
    const maximum = observer.triage_max_tokens;
    typeof(maximum) == TYPE_INT && maximum >= 1 && maximum <= 4096 || raise(E_INVARG);
    const opts = $llm_chat_opts:mk():with_max_tokens(maximum):with_temperature(0):with_tool_choice('none);
    const response = $llm_provider:chat_as(client, observer, 'triage,
      {["role" -> "user", "content" -> prompt]}, opts, model, false, false);
    if (!valid(observer) || observer.owner != owner || !observer.enabled
        || observer.agent != agent || observer.location != destination || !valid(agent)
        || agent.owner != owner || agent.client != client)
      return false;
    endif
    strcmp(toliteral(agent:get_context()), toliteral(context)) == 0 || return false;
    return $llm_observer_policy:decision(response);
  endmethod

  method decision owner: ARCH_WIZARD
    "Accept only ENGAGE as the complete visible answer, without native tool calls.";
    const {raw} = args;
    const response = $llm_response:mk(raw);
    response:is_valid() && !response:has_tool_calls() || return false;
    return response:content():trim() == "ENGAGE";
  endmethod
endobject
