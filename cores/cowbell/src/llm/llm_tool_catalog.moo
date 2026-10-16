object LLM_TOOL_CATALOG [
  import_export_id -> "llm_tool_catalog",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Tool Catalog"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method normalize owner: ARCH_WIZARD
    "Copy descriptor data into a canonical immutable value without calling supplied behavior.";
    const {definition} = args;
    set_task_perms(caller_perms());
    let tool = false;
    if (typeof(definition) == TYPE_MAP)
      tool = $llm_agent_tool:mk(definition["name"], definition["description"],
        definition["input_schema"], definition["target_obj"], definition["target_verb"],
        `definition["effect"] ! E_RANGE => 'unknown');
    elseif (typeof(definition) == TYPE_FLYWEIGHT)
      tool = $llm_agent_tool:mk(definition.name, definition.description, definition.parameters,
        definition.target_obj, definition.target_verb, `definition.effect ! E_PROPNF => 'unknown');
    else
      raise(E_TYPE, "Tool definition must be a descriptor or map");
    endif
    length(tool.name) > 0 && length(tool.name) <= 64 || raise(E_INVARG, "Invalid tool name");
    for position in [1..length(tool.name)]
      const character = tool.name[position];
      index("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-", character) > 0 || raise(E_INVARG, "Invalid tool name");
    endfor
    valid(tool.target_obj) && length(tool.target_verb) > 0 || raise(E_INVARG, "Invalid tool target");
    tool.parameters["type"] == "object" || raise(E_INVARG, "Tool input must be an object schema");
    $llm_tool_schema:check(tool.parameters);
    return tool;
  endmethod

  method compose owner: ARCH_WIZARD
    "Combine catalogs. Exact repeats are idempotent; conflicting names fail before registration.";
    const {@catalogs} = args;
    set_task_perms(caller_perms());
    let by_name = [];
    let result = {};
    for catalog in (catalogs)
      typeof(catalog) == TYPE_LIST || raise(E_TYPE);
      for definition in (catalog)
        const tool = $llm_tool_catalog:normalize(definition);
        if (maphaskey(by_name, tool.name))
          strcmp(toliteral(by_name[tool.name]), toliteral(tool)) == 0 || raise(E_INVARG, "Conflicting tool: " + tool.name);
        else
          by_name[tool.name] = tool;
          result = {@result, tool};
        endif
      endfor
    endfor
    return result;
  endmethod

endobject
