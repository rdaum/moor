object RLM_AGENT [
  import_export_id -> "rlm_agent",
  import_export_hierarchy -> {"llm"}
]
  name: "RLM Agent"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property actor (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property child_agents (owner: ARCH_WIZARD, flags: "rc") = [];
  property client (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property context (owner: ARCH_WIZARD, flags: "rc") = [];
  property depth (owner: ARCH_WIZARD, flags: "rc") = 0;
  property findings (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property iteration (owner: ARCH_WIZARD, flags: "rc") = 0;
  property last_error (owner: ARCH_WIZARD, flags: "rc") = 0;
  property last_tool (owner: ARCH_WIZARD, flags: "rc") = "";
  property last_tool_citations (owner: ARCH_WIZARD, flags: "rc") = {};
  property max_depth (owner: ARCH_WIZARD, flags: "r") = 5;
  property max_iterations (owner: ARCH_WIZARD, flags: "r") = 100;
  property parent_agent (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property progress_callback (owner: ARCH_WIZARD, flags: "rc") = 0;
  property query (owner: ARCH_WIZARD, flags: "rc") = "";
  property result (owner: ARCH_WIZARD, flags: "rc") = 0;
  property status (owner: ARCH_WIZARD, flags: "rc") = "pending";
  property system_prompt (owner: ARCH_WIZARD, flags: "rc") = "";
  property tools (owner: ARCH_WIZARD, flags: "rc") = {};
  property workspace (owner: ARCH_WIZARD, flags: "rc") = {};

  verb add_tool (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Register a tool for this agent to use.";
    const principal = caller_perms();
    valid(principal) && (principal == this.owner || principal.wizard) || raise(E_PERM);
    "Accepts both flyweight tools and external tool maps.";
    const {tool_name, tool} = args;
    typeof(tool_name) != TYPE_STR && raise(E_TYPE, "tool_name must be string");
    "Accept flyweights or maps with input_schema";
    if (typeof(tool) != TYPE_FLYWEIGHT && typeof(tool) != TYPE_MAP)
      raise(E_TYPE, "tool must be flyweight or map");
    endif
    if (typeof(tool) == TYPE_MAP && !maphaskey(tool, "input_schema"))
      raise(E_TYPE, "tool map must have input_schema");
    endif
    this.tools[tool_name] = tool;
  endverb

  verb _find_tool (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Find a registered tool by name. Returns flyweight or #-1 if not found.";
    caller == this || raise(E_PERM);
    const {tool_name} = args;
    maphaskey(this.tools, tool_name) && return this.tools[tool_name];
    return #-1;
  endverb

  verb _get_tool_schemas (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Get OpenAI-format tool schemas from registered tools.";
    let schema = 0;
    let tool = 0;
    caller == this || raise(E_PERM);
    let schemas = {};
    for tool_name in (mapkeys(this.tools))
      tool = this.tools[tool_name];
      if (typeof(tool) == TYPE_FLYWEIGHT)
        "Flyweight tool - use its to_schema method";
        schemas = {@schemas, tool:to_schema()};
      elseif (typeof(tool) == TYPE_MAP && maphaskey(tool, "input_schema"))
        "External tool format - convert to OpenAI format";
        schema = ["type" -> "function", "function" -> ["name" -> tool["name"], "description" -> tool["description"], "parameters" -> tool["input_schema"]]];
        schemas = {@schemas, schema};
      endif
    endfor
    return schemas;
  endverb

  verb _call_llm (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Call LLM API with tool schemas. Returns $llm_response flyweight.";
    let cb = 0;
    let err_str = 0;
    let response = 0;
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {tool_schemas, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    const max_retries = 3;
    for retry_count in [0..max_retries]
      try
        response = this.client:chat(this.context, 0, false, false, tool_schemas);
        suspend(0);
        return $llm_response:mk(response);
      except e (ANY)
        "Log full error details via progress callback";
        cb = this.progress_callback;
        if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
          err_str = toliteral(e);
          `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"LLM Error (attempt " + tostr(retry_count + 1) + "/" + tostr(max_retries + 1) + "): " + err_str}) ! ANY';
          "If error message is long, show more of it";
          if (length(e) >= 2 && typeof(e[2]) == TYPE_STR && length(e[2]) > 100)
            `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"Full error: " + e[2]}) ! ANY';
          endif
        endif
        "Store last error for debugging";
        this.last_error = e;
        retry_count >= max_retries && raise(E_INVARG, "LLM API call failed: " + toliteral(e));
        suspend(retry_count + 1);
      endtry
    endfor
  endverb

  verb _execute_tool_call (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Execute a single tool call. Returns result map for context.";
    let result = 0;
    let target = 0;
    let tool = 0;
    let val = 0;
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    const {tool_call, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    const tool_name = tool_call["function"]["name"];
    const raw_args = tool_call["function"]["arguments"];
    const tool_call_id = tool_call["id"];
    "Parse JSON args if needed";
    let tool_args = [];
    if (typeof(raw_args) == TYPE_STR)
      try
        tool_args = parse_json(raw_args);
      except (ANY)
        return ["tool_call_id" -> tool_call_id, "role" -> "tool", "name" -> tool_name, "content" -> "ERROR: Invalid JSON in arguments. Please pass a clean JSON object."];
      endtry
    else
      tool_args = raw_args;
    endif
    "Only legacy built-ins coerce arguments. Catalog tools validate their published schemas.";
    typeof(tool_args) == TYPE_MAP || return ["tool_call_id" -> tool_call_id, "role" -> "tool", "name" -> tool_name, "content" -> "ERROR: Tool arguments must be a JSON object."];
    if (tool_name in {"moo_eval", "spawn_agent", "report_finding", "think"})
      for key in (mapkeys(tool_args))
        val = tool_args[key];
        if (typeof(val) == TYPE_STR)
          tool_args[key] = this:_coerce_string(val);
        elseif (typeof(val) == TYPE_MAP)
          tool_args[key] = this:_coerce_string(val);
        elseif (typeof(val) != TYPE_LIST && typeof(val) != TYPE_BOOL && typeof(val) != TYPE_INT && typeof(val) != TYPE_FLOAT)
          tool_args[key] = this:_coerce_string(val);
        endif
      endfor
    endif
    "Handle built-in RLM tools first";
    if (tool_name == "moo_eval")
      result = this:_builtin_eval(tool_args, request_principal);
    elseif (tool_name == "spawn_agent")
      result = this:_builtin_spawn(tool_args, request_principal);
    elseif (tool_name == "report_finding")
      result = this:_builtin_report(tool_args, request_principal);
    elseif (tool_name == "think")
      result = this:_builtin_think(tool_args, request_principal);
    else
      "Try registered tools";
      tool = this:_find_tool(tool_name);
      if (tool == #-1)
        result = "ERROR: Unknown tool: " + tool_name;
      elseif (typeof(tool) == TYPE_FLYWEIGHT)
        try
          set_task_perms(request_principal);
          result = tool:execute(tool_args, request_principal);
        except e (ANY)
          result = "ERROR: " + tostr(e[1]) + " - " + tostr(e[2]);
        endtry
      elseif (typeof(tool) == TYPE_MAP && maphaskey(tool, "target_verb"))
        try
          set_task_perms(request_principal);
          target = tool["target_obj"];
          verb = tool["target_verb"];
          result = target:(verb)(tool_args, request_principal);
        except e (ANY)
          result = "ERROR: " + tostr(e[1]) + " - " + tostr(e[2]);
        endtry
      else
        result = "ERROR: Invalid tool format for: " + tool_name;
      endif
    endif
    "Structured tool results carry their own paging contract. Do not truncate JSON or source.";
    if (typeof(result) == TYPE_MAP && maphaskey(result, "ok"))
      return ["tool_call_id" -> tool_call_id, "role" -> "tool", "name" -> tool_name, "content" -> generate_json(result)];
    endif
    "Truncate legacy text results";
    let content_out = typeof(result) == TYPE_STR ? result | toliteral(result);
    const max_len = 4000;
    if (length(content_out) > max_len)
      content_out = content_out[1..max_len] + "\n... [TRUNCATED]";
    endif
    return ["tool_call_id" -> tool_call_id, "role" -> "tool", "name" -> tool_name, "content" -> content_out];
  endverb

  verb initialize (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Called automatically on creation. Sets defaults.";
    pass();
    this.status = 'pending;
    this.result = 0;
    this.child_agents = {};
    this.context = {};
    this.tools = [];
  endverb

  verb _builtin_eval (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Execute MOO code with access to workspace. Returns truncated output.";
    let err = 0;
    let msg = 0;
    let output = 0;
    let result = 0;
    let trimmed = 0;
    let unwrapped = 0;
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    let {tool_args, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    "Parse JSON if needed";
    if (typeof(tool_args) == TYPE_STR)
      try
        tool_args = parse_json(tool_args);
      except (ANY)
        tool_args = [];
      endtry
    endif
    let code = tool_args["code"];
    typeof(code) != TYPE_STR && return "ERROR: code must be a string, got " + typeof(code);
    "Unwrap JSON-like structures that LLMs sometimes produce";
    "Handle [\"code\"] array format";
    if (length(code) > 4 && code[1] == "[" && code[2] == "\"")
      try
        unwrapped = parse_json(code);
        if (typeof(unwrapped) == TYPE_LIST && length(unwrapped) == 1 && typeof(unwrapped[1]) == TYPE_STR)
          code = unwrapped[1];
        elseif (typeof(unwrapped) == TYPE_LIST)
          code = this:_coerce_string(unwrapped);
        endif
      except (ANY)
      endtry
    endif
    "Handle {\"key\": \"value\"} object format - extract string values";
    if (length(code) > 2 && code[1] == "{" && code[2] == "\"")
      try
        unwrapped = parse_json(code);
        if (typeof(unwrapped) == TYPE_MAP)
          code = this:_coerce_string(unwrapped);
        endif
      except (ANY)
      endtry
    endif
    "Strip trailing brackets from malformed output";
    code = code:trim();
    while (length(code) > 0 && code[length(code)] == "]")
      code = code[1..length(code) - 1]:trim();
    endwhile
    while (length(code) > 0 && code[length(code)] == "}")
      code = code[1..length(code) - 1]:trim();
    endwhile
    "Check if code has return - if not and it's a simple expression, add return";
    const has_return = index(code, "return ") > 0 || index(code, "return(") > 0;
    if (!has_return)
      trimmed = code:trim();
      if (length(trimmed) > 0 && trimmed[length(trimmed)] == ";")
        trimmed = trimmed[1..length(trimmed) - 1]:trim();
      endif
      "If no internal semicolons, it's likely a simple expression - wrap in return";
      if (index(trimmed, ";") == 0 && length(trimmed) > 0)
        code = "return " + trimmed + ";";
      endif
    endif
    "Final sanity check - if code is empty or garbage, report it";
    if (length(code:trim()) == 0)
      return "ERROR: Empty code after cleanup. Original input was malformed.";
    endif
    "Inject agent reference and actor";
    const injected_code = "agent = " + toliteral(this) + "; actor = " + toliteral(request_principal) + "; " + code;
    try
      set_task_perms(request_principal);
      if (request_principal.wizard)
        server_log("AGENT EVAL (WIZARD): " + code);
      endif
      result = eval(injected_code);
      if (!result[1])
        err = result[2];
        if (typeof(err) == TYPE_MAP && maphaskey(err, "message"))
          msg = "ERROR: " + err["message"] + (maphaskey(err, "line") ? " at line " + tostr(err["line"]) | "");
          if (index(msg, "expected an operator") || index(msg, "expected an expression"))
            msg = msg + " (HINT: Check syntax - MOO uses {1,2,3} for lists, not [1,2,3])";
          endif
          return msg;
        endif
        return "ERROR: " + toliteral(err);
      endif
      output = toliteral(result[2]);
      if (length(output) > 4000)
        output = output[1..4000] + "\n... [TRUNCATED]";
      endif
      return output;
    except e (ANY)
      return "ERROR: " + tostr(e[1]) + " - " + tostr(e[2]);
    endtry
  endverb

  verb _builtin_spawn (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Spawn a child agent with a sub-query and workspace subset.";
    let short_query = 0;
    let ws = 0;
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    let {tool_args, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    "Parse JSON if needed";
    if (typeof(tool_args) == TYPE_STR)
      tool_args = parse_json(tool_args);
    endif
    const sub_query = tool_args["query"];
    typeof(sub_query) != TYPE_STR && return "ERROR: query must be a string";
    "Check depth limit";
    if (this.depth >= this.max_depth)
      return "ERROR: Maximum recursion depth (" + tostr(this.max_depth) + ") reached. Cannot spawn more agents.";
    endif
    "Announce spawn via callback";
    const cb = this.progress_callback;
    if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
      short_query = length(sub_query) > 60 ? sub_query[1..60] + "..." | sub_query;
      `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce", {"Spawning child agent: \"" + short_query + "\""}) ! ANY';
    endif
    "Get workspace for child - either explicit or empty";
    let child_workspace = [];
    if (maphaskey(tool_args, "workspace"))
      ws = tool_args["workspace"];
      "Parse if it's a JSON string";
      if (typeof(ws) == TYPE_STR)
        child_workspace = `parse_json(ws) ! ANY => []';
      elseif (typeof(ws) == TYPE_MAP)
        child_workspace = ws;
      endif
    endif
    "Create anonymous child agent - garbage collected when parent completes";
    const child = create($rlm_agent, request_principal, 1);
    child.client = this.client;
    child.max_depth = this.max_depth;
    child.max_iterations = this.max_iterations;
    child.progress_callback = this.progress_callback;
    "Copy registered tools to child";
    for tool_name in (mapkeys(this.tools))
      child:add_tool(tool_name, this.tools[tool_name]);
    endfor
    "Setup and run child with workspace";
    child:setup(sub_query, request_principal, this, this.depth + 1, child_workspace);
    this.child_agents = {@this.child_agents, child};
    const result = child:run(request_principal);
    "Announce completion";
    if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
      `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce", {"Child agent completed (depth " + tostr(this.depth + 1) + ")"}) ! ANY';
    endif
    "Return child's result";
    return "Child agent (depth " + tostr(this.depth + 1) + ") completed.\nResult: " + (typeof(result) == TYPE_STR ? result | toliteral(result));
  endverb

  verb _builtin_report (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Record a finding or report final answer.";
    let has_announce = 0;
    let loc = 0;
    let preview = 0;
    caller == this || raise(E_PERM);
    const principal = caller_perms();
    let {tool_args, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    "Parse JSON if needed";
    if (typeof(tool_args) == TYPE_STR)
      try
        tool_args = parse_json(tool_args);
      except (ANY)
        tool_args = [];
      endtry
    endif
    let subject = tool_args["subject"];
    let content = tool_args["content"];
    const is_final = maphaskey(tool_args, "final") && tool_args["final"];
    "Identify and resolve any mangled structures in content";
    content = this:_coerce_string(content);
    "Validate and coerce subject";
    if (typeof(subject) != TYPE_STR)
      subject = typeof(subject) == TYPE_MAP || typeof(subject) == TYPE_LIST ? toliteral(subject) | tostr(subject);
    endif
    "Store finding in shared knowledge base";
    if (valid(this.findings))
      this.findings:assert({this.depth, subject, content});
    endif
    "Notify actor's location if it has _announce (like $agent_room)";
    if (valid(request_principal) && valid(request_principal.location))
      loc = request_principal.location;
      has_announce = false;
      try
        verb_info(loc, "_announce");
        has_announce = true;
      except (ANY)
      endtry
      if (has_announce)
        "If it's final, we let the Room handle the wrap-up announcement";
        "unless we aren't being tracked by a specialized room.";
        if (!is_final)
          preview = content[1..min(1000, length(content))];
          if (length(content) > 1000)
            preview = preview + "...";
          endif
          loc:_announce("Finding [" + subject + "] " + preview);
        endif
      endif
    endif
    "If final answer, set result and mark complete";
    if (is_final)
      this.result = content;
      this.status = 'complete;
      return "Final answer recorded. Agent will terminate.";
    endif
    return "Finding recorded: " + subject;
  endverb

  verb _get_builtin_schemas (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Return OpenAI-format schemas for the builtin RLM tools.";
    caller == this || raise(E_PERM);
    const eval_schema = ["type" -> "function", "function" -> ["name" -> "moo_eval", "description" -> "Execute a MOO program to query state or calculate values. IMPORTANT: You MUST use 'return <expression>;' if you want to see a result. This tool takes full statements, not just a single expression.", "parameters" -> ["type" -> "object", "properties" -> ["code" -> ["type" -> "string", "description" -> "MOO code block. Variable 'agent' refers to you. Access workspace via 'agent.workspace'. MUST include 'return' to get data back. Example: 'return ctime();' or 'return verbs(#123);'"], "rationale" -> ["type" -> "string", "description" -> "What you're trying to learn from this query."]], "required" -> {"code", "rationale"}]]];
    const spawn_schema = ["type" -> "function", "function" -> ["name" -> "spawn_agent", "description" -> "Spawn a child agent with a focused sub-task and a subset of context. The child's workspace becomes its context to query. Child runs to completion and returns its result.", "parameters" -> ["type" -> "object", "properties" -> ["query" -> ["type" -> "string", "description" -> "The sub-task for the child. Be specific about what you need answered."], "workspace" -> ["type" -> "object", "description" -> "Context for the child - typically a partition or filtered subset."]], "required" -> {"query"}]]];
    const report_schema = ["type" -> "function", "function" -> ["name" -> "report_finding", "description" -> "Record a finding or report your final answer. Findings persist in a shared knowledge base. Use final=true when done.", "parameters" -> ["type" -> "object", "properties" -> ["subject" -> ["type" -> "string", "description" -> "Category of finding (e.g., 'summary', 'issue', 'pattern')."], "content" -> ["type" -> "string", "description" -> "The finding or answer."], "final" -> ["type" -> "boolean", "description" -> "True if this is your complete answer. Agent terminates after."]], "required" -> {"subject", "content"}]]];
    const think_schema = ["type" -> "function", "function" -> ["name" -> "think", "description" -> "Share your current thinking or status with the user. Use this to explain what you're doing, why, or what you've learned. Helps the user follow your progress.", "parameters" -> ["type" -> "object", "properties" -> ["thought" -> ["type" -> "string", "description" -> "Your current thinking, plan, or status update."]], "required" -> {"thought"}]]];
    return {eval_schema, spawn_schema, report_schema, think_schema};
  endverb

  verb run (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Main agent loop. Execute until complete or max iterations.";
    let assistant_msg = 0;
    let brief = 0;
    let cb = 0;
    let cleaned_args = 0;
    let content = 0;
    let dsml_tool_calls = 0;
    let expr = 0;
    let findings = 0;
    let nudge = 0;
    let reasoning = 0;
    let reminder = 0;
    let res_content = 0;
    let response = 0;
    let status_msg = 0;
    let thinking = 0;
    let tool_calls_to_process = 0;
    let tool_name = 0;
    let tool_result_map = 0;
    let tool_results = 0;
    let used_dsml = 0;
    const principal = caller_perms();
    const {?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    principal == this.owner || principal.wizard || raise(E_PERM);
    !valid(this.client) && raise(E_INVARG, "No LLM client configured");
    this.status = 'running;
    this.iteration = 0;
    this.last_tool = "";
    const tool_schemas = {@this:_get_builtin_schemas(), @this:_get_tool_schemas()};
    const wrap_up_at = toint(tofloat(this.max_iterations) * 0.8);
    for iteration in [1..this.max_iterations]
      this.iteration = iteration;
      if (this.status == 'complete)
        break;
      endif
      cb = this.progress_callback;
      if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
        `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], cb[2], {this, iteration, this.last_tool}) ! ANY';
      endif
      if (iteration == wrap_up_at)
        reminder = ["role" -> "system", "content" -> "SYSTEM REMINDER: You are at iteration " + tostr(iteration) + " of " + tostr(this.max_iterations) + ". Please wrap up soon. Use report_finding with final=true when done."];
        this.context = {@this.context, reminder};
      endif
      if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
        `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"Thinking..."}) ! ANY';
      endif
      response = this:_call_llm(tool_schemas, request_principal);
      if (!response:is_valid())
        this.status = 'failed;
        this.result = "LLM returned invalid response";
        return this.result;
      endif
      reasoning = response:reasoning();
      thinking = response:content();
      if (reasoning && length(reasoning) > 0 && typeof(cb) == TYPE_LIST && length(cb) >= 2)
        "Let _announce_thinking handle truncation after processing";
        `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_thinking", {reasoning}) ! ANY';
      elseif (thinking && length(thinking) > 0 && typeof(cb) == TYPE_LIST && length(cb) >= 2)
        "Let _announce_thinking handle truncation after stripping DSML";
        `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_thinking", {thinking}) ! ANY';
      endif
      "Get tool calls - either from response or parsed from DSML in content";
      tool_calls_to_process = response:tool_calls();
      used_dsml = false;
      if (!response:has_tool_calls())
        "Check for DSML markup in content (DeepSeek compatibility)";
        content = response:content() || "";
        dsml_tool_calls = this:_parse_dsml_tool_calls(content);
        if (length(dsml_tool_calls) > 0)
          "Found DSML tool calls - use those instead";
          if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
            `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"Parsed " + tostr(length(dsml_tool_calls)) + " tool call(s) from DSML"}) ! ANY';
          endif
          tool_calls_to_process = dsml_tool_calls;
          used_dsml = true;
        else
          "No tool calls and no DSML - nudge the model";
          this.context = {@this.context, response:message()};
          nudge = ["role" -> "system", "content" -> "SYSTEM: You responded with text but didn't call any tools. If you're done, call report_finding with final=true. If not, continue using tools to complete the task."];
          this.context = {@this.context, nudge};
          suspend(0);
          continue;
        endif
      endif
      tool_results = {};
      for tool_call in (tool_calls_to_process)
        tool_name = tool_call["function"]["name"];
        this.last_tool = tool_name;
        tool_result_map = this:_execute_tool_call(tool_call, request_principal);
        tool_results = {@tool_results, tool_result_map};
        cleaned_args = typeof(tool_call["function"]["arguments"]) == TYPE_STR ? `parse_json(tool_call["function"]["arguments"]) ! ANY => []' | tool_call["function"]["arguments"];
        if (typeof(cleaned_args) == TYPE_MAP)
          for key in (mapkeys(cleaned_args))
            cleaned_args[key] = this:_resolve_citations(cleaned_args[key])['content];
          endfor
        endif
        status_msg = tool_name;
        if (tool_name == "moo_eval" && typeof(cleaned_args) == TYPE_MAP && maphaskey(cleaned_args, "code"))
          expr = cleaned_args["code"];
          if (length(expr) > 60)
            expr = expr[1..60] + "...";
          endif
          status_msg = "moo_eval: " + expr;
        elseif (typeof(cleaned_args) == TYPE_MAP && maphaskey(cleaned_args, "object"))
          status_msg = tool_name + ": " + cleaned_args["object"];
        elseif (typeof(cleaned_args) == TYPE_MAP && maphaskey(cleaned_args, "target"))
          status_msg = tool_name + ": " + cleaned_args["target"];
        elseif (typeof(cleaned_args) == TYPE_MAP && maphaskey(cleaned_args, "topic"))
          status_msg = tool_name + ": " + cleaned_args["topic"];
        elseif (typeof(cleaned_args) == TYPE_MAP && maphaskey(cleaned_args, "name"))
          status_msg = tool_name + ": " + cleaned_args["name"];
        endif
        if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
          `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {status_msg}) ! ANY';
        endif
        if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
          res_content = tool_result_map["content"];
          if (index(res_content, "ERROR") == 1)
            `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"  \u2192 " + res_content}) ! ANY';
          elseif (tool_name == "moo_eval")
            "Special case for eval: show result preview";
            brief = res_content;
            if (length(brief) > 500)
              brief = brief[1..500] + "...";
            endif
            `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"  \u2192 " + brief}) ! ANY';
          elseif (tool_name != "think" && tool_name != "report_finding" && tool_name != "show_verb" && tool_name != "program_verb" && length(res_content) > 0 && length(res_content) < 200)
            `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_status", {"  \u2192 " + res_content}) ! ANY';
          endif
        endif
        if (this.status == 'complete)
          break;
        endif
      endfor
      "Build assistant message - synthetic if we parsed DSML";
      if (used_dsml)
        "Construct proper assistant message with tool_calls for API compatibility";
        assistant_msg = ["role" -> "assistant", "tool_calls" -> tool_calls_to_process];
      else
        assistant_msg = response:message();
      endif
      this.context = {@this.context, assistant_msg, @tool_results};
      suspend(0);
    endfor
    if (this.status != 'complete)
      this.status = 'failed;
      findings = valid(this.findings) ? this.findings:tuples() | {};
      if (length(findings) > 0)
        this.result = "Max iterations reached. Partial findings:\n";
        for f in (findings)
          this.result = this.result + "- " + tostr(f[2]) + ": " + tostr(f[3]) + "\n";
        endfor
      else
        this.result = "Max iterations (" + tostr(this.max_iterations) + ") reached without completing";
      endif
    endif
    return this.result;
  endverb

  verb ask (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Convenience: create an agent, run it, and return the result.";
    "Usage: $rlm_agent:ask(query, actor, client)";
    "Agent is anonymous - garbage collected after use.";
    const {query, actor, client} = args;
    const principal = caller_perms();
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    set_task_perms(actor);
    const agent = $rlm_agent:create(true);
    agent:set_owner(actor);
    agent.client = client;
    agent:setup(query, actor);
    return agent:run(actor);
  endverb

  verb get_findings (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Get all findings from this agent's knowledge base.";
    set_task_perms(caller_perms());
    !valid(this.findings) && return {};
    return this.findings:tuples();
  endverb

  verb setup (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set up an RLM agent instance with query, actor, and optional initial context.";
    const {query, actor, ?parent = #-1, ?depth = 0, ?initial_workspace = []} = args;
    const principal = caller_perms();
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    actor == principal || principal.wizard || raise(E_PERM);
    principal == this.owner || principal.wizard || raise(E_PERM);
    this.query = query;
    this.actor = actor;
    this.parent_agent = parent;
    this.depth = depth;
    this.status = 'pending;
    this.result = 0;
    this.child_agents = {};
    this.workspace = initial_workspace;
    "Create findings relation for this subtree (or inherit from parent)";
    "Uses anonymous object - garbage collected with the agent";
    if (valid(parent) && valid(parent.findings))
      this.findings = parent.findings;
    else
      set_task_perms(actor);
      this.findings = $relation:create(true);
    endif
    "Build mode-aware system prompt";
    const q = query:lowercase();
    let mode = "mixed";
    const inspect_hints = {"inspect", "explain", "review", "understand", "show", "list", "find", "what is", "why"};
    const change_hints = {"create", "build", "add", "modify", "change", "fix", "update", "program", "set", "remove", "delete"};
    let inspect_score = 0;
    let change_score = 0;
    for h in (inspect_hints)
      if (index(q, h) > 0)
        inspect_score = inspect_score + 1;
      endif
    endfor
    for h in (change_hints)
      if (index(q, h) > 0)
        change_score = change_score + 1;
      endif
    endfor
    if (inspect_score > 0 && change_score == 0)
      mode = "inspect";
    elseif (change_score > 0 && inspect_score == 0)
      mode = "change";
    endif
    const guide = this:_get_guide() + "\n\n" + $change_tools.guide;
    let mode_policy = "MODE: MIXED - Inspect first, then change only where needed.";
    if (mode == "inspect")
      mode_policy = "MODE: INSPECT - Do not modify world state unless explicitly requested. Focus on reading and reporting.";
    elseif (mode == "change")
      mode_policy = "MODE: CHANGE - Make requested changes, but inspect target code/objects before writing.";
    endif
    const prompt = "You are a MOO building and maintenance agent. Use tools carefully and deliver concrete results.\n\nTASK: " + query + "\n\n" + mode_policy + "\n\nCOMPLETION CONTRACT:\n- You must finish by calling report_finding with final=true exactly once.\n- Do not end with plain assistant text alone.\n\nTOOL ARGUMENT CONTRACT:\n- Pass clean JSON object arguments only.\n- Keep object and verb separate (object=\"$room\", verb=\"description\").\n- Never include protocol or prefix noise in arguments.\n- For moo_eval, include return when you need output and terminate statements with semicolons.\n\nWORKFLOW CONTRACT:\n1. Think briefly about plan.\n2. Read before write: inspect existing verbs/properties/objects first.\n3. Apply minimal changes needed.\n4. Verify with follow-up reads/eval.\n5. Report findings and finish with report_finding(final=true).\n\nERROR RECOVERY CONTRACT:\n- On tool error: retry once with corrected arguments.\n- If second attempt fails, choose an alternate tool/path.\n- If blocked, report a concise blocker with exact error text.\n\nITERATION BUDGETING:\n- By iteration 20: have completed discovery and selected approach.\n- By iteration 35: have executed core change or proven blocker.\n- By iteration 45: run verification and prepare final report.\n\nPRIVACY SCOPE:\n- In shared rooms, do not reveal other users' private task details unless actor is a wizard.\n\n" + guide;
    this.system_prompt = prompt;
    this.context = {["role" -> "system", "content" -> this.system_prompt]};
    return this;
  endverb

  verb load_external_tools (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Load tools from #0:external_agent_tools() into this agent.";
    const principal = caller_perms();
    valid(principal) && (principal == this.owner || principal.wizard) || raise(E_PERM);
    "Optional filter list to load only specific tools.";
    let filter = 0;
    let tool_name = 0;
    {?filter = {}} = args;
    const external_tools = $external_agent_tools();
    for tool in (external_tools)
      tool_name = tool["name"];
      "Skip if filter provided and tool not in filter";
      if (length(filter) > 0 && !(tool_name in filter))
        continue;
      endif
      "Store the tool map directly - execution will handle it";
      this.tools[tool_name] = tool;
    endfor
    return mapkeys(this.tools);
  endverb

  verb _builtin_think (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Share thinking/status with the user via progress callback.";
    let val = 0;
    const principal = caller_perms();
    let {tool_args, ?request_principal = this.actor} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    "Handle malformed args - LLM sometimes sends weird map structures";
    let thought = "";
    if (typeof(tool_args) == TYPE_MAP)
      if (maphaskey(tool_args, "thought"))
        thought = tool_args["thought"];
      else
        "No 'thought' key - extract any string values";
        for key in (mapkeys(tool_args))
          val = tool_args[key];
          if (typeof(val) == TYPE_STR && length(val) > length(thought))
            thought = val;
          endif
        endfor
      endif
    elseif (typeof(tool_args) == TYPE_STR)
      thought = tool_args;
    endif
    "Ensure thought is a string";
    if (typeof(thought) != TYPE_STR)
      thought = tostr(thought);
    endif
    !thought || length(thought) == 0 && return "Please provide a thought to share.";
    "Announce via callback if available";
    const cb = this.progress_callback;
    if (typeof(cb) == TYPE_LIST && length(cb) >= 2)
      `$rlm_agent:_invoke_request_callback(this, request_principal, cb[1], "_announce_thinking", {thought}) ! ANY';
    endif
    return "Shared: " + thought[1..min(50, length(thought))] + (length(thought) > 50 ? "..." | "");
  endverb

  verb _get_guide (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Return concise MOO programming guide for agents.";
    return "MOO QUICK GUIDE\n\nSYNTAX:\n- Lists use {a, b, c}.\n- Maps use [\"key\" -> value].\n- Indices are 1-based.\n\nVERB SHAPES:\n- Command verbs (player commands): flags rd, argspec matches parser usage.\n- Method verbs (code calls): flags rxd, commonly argspec this none this.\n\nEVENT OUTPUT:\n- Use player:inform_current($event:mk_info(...)) for direct responses.\n- Use room announce events for world-visible actions.\n- Do not use notify() directly.\n\nREAD BEFORE WRITE:\n- Before editing a verb, inspect/read current code first.\n- For object behavior questions, inspect object then read relevant verbs.\n\nWRITING CODE:\n- program_verb code must be one clean string with \\n for newlines.\n- Keep changes small and targeted; prefer early returns.\n\nmoo_eval REMINDER:\n- Use return to surface values you need to inspect.\n- End statements with semicolons.\n\nREPORTING:\n- If you changed behavior, verify and summarize what changed and why.\n- If blocked, report exact failing step and error text.";
  endverb

  verb _coerce_string (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Aggressively convert mangled model output into a single string.";
    let coerced = 0;
    let end_idx = 0;
    let idx = 0;
    let key_str = 0;
    let parts = 0;
    let patterns = 0;
    let result = 0;
    let v = 0;
    let val_str = 0;
    let {val} = args;
    if (typeof(val) == TYPE_STR)
      "Clean up hallucinated internal protocol tokens common in some model outputs (Kimi 2.5, DeepSeek)";
      patterns = {"<|tool_call_begin|>", "<|tool_call_end|>", "<|tool_call_argument_begin|>", "<|tool_calls_section_begin|>", "<|tool_calls_section_end|>", "<|DSML|", "<\uFF5CDSML\uFF5C", "functions.", ">functions.", "</thought>", "<|thought|>", "_of_thought"};
      for p in (patterns)
        idx = index(val, p);
        while (idx > 0)
          if (p[1] == "<" || p[1] == ">")
            end_idx = index(val[idx..$], ">") || index(val[idx..$], " ");
            if (end_idx)
              val = val[1..idx - 1] + val[idx + end_idx..$];
            else
              val = val[1..idx - 1] + val[idx + length(p)..$];
            endif
          else
            val = val[1..idx - 1] + val[idx + length(p)..$];
          endif
          idx = index(val, p);
        endwhile
      endfor
      "Strip remaining punctuation garbage at start";
      while (length(val) > 0 && index(" {\"\":>])}", val[1]))
        val = val[2..$];
      endwhile
      return val:trim();
    elseif (typeof(val) == TYPE_LIST)
      if (length(val) == 1)
        return this:_coerce_string(val[1]);
      endif
      parts = {};
      for item in (val)
        coerced = this:_coerce_string(item);
        if (length(coerced) > 0)
          parts = {@parts, coerced};
        endif
      endfor
      return parts:join(" ");
    elseif (typeof(val) == TYPE_MAP)
      result = "";
      for key in (mapkeys(val))
        key_str = typeof(key) == TYPE_STR ? key | tostr(key);
        key_str = this:_coerce_string(key_str);
        v = val[key];
        val_str = typeof(v) == TYPE_STR ? v | typeof(v) == TYPE_INT || typeof(v) == TYPE_FLOAT ? tostr(v) | this:_coerce_string(v);
        result = result + key_str + val_str;
      endfor
      return result;
    endif
    return tostr(val);
  endverb

  verb _resolve_citations (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Resolve citation structures in LLM output to clean content.";
    "Returns map with 'content key containing the cleaned value.";
    const {val} = args;
    "Simple types pass through";
    if (typeof(val) == TYPE_STR)
      return ['content -> val];
    elseif (typeof(val) == TYPE_INT || typeof(val) == TYPE_FLOAT)
      return ['content -> tostr(val)];
    elseif (typeof(val) == TYPE_OBJ)
      return ['content -> tostr(val)];
    endif
    "Use _coerce_string for complex types (maps, lists)";
    return ['content -> this:_coerce_string(val)];
  endverb

  verb _parse_dsml_tool_calls (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Parse DSML markup from DeepSeek and convert to tool_calls format.";
    "Returns list of tool call maps, or empty list if no DSML found.";
    let block_end = 0;
    let end_calls = 0;
    let end_param = 0;
    let func_name = 0;
    let invoke_start = 0;
    let name_end = 0;
    let name_start = 0;
    let next_invoke = 0;
    let next_param = 0;
    let param_name = 0;
    let param_search_pos = 0;
    let param_start = 0;
    let param_value = 0;
    let params = 0;
    let pname_end = 0;
    let pname_start = 0;
    let tag_close = 0;
    let tool_call = 0;
    let value_end = 0;
    let value_start = 0;
    const {content} = args;
    !content && return {};
    "Check for DSML markers";
    const dsml_marker = "<\uFF5CDSML\uFF5C";
    if (!index(content, dsml_marker))
      return {};
    endif
    let tool_calls = {};
    "Find all invoke blocks";
    let pos = 1;
    while (pos <= length(content))
      "Find next invoke";
      invoke_start = index(content[pos..length(content)], "<\uFF5CDSML\uFF5Cinvoke");
      if (!invoke_start)
        break;
      endif
      invoke_start = invoke_start + pos - 1;
      "Extract name attribute";
      name_start = index(content[invoke_start..length(content)], "name=\"");
      if (!name_start)
        break;
      endif
      name_start = name_start + invoke_start - 1 + 6;
      name_end = index(content[name_start..length(content)], "\"");
      if (!name_end)
        break;
      endif
      func_name = content[name_start..name_start + name_end - 2];
      "Find parameters for this invoke";
      params = [];
      "Find the end of this invoke block (next invoke or end of function_calls)";
      next_invoke = index(content[invoke_start + 10..length(content)], "<\uFF5CDSML\uFF5Cinvoke");
      end_calls = index(content[invoke_start..length(content)], "</\uFF5CDSML\uFF5Cfunction_calls>");
      if (next_invoke)
        block_end = invoke_start + 10 + next_invoke - 2;
      elseif (end_calls)
        block_end = invoke_start + end_calls - 2;
      else
        block_end = length(content);
      endif
      "Extract parameters within this invoke block";
      param_search_pos = invoke_start;
      while (param_search_pos < block_end)
        param_start = index(content[param_search_pos..block_end], "<\uFF5CDSML\uFF5Cparameter");
        if (!param_start)
          break;
        endif
        param_start = param_start + param_search_pos - 1;
        "Extract parameter name";
        pname_start = index(content[param_start..block_end], "name=\"");
        if (!pname_start)
          break;
        endif
        pname_start = pname_start + param_start - 1 + 6;
        pname_end = index(content[pname_start..block_end], "\"");
        if (!pname_end)
          break;
        endif
        param_name = content[pname_start..pname_start + pname_end - 2];
        "Find the end of this parameter tag";
        tag_close = index(content[pname_start..block_end], ">");
        if (!tag_close)
          break;
        endif
        value_start = pname_start + tag_close;
        "Find the end of parameter value (next parameter or end of block)";
        next_param = index(content[value_start..block_end], "<\uFF5CDSML\uFF5Cparameter");
        end_param = index(content[value_start..block_end], "</\uFF5CDSML\uFF5Cparameter>");
        if (end_param && (!next_param || end_param < next_param))
          value_end = value_start + end_param - 2;
        elseif (next_param)
          value_end = value_start + next_param - 2;
        else
          value_end = block_end;
        endif
        param_value = content[value_start..value_end];
        "Trim whitespace";
        while (length(param_value) > 0 && param_value[1] == " ")
          param_value = param_value[2..length(param_value)];
        endwhile
        while (length(param_value) > 0 && param_value[length(param_value)] == " ")
          param_value = param_value[1..length(param_value) - 1];
        endwhile
        params[param_name] = param_value;
        param_search_pos = value_end + 1;
      endwhile
      "Create tool call entry with STRING keys to match OpenAI format";
      tool_call = ["id" -> tostr(random()), "type" -> "function", "function" -> ["name" -> func_name, "arguments" -> generate_json(params)]];
      tool_calls = {@tool_calls, tool_call};
      pos = block_end + 1;
    endwhile
    return tool_calls;
  endverb

  verb _invoke_request_callback (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Invoke an external progress callback under the captured requester.";
    const {subject, principal, callback, callback_verb, callback_args} = args;
    caller == subject || raise(E_PERM);
    const incoming = caller_perms();
    incoming == principal || incoming.wizard || raise(E_PERM);
    set_task_perms(principal);
    return callback:(callback_verb)(@callback_args);
  endverb
endobject
