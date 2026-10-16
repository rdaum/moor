object LLM_PROVIDER [
  import_export_id -> "llm_provider",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Provider Boundary"
  parent: ROOT
  owner: ARCH_WIZARD

  method chat_as owner: ARCH_WIZARD
    "Invoke a client under an authenticated actor; caller billing fields cannot authorize delegation.";
    const principal = caller_perms();
    const {client, actor, purpose, input, ?opts = false, ?model = false, ?stream = false, ?tools = false} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    set_task_perms(actor);
    return client:chat(input, opts, model, stream, tools, purpose);
  endmethod

  method simple_as owner: ARCH_WIZARD
    "Invoke a text-only client query with the captured actor and purpose.";
    const principal = caller_perms();
    const {client, actor, purpose, query} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    set_task_perms(actor);
    return client:simple_query(query, purpose);
  endmethod

  method options owner: ARCH_WIZARD
    "Read options as inert slots; never invoke a supplied delegate or accept transport overrides.";
    const {options} = args;
    set_task_perms(caller_perms());
    options == false && return ["max_tokens" -> 2048];
    typeof(options) == TYPE_FLYWEIGHT || raise(E_TYPE, "Expected chat options.");
    let result = ["max_tokens" -> 2048];
    for key in (mapkeys(flyslots(options)))
      const name = tostr(key);
      name in {"temperature", "max_tokens", "tool_choice", "top_p", "frequency_penalty", "presence_penalty", "stop", "response_format"}
        || raise(E_INVARG, "Unsupported chat option.");
      let value = flyslots(options)[key];
      if (name == "tool_choice" && typeof(value) == TYPE_SYM)
        value = tostr(value);
      endif
      result[name] = value;
    endfor
    typeof(result["max_tokens"]) == TYPE_INT && result["max_tokens"] >= 1 && result["max_tokens"] <= 16384
      || raise(E_INVARG, "Output token limit must be 1 to 16384.");
    for entry in ({ {"temperature", 0, 2}, {"top_p", 0, 1}, {"frequency_penalty", -2, 2}, {"presence_penalty", -2, 2} })
      const {name, low, high} = entry;
      if (maphaskey(result, name))
        const value = result[name];
        typeof(value) in {TYPE_INT, TYPE_FLOAT} && tofloat(value) >= tofloat(low) && tofloat(value) <= tofloat(high) || raise(E_INVARG, "Invalid sampling option.");
      endif
    endfor
    if (maphaskey(result, "stop"))
      const values = typeof(result["stop"]) == TYPE_STR ? {result["stop"]} | result["stop"];
      typeof(values) == TYPE_LIST && length(values) <= 4 || raise(E_INVARG, "Invalid stop sequences.");
      for value in (values)
        typeof(value) == TYPE_STR && length(value) <= 256 || raise(E_INVARG, "Invalid stop sequence.");
      endfor
    endif
    if (maphaskey(result, "tool_choice"))
      const choice = result["tool_choice"];
      if (typeof(choice) == TYPE_MAP)
        const function = `choice["function"] ! E_RANGE => []';
        length(choice) == 2 && `choice["type"] ! E_RANGE => ""' == "function" && typeof(function) == TYPE_MAP
          && length(function) == 1 && maphaskey(function, "name") && typeof(function["name"]) == TYPE_STR
          && length(function["name"]) > 0 && length(function["name"]) <= 64 || raise(E_INVARG, "Invalid named tool choice.");
      else
        choice in {"auto", "none", "required"} || raise(E_INVARG, "Unsupported tool choice.");
      endif
    endif
    if (maphaskey(result, "response_format"))
      strcmp(toliteral(result["response_format"]), toliteral(["type" -> "json_object"])) == 0
        || raise(E_INVARG, "Unsupported response format.");
    endif
    return result;
  endmethod

  method _configuration owner: ARCH_WIZARD
    "Capture approved credentials and endpoint together; a child never inherits its parent's secret.";
    caller == $llm_provider && caller_perms().wizard || raise(E_PERM);
    const {client, actor} = args;
    typeof(client) == TYPE_OBJ && valid(client) && isa(client, $llm_client) || raise(E_TYPE);
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == client.owner || actor.wizard || client.shared || actor in client.allowed_users
      || raise(E_PERM, "This LLM client has not been shared with you.");
    !is_clear_property(client, "api_key") || raise(E_PERM, "Configure a key on this client; inherited keys cannot be used.");
    const key = client.api_key;
    typeof(key) == TYPE_STR && length(key) > 0 || raise(E_PERM, "LLM API key not configured.");
    const endpoint = client.api_endpoint;
    typeof(endpoint) == TYPE_STR && length(endpoint) <= 4096
      && (endpoint:starts_with("https://") || endpoint:starts_with("http://"))
      || raise(E_INVARG, "Invalid LLM endpoint.");
    return ['client -> client, 'endpoint -> endpoint, 'key -> key, 'model -> client.model];
  endmethod

  method chat owner: ARCH_WIZARD
    "Validate one immutable request snapshot, then account for every bounded HTTP attempt.";
    const actor = caller_perms();
    const {client, input, ?opts = false, ?model_override = false, ?stream = false, ?tools = false, ?purpose = 'conversation} = args;
    const config = $llm_provider:_configuration(client, actor);
    stream == false || raise(E_INVARG, "Streaming is not supported.");
    purpose in {'conversation, 'compaction, 'triage, 'facts, 'suggestion, 'no_tools} || raise(E_INVARG, "Invalid request purpose.");
    const messages = typeof(input) == TYPE_STR ? {["role" -> "user", "content" -> input]} | input;
    typeof(messages) == TYPE_LIST && length(messages) > 0 && length(messages) <= 256 || raise(E_INVARG, "Invalid message count.");
    for message in (messages)
      typeof(message) == TYPE_MAP && maphaskey(message, "role") || raise(E_INVARG, "Invalid message.");
      message["role"] in {"system", "user", "assistant", "tool"} || raise(E_INVARG, "Invalid message role.");
    endfor
    const model = model_override == false ? config['model] | model_override;
    typeof(model) == TYPE_STR && length(model) > 0 && length(model) <= 256 || raise(E_INVARG, "Invalid model.");
    const parameters = $llm_provider:options(opts);
    let body = ["model" -> model, "messages" -> messages, "stream" -> false];
    for key in (mapkeys(parameters))
      body[key] = parameters[key];
    endfor
    if (tools != false)
      typeof(tools) == TYPE_LIST && length(tools) <= 128 || raise(E_INVARG, "Invalid tool count.");
      length(tools) > 0 && (body["tools"] = tools);
    endif
    value_bytes(body) <= 1048576 || raise(E_QUOTA, "LLM request exceeds 1 MiB.");
    const payload = generate_json(body);
    value_bytes(payload) <= 1048576 || raise(E_QUOTA, "Encoded LLM request exceeds 1 MiB.");
    "Memory size includes string bytes and container overhead; it is a conservative reservation, not reported usage.";
    const reservation = value_bytes(payload) + body["max_tokens"];
    return $llm_provider:_request(config, actor, "POST", config['endpoint], payload, reservation, purpose, model);
  endmethod

  method models owner: ARCH_WIZARD
    "Fetch model metadata with the same credential policy, bounded transport, and attempt log.";
    const actor = caller_perms();
    const {client} = args;
    const config = $llm_provider:_configuration(client, actor);
    let endpoint = config['endpoint];
    for suffix in ({"/chat/completions", "/completions", "/responses"})
      if (endpoint:ends_with(suffix))
        endpoint = endpoint[1..length(endpoint) - length(suffix)];
        break;
      endif
    endfor
    if (!endpoint:ends_with("/models"))
      endpoint = endpoint:ends_with("/") ? endpoint + "models" | endpoint + "/models";
    endif
    return $llm_provider:_request(config, actor, "GET", endpoint, "", 0, 'models, "");
  endmethod

  method _request owner: ARCH_WIZARD
    "Retry only explicit transient HTTP statuses, retaining the same payload and a shared deadline.";
    caller == $llm_provider && caller_perms().wizard || raise(E_PERM);
    const {config, actor, method, endpoint, payload, reservation, purpose, model} = args;
    const deadline = time() + 60;
    const headers = {{"Content-Type", "application/json"}, {"Authorization", "Bearer " + config['key]}};
    for attempt in [1..3]
      const current = $llm_provider:_configuration(config['client], actor);
      strcmp(current['key], config['key]) == 0 && strcmp(current['endpoint], config['endpoint]) == 0
        || raise(E_PERM, "LLM configuration changed during the request.");
      const remaining = deadline - time();
      remaining > 0 || raise(E_QUOTA, "LLM request deadline exceeded.");
      const id = $llm_account:reserve(actor, reservation, deadline + 5, purpose, model);
      let response = {};
      try
        response = worker_request('curl, {method, endpoint, payload, headers,
          ["max_bytes" -> 1048576, "strict_utf8" -> true]}, ["timeout_seconds" -> tofloat(min(30, remaining))]);
      except (ANY)
        $llm_account:finish(actor, id, false, 'transport_error);
        raise(E_INVARG, "LLM transport failed; provider usage is unknown.");
      endtry
      if (typeof(response) != TYPE_LIST || length(response) != 3 || typeof(response[1]) != TYPE_INT)
        $llm_account:finish(actor, id, false, 'invalid_transport);
        raise(E_INVARG, "Invalid LLM transport response.");
      endif
      const status = response[1];
      if (status < 200 || status >= 300)
        $llm_account:finish(actor, id, false, "http_" + tostr(status));
        if (status in {408, 429, 500, 502, 503, 504} && attempt < 3 && time() + attempt < deadline)
          suspend(attempt);
          continue;
        endif
        raise(status in {401, 403} ? E_PERM | E_INVARG, "LLM HTTP status " + tostr(status) + ".");
      endif
      let parsed = [];
      try
        typeof(response[3]) == TYPE_STR && length(response[3]) > 0 || raise(E_INVARG);
        parsed = parse_json(response[3]);
        typeof(parsed) == TYPE_MAP || raise(E_TYPE);
      except (ANY)
        $llm_account:finish(actor, id, false, 'invalid_json);
        raise(E_INVARG, "LLM returned invalid JSON.");
      endtry
      const usage = purpose == 'models ? ["total_tokens" -> 0] | `parsed["usage"] ! E_RANGE => false';
      $llm_account:finish(actor, id, usage, 'complete);
      return parsed;
    endfor
    raise(E_INVARG, "LLM retry limit exceeded.");
  endmethod
endobject
