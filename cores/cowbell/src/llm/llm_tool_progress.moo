object LLM_TOOL_PROGRESS [
  import_export_id -> "llm_tool_progress",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Tool Progress"
  parent: ROOT
  owner: ARCH_WIZARD

  method _text owner: ARCH_WIZARD
    "Capture a short scalar field without invoking supplied behavior or retaining object references.";
    const {values, key, ?limit = 96} = args;
    !maphaskey(values, key) && return "";
    const value = values[key];
    typeof(value) in {TYPE_STR, TYPE_INT, TYPE_FLOAT, TYPE_BOOL, TYPE_OBJ}
      || raise(E_TYPE, "Progress fields must be scalar values.");
    const text = tostr(value);
    return length(text) > limit ? text[1..limit] + "..." | text;
  endmethod

  method mk owner: ARCH_WIZARD
    "Capture bounded literal progress from current tool arguments; rendering performs no world lookup.";
    let {label, name, values} = args;
    typeof(label) == TYPE_STR && length(label) <= 32 || raise(E_INVARG);
    typeof(name) == TYPE_STR && length(name) > 0 && length(name) <= 128 || raise(E_INVARG);
    value_bytes(values) <= 5242880 || raise(E_QUOTA, "Progress arguments exceed 5 MiB.");
    typeof(values) == TYPE_STR && (values = parse_json(values));
    typeof(values) == TYPE_MAP || raise(E_TYPE);
    const action = strsub(name[1]:uppercase() + name[2..$], "_", " ");
    const kind = name == "ask_user" ? 'question | name == "explain" ? 'explanation | 'operation;
    let detail = "";
    let input = "";
    if (kind == 'question)
      detail = $llm_tool_progress:_text(values, "question", 60);
      if (maphaskey(values, "choices") && typeof(values["choices"]) == TYPE_LIST && length(values["choices"]) > 0)
        input = "options";
      else
        input = $llm_tool_progress:_text(values, "input_type", 32);
      endif
    elseif (kind == 'explanation)
      detail = $llm_tool_progress:_text(values, "message", 256);
    else
      for key in ({"object", "target", "to_room", "target_room", "source_room"})
        detail = $llm_tool_progress:_text(values, key);
        if (detail != "")
          break;
        endif
      endfor
      const verb_name = $llm_tool_progress:_text(values, "verb");
      const property_name = $llm_tool_progress:_text(values, "property");
      verb_name != "" && (detail = detail + ":" + verb_name);
      property_name != "" && (detail = detail + "." + property_name);
      let parts = detail == "" ? {} | {detail};
      for field in ({"name", "direction", "function_name", "subject", "key", "description"})
        const text = $llm_tool_progress:_text(values, field);
        text != "" && (parts = {@parts, text});
      endfor
      const destination = $llm_tool_progress:_text(values, "destination");
      destination != "" && (parts = {@parts, "to " + destination});
      const parent_spec = $llm_tool_progress:_text(values, "parent");
      parent_spec != "" && (parts = {@parts, "from " + parent_spec});
      const pattern = $llm_tool_progress:_text(values, "pattern", 128);
      pattern != "" && (parts = {@parts, "literal " + toliteral(pattern)});
      const first = $llm_tool_progress:_text(values, "start_line", 20);
      const last = $llm_tool_progress:_text(values, "end_line", 20);
      if (first != "" && last != "")
        parts = {@parts, "lines " + first + "-" + last};
      elseif (first != "")
        parts = {@parts, "from line " + first};
      elseif (last != "")
        parts = {@parts, "through line " + last};
      endif
      detail = parts:join("; ");
    endif
    let reason = $llm_tool_progress:_text(values, "reason", 128);
    reason == "" && (reason = $llm_tool_progress:_text(values, "rationale", 128));
    const progress = <$llm_tool_progress, .label = label, .action = action, .kind = kind,
      .detail = detail, .input = input, .reason = reason>;
    value_bytes(progress) <= 8192 || raise(E_QUOTA, "Captured tool progress exceeds 8 KiB.");
    return progress;
  endmethod

  method hud owner: ARCH_WIZARD
    "Render the captured progress as literal HUD text, with a separate question marker.";
    if (this.kind == 'question)
      return "[QUESTION] Asking: " + this.detail + (this.input == "" ? "" | " [" + this.input + "]");
    elseif (this.kind == 'explanation)
      return "[" + this.label + "] " + this.detail;
    endif
    return "[" + this.label + "] " + this.action + (this.detail == "" ? "" | ": " + this.detail)
      + (this.reason == "" ? "" | " (" + this.reason + ")");
  endmethod

  method speech owner: ARCH_WIZARD
    "Render the same captured progress without HUD brackets for speech clients.";
    if (this.kind == 'question)
      return "Question: " + this.detail + (this.input == "" ? ""
        | this.input == "options" ? " with options" | ", " + this.input + " input");
    elseif (this.kind == 'explanation)
      return this.label + ": " + this.detail;
    endif
    return this.label + ": " + this.action + (this.detail == "" ? "" | ": " + this.detail)
      + (this.reason == "" ? "" | ". " + this.reason);
  endmethod
endobject
