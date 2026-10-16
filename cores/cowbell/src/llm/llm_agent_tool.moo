object LLM_AGENT_TOOL [
  import_export_id -> "llm_agent_tool",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Agent Tool"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for LLM agent tool definitions. Converts to OpenAI tool schema and executes tool calls.";

  method mk owner: HACKER
    "Create a tool definition flyweight";
    const {name, description, parameters, target_obj, target_verb, ?effect = 'unknown} = args;
    typeof(name) == TYPE_STR || raise(E_TYPE);
    typeof(description) == TYPE_STR || raise(E_TYPE);
    typeof(parameters) == TYPE_MAP || raise(E_TYPE);
    typeof(target_obj) == TYPE_OBJ || raise(E_TYPE);
    typeof(target_verb) == TYPE_STR || raise(E_TYPE);
    effect in {'inspection, 'mutation, 'presentation, 'input, 'external, 'unknown} || raise(E_INVARG);
    const schema = length(parameters) == 0 ? ["type" -> "object", "properties" -> []] | parameters;
    return <this, .effect = effect, .name = name, .description = description, .parameters = schema, .target_obj = target_obj, .target_verb = target_verb>;
  endmethod

  method to_schema owner: HACKER
    "Convert tool definition to OpenAI tool schema format";
    return ["type" -> "function", "function" -> ["name" -> this.name, "description" -> this.description, "parameters" -> this.parameters]];
  endmethod

  method to_mcp_schema owner: HACKER
    "Convert tool definition to MCP (Model Context Protocol) format for external agents";
    const endpoint = isa(this.target_obj, $llm_tool_provider) ? "mcp_" + this.target_verb | this.target_verb;
    return ["name" -> this.name, "description" -> this.description, "input_schema" -> this.parameters, "target_obj" -> this.target_obj, "target_verb" -> endpoint];
  endmethod

  method execute owner: ARCH_WIZARD
    "Dispatch a tool as its authenticated caller. Explicit actor delegation requires a wizard caller.";
    const principal = caller_perms();
    isa(caller, $llm_agent) || raise(E_PERM);
    const {args_json, ?actor = caller_perms()} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM, "Tool actor must be valid");
    principal == actor || principal.wizard || raise(E_PERM, "Tool caller cannot impersonate actor");
    set_task_perms(actor);
    const tool_args = $llm_tool_schema:arguments(args_json, this.parameters);
    const prefixed_verb = "_tool_" + this.target_verb;
    if (respond_to(this.target_obj, prefixed_verb))
      return this.target_obj:(prefixed_verb)(tool_args, actor);
    elseif (respond_to(this.target_obj, this.target_verb))
      return this.target_obj:(this.target_verb)(tool_args, actor);
    endif
    raise(E_VERBNF, "Tool handler not found: " + prefixed_verb + " or " + this.target_verb);
  endmethod
endobject
