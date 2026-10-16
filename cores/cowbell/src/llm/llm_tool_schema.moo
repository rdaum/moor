object LLM_TOOL_SCHEMA [
  import_export_id -> "llm_tool_schema",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Tool Schema"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method arguments owner: ARCH_WIZARD
    "Decode bounded arguments and check the published schema before a tool can have effects.";
    const {parameters, schema} = args;
    set_task_perms(caller_perms());
    value_bytes(parameters) <= 5242880 || raise(E_QUOTA, "Tool arguments exceed 5 MiB.");
    const value = typeof(parameters) == TYPE_STR ? parse_json(parameters) | parameters;
    typeof(value) == TYPE_MAP || raise(E_TYPE, "Tool arguments must be an object.");
    $llm_tool_schema:validate(value, schema);
    return value;
  endmethod

  method validate owner: ARCH_WIZARD
    "Validate the tool schema subset without coercing values or invoking supplied behavior.";
    const {value, schema, ?path = "arguments", ?depth = 0} = args;
    set_task_perms(caller_perms());
    depth <= 16 || raise(E_QUOTA, "Tool arguments are nested too deeply.");
    typeof(schema) == TYPE_MAP || raise(E_TYPE, "Invalid tool schema.");
    const kind = typeof(value);
    const type_name = kind == TYPE_MAP ? "object" | kind == TYPE_LIST ? "array" | kind == TYPE_STR ? "string" | kind == TYPE_INT ? "integer" | kind == TYPE_BOOL ? "boolean" | kind == TYPE_FLOAT ? "number" | kind == TYPE_OBJ && value == #-1 ? "null" | "unsupported";
    if (maphaskey(schema, "type"))
      const types = typeof(schema["type"]) == TYPE_LIST ? schema["type"] | {schema["type"]};
      (type_name in types) || (kind == TYPE_INT && "number" in types) || raise(E_INVARG, path + " has the wrong type.");
    endif
    if (maphaskey(schema, "enum"))
      let matched = false;
      for option in (schema["enum"])
        if (strcmp(toliteral(value), toliteral(option)) == 0)
          matched = true;
        endif
      endfor
      matched || raise(E_INVARG, path + " has an unsupported value.");
    endif
    if (kind == TYPE_STR)
      length(value) <= `schema["maxLength"] ! E_RANGE => 4194304' || raise(E_QUOTA, path + " is too long.");
      length(value) >= `schema["minLength"] ! E_RANGE => 0' || raise(E_INVARG, path + " is too short.");
    elseif (kind in {TYPE_INT, TYPE_FLOAT})
      for key in ({"minimum", "maximum"})
        if (maphaskey(schema, key))
          const bound = schema[key];
          const floating = kind == TYPE_FLOAT || typeof(bound) == TYPE_FLOAT;
          const left = floating ? tofloat(value) | value;
          const right = floating ? tofloat(bound) | bound;
          (key == "minimum" ? left >= right | left <= right) || raise(E_INVARG, path + " is out of range.");
        endif
      endfor
    elseif (kind == TYPE_LIST)
      length(value) <= `schema["maxItems"] ! E_RANGE => 4096' || raise(E_QUOTA, path + " has too many entries.");
      length(value) >= `schema["minItems"] ! E_RANGE => 0' || raise(E_INVARG, path + " has too few entries.");
      const item_schema = `schema["items"] ! E_RANGE => []';
      for item in (value)
        $llm_tool_schema:validate(item, item_schema, path + "[]", depth + 1);
      endfor
    elseif (kind == TYPE_MAP)
      length(value) <= 4096 || raise(E_QUOTA, path + " has too many fields.");
      const properties = `schema["properties"] ! E_RANGE => []';
      for required in (`schema["required"] ! E_RANGE => {}')
        $llm_tool_schema:has_key(value, required) || raise(E_INVARG, path + "." + required + " is required.");
      endfor
      for key in (mapkeys(value))
        typeof(key) == TYPE_STR || raise(E_INVARG, "Argument keys must be strings.");
        if ($llm_tool_schema:has_key(properties, key))
          $llm_tool_schema:validate(value[key], properties[key], path + "." + key, depth + 1);
        else
          const extra = `schema["additionalProperties"] ! E_RANGE => true';
          extra || raise(E_INVARG, path + "." + key + " is not a supported argument.");
          $llm_tool_schema:validate(value[key], typeof(extra) == TYPE_MAP ? extra | [], path + "." + key, depth + 1);
        endif
      endfor
    endif
    return true;
  endmethod

  method check owner: ARCH_WIZARD
    "Reject unsupported constraints when a tool is registered, before advertising its schema.";
    const {schema, ?depth = 0} = args;
    set_task_perms(caller_perms());
    typeof(schema) == TYPE_MAP || raise(E_TYPE, "Invalid tool schema.");
    depth <= 16 || raise(E_QUOTA, "Tool schema is nested too deeply.");
    for key in (mapkeys(schema))
      key in {"type", "properties", "required", "additionalProperties", "items", "enum", "description",
        "title", "default", "minLength", "maxLength", "minItems", "maxItems", "minimum", "maximum"}
        || raise(E_INVARG, "Unsupported tool schema constraint: " + tostr(key));
    endfor
    if (maphaskey(schema, "type"))
      const types = typeof(schema["type"]) == TYPE_LIST ? schema["type"] | {schema["type"]};
      length(types) > 0 || raise(E_INVARG, "Empty schema type list.");
      for kind in (types)
        kind in {"object", "array", "string", "integer", "number", "boolean", "null"}
          || raise(E_INVARG, "Unsupported schema type.");
      endfor
    endif
    const properties = `schema["properties"] ! E_RANGE => []';
    typeof(properties) == TYPE_MAP || raise(E_TYPE, "Schema properties must be an object.");
    for key in (mapkeys(properties))
      typeof(key) == TYPE_STR || raise(E_TYPE, "Schema field names must be strings.");
      $llm_tool_schema:check(properties[key], depth + 1);
    endfor
    const required = `schema["required"] ! E_RANGE => {}';
    typeof(required) == TYPE_LIST || raise(E_TYPE, "Schema required must be a list.");
    for key in (required)
      typeof(key) == TYPE_STR && $llm_tool_schema:has_key(properties, key)
        || raise(E_INVARG, "Required field is absent from the schema.");
    endfor
    for key in ({"items", "additionalProperties"})
      if (maphaskey(schema, key))
        const nested = schema[key];
        if (key == "additionalProperties" && typeof(nested) == TYPE_BOOL)
          continue;
        endif
        $llm_tool_schema:check(nested, depth + 1);
      endif
    endfor
    if (maphaskey(schema, "enum"))
      typeof(schema["enum"]) == TYPE_LIST && length(schema["enum"]) > 0 || raise(E_INVARG, "Invalid schema enum.");
    endif
    for key in ({"minLength", "maxLength", "minItems", "maxItems"})
      if (maphaskey(schema, key))
        typeof(schema[key]) == TYPE_INT && schema[key] >= 0 || raise(E_INVARG, "Invalid schema bound.");
      endif
    endfor
    for key in ({"minimum", "maximum"})
      if (maphaskey(schema, key))
        typeof(schema[key]) in {TYPE_INT, TYPE_FLOAT} || raise(E_INVARG, "Invalid numeric schema bound.");
      endif
    endfor
    return true;
  endmethod

  method has_key owner: ARCH_WIZARD
    "Match JSON field names exactly; MOO's ordinary map lookup ignores letter case.";
    const {value, name} = args;
    set_task_perms(caller_perms());
    for key in (mapkeys(value))
      if (typeof(key) == TYPE_STR && strcmp(key, name) == 0)
        return true;
      endif
    endfor
    return false;
  endmethod
endobject
