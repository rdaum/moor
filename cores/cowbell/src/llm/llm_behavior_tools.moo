object LLM_BEHAVIOR_TOOLS [
  import_export_id -> "llm_behavior_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Behavior Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Reusable behavior operations for in-world and external agents.";

  method get_tools owner: ARCH_WIZARD
    "Return the domain's immutable tool descriptors with explicit effect classifications.";
    const target_obj = this;
    let tools = {};
    tools = {@tools, $llm_agent_tool:mk("list_rules", "List all rule properties (*_rule) on an object and their current expressions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#10', '$container', 'chest')"]], "required" -> {"object"}], target_obj, "list_rules", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("set_rule", "Set an access control rule on an object property. Rules are logical expressions like 'input.Key == #123' or 'NOT is_locked(object: input.This)'.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule, e.g., 'lock_rule')"], "expression" -> ["type" -> "string", "description" -> "Condition using AND, OR, NOT, comparisons, and named predicate arguments"]], "required" -> {"object", "property", "expression"}], target_obj, "set_rule", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("show_rule", "Display the current expression for a specific rule property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Rule property name (must end with _rule)"]], "required" -> {"object", "property"}], target_obj, "show_rule", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("test_rule", "Test a rule expression with specific variable bindings to see if it evaluates successfully.", ["type" -> "object", "properties" -> ["expression" -> ["type" -> "string", "description" -> "Rule expression to test (e.g., 'input.Key == #123')"], "bindings" -> ["type" -> "object", "description" -> "Variable bindings as key-value pairs (e.g., {\"This\": \"#123\", \"Accessor\": \"#5\", \"Key\": \"#456\"})"]], "required" -> {"expression", "bindings"}], target_obj, "tool_test_rule", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("evaluate_rule",
      "Evaluate a readable stored rule. Optional bindings use JSON values and explicit object references such as #123.",
      ["type" -> "object", "properties" -> [
        "object" -> ["type" -> "string", "description" -> "Object containing the rule"],
        "property" -> ["type" -> "string", "description" -> "Property ending in _rule"],
        "bindings" -> ["type" -> "object", "description" -> "Rule inputs; omitted bindings are empty"]],
        "required" -> {"object", "property"}], target_obj, "evaluate_rule", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("author_reaction",
      "Use the same reaction, state, and rule commands as a builder. Call with command=reaction and text=help first. "
        + "The help is generated from current definitions. New reactions start disabled. Inspect and preview before enabling.",
      ["type" -> "object", "properties" -> [
        "command" -> ["type" -> "string", "enum" -> {"reaction", "state", "rule", "facts"}],
        "text" -> ["type" -> "string", "description" -> "Arguments after @reaction, @state, @rule, or @facts. Do not include the command name."]],
        "required" -> {"command", "text"}], target_obj, "author_reaction", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("list_reactions", "List all reactions on an object with detailed information including triggers, conditions, effects, and enabled status.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#12', 'here', 'cupboard')"]], "required" -> {"object"}], target_obj, "list_reactions", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("list_messages", "List message template properties (*_msg) and message bags (*_msg_bag/_msgs) on an object.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object to inspect (e.g., '#62', '$room', 'here')"]], "required" -> {"object"}], target_obj, "list_messages", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("get_message_template", "Show a single message template or list the entries of a message bag.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"]], "required" -> {"object", "property"}], target_obj, "get_message_template", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("set_message_template", "Set a message template on an object property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msg, _msgs, or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], target_obj, "set_message_template", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("add_message_template", "Append a message template to a message bag property (_msgs or _msg_bag).", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "template" -> ["type" -> "string", "description" -> "Template string using {sub} syntax"]], "required" -> {"object", "property", "template"}], target_obj, "add_message_template", 'mutation)};
    tools = {@tools, $llm_agent_tool:mk("delete_message_template", "Remove a message entry by index from a message bag property.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "Object reference"], "property" -> ["type" -> "string", "description" -> "Property name (must end with _msgs or _msg_bag)"], "index" -> ["type" -> "integer", "description" -> "1-based index to remove"]], "required" -> {"object", "property", "index"}], target_obj, "delete_message_template", 'mutation)};
    let bounded = {};
    for tool in (tools)
      let schema = tool.parameters;
      let fields = schema["properties"];
      for field in (mapkeys(fields))
        let definition = fields[field];
        if (`definition["type"] ! E_RANGE => ""' == "string")
          definition["maxLength"] = field in {"template", "expression", "text"} ? 8192 | 256;
        elseif (`definition["type"] ! E_RANGE => ""' == "integer")
          definition["minimum"] = 1;
          definition["maximum"] = 1024;
        endif
        fields[field] = definition;
      endfor
      fields["rationale"] = ["type" -> "string", "maxLength" -> 4096];
      schema["properties"] = fields;
      schema["additionalProperties"] = false;
      bounded = {@bounded, $llm_agent_tool:mk(tool.name, tool.description, schema,
        this, tool.target_verb, tool.effect)};
    endfor
    return bounded;
  endmethod

  method _snapshot owner: ARCH_WIZARD
    "Capture optional properties, referenced bags, and authoring state before approval.";
    const {actor, target, kind, member} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    valid(target) || raise(E_INVARG, "Target no longer exists.");
    if (kind == 'message || kind == 'optional_property)
      let state = {target, target.owner, parent(target), member, false};
      try
        const value = target.(member);
        state = {target, target.owner, parent(target), member, true,
          property_info(target, member), value};
        if (kind == 'message && typeof(value) == TYPE_OBJ && isa(value, $msg_bag))
          state = {@state, value.owner, property_info(value, "entries"), value.entries};
        endif
      except (E_PROPNF)
        "A missing property is a distinct state, including after a callback creates it.";
      endtry
      return state;
    elseif (kind == 'author)
      let state = [];
      const fields = target.reaction_state_fields;
      let visited = 0;
      for definer in ({target, @ancestors(target)})
        for property in (`properties(definer) ! E_PERM => {}')
          visited = visited + 1;
          visited <= 1024 || raise(E_QUOTA, "Too many properties for an authoring preview.");
          const name = tostr(property);
          if (name:ends_with("_reaction") || name:ends_with("_rule")
              || name in {"reaction_state_fields", "fact_schema", "set_version"}
              || name:starts_with("tuple_") || maphaskey(fields, tosym(name)))
            state[property] = {property_info(target, property), target.(property)};
          endif
        endfor
      endfor
      const destination = typeof(member) == TYPE_OBJ
        ? this:_snapshot(actor, member, 'author, false) | {};
      const snapshot = {target, target.owner, parent(target), state, destination};
      length(toliteral(snapshot)) <= 1048576 || raise(E_QUOTA, "Authoring preview is too large.");
      return snapshot;
    endif
    return pass(@args);
  endmethod

  method list_rules owner: ARCH_WIZARD
    "Tool: List all rule properties on an object";
    let prop = 0;
    let prop_name = 0;
    let rule_str = 0;
    let value = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const rule_props = $obj_utils:rule_properties(target_obj, actor);
    !rule_props && return "No rule properties found on " + tostr(target_obj) + ".";
    let lines = {"Rules on " + tostr(target_obj) + ":"};
    for prop_info in (rule_props)
      {prop, value} = prop_info;
      prop_name = tostr(prop);
      if (value == 0)
        lines = {@lines, "  " + prop_name + ": (no rule set)"};
      elseif (typeof(value) == TYPE_FLYWEIGHT)
        rule_str = $rule_engine:decompile_rule(value);
        lines = {@lines, "  " + prop_name + ": " + rule_str};
      else
        lines = {@lines, "  " + prop_name + ": " + toliteral(value)};
      endif
    endfor
    return lines:join("\n");
  endmethod

  method set_rule owner: ARCH_WIZARD
    "Tool: Set a rule on an object property";
    let expression = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, expression} = {args_map["object"], args_map["property"], args_map["expression"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set rules on " + tostr(target_obj));
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property name must end with '_rule'");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    const prop_exists = prop_key != E_PROPNF;
    const preview = this:_preview(actor, target_obj, 'optional_property, prop_exists ? prop_key | prop_name);
    set_task_perms(actor);
    const compiled = $rule_engine:parse_expression(expression, prop_name, $rule_engine:policy_scope(prop_name));
    const validation = $rule_engine:validate_rule(compiled);
    !validation['valid] && raise(E_INVARG, "Rule validation failed: " + validation['warnings]:join("; "));
    const approval = this:_approve(actor, 'set_rule, target_obj, 'optional_property,
      prop_exists ? prop_key | prop_name, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    if (prop_exists)
      target_obj.(prop_key) = compiled;
    else
      add_property(target_obj, prop_name, compiled, {actor, "r"});
    endif
    return "Set " + tostr(target_obj) + "." + prop_name + " = " + expression;
  endmethod

  method show_rule owner: ARCH_WIZARD
    "Tool: Show a specific rule property";
    let obj_spec = 0;
    let prop_name = 0;
    let rule_str = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name} = {args_map["object"], args_map["property"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    prop_name:ends_with("_rule") || raise(E_INVARG, "Property name must end with '_rule'");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && return tostr(target_obj) + "." + prop_name + " is not defined.";
    set_task_perms(actor);
    const value = target_obj.(prop_key);
    if (value == 0)
      return tostr(target_obj) + "." + prop_name + ": (no rule set)";
    elseif (typeof(value) == TYPE_FLYWEIGHT)
      rule_str = $rule_engine:decompile_rule(value);
      return tostr(target_obj) + "." + prop_name + ": " + rule_str;
    endif
    return tostr(target_obj) + "." + prop_name + ": " + toliteral(value);
  endmethod

  method tool_test_rule owner: ARCH_WIZARD
    "Evaluate a condition using supplied inputs. Preserve literal text and query errors.";
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const expression = args_map["expression"];
    typeof(expression) == TYPE_STR || raise(E_TYPE, "expression must be string");
    const inputs = $llm_behavior_tools:_rule_inputs(args_map["bindings"]);
    let input_types = [];
    for name in (mapkeys(inputs))
      input_types[name] = $behavior_schema:type_of(inputs[name]);
    endfor
    const compiled = $rule_engine:parse_expression(expression, 'test_rule,
      $rule_engine:scope(input_types));
    const result = $rule_engine:evaluate(compiled, inputs);
    if (result['success])
      return "Condition matched. Selected values: " + toliteral(result['bindings]);
    endif
    return "Condition did not match the supplied inputs.";
  endmethod

  method _rule_inputs owner: ARCH_WIZARD
    "Convert bounded JSON input names and explicit object literals without evaluating code.";
    const {bindings} = args;
    typeof(bindings) == TYPE_MAP || raise(E_TYPE, "Bindings must be a JSON object.");
    length(bindings) <= 64 && value_bytes(bindings) <= 16384 || raise(E_QUOTA, "Too many rule inputs.");
    $llm_value:validate_json(bindings);
    let inputs = [];
    for key in (mapkeys(bindings))
      length(key) > 0 && length(key) <= 64 || raise(E_INVARG, "Invalid rule input name.");
      let value = bindings[key];
      if (typeof(value) == TYPE_STR && value:starts_with("#"))
        value = fromliteral(value);
        typeof(value) == TYPE_OBJ || raise(E_INVARG, "Expected an object reference for " + key);
      endif
      inputs[tosym(key)] = value;
    endfor
    return inputs;
  endmethod

  method evaluate_rule owner: ARCH_WIZARD
    "Evaluate a readable stored rule with optional JSON inputs under the authenticated actor.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const name = request["property"];
    name:ends_with("_rule") || raise(E_INVARG, "Property must end with _rule.");
    const target = $llm_tool_provider:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const rule = target.(name);
    rule == 0 && return tostr(target) + "." + name + " = (not set) - cannot evaluate";
    const inputs = $llm_behavior_tools:_rule_inputs(`request["bindings"] ! E_RANGE => []');
    const result = $rule_engine:evaluate(rule, inputs);
    let lines = {"Evaluation of " + tostr(target) + "." + name + ":",
      "Expression: " + $rule_engine:decompile_rule(rule), "Initial bindings: " + toliteral(inputs),
      "Success: " + tostr(result['success])};
    if (result['success])
      lines = {@lines, "Result bindings: " + toliteral(result['bindings])};
    endif
    if (length(result['alternatives]) > 0)
      lines = {@lines, "Alternatives: " + tostr(length(result['alternatives])) + " more solutions found"};
    endif
    if (length(result['warnings]) > 0)
      lines = {@lines, "Warnings: " + result['warnings]:join("; ")};
    endif
    value_bytes(lines) <= 65536 || raise(E_QUOTA, "Rule evaluation output is too large.");
    return lines:join("\n");
  endmethod

  method list_reactions owner: ARCH_WIZARD
    "Show complete reaction definitions through the same service used by builders.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const host = $builder_features:_author_object(toliteral(request["object"]), actor);
    (host.owner == actor || actor.wizard) || raise(E_PERM);
    let lines = {};
    for entry in ($obj_utils:reaction_properties(host, actor))
      lines = {@lines, @$builder_features:_author_inspect(host, tostr(entry[1])[1..$ - 9],
        $reaction:_definition(entry[2]))};
    endfor
    return length(lines) == 0 ? "No reactions." | lines:join("\n");
  endmethod

  method list_messages owner: ARCH_WIZARD
    "Tool: List message template properties on an object";
    let is_bag = 0;
    let is_compiled = 0;
    let prop = 0;
    let prop_name = 0;
    let value = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target_obj = this:_resolve_object(args_map["object"], actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const msg_props = $obj_utils:message_properties(target_obj, actor);
    !msg_props && return "No message properties found on " + tostr(target_obj) + ".";
    let lines = {"Messages on " + tostr(target_obj) + ":"};
    for prop_info in (msg_props)
      {prop, value} = prop_info;
      prop_name = tostr(prop);
      if ($msg_bag:is_msg_bag(value))
        lines = {@lines, "  " + prop_name + ": " + tostr(value) + " (" + tostr(length(value:entries())) + " entries)"};
      elseif (typeof(value) == TYPE_STR)
        lines = {@lines, "  " + prop_name + ": \"" + (length(value) > 50 ? value[1..50] + "..." | value) + "\""};
      elseif (typeof(value) == TYPE_FLYWEIGHT)
        lines = {@lines, "  " + prop_name + ": (compiled template)"};
      elseif (typeof(value) == TYPE_LIST)
        "Check if it's a compiled template (first item not a list) or a message bag (list of lists)";
        is_compiled = length(value) > 0 && typeof(value[1]) != TYPE_LIST;
        is_bag = length(value) > 0 && typeof(value[1]) == TYPE_LIST;
        if (is_compiled)
          lines = {@lines, "  " + prop_name + ": (compiled template)"};
        elseif (is_bag)
          lines = {@lines, "  " + prop_name + ": [" + tostr(length(value)) + " entries]"};
        else
          lines = {@lines, "  " + prop_name + ": (empty)"};
        endif
      else
        lines = {@lines, "  " + prop_name + ": " + toliteral(value)};
      endif
    endfor
    return lines:join("\n");
  endmethod

  method get_message_template owner: ARCH_WIZARD
    "Tool: Get a message template value";
    let entries = 0;
    let entry = 0;
    let lines = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name} = {args_map["object"], args_map["property"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && return tostr(target_obj) + "." + prop_name + " is not defined.";
    set_task_perms(actor);
    const value = target_obj.(prop_key);
    "Check for message bag (object or flyweight)";
    if ($msg_bag:is_msg_bag(value))
      entries = value:entries();
      lines = {tostr(target_obj) + "." + prop_name + ": " + tostr(value)};
      for i in [1..length(entries)]
        entry = entries[i];
        lines = {@lines, "  " + tostr(i) + ": " + `$sub_utils:decompile(entry) ! ANY => toliteral(entry)'};
      endfor
      return lines:join("\n");
    endif
    "Check for compiled template (list starting with non-list)";
    const is_compiled = typeof(value) == TYPE_LIST && length(value) > 0 && typeof(value[1]) != TYPE_LIST;
    if (typeof(value) == TYPE_LIST && is_compiled)
      return tostr(target_obj) + "." + prop_name + ": " + $sub_utils:decompile(value);
    elseif (typeof(value) == TYPE_LIST)
      "List of message entries";
      lines = {tostr(target_obj) + "." + prop_name + " (" + tostr(length(value)) + " entries):"};
      for i in [1..length(value)]
        entry = value[i];
        lines = {@lines, "  " + tostr(i) + ": " + `$sub_utils:decompile(entry) ! ANY => toliteral(entry)'};
      endfor
      return lines:join("\n");
    elseif (typeof(value) == TYPE_FLYWEIGHT)
      return tostr(target_obj) + "." + prop_name + ": (compiled template)";
    elseif (typeof(value) == TYPE_STR)
      return tostr(target_obj) + "." + prop_name + ": \"" + value + "\"";
    endif
    return tostr(target_obj) + "." + prop_name + ": " + toliteral(value);
  endmethod

  method set_message_template owner: ARCH_WIZARD
    "Tool: Set a message template on an object";
    let obj_spec = 0;
    let prop_name = 0;
    let template = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, template} = {args_map["object"], args_map["property"], args_map["template"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set messages on " + tostr(target_obj));
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    const member = prop_key == E_PROPNF ? prop_name | prop_key;
    const preview = this:_preview(actor, target_obj, 'message, member);
    const compiled = $sub_utils:compile(template, target_obj:message_bindings(prop_name));
    const approval = this:_approve(actor, 'set_message_template, target_obj, 'message, member, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    if (prop_key != E_PROPNF)
      $obj_utils:set_compiled_message(target_obj, prop_key, compiled, actor);
    elseif (prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag"))
      $obj_utils:add_message_entry(target_obj, prop_name, compiled, actor);
    else
      set_task_perms(actor);
      add_property(target_obj, prop_name, compiled, {actor, "rc"});
    endif
    return "Set " + tostr(target_obj) + "." + prop_name + ": " + template;
  endmethod

  method add_message_template owner: ARCH_WIZARD
    "Tool: Add a message to a message bag";
    let obj_spec = 0;
    let prop_name = 0;
    let template = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, template} = {args_map["object"], args_map["property"], args_map["template"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to set messages on " + tostr(target_obj));
    prop_name:ends_with("_msgs") || prop_name:ends_with("_msg_bag") || raise(E_INVARG, "Property must end with _msgs or _msg_bag");
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    const member = prop_key == E_PROPNF ? prop_name | prop_key;
    const preview = this:_preview(actor, target_obj, 'message, member);
    const compiled = $sub_utils:compile(template, target_obj:message_bindings(prop_name));
    const approval = this:_approve(actor, 'add_message_template, target_obj, 'message, member, args_map, preview);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    $obj_utils:add_message_entry(target_obj, prop_name, compiled, actor);
    return "Added message to " + tostr(target_obj) + "." + prop_name + ": " + template;
  endmethod

  method delete_message_template owner: ARCH_WIZARD
    "Tool: Delete a message from a message bag by index";
    let index = 0;
    let obj_spec = 0;
    let prop_name = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    {obj_spec, prop_name, index} = {args_map["object"], args_map["property"], args_map["index"]};
    const target_obj = this:_resolve_object(obj_spec, actor);
    typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
    valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    !actor.wizard && target_obj.owner != actor && raise(E_PERM, "You must be owner or wizard to modify messages on " + tostr(target_obj));
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && raise(E_INVARG, "Message bag not found on " + tostr(target_obj) + "." + prop_name);
    const approval = this:_approve(actor, 'delete_message_template, target_obj, 'message, prop_key, args_map);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    $obj_utils:remove_message_entry(target_obj, prop_key, index, actor);
    return "Deleted message " + tostr(index) + " from " + tostr(target_obj) + "." + prop_name;
  endmethod

  method author_reaction owner: ARCH_WIZARD
    "Author behavior through the same authenticated service used by text commands.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    const command = tosym(request["command"]);
    command in {'reaction, 'state, 'rule, 'facts} || raise(E_INVARG);
    actor.is_builder || raise(E_PERM, "Builder privileges are required.");
    set_task_perms(actor);
    const words = $builder_features:_author_tokens(request["text"]);
    if (length(words) == 0 || words[1] in {"help", "events", "steps", "actions", "facts", "conditions"})
      return $builder_features:author(command, request["text"], actor):join("\n");
    endif
    const host = $builder_features:_author_object(words[1], actor);
    host.owner == actor || actor.wizard || raise(E_PERM, "You do not own the target.");
    const effect = this:_author_effect(command, words);
    let destination = false;
    if (effect == 'reaction_copy)
      length(words) >= 5 || raise(E_INVARG, "Use copy to OBJECT.");
      destination = $builder_features:_author_object(words[5], actor);
      destination.owner == actor || actor.wizard || raise(E_PERM, "You do not own the destination.");
    endif
    if (effect != 'inspection)
      const target = typeof(destination) == TYPE_OBJ ? destination | host;
      const other = typeof(destination) == TYPE_OBJ ? host | false;
      let proposal = mapmerge(request, ["resolved_target" -> tostr(host)]);
      if (typeof(destination) == TYPE_OBJ)
        proposal["resolved_destination"] = tostr(destination);
      endif
      const approval = this:_approve(actor, effect, target, 'author, other, proposal);
      typeof(approval) == TYPE_BOOL && approval || return approval;
    endif
    return $builder_features:author_on(command, host, words, actor, destination):join("\n");
  endmethod

  method _author_effect owner: ARCH_WIZARD
    "Classify builder operations so inspection stays immediate and approvals stay operation-specific.";
    const {command, words} = args;
    if (command == 'reaction)
      const operation = length(words) < 3 ? "show" | words[3];
      if (operation in {"show", "check", "preview", "preview-message"}
          || (operation == "trace" && length(words) == 3))
        return 'inspection;
      endif
      operation in {"trace", "after", "when", "if", "then", "step", "remove-step", "with",
        "select", "enable", "disable", "remove", "copy"} || raise(E_INVARG, "Unknown reaction operation.");
      return tosym("reaction_" + operation);
    elseif (command == 'state)
      length(words) == 1 && return 'inspection;
      return length(words) > 2 && words[3] == "set" ? 'state_set | 'state_declare;
    elseif (command == 'facts)
      (length(words) == 1 || words == {words[1], "show"}) && return 'inspection;
      words[2] in {"define", "add", "remove"} || raise(E_INVARG, "Unknown fact operation.");
      return tosym("facts_" + words[2]);
    endif
    return 'author_rule;
  endmethod

  method test_tool_get_message_template_actor_permissions owner: ARCH_WIZARD
    "get_message_template should read a private target property under ordinary actor permissions.";
    let msg_result = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msg", "secret message", {$test_player, ""});
      msg_result = this:get_message_template(["object" -> tostr(target), "property" -> "wearable_private_msg"], $test_player);
      $test_utils:assert_true(index(msg_result, "\"secret message\"") > 0, "get_message_template should read a non-readable property under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_list_messages_actor_permissions owner: ARCH_WIZARD
    "list_messages should enumerate and read private target message properties under ordinary actor permissions.";
    let msg_list = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msg", "secret message", {$test_player, ""});
      msg_list = this:list_messages(["object" -> tostr(target)], $test_player);
      $test_utils:assert_true(index(msg_list, "wearable_private_msg") > 0, "list_messages should read private message properties under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_show_rule_actor_permissions owner: ARCH_WIZARD
    "show_rule should read a private target rule under ordinary actor permissions.";
    let rule_result = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_rule", 0, {$test_player, ""});
      rule_result = this:show_rule(["object" -> tostr(target), "property" -> "wearable_private_rule"], $test_player);
      $test_utils:assert_true(index(rule_result, "(no rule set)") > 0, "show_rule should read a non-readable rule under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_list_rules_actor_permissions owner: ARCH_WIZARD
    "list_rules should enumerate and read private target rule properties under ordinary actor permissions.";
    let rule_list = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_rule", 0, {$test_player, ""});
      rule_list = this:list_rules(["object" -> tostr(target)], $test_player);
      $test_utils:assert_true(index(rule_list, "wearable_private_rule") > 0, "list_rules should read private rule properties under property ownership");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_set_rule_actor_permissions owner: ARCH_WIZARD
    "set_rule should write a private target rule under ordinary actor permissions.";
    let set_result = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_set_rule", 1, {$test_player, ""});
      set_result = this:set_rule(["object" -> tostr(target), "property" -> "wearable_set_rule", "expression" -> "TRUE"], $test_player);
      $test_utils:assert_true(index(set_result, "Set ") > 0, "set_rule should write a non-writable rule through a property_write grant");
      $test_utils:assert_true($rule_engine:evaluate(target.wearable_set_rule, [])['success], "The stored condition permits the operation");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_set_message_template_actor_permissions owner: ARCH_WIZARD
    "set_message_template should write a private target message property under ordinary actor permissions.";
    let set_result = 0;
    const target = $thing:create(0);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_set_msg", "old", {$test_player, ""});
      set_result = this:set_message_template(["object" -> tostr(target), "property" -> "wearable_set_msg", "template" -> "new message"], $test_player);
      $test_utils:assert_true(index($sub_utils:decompile(target.wearable_set_msg), "new message") > 0, "set_message_template should store the compiled template");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_add_message_template_actor_permissions owner: ARCH_WIZARD
    "add_message_template should append to a private message bag under ordinary actor permissions.";
    let add_result = 0;
    const target = $thing:create(0);
    const bag = create($msg_bag, $test_player, 1);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      add_property(target, "wearable_private_msgs", bag, {$test_player, ""});
      add_result = this:add_message_template(["object" -> tostr(target), "property" -> "wearable_private_msgs", "template" -> "added message"], $test_player);
      $test_utils:assert_eq(length(bag.entries), 1, "add_message_template should append one bag entry");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

  method test_tool_delete_message_template_actor_permissions owner: ARCH_WIZARD
    "delete_message_template should remove from a private message bag under ordinary actor permissions.";
    let del_result = 0;
    const target = $thing:create(0);
    const bag = create($msg_bag, $test_player, 1);
    try
      target.owner = $test_player;
      target.name = "Wearable Private Probe";
      target.r = 1;
      bag.entries = {$sub_utils:compile("first"), $sub_utils:compile("second")};
      add_property(target, "wearable_private_msgs", bag, {$test_player, ""});
      del_result = this:delete_message_template(["object" -> tostr(target), "property" -> "wearable_private_msgs", "index" -> 1], $test_player);
      $test_utils:assert_eq(length(bag.entries), 1, "delete_message_template should remove one bag entry");
      $test_utils:assert_eq($sub_utils:decompile(bag.entries[1]), "second", "Deletion preserves the remaining template");
    finally
      `set_task_perms($arch_wizard) ! ANY => 0';
      `$test_utils:destroy_if_valid(target) ! ANY => 0';
    endtry
    return true;
  endmethod

endobject
