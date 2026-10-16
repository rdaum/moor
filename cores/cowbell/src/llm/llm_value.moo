object LLM_VALUE [
  import_export_id -> "llm_value",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Value"
  parent: ROOT
  owner: ARCH_WIZARD

  method validate_json owner: ARCH_WIZARD
    "Allow only bounded JSON values; values cannot retain live objects or callbacks.";
    const {value, ?depth = 0} = args;
    depth <= 16 || raise(E_QUOTA, "LLM values are nested too deeply");
    const kind = typeof(value);
    if (kind == TYPE_MAP)
      for key in (mapkeys(value))
        typeof(key) == TYPE_STR || raise(E_TYPE, "LLM value keys must be strings");
        $llm_value:validate_json(value[key], depth + 1);
      endfor
    elseif (kind == TYPE_LIST)
      for item in (value)
        $llm_value:validate_json(item, depth + 1);
      endfor
    else
      kind in {TYPE_STR, TYPE_INT, TYPE_FLOAT, TYPE_BOOL} || (kind == TYPE_OBJ && value == #-1)
        || raise(E_TYPE, "LLM values must be JSON data");
    endif
  endmethod

endobject
