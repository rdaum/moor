object AGENT_ROOM [
  import_export_id -> "agent_room",
  import_export_hierarchy -> {"llm"}
]
  name: "Generic Agent Room"
  parent: ROOM
  owner: ARCH_WIZARD
  readable: true

  property active_tasks (owner: ARCH_WIZARD, flags: "rc") = {};
  property agent (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property current_task (owner: ARCH_WIZARD, flags: "rc") = "";
  property history (owner: ARCH_WIZARD, flags: "rc") = {};
  property llm_client (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property loop_task (owner: ARCH_WIZARD, flags: "rc") = 0;
  property max_history_context_items (owner: ARCH_WIZARD, flags: "rc") = 4;
  property max_pending_per_player (owner: ARCH_WIZARD, flags: "rc") = 3;
  property max_query_length (owner: ARCH_WIZARD, flags: "rc") = 2000;
  property max_workers (owner: ARCH_WIZARD, flags: "rc") = 3;
  property task_id (owner: ARCH_WIZARD, flags: "rc") = 0;
  property task_queue (owner: ARCH_WIZARD, flags: "rc") = {};
  property task_requester (owner: ARCH_WIZARD, flags: "rc") = #-1;

  override description (owner: ARCH_WIZARD, flags: "rc") = "A virtual workspace for agent interaction.";

  verb do (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Catch-all command verb - queues natural language commands for the agent.";
    "Usage: do <anything> - queues the command for agent processing";
    const incoming = caller_perms();
    const requester = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(requester) == TYPE_OBJ && valid(requester) || raise(E_PERM);
    const command_text = argstr;
    if (!command_text)
      requester:inform_current($event:mk_info(requester, "What would you like me to do?"));
      return;
    endif
    let max_len = `this.max_query_length ! E_PROPNF => 2000';
    if (max_len < 1)
      max_len = 1;
    endif
    if (length(command_text) > max_len)
      requester:inform_current($event:mk_error(requester, "Task is too long (" + tostr(length(command_text)) + " chars). Max is " + tostr(max_len) + "."));
      return;
    endif
    let max_pending = `this.max_pending_per_player ! E_PROPNF => 3';
    if (max_pending < 1)
      max_pending = 1;
    endif
    let pending_for_player = 0;
    if (this.current_task && this.task_requester == requester)
      pending_for_player = pending_for_player + 1;
    endif
    for task in (this.task_queue)
      if (`task['player] ! ANY => #-1' == requester)
        pending_for_player = pending_for_player + 1;
      endif
    endfor
    if (pending_for_player >= max_pending)
      requester:inform_current($event:mk_error(requester, "You already have " + tostr(pending_for_player) + " task(s) queued/running here. Please wait for one to finish."));
      return;
    endif
    "Ensure event loop is running and send task";
    this:_ensure_loop();
    let msg = ["type" -> "task", 'query -> command_text, 'player -> requester, 'queued_at -> time()];
    msg["authorization"] = $agent_room:_authorize_task(this, msg);
    try
      task_send(this.loop_task, msg);
    except e (ANY)
      "Loop may have died - restart and retry once";
      this:_start_loop();
      try
        task_send(this.loop_task, msg);
      except e2 (ANY)
        requester:inform_current($event:mk_error(requester, "Failed to queue task."));
        return;
      endtry
    endtry
    requester:inform_current($event:mk_info(requester, "Queued: \"" + command_text + "\""));
  endverb

  method _announce owner: ARCH_WIZARD
    "Announce an agent message to the room using proper room events.";
    const {message} = args;
    "Create a proper room event";
    const agent = valid(this.agent) ? this.agent | this;
    const event = $event:mk_announce(agent, message);
    this:announce(event);
  endmethod

  verb halt (none none none) owner: ARCH_WIZARD flags: "xd"
    "Stop all current tasks and the event loop.";
    this:_stop_loop();
    const queue_len = length(this.task_queue);
    let msg = "Stopped all background tasks.";
    if (queue_len > 0)
      msg = msg + " " + tostr(queue_len) + " queued tasks remain.";
    endif
    player:inform_current($event:mk_info(player, msg));
  endverb

  verb status (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show current agent status.";
    let agent = 0;
    let iter = 0;
    let max_iter = 0;
    let mine = 0;
    let pct = 0;
    let task_desc = 0;
    let lines = {};
    let show_details = `player.wizard ! ANY => false';
    if (!show_details && this.current_task && this.task_requester == player)
      show_details = true;
    endif
    if (this.current_task)
      if (show_details)
        "Truncate long task descriptions";
        task_desc = this.current_task;
        if (length(task_desc) > 60)
          task_desc = task_desc[1..60] + "...";
        endif
        lines = {@lines, "Task: \"" + task_desc + "\""};
        if (valid(this.agent))
          agent = this.agent;
          iter = agent.iteration;
          max_iter = agent.max_iterations;
          pct = toint(iter * 100 / max_iter);
          lines = {@lines, "Status: " + tostr(agent.status) + "  |  Iteration: " + tostr(iter) + "/" + tostr(max_iter) + " (" + tostr(pct) + "%)"};
          if (agent.last_tool && length(agent.last_tool) > 0)
            lines = {@lines, "Last tool: " + agent.last_tool};
          endif
        endif
      else
        lines = {@lines, "A task is currently running for another requester."};
      endif
    else
      lines = {@lines, "No task running."};
    endif
    "Active workers";
    const active_count = length(this.active_tasks);
    if (active_count > 1)
      lines = {@lines, "Active workers: " + tostr(active_count)};
    endif
    "Queue info";
    const queue_len = length(this.task_queue);
    if (queue_len > 0)
      if (`player.wizard ! ANY => false')
        lines = {@lines, "Queue: " + tostr(queue_len) + " pending"};
      else
        mine = 0;
        for task in (this.task_queue)
          if (`task['player] ! ANY => #-1' == player)
            mine = mine + 1;
          endif
        endfor
        lines = {@lines, "Queue: " + tostr(mine) + " yours pending"};
      endif
    endif
    "Loop status";
    if (this.loop_task > 0)
      lines = {@lines, "Event loop: running"};
    endif
    player:inform_current($event:mk_info(player, lines:join("\n")));
  endverb

  verb queue (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show pending tasks in the queue.";
    let task = 0;
    let task_player = 0;
    if (length(this.task_queue) == 0)
      return player:inform_current($event:mk_info(player, "Queue is empty."));
    endif
    const show_all = `player.wizard ! ANY => false';
    let lines = {"Pending tasks:"};
    let shown = 0;
    let hidden = 0;
    for i in [1..length(this.task_queue)]
      task = this.task_queue[i];
      task_player = `task['player] ! ANY => #-1';
      if (show_all || task_player == player)
        lines = {@lines, "  " + tostr(i) + ". \"" + task['query] + "\""};
        shown = shown + 1;
      else
        hidden = hidden + 1;
      endif
    endfor
    if (shown == 0)
      lines = {@lines, "  (No pending tasks for you.)"};
    endif
    if (hidden > 0 && !show_all)
      lines = {@lines, "  (" + tostr(hidden) + " task(s) for other requesters hidden)"};
    endif
    player:inform_current($event:mk_info(player, $format.block:mk(@lines)));
  endverb

  verb history (none none none) owner: ARCH_WIZARD flags: "xd"
    "Show completed task history.";
    let entry = 0;
    let i = 0;
    let query_preview = 0;
    let status_str = 0;
    if (length(this.history) == 0)
      return player:inform_current($event:mk_info(player, "No task history yet."));
    endif
    const show_all = `player.wizard ! ANY => false';
    let lines = {"Task history (most recent first):"};
    "Show up to 10 relevant entries, newest first";
    const hist_len = length(this.history);
    let shown = 0;
    for offset in [0..hist_len - 1]
      if (shown >= 10)
        break;
      endif
      i = hist_len - offset;
      entry = this.history[i];
      if (!show_all && `entry['requester] ! ANY => #-1' != player)
        continue;
      endif
      status_str = tostr(entry['status]);
      query_preview = entry['query][1..min(50, length(entry['query]))];
      if (length(entry['query]) > 50)
        query_preview = query_preview + "...";
      endif
      lines = {@lines, "  [" + status_str + "] \"" + query_preview + "\""};
      shown = shown + 1;
    endfor
    if (shown == 0)
      lines = {@lines, "  (No history entries for you.)"};
    endif
    player:inform_current($event:mk_info(player, $format.block:mk(@lines)));
  endverb

  verb description (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Formatted description with explicit newlines.";
    const intro = "A virtual workspace where you can interact with an AI agent.";
    const cmd_header = $format.title:mk("Commands", 4);
    const cmds = $format.list:mk({"do <task>" + " - Queue a task for the agent", "status" + " - Check current activity", "halt" + " - Interrupt current task", "queue" + " - Show pending tasks", "history" + " - Show completed tasks"}, false);
    return {intro, cmd_header, cmds};
  endverb

  method _tool_present_code owner: ARCH_WIZARD
    "Tool: Present formatted code to the room.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const title = maphaskey(args_map, "title") ? args_map["title"] | "Code";
    let code = args_map["code"];
    const language = maphaskey(args_map, "language") ? args_map["language"] | 'moo;
    "Handle list of lines (from verb_code) - join into string";
    if (typeof(code) == typeof({}))
      code = code:join("\n");
    elseif (typeof(code) != typeof(""))
      code = toliteral(code);
    endif
    let blocks = {};
    length(title) && (blocks = {$format.title:mk(title)});
    blocks = {@blocks, $format.code:mk(code, language)};
    this:_announce($format.block:mk(@blocks));
    return "Code presented: " + title;
  endmethod

  method _tool_present_report owner: ARCH_WIZARD
    "Tool: Present a formatted report/document to the room.";
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    let title = maphaskey(args_map, "title") ? args_map["title"] | "";
    let content = args_map["content"];
    "Get the agent - it's stored on this.agent during task execution";
    const agent = valid(this.agent) ? this.agent | #-1;
    "Use agent's _coerce_string if available";
    if (valid(agent) && "_coerce_string" in verbs(agent))
      content = agent:_coerce_string(content);
      title = typeof(title) != TYPE_STR ? agent:_coerce_string(title) | title;
    else
      "Fallback: manually handle complex types";
      content = this:_flatten_content(content);
      title = typeof(title) != TYPE_STR ? tostr(title) | title;
    endif
    let blocks = {};
    length(title) && (blocks = {$format.title:mk(title)});
    this:_announce($format.block:mk(@blocks, content));
    return "Report presented: " + (title ? title | "(untitled)");
  endmethod

  method _tool_present_table owner: ARCH_WIZARD
    "Tool: Present a formatted table to the room.";
    let clean_headers = 0;
    let clean_row = 0;
    let clean_rows = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    let title = maphaskey(args_map, "title") ? args_map["title"] | "";
    let headers = args_map["headers"];
    let rows = args_map["rows"];
    "Get agent for coercion";
    const agent = valid(this.agent) ? this.agent | #-1;
    "Coerce title if needed";
    if (typeof(title) != TYPE_STR)
      title = valid(agent) && "_coerce_string" in verbs(agent) ? agent:_coerce_string(title) | tostr(title);
    endif
    "Coerce headers to strings";
    if (typeof(headers) == TYPE_LIST)
      clean_headers = {};
      for h in (headers)
        if (typeof(h) != TYPE_STR)
          h = valid(agent) && "_coerce_string" in verbs(agent) ? agent:_coerce_string(h) | tostr(h);
        endif
        clean_headers = {@clean_headers, h};
      endfor
      headers = clean_headers;
    else
      raise(E_TYPE, "headers must be a list");
    endif
    "Coerce row cells to strings";
    if (typeof(rows) == TYPE_LIST)
      clean_rows = {};
      for row in (rows)
        if (typeof(row) == TYPE_LIST)
          clean_row = {};
          for cell in (row)
            if (typeof(cell) != TYPE_STR)
              cell = valid(agent) && "_coerce_string" in verbs(agent) ? agent:_coerce_string(cell) | tostr(cell);
            endif
            clean_row = {@clean_row, cell};
          endfor
          clean_rows = {@clean_rows, clean_row};
        endif
      endfor
      rows = clean_rows;
    else
      raise(E_TYPE, "rows must be a list of lists");
    endif
    let blocks = {};
    length(title) && (blocks = {$format.title:mk(title)});
    blocks = {@blocks, $format.table:mk(headers, rows)};
    this:_announce($format.block:mk(@blocks));
    return "Table presented: " + tostr(length(rows)) + " rows";
  endmethod

  method _get_room_tools owner: ARCH_WIZARD
    "Return tool definitions for this room's presentation tools.";
    return {["name" -> "show_verb", "description" -> "Display a verb's source code. Always include 'reason' to explain WHY you're reading this code.", "target_obj" -> this, "target_verb" -> "_tool_show_verb", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference like '$room', '$thing', '#123'"], "verb" -> ["type" -> "string", "description" -> "Verb name to display"], "reason" -> ["type" -> "string", "description" -> "WHY you're reading this - e.g. 'to understand how wearables work'"]], "required" -> {"object", "verb", "reason"}]], ["name" -> "present_report", "description" -> "Present a prose report. Write clear explanatory text about what you DID or found.", "target_obj" -> this, "target_verb" -> "_tool_present_report", "input_schema" -> ["type" -> "object", "properties" -> ["title" -> ["type" -> "string", "description" -> "Report title"], "content" -> ["type" -> "string", "description" -> "Report text - prose paragraphs"]], "required" -> {"content"}]], ["name" -> "present_table", "description" -> "Present tabular data like verb lists or property comparisons.", "target_obj" -> this, "target_verb" -> "_tool_present_table", "input_schema" -> ["type" -> "object", "properties" -> ["title" -> ["type" -> "string", "description" -> "Table title"], "headers" -> ["type" -> "array", "items" -> ["type" -> "string"], "description" -> "Column headers"], "rows" -> ["type" -> "array", "items" -> ["type" -> "array"], "description" -> "Table rows"]], "required" -> {"headers", "rows"}]], ["name" -> "program_verb", "description" -> "Set the MOO code for a verb. IMPORTANT: 'code' must be a single string containing the COMPLETE verb body. Use \\n for line breaks. Example: code=\"\\\"Docstring\\\";\\nplayer:inform_current($event:mk_info(player, ctime()));\"", "target_obj" -> this, "target_verb" -> "_tool_program_verb", "input_schema" -> ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference like '$thing', '#123'"], "verb" -> ["type" -> "string", "description" -> "Verb name to program"], "code" -> ["type" -> "string", "description" -> "Complete MOO code as a SINGLE STRING. Use \\n for newlines. NOT an object/map."]], "required" -> {"object", "verb", "code"}]]};
  endmethod

  method _tool_show_verb owner: ARCH_WIZARD
    "Tool: Fetch and present verb code with explanation of why.";
    let preview_lines = 0;
    let preview_str = 0;
    const {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    let obj_ref = args_map["object"];
    let verb_name = args_map["verb"];
    const reason = maphaskey(args_map, "reason") ? args_map["reason"] | "";
    typeof(obj_ref) != TYPE_STR && raise(E_TYPE, "object must be a string like '$room' or '#123'");
    typeof(verb_name) != TYPE_STR && raise(E_TYPE, "verb must be a string");
    "Robustness: strip hallucinated colon/separator prefixes from object ref";
    obj_ref = obj_ref:trim();
    while (length(obj_ref) > 0 && (obj_ref[1] == ":" || obj_ref[1] == " " || obj_ref[1] == "\t"))
      obj_ref = obj_ref[2..$]:trim();
    endwhile
    "Robustness: strip hallucinated colon/separator prefixes from verb name";
    verb_name = verb_name:trim();
    while (length(verb_name) > 0 && (verb_name[1] == ":" || verb_name[1] == " " || verb_name[1] == "\t"))
      verb_name = verb_name[2..$]:trim();
    endwhile
    "Resolve object reference";
    const obj = $match:match_object(obj_ref);
    !valid(obj) && raise(E_INVARG, "Could not find object: " + obj_ref);
    "Get verb code";
    const code_lines = `verb_code(obj, verb_name) ! E_VERBNF => 0';
    code_lines == 0 && raise(E_VERBNF, "Verb not found: " + verb_name);
    "Get verb metadata";
    const info = verb_info(obj, verb_name);
    const flags = info[2];
    const argspec = info[3];
    "Build content with context and reason";
    let context_line = "[Reading verb code]";
    if (reason && length(reason) > 0)
      context_line = context_line + " " + reason;
    endif
    const title = tostr(obj) + ":" + verb_name + " [" + flags + "] " + argspec;
    "Show first 12 lines as preview for the room";
    const total_lines = length(code_lines);
    const max_preview = 12;
    if (total_lines > max_preview)
      preview_lines = code_lines[1..max_preview];
      preview_str = preview_lines:join("\n");
    else
      preview_str = code_lines:join("\n");
    endif
    const omitted = total_lines > max_preview ? tostr(total_lines - max_preview) + " more lines." | "";
    const report = $format.block:mk($format.title:mk(title), context_line,
      $format.code:mk(preview_str, "moo", 1), omitted);
    this:_announce(report);
    "RETURN FULL CODE to the agent so it can actually see it";
    return ["code" -> code_lines:join("\n"), "flags" -> flags, "argspec" -> argspec, "total_lines" -> total_lines];
  endmethod

  verb look_self (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Override look_self to include formatted agent status.";
    "Get base look from parent";
    let iter = 0;
    let last_tool = 0;
    let max_iter = 0;
    let pct = 0;
    let query_preview = 0;
    let recent = 0;
    let task_preview = 0;
    const look_data = pass(@args);
    "Build status section with explicit newlines";
    const status_header = $format.title:mk("Agent Status", 4);
    let status_items = {};
    if (this.current_task && length(this.current_task) > 0)
      task_preview = this.current_task[1..min(50, length(this.current_task))];
      if (length(this.current_task) > 50)
        task_preview = task_preview + "...";
      endif
      status_items = {@status_items, "State: " + "Working"};
      status_items = {@status_items, "Task: " + ("\"" + task_preview + "\"")};
      "Get progress info if agent exists";
      if (valid(this.agent))
        iter = `this.agent.iteration ! ANY => 0';
        max_iter = `this.agent.max_iterations ! ANY => 50';
        last_tool = `this.agent.last_tool ! ANY => ""';
        if (iter > 0)
          pct = toint(iter * 100 / max_iter);
          status_items = {@status_items, "Progress: " + (tostr(iter) + "/" + tostr(max_iter)) + " (" + tostr(pct) + "%)"};
        endif
        if (last_tool && length(last_tool) > 0)
          status_items = {@status_items, "Tool: " + last_tool};
        endif
      endif
    else
      status_items = {@status_items, "State: " + "Idle"};
    endif
    "Queue info";
    const queue_len = length(this.task_queue);
    if (queue_len > 0)
      status_items = {@status_items, "Queue: " + (tostr(queue_len) + " pending")};
    endif
    const status_list = $format.list:mk(status_items, false);
    "History summary";
    let history_part = {};
    const hist_len = length(this.history);
    if (hist_len > 0)
      recent = this.history[hist_len];
      query_preview = recent['query][1..min(40, length(recent['query]))];
      if (length(recent['query]) > 40)
        query_preview = query_preview + "...";
      endif
      history_part = {"\n" + "Last Task:" + " [" + (tostr(recent['status])) + "] " + query_preview};
    endif
    "Combine description with status as list";
    let base_desc = look_data.description;
    if (typeof(base_desc) != TYPE_LIST)
      base_desc = {base_desc};
    endif
    const new_desc = {@base_desc, status_header, status_list, @history_part};
    "Return updated flyweight";
    const contents_list = flycontents(look_data);
    const exits = `look_data.exits ! E_PROPNF => {}';
    const ambient = `look_data.ambient_passages ! E_PROPNF => {}';
    const actions = `look_data.actions ! E_PROPNF => {}';
    return <look_data.delegate, .what = look_data.what, .title = look_data.title, .description = new_desc, .exits = exits, .ambient_passages = ambient, .actions = actions, {@contents_list}>;
  endverb

  method _on_progress owner: ARCH_WIZARD
    "Called by agent during run to report progress. Ephemeral to requester.";
    let max_iter = 0;
    let msg = 0;
    let pct = 0;
    const {agent, iteration, last_tool} = args;
    "Only report every 5 iterations, or on first iteration";
    if (iteration == 1 || iteration % 5 == 0)
      max_iter = agent.max_iterations;
      pct = toint(iteration * 100 / max_iter);
      msg = "Progress: " + tostr(iteration) + "/" + tostr(max_iter) + " (" + tostr(pct) + "%)";
      if (last_tool && length(last_tool) > 0)
        msg = msg + " - last: " + last_tool;
      endif
      this:_tell_requester(msg);
    endif
  endmethod

  verb reset (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Clear all agent state - kill tasks, clear queue, history, current task.";
    this:_stop_loop();
    "Clear all state";
    this.task_queue = {};
    this.history = {};
    player:inform_current($event:mk_info(player, "Agent room fully reset. Queue, history, and context cleared."));
  endverb

  verb compact (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Compact the current agent's context by summarizing it.";
    const agent = this.agent;
    !valid(agent) && return player:inform_current($event:mk_info(player, "No agent currently running."));
    const ctx = agent.context;
    length(ctx) < 3 && return player:inform_current($event:mk_info(player, "Context too short to compact."));
    "Build summary request";
    const summary_prompt = "Summarize the conversation so far in 2-3 sentences. Focus on: what task was given, what actions were taken, what was learned. Be concise.";
    const summary_messages = {@ctx, ["role" -> "user", "content" -> summary_prompt]};
    "Call LLM for summary";
    const client = agent.client;
    const response = client:chat(summary_messages, []);
    !response:is_valid() && return player:inform_current($event:mk_info(player, "Failed to generate summary."));
    const summary = response:content();
    "Replace context with system prompt + summary";
    const sys_msg = ctx[1];
    agent.context = {sys_msg, ["role" -> "user", "content" -> "CONTEXT SUMMARY (conversation was compacted):\n" + summary + "\n\nContinue working on the task."]};
    const old_len = length(ctx);
    const new_len = length(agent.context);
    player:inform_current($event:mk_info(player, "Compacted context from " + tostr(old_len) + " to " + tostr(new_len) + " messages."));
  endverb

  method _announce_thinking owner: ARCH_WIZARD
    "Send agent's thinking to requester only (ephemeral).";
    let end_tag = 0;
    let parsed = 0;
    let parts = 0;
    let start = 0;
    let val = 0;
    let {thinking} = args;
    thinking && length(thinking) > 0 || return;
    "Convert non-strings to readable form";
    if (typeof(thinking) != TYPE_STR)
      if (typeof(thinking) == TYPE_MAP)
        parts = {};
        for key in (mapkeys(thinking))
          val = thinking[key];
          if (typeof(val) == TYPE_STR && length(val) > 5)
            parts = {@parts, val};
          endif
        endfor
        thinking = length(parts) > 0 ? parts:join(" ") | "";
      else
        thinking = tostr(thinking);
      endif
    endif
    "Strip DSML markup that some models produce";
    if (index(thinking, "<\uFF5CDSML\uFF5C") > 0 || index(thinking, "<|DSML|") > 0)
      "Extract text content from DSML, skip the markup";
      "Find content after parameter declarations";
      start = index(thinking, "string=\"true\">");
      if (start > 0)
        thinking = thinking[start + 14..length(thinking)];
        "Remove any closing tags";
        end_tag = index(thinking, "</");
        if (end_tag > 0)
          thinking = thinking[1..end_tag - 1];
        endif
      else
        "Can't parse DSML, just strip the tags roughly";
        thinking = strsub(thinking, "<\uFF5CDSML\uFF5Cfunction_calls>", "");
        thinking = strsub(thinking, "<\uFF5CDSML\uFF5Cinvoke name=\"think\">", "");
        thinking = strsub(thinking, "<\uFF5CDSML\uFF5Cparameter name=\"thought\" string=\"true\">", "");
        thinking = strsub(thinking, "<|DSML|function_calls>", "");
        thinking = strsub(thinking, "<|DSML|invoke name=\"think\">", "");
        thinking = strsub(thinking, "<|DSML|parameter name=\"thought\" string=\"true\">", "");
      endif
      thinking = thinking:trim();
    endif
    "Skip if empty or very short after cleanup";
    length(thinking) < 10 && return;
    "Skip if it's mostly JSON garbage";
    if (length(thinking) > 0 && thinking[1] == "{")
      try
        parsed = parse_json(thinking);
        if (typeof(parsed) == TYPE_MAP)
          parts = {};
          for key in (mapkeys(parsed))
            val = parsed[key];
            if (typeof(val) == TYPE_STR && length(val) > 5)
              parts = {@parts, val};
            endif
          endfor
          thinking = length(parts) > 0 ? parts:join(" ") | "";
        endif
      except (ANY)
      endtry
    endif
    length(thinking) < 10 && return;
    "Truncate AFTER all processing - show reasonable amount of clean text";
    if (length(thinking) > 500)
      thinking = thinking[1..500] + "...";
    endif
    this:_tell_requester("Thinking: " + thinking);
  endmethod

  method _announce_status owner: ARCH_WIZARD
    "Send lightweight status update to requester only (ephemeral).";
    const {status} = args;
    status && length(status) > 0 || return;
    this:_tell_requester(status);
  endmethod

  method _tool_program_verb owner: ARCH_WIZARD
    "Tool: Program a verb with code preview.";
    let args_info = 0;
    let info = 0;
    let is_all_strings = 0;
    let preview_lines = 0;
    let preview_str = 0;
    let unique_cites = 0;
    let val = 0;
    let {args_map, actor} = args;
    $agent_building_tools:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const obj_ref = args_map["object"];
    const verb_name = args_map["verb"];
    let code = args_map["code"];
    "Handle case where code is wrapped in a map or list (common LLM mistakes)";
    if (typeof(code) == TYPE_MAP)
      "Try to extract the first string value";
      for key in (mapkeys(code))
        val = code[key];
        if (typeof(val) == TYPE_STR)
          code = val;
          break;
        endif
      endfor
    elseif (typeof(code) == TYPE_LIST)
      "Try to join list of strings";
      is_all_strings = true;
      for item in (code)
        if (typeof(item) != TYPE_STR)
          is_all_strings = false;
          break;
        endif
      endfor
      if (is_all_strings && length(code) > 0)
        code = code:join("\n");
      else
        "Not a simple list of strings - might be a mangled structure";
        code = toliteral(code);
      endif
    endif
    typeof(code) != TYPE_STR && raise(E_TYPE, "code must be a string, got: " + typeof(code));
    "Check for citations if the agent is $rlm_agent";
    let agent_obj = #-1;
    try
      agent_obj = caller;
      typeof(agent_obj) == TYPE_OBJ || (agent_obj = #-1);
    except (ANY)
    endtry
    const citations = valid(agent_obj) ? agent_obj.last_tool_citations | {};
    "Get current verb info for the preview";
    const target_obj = $match:match_object(obj_ref, actor);
    let v_flags = "???";
    let v_args = "...";
    if (valid(target_obj))
      try
        info = verb_info(target_obj, verb_name);
        v_flags = info[2];
        args_info = verb_args(target_obj, verb_name);
        v_args = args_info[1] + " " + args_info[2] + " " + args_info[3];
      except (ANY)
      endtry
    endif
    "Show code preview";
    const code_lines = code:split("\n");
    const total_lines = length(code_lines);
    const max_preview = 8;
    if (total_lines > max_preview)
      preview_lines = code_lines[1..max_preview];
      preview_str = preview_lines:join("\n");
    else
      preview_str = code;
    endif
    "Build announcement";
    let citation_msg = "";
    if (length(citations) > 0)
      "Format citations nicely";
      unique_cites = {};
      for c in (citations)
        if (!(c in unique_cites))
          unique_cites = {@unique_cites, c};
        endif
      endfor
      citation_msg = (" (citing " + unique_cites:join(", ") + ")");
    endif
    const context_line = "[Programming verb]" + citation_msg;
    const title = obj_ref + ":" + verb_name + " [" + v_flags + "] " + v_args + " (" + tostr(total_lines) + " lines)";
    const title_fw = $format.title:mk(title, 4);
    const code_fw = $format.code:mk(preview_str, 'moo, 1);
    const omitted = total_lines > max_preview ? tostr(total_lines - max_preview) + " more lines." | "";
    const content = $format.block:mk($format.paragraph:mk(context_line), title_fw, code_fw, omitted);
    "Announce to room";
    let final_agent = this;
    if (valid(this.agent))
      final_agent = this.agent;
    elseif (valid(agent_obj))
      final_agent = agent_obj;
    endif
    const event = $event:mk_announce(final_agent, content);
    this:announce(event);
    "Update args_map with extracted code and delegate";
    args_map["code"] = code;
    return $agent_building_tools:program_verb(args_map, actor);
  endmethod

  method _tell_requester owner: ARCH_WIZARD
    "Send ephemeral message to the task requester only.";
    const {message} = args;
    const requester = this.task_requester;
    !valid(requester) && return;
    const content = ("  \u2192 " + message);
    requester:inform_current($event:mk_info(requester, content));
  endmethod

  verb "fix revise" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Request correction/revision to previous work - continues with existing context.";
    "Usage: fix <instructions> - tells agent to revise existing work";
    let h = 0;
    const incoming = caller_perms();
    const requester = incoming == #-1 && caller == player && valid(player) && is_player(player) ? player | incoming;
    typeof(requester) == TYPE_OBJ && valid(requester) || raise(E_PERM);
    const command_text = argstr;
    if (!command_text)
      requester:inform_current($event:mk_info(requester, "What needs to be fixed or revised?"));
      return;
    endif
    "Get the most recent history entry for this requester";
    let prev_context = {};
    let prev_query = "";
    for i in [length(this.history)..1]
      h = this.history[i];
      if (h['player] == requester)
        prev_context = h['context] || {};
        prev_query = h['query];
        break;
      endif
    endfor
    if (length(prev_context) == 0)
      requester:inform_current($event:mk_info(requester, "No previous task found to continue from. Use 'do' to start a new task."));
      return;
    endif
    "Build the revision query";
    const revision_query = "REVISION REQUEST: Please fix/revise the previous work as follows: " + command_text;
    "Ensure event loop is running and send task with context";
    this:_ensure_loop();
    let msg = ["type" -> "task", 'query -> revision_query, 'player -> requester, 'queued_at -> time(), 'context -> prev_context];
    msg["authorization"] = $agent_room:_authorize_task(this, msg);
    try
      task_send(this.loop_task, msg);
    except e (ANY)
      "Loop may have died - restart and retry once";
      this:_start_loop();
      try
        task_send(this.loop_task, msg);
      except e2 (ANY)
        requester:inform_current($event:mk_error(requester, "Failed to queue revision."));
        return;
      endtry
    endtry
    requester:inform_current($event:mk_info(requester, "Revision queued (continuing from previous context): \"" + command_text + "\""));
  endverb

  method _execute_task owner: ARCH_WIZARD
    "Internal verb to execute a single task from the queue.";
    let brief = 0;
    let content = 0;
    let h = 0;
    let history_summary = 0;
    let max_hist = 0;
    let msg = 0;
    let report = 0;
    let result = 0;
    let selected = 0;
    let status = 0;
    let title = 0;
    caller == this || raise(E_PERM);
    const {task} = args;
    $agent_room:_consume_task(this, task);
    const task_player = task['player];
    "Set task tracking properties for status display";
    this.task_requester = task_player;
    this.current_task = task['query];
    "Announce to room that someone started a task - FULL prompt";
    this:_announce(task_player.name + " requested: \"" + task['query] + "\"");
    "Create persistent agent (with UUID) so it can be cited in eval()";
    const agent = create($rlm_agent, task_player);
    this.agent = agent;
    agent.client = valid(this.llm_client) ? this.llm_client | $llm_client;
    agent.max_iterations = 50;
    agent.progress_callback = {this, "_on_progress"};
    "Load all external tools";
    agent:load_external_tools();
    "Add room-specific presentation tools";
    for tool in (this:_get_room_tools())
      agent:add_tool(tool["name"], tool);
    endfor
    "Check if this is a continuation (fix) with preserved context";
    if (maphaskey(task, 'context) && typeof(task['context]) == typeof({}))
      "Restore previous context and add the new query";
      agent.context = task['context];
      agent.context = {@agent.context, ["role" -> "user", "content" -> task['query]]};
      agent.actor = task_player;
    else
      "Fresh setup";
      agent:setup(task['query], task_player);
      "Inject requester-local history summary if available";
      max_hist = `this.max_history_context_items ! E_PROPNF => 4';
      if (max_hist < 0)
        max_hist = 0;
      endif
      if (max_hist > 0 && length(this.history) > 0)
        selected = {};
        for i in [length(this.history)..1]
          h = this.history[i];
          if (`h['requester] ! ANY => #-1' == task_player)
            selected = {@selected, h};
            if (length(selected) >= max_hist)
              break;
            endif
          endif
        endfor
        if (length(selected) > 0)
          history_summary = "YOUR RECENT TASKS IN THIS ROOM (for continuity):\n";
          for h in (selected)
            history_summary = history_summary + "- " + h['query] + " -> " + tostr(h['status]) + "\n";
          endfor
          agent.context = {@agent.context, ["role" -> "system", "content" -> history_summary]};
        endif
      endif
    endif
    "Run agent";
    try
      result = agent:run(task_player);
      status = agent.status;
    except e (ANY)
      result = "Error: " + tostr(e[1]) + " - " + tostr(e[2]);
      status = 'failed;
    endtry
    "Record in history - include context for potential continuation";
    const entry = ['query -> task['query], 'result -> result, 'status -> status, 'finished_at -> time(), 'requester -> task_player, 'context -> agent.context];
    this.history = {@this.history, entry};
    "Announce completion to room with formatted result";
    if (status == 'complete)
      title = "Task Completed for " + task_player.name;
      content = result;
      if (valid(agent) && "_coerce_string" in verbs(agent))
        content = agent:_coerce_string(result);
      endif
      content = typeof(content) == typeof("") ? content | toliteral(content);
      report = $format.block:mk($format.title:mk(title), content);
      this:_announce(report);
    else
      brief = typeof(result) == typeof("") ? result | toliteral(result);
      if (length(brief) > 100)
        brief = brief[1..100] + "...";
      endif
      msg = ("  \u2192 Task " + tostr(status) + ": " + brief);
      task_player:inform_current($event:mk_info(task_player, msg));
      this:_announce("Task failed for " + task_player.name + ".");
    endif
    "Clear task tracking properties";
    this.current_task = "";
    this.agent = #-1;
    this.task_requester = #-1;
    "Cleanup agent object";
    try
      agent:destroy();
    except (ANY)
    endtry
  endmethod

  method _flatten_content owner: ARCH_WIZARD
    "Flatten complex content (lists, maps) into readable text.";
    let flat = 0;
    let key_str = 0;
    let parsed = 0;
    let parts = 0;
    let result = 0;
    let v = 0;
    let val_str = 0;
    let {val} = args;
    if (typeof(val) == TYPE_STR)
      "Check if string looks like JSON array and unwrap";
      if (length(val) > 4 && val[1] == "[" && val[2] == "\"")
        try
          parsed = parse_json(val);
          if (typeof(parsed) == TYPE_LIST)
            return this:_flatten_content(parsed);
          endif
        except (ANY)
        endtry
      endif
      return val;
    elseif (typeof(val) == TYPE_LIST)
      "Flatten list items";
      parts = {};
      for item in (val)
        flat = this:_flatten_content(item);
        if (length(flat) > 0)
          parts = {@parts, flat};
        endif
      endfor
      return parts:join(" ");
    elseif (typeof(val) == TYPE_MAP)
      "LLM fragmentation: keys and values both contain text fragments.";
      "Interleave keys and values to reconstruct.";
      result = "";
      for key in (mapkeys(val))
        key_str = typeof(key) == TYPE_STR ? key | tostr(key);
        v = val[key];
        val_str = typeof(v) == TYPE_STR ? v | typeof(v) == TYPE_INT || typeof(v) == TYPE_FLOAT ? tostr(v) | this:_flatten_content(v);
        result = result + key_str + val_str;
      endfor
      return result;
    endif
    return tostr(val);
  endmethod

  method _event_loop owner: ARCH_WIZARD
    "Main event loop. Receives tasks via task_recv, dispatches workers.";
    let all_tasks = 0;
    let done_tid = 0;
    let live_tids = 0;
    let max_workers = 0;
    let messages = 0;
    let msg_type = 0;
    let my_loop = 0;
    let new_active = 0;
    let task = 0;
    let timeout = 0;
    let pending = {};
    let active = {};
    const loop_id = task_id();
    let should_exit = false;
    while (!should_exit)
      "Wait for messages - short timeout when workers active, longer when idle";
      timeout = length(active) > 0 ? 5 | 60;
      messages = task_recv(timeout);
      "Process incoming messages";
      for msg in (messages)
        if (typeof(msg) != TYPE_MAP)
          continue;
        endif
        msg_type = `msg["type"] ! ANY => ""';
        if (msg_type == "task")
          pending = {@pending, msg};
        elseif (msg_type == "worker_done")
          "Worker finished - remove from active list";
          done_tid = `msg["tid"] ! ANY => 0';
          new_active = {};
          for atid in (active)
            if (atid != done_tid)
              new_active = {@new_active, atid};
            endif
          endfor
          active = new_active;
        elseif (msg_type == "stop")
          "Graceful shutdown - kill all active workers";
          for atid in (active)
            `kill_task(atid) ! ANY';
          endfor
          active = {};
          this.task_queue = pending;
          this.active_tasks = {};
          this.current_task = "";
          this.agent = #-1;
          this.task_requester = #-1;
          should_exit = true;
        endif
      endfor
      if (should_exit)
        break;
      endif
      "Clean up dead workers (safety net)";
      if (length(active) > 0)
        all_tasks = queued_tasks();
        live_tids = {};
        for t in (all_tasks)
          live_tids = {@live_tids, t[1]};
        endfor
        new_active = {};
        for atid in (active)
          if (atid in live_tids)
            new_active = {@new_active, atid};
          endif
        endfor
        active = new_active;
      endif
      "Dispatch pending tasks to workers (configurable concurrent limit)";
      max_workers = `this.max_workers ! E_PROPNF => 3';
      if (max_workers < 1)
        max_workers = 1;
      elseif (max_workers > 10)
        max_workers = 10;
      endif
      while (length(pending) > 0 && length(active) < max_workers)
        {task, @pending} = pending;
        my_loop = loop_id;
        fork worker_tid (0)
          set_task_perms(this.owner);
          try
            this:_execute_task(task);
          except e (ANY)
            set_task_perms(this.owner, {{"builtin_call", "server_log"}});
            server_log("AGENT WORKER FAILED on " + tostr(this) + ": " + toliteral(e));
          endtry
          "Signal completion back to the loop";
          `task_send(my_loop, ["type" -> "worker_done", "tid" -> task_id()]) ! ANY';
        endfork
        active = {@active, worker_tid};
      endwhile
      "Update visible state for status/queue commands";
      this.task_queue = pending;
      this.active_tasks = active;
      "If idle (no pending, no active) and no new messages arrived, exit";
      if (length(pending) == 0 && length(active) == 0 && length(messages) == 0)
        break;
      endif
    endwhile
    "Loop exiting - clean up";
    this.loop_task = 0;
    this.task_id = 0;
    this.active_tasks = {};
  endmethod

  method _start_loop owner: ARCH_WIZARD
    "Fork the event loop task and store its ID.";
    "Stop any existing loop first to prevent duplicates.";
    const lt = this.loop_task;
    this.loop_task = 0;
    this.task_id = 0;
    if (lt > 0)
      `kill_task(lt) ! ANY';
    endif
    "Kill any orphaned active workers";
    if (typeof(this.active_tasks) == TYPE_LIST)
      for atid in (this.active_tasks)
        `kill_task(atid) ! ANY';
      endfor
      this.active_tasks = {};
    endif
    fork tid (0)
      set_task_perms(this.owner);
      this:_event_loop();
    endfork
    this.loop_task = tid;
    this.task_id = tid;
    commit();
  endmethod

  method _stop_loop owner: ARCH_WIZARD
    "Kill the event loop and all active workers.";
    const lt = this.loop_task;
    this.loop_task = 0;
    this.task_id = 0;
    "Kill the loop task";
    if (lt > 0)
      `kill_task(lt) ! ANY';
    endif
    "Kill all active worker tasks";
    if (typeof(this.active_tasks) == TYPE_LIST)
      for atid in (this.active_tasks)
        `kill_task(atid) ! ANY';
      endfor
      this.active_tasks = {};
    endif
    this.current_task = "";
    this.agent = #-1;
    this.task_requester = #-1;
    commit();
  endmethod

  method _ensure_loop owner: ARCH_WIZARD
    "Start the event loop if not already running.";
    if (this.loop_task <= 0)
      this:_start_loop();
    endif
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
    const context = maphaskey(task, 'context) ? task['context] | {};
    typeof(context) == TYPE_LIST || raise(E_TYPE);
    return paseto_make_local(["purpose" -> "agent_room_task", "room" -> room, "actor" -> actor, "query" -> query, "context" -> context, "jti" -> uuid()]);
  endmethod

  method _consume_task owner: ARCH_WIZARD
    "Verify and consume the bounded request before callbacks, suspension, or task effects.";
    const principal = caller_perms();
    valid(principal) && principal.wizard || raise(E_PERM);
    const {room, task} = args;
    typeof(task) == TYPE_MAP && maphaskey(task, "authorization") || raise(E_PERM);
    let claims = [];
    try
      claims = paseto_verify_local(task["authorization"]);
    except (E_TYPE, E_INVARG)
      raise(E_PERM, "Invalid room task authorization");
    endtry
    typeof(claims) == TYPE_MAP || raise(E_PERM);
    for key in ({"purpose", "room", "actor", "query", "context", "jti"})
      maphaskey(claims, key) || raise(E_PERM, "Incomplete room task authorization");
    endfor
    const context = maphaskey(task, 'context) ? task['context] | {};
    claims["purpose"] == "agent_room_task" && claims["room"] == room && claims["actor"] == task['player] && claims["query"] == task['query] && claims["context"] == context || raise(E_PERM, "Room task authorization does not match request");
    typeof(claims["actor"]) == TYPE_OBJ && valid(claims["actor"]) || raise(E_PERM);
    typeof(claims["jti"]) == TYPE_STR && length(claims["jti"]) > 0 || raise(E_PERM);
    !$root:_capability_is_revoked(claims) || raise(E_PERM, "Room task authorization already consumed");
    $root:_revoke_capability_token(<$root, .jti = claims["jti"]>);
    return claims;
  endmethod
endobject
