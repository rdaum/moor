object LLM_CLIENT [
  import_export_id -> "llm_client",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Client"
  parent: ROOT
  owner: HACKER
  readable: true

  property api_endpoint (owner: HACKER, flags: "rc") = 0;
  property api_key (owner: ARCH_WIZARD, flags: "") = 0;
  property shared (owner: ARCH_WIZARD, flags: "") = false;
  property allowed_users (owner: ARCH_WIZARD, flags: "") = {};
  property model (owner: HACKER, flags: "rc") = 0;

  override description (owner: HACKER, flags: "rc") = "OpenAI-compatible LLM API client for chat completions using worker_request.";

  method configure_world owner: ARCH_WIZARD
    "Configure the shared world provider, assistance, and Mr. Welcome. Only wizards may call; no network requests are made.";
    "Returns true when Mr. Welcome uses this client. A running Mr. Welcome keeps his client, so startup is not blocked.";
    caller_perms().wizard && this == $llm_client || raise(E_PERM);
    const {endpoint, model, key} = args;
    typeof(endpoint) == TYPE_STR && length(endpoint) <= 4096
      && (endpoint:starts_with("https://") || endpoint:starts_with("http://"))
      || raise(E_INVARG, "Use an HTTP or HTTPS completion endpoint.");
    typeof(model) == TYPE_STR && length(model) > 0 && length(model) <= 256
      || raise(E_INVARG, "Enter a model name.");
    typeof(key) == TYPE_STR && length(key) > 0 && length(key) <= 8192
      || raise(E_INVARG, "Enter an API key.");
    const agent = $mr_welcome.agent;
    if (valid(agent) && agent.client != this)
      "A running flag can outlive an interrupted turn; leave the client rather than block startup.";
      if (agent.running)
        server_log("configure_world: Mr. Welcome is running; his client is unchanged.");
      else
        agent.client = this;
      endif
    endif
    this.api_endpoint = endpoint;
    this.model = model;
    this:set_api_key(key);
    this:set_access(true);
    $player.suggestions_llm_client = this;
    $mr_welcome.enabled = true;
    "False means Mr. Welcome kept a different client.";
    return !valid(agent) || agent.client == this;
  endmethod

  method chat owner: ARCH_WIZARD
    "Use the canonical provider with the incoming principal, without inherited credential authority.";
    set_task_perms(caller_perms());
    return $llm_provider:chat(this, @args);
  endmethod

  method set_api_key owner: ARCH_WIZARD
    "Set a local credential after canonical owner, wizard, or bearer-capability authorization.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_api_key);
    const {key} = args;
    typeof(key) == TYPE_STR && length(key) <= 8192 || raise(E_INVARG);
    target.api_key = key;
  endmethod

  method set_access owner: ARCH_WIZARD
    "Let the client owner choose public use or a list of authorized actors.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_access);
    const {shared, ?users = {}} = args;
    typeof(shared) == TYPE_BOOL && typeof(users) == TYPE_LIST && length(users) <= 256 || raise(E_TYPE);
    for user in (users)
      typeof(user) == TYPE_OBJ && valid(user) || raise(E_TYPE);
    endfor
    target.shared = shared;
    target.allowed_users = users;
  endmethod

  method simple_query owner: ARCH_WIZARD
    "Return exact visible content; consumers that expect JSON must parse and validate it themselves.";
    const {query, ?purpose = 'suggestion} = args;
    set_task_perms(caller_perms());
    const response = $llm_response:mk(this:chat(query, false, false, false, false, purpose));
    response:is_valid() && !response:has_tool_calls() || raise(E_INVARG, "Invalid text-only LLM response.");
    return response:content();
  endmethod

  method is_configured owner: ARCH_WIZARD
    "Report whether this client has its own key; ancestor credentials never configure a descendant.";
    return !is_clear_property(this, "api_key") && typeof(this.api_key) == TYPE_STR && length(this.api_key) > 0;
  endmethod

  method list_models owner: ARCH_WIZARD
    "List bounded model metadata using the same credential-use policy as generation.";
    const {?mode = "normalized", ?limit = 0} = args;
    set_task_perms(caller_perms());
    const parsed = $llm_provider:models(this);
    const entries = `parsed["data"] ! E_RANGE => {}';
    typeof(entries) == TYPE_LIST || raise(E_INVARG, "Invalid model list.");
    const cap = typeof(limit) == TYPE_INT && limit > 0 ? min(limit, 4096) | 4096;
    const selected = length(entries) > cap ? entries[1..cap] | entries;
    if (mode == "raw")
      return ["data" -> selected];
    endif
    let rows = {};
    for entry in (selected)
      typeof(entry) == TYPE_MAP && maphaskey(entry, "id") && typeof(entry["id"]) == TYPE_STR || raise(E_INVARG, "Invalid model entry.");
      let row = ["id" -> entry["id"]];
      for key in ({"owned_by", "name", "context_length", "description"})
        if (maphaskey(entry, key))
          row[key] = entry[key];
        endif
      endfor
      rows = {@rows, row};
    endfor
    return ["count" -> length(entries), "shown" -> length(rows), "models" -> rows];
  endmethod
endobject
