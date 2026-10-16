object LLM_CODING_ROOM [
  import_export_id -> "llm_coding_room",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Coding Room"
  parent: AGENT_ROOM
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override max_iterations (owner: ARCH_WIZARD, flags: "rc") = 30;
  override system_prompt (owner: ARCH_WIZARD, flags: "rc") = "You are a room-based coding agent for MOO development. Use tools to inspect code, propose targeted changes, and program verbs safely. Prefer showing concise reports and code snippets.";
  override task_timeout_seconds (owner: ARCH_WIZARD, flags: "rc") = 180;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Room-based coding agent that uses $llm_agent for task execution and tool orchestration.";

  method _ensure_room_client owner: ARCH_WIZARD
    "Ensure this room has a dedicated client object (not global $llm_client).";
    $agent_room:_require_owner(this, caller_perms());
    if (valid(this.llm_client) && this.llm_client != $llm_client)
      return this.llm_client;
    endif
    const c = create($llm_client, this.owner, 1);
    c.name = "LLM client for " + tostr(this);
    this.llm_client = c;
    return c;
  endmethod

  method client_config_status owner: ARCH_WIZARD
    "Return a map describing the room's effective client config.";
    $agent_room:_require_owner(this, caller_perms());
    const client = valid(this.llm_client) ? this.llm_client | $llm_client;
    const endpoint = `client.api_endpoint ! ANY => ""';
    const model = `client.model ! ANY => ""';
    const actor = caller_perms();
    set_task_perms(actor);
    const key_set = client:is_configured();
    return ["client" -> client, "room_client" -> valid(this.llm_client) ? this.llm_client | #-1, "using_global_default" -> !(valid(this.llm_client) && this.llm_client != $llm_client), "endpoint" -> endpoint, "model" -> model, "api_key_set" -> key_set];
  endmethod

  verb "@model model" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Model and provider config for this room.";
    "Usage:";
    "  @model                                   (show status)";
    "  @model <provider/model>                  (set model)";
    "  @model model <provider/model>            (set model)";
    "  @model openrouter [model]                (set OpenRouter endpoint + model)";
    "  @model endpoint <url>                    (set endpoint)";
    "  @model key <api-key>                     (set API key)";
    "  @model list [filters]                    (list models)";
    "  @model global                            (use global $llm_client)";
    "List filters:";
    "  provider <name> | find <text> | limit <n> | offset <n> | all | raw";
    "Examples:";
    "  @model list provider openai limit 20";
    "  @model list find gpt-5";
    "  @model list openai gpt-5 limit 30";
    let client = 0;
    let filtered = 0;
    let finish = 0;
    let hay = 0;
    let hdr = 0;
    let i = 0;
    let id = 0;
    let keep = 0;
    let limit = 0;
    let lines = 0;
    let low = 0;
    let model = 0;
    let models = 0;
    let msg = 0;
    let name = 0;
    let offset = 0;
    let owner = 0;
    let parsed = 0;
    let part = 0;
    let pfx = 0;
    let provider = 0;
    let query = 0;
    let raw_limit = 0;
    let raw_mode = 0;
    let rows = 0;
    let s = 0;
    let show_all = 0;
    let shown = 0;
    let shown_rows = 0;
    let start = 0;
    let tok = 0;
    let tokens = 0;
    let total = 0;
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    $agent_room:_require_owner(this, actor);
    set_task_perms(actor);
    const raw = argstr:trim();
    if (!raw)
      s = this:client_config_status();
      lines = {"Model config:", "  model: " + tostr(s["model"]), "  endpoint: " + tostr(s["endpoint"]), "  using_global_default: " + toliteral(s["using_global_default"]), "", "Usage:", "  @model <provider/model>", "  @model model <provider/model>", "  @model openrouter [provider/model]", "  @model endpoint <url>", "  @model key <api-key>", "  @model list [provider <name>] [find <text>] [limit <n>] [offset <n>] [all] [raw]", "  @model global"};
      return actor:inform_current($event:mk_info(actor, lines:join("\n")));
    endif
    const space = " " in raw;
    const cmd = space ? raw[1..space - 1]:lowercase() | raw:lowercase();
    const rest = space ? raw[space + 1..$]:trim() | "";
    if (cmd == "global" || cmd == "reset")
      this.llm_client = #-1;
      return actor:inform_current($event:mk_info(actor, "This room now uses global $llm_client."));
    endif
    if (cmd == "endpoint")
      if (!rest)
        return actor:inform_current($event:mk_error(actor, "Usage: @model endpoint <url>"));
      endif
      client = this:_ensure_room_client();
      client.api_endpoint = rest;
      this.llm_client = client;
      return actor:inform_current($event:mk_info(actor, "Room endpoint set to " + rest + "."));
    endif
    if (cmd == "key")
      if (!rest)
        return actor:inform_current($event:mk_error(actor, "Usage: @model key <api-key>"));
      endif
      client = this:_ensure_room_client();
      client:set_api_key(rest);
      this.llm_client = client;
      return actor:inform_current($event:mk_info(actor, "Room API key updated."));
    endif
    if (cmd == "list")
      client = valid(this.llm_client) ? this.llm_client | $llm_client;
      provider = "";
      query = "";
      limit = 80;
      offset = 0;
      show_all = 0;
      raw_mode = 0;
      if (rest)
        tokens = rest:split(" ");
        i = 1;
        while (i <= length(tokens))
          tok = tokens[i]:trim();
          low = tok:lowercase();
          if (low == "raw")
            raw_mode = 1;
          elseif (low == "all")
            show_all = 1;
          elseif (low == "provider" && i < length(tokens))
            provider = tokens[i + 1]:trim():lowercase();
            i = i + 1;
          elseif (low == "find" && i < length(tokens))
            query = "";
            for j in [i + 1..length(tokens)]
              part = tokens[j]:trim();
              if (part)
                query = query ? query + " " + part | part;
              endif
            endfor
            query = query:lowercase();
            i = length(tokens);
          elseif (low == "limit" && i < length(tokens))
            parsed = tonum(tokens[i + 1]);
            parsed > 0 && (limit = parsed);
            i = i + 1;
          elseif (low == "offset" && i < length(tokens))
            parsed = tonum(tokens[i + 1]);
            parsed >= 0 && (offset = parsed);
            i = i + 1;
          else
            if (!provider)
              provider = low;
            else
              query = query ? query + " " + tok | tok;
            endif
          endif
          i = i + 1;
        endwhile
      endif
      query = query:trim():lowercase();
      if (raw_mode)
        try
          raw_limit = show_all ? 0 | limit;
          models = client:list_models("raw", raw_limit);
        except e (ANY)
          msg = "Model list failed: " + tostr(e[1]) + " - " + tostr(e[2]);
          return actor:inform_current($event:mk_error(actor, msg));
        endtry
        return actor:inform_current($event:mk_info(actor, toliteral(models)));
      endif
      try
        models = client:list_models(0);
      except e (ANY)
        msg = "Model list failed: " + tostr(e[1]) + " - " + tostr(e[2]);
        return actor:inform_current($event:mk_error(actor, msg));
      endtry
      if (!(typeof(models) == TYPE_MAP && maphaskey(models, "models") && typeof(models["models"]) == TYPE_LIST))
        return actor:inform_current($event:mk_info(actor, "Model response:\n" + toliteral(models)));
      endif
      rows = models["models"];
      filtered = {};
      for entry in (rows)
        if (typeof(entry) == TYPE_MAP)
          id = maphaskey(entry, "id") ? tostr(entry["id"]) | "";
          if (id)
            owner = maphaskey(entry, "owned_by") ? tostr(entry["owned_by"]) | "";
            name = maphaskey(entry, "name") ? tostr(entry["name"]) | "";
            keep = 1;
            if (provider)
              pfx = provider + "/";
              if (!(owner:lowercase() == provider || index(id:lowercase(), pfx) == 1))
                keep = 0;
              endif
            endif
            if (keep && query)
              hay = (id + " " + owner + " " + name):lowercase();
              if (!(index(hay, query) > 0))
                keep = 0;
              endif
            endif
            if (keep)
              filtered = {@filtered, entry};
            endif
          endif
        endif
      endfor
      total = length(filtered);
      show_all && (limit = total);
      limit <= 0 && (limit = 80);
      offset < 0 && (offset = 0);
      start = offset + 1;
      if (start > total)
        shown_rows = {};
      else
        finish = start + limit - 1;
        finish > total && (finish = total);
        shown_rows = filtered[start..finish];
      endif
      shown = length(shown_rows);
      hdr = "Models (showing " + tostr(shown) + " of " + tostr(total) + ")";
      provider && (hdr = hdr + " [provider=" + provider + "]");
      query && (hdr = hdr + " [find=" + query + "]");
      offset > 0 && (hdr = hdr + " [offset=" + tostr(offset) + "]");
      lines = {hdr + ":"};
      for i in [1..shown]
        entry = shown_rows[i];
        id = tostr(entry["id"]);
        owner = maphaskey(entry, "owned_by") ? " [" + tostr(entry["owned_by"]) + "]" | "";
        lines = {@lines, "  " + tostr(offset + i) + ". " + id + owner};
      endfor
      if (!shown)
        lines = {@lines, "  (no matches)"};
      endif
      if (maphaskey(models, "endpoint"))
        lines = {@lines, "", "Endpoint: " + tostr(models["endpoint"])};
      endif
      if (total > shown + offset)
        lines = {@lines, "", "Tip: use @model list ... offset " + tostr(offset + shown) + " for next page."};
      endif
      return actor:inform_current($event:mk_info(actor, lines:join("\n")));
    endif
    if (cmd == "openrouter" || cmd == "or")
      model = rest != "" ? rest | "openrouter/auto";
      client = this:_ensure_room_client();
      client.api_endpoint = "https://openrouter.ai/api/v1/chat/completions";
      client.model = model;
      this.llm_client = client;
      msg = "Room client set to OpenRouter with model " + model + ". Use @model key <OPENROUTER_API_KEY> if needed.";
      return actor:inform_current($event:mk_info(actor, msg));
    endif
    if (cmd == "model")
      if (!rest)
        return actor:inform_current($event:mk_error(actor, "Usage: @model model <provider/model>"));
      endif
      client = this:_ensure_room_client();
      client.model = rest;
      this.llm_client = client;
      return actor:inform_current($event:mk_info(actor, "Room model set to " + rest + "."));
    endif
    client = this:_ensure_room_client();
    client.model = raw;
    this.llm_client = client;
    actor:inform_current($event:mk_info(actor, "Room model set to " + raw + "."));
  endverb

  verb "tools tool @tools agenttools listtools" (none none none) owner: ARCH_WIZARD flags: "xd"
    "List currently available agent tools for this room.";
    const incoming = caller_perms();
    const actor = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    set_task_perms(actor);
    let desc = 0;
    let inserted = 0;
    let name = 0;
    let target = 0;
    let target_obj = 0;
    let target_verb = 0;
    const all = $agent_room:tools_for(this, actor);
    let seen = [];
    let rows = {};
    for tool in (all)
      if (typeof(tool) == TYPE_FLYWEIGHT)
        name = `tool.name ! ANY => ""';
        desc = `tool.description ! ANY => ""';
        target_obj = `tool.target_obj ! ANY => #-1';
        target_verb = `tool.target_verb ! ANY => ""';
      elseif (typeof(tool) == TYPE_MAP)
        name = `tool["name"] ! ANY => ""';
        desc = `tool["description"] ! ANY => ""';
        target_obj = `tool["target_obj"] ! ANY => #-1';
        target_verb = `tool["target_verb"] ! ANY => ""';
      else
        continue;
      endif
      if (typeof(name) != TYPE_STR || !name || maphaskey(seen, name))
        continue;
      endif
      seen[name] = 1;
      typeof(desc) != TYPE_STR && (desc = toliteral(desc));
      desc = strsub(desc, "\n", " ");
      if (length(desc) > 120)
        desc = desc[1..120] + "...";
      endif
      target = "";
      if (typeof(target_obj) == TYPE_OBJ && typeof(target_verb) == TYPE_STR && length(target_verb) > 0)
        target = tostr(target_obj) + ":" + target_verb;
      endif
      rows = {@rows, {name, {name, target, desc}}};
    endfor
    if (!rows)
      return actor:inform_current($event:mk_info(actor, "No tools are currently registered."):with_audience('utility):with_group('tools));
    endif
    let sorted = {};
    for entry in (rows)
      inserted = 0;
      for i in [1..length(sorted)]
        if (entry[1] < sorted[i][1])
          sorted = {@sorted[1..i - 1], entry, @sorted[i..$]};
          inserted = 1;
          break;
        endif
      endfor
      !inserted && (sorted = {@sorted, entry});
    endfor
    let table_rows = {};
    for entry in (sorted)
      table_rows = {@table_rows, entry[2]};
    endfor
    const headers = {"Tool", "Target", "Description"};
    const table_obj = $format.table:mk(headers, table_rows);
    const title_obj = $format.title:mk("Available Tools (" + tostr(length(table_rows)) + ")");
    const content = $format.block:mk(title_obj, table_obj);
    actor:inform_current($event:mk_info(actor, content):with_audience('utility):with_group('tools));
  endverb

  verb "resume continue retry" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Resume the most recent failed task for this requester using saved context.";
    let query = 0;
    const incoming = caller_perms();
    const requester = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(requester) == TYPE_OBJ && valid(requester) || raise(E_PERM);
    const extra = argstr:trim();
    const entry = $agent_room:latest_history(this, requester, 'failed);
    if (`entry['recovery_required] ! E_RANGE => false')
      return requester:inform_current($event:mk_error(requester, "This task has unresolved tool effects. Inspect the actual changes and submit a new request with explicit recovery instructions."));
    endif
    if (!entry)
      return requester:inform_current($event:mk_error(requester, "No failed task with resumable context was found."));
    endif
    const ctx = `entry['context] ! ANY => {}';
    if (typeof(ctx) != TYPE_LIST || length(ctx) == 0)
      return requester:inform_current($event:mk_error(requester, "That failed task has no saved context to resume from."));
    endif
    const base_query = `entry['query] ! ANY => "previous task"';
    if (extra)
      query = "Continue from your previous attempt on: " + base_query + "\n\nAdditional direction: " + extra;
    else
      query = "Continue from your previous attempt on: " + base_query + ". Use existing context and avoid repeating completed work.";
    endif
    set_task_perms(requester);
    try
      $llm_room_runtime:submit(this, requester, query, ctx, `connection() ! E_INVARG => 0');
    except error (E_QUOTA, E_INVARG)
      return requester:inform_current($event:mk_error(requester, tostr(error[2])));
    endtry
    requester:inform_current($event:mk_info(requester, "Queued resume: " + base_query));
  endverb
endobject
