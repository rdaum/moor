object LLM_TOOL_RESULT [
  import_export_id -> "llm_tool_result",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Tool Result"
  parent: ROOT
  owner: ARCH_WIZARD

  method mk owner: ARCH_WIZARD
    "Create a bounded immutable JSON outcome; status controls execution, never the wording of its text.";
    const {status, ?value = "", ?code = "", ?wire = false} = args;
    status in {'success, 'failure, 'cancelled, 'waiting, 'unknown} || raise(E_INVARG);
    typeof(code) == TYPE_STR || raise(E_TYPE);
    typeof(wire) == TYPE_MAP || (typeof(wire) == TYPE_BOOL && !wire)
      || raise(E_TYPE, "Tool wire data must be an object or false.");
    value_bytes({value, code, wire}) <= 65536 || raise(E_QUOTA, "Tool outcome exceeds 64 KiB.");
    $llm_value:validate_json(value);
    $llm_value:validate_json(wire);
    const result = <$llm_tool_result, .status = status, .value = value, .code = code, .wire = wire>;
    value_bytes(result) <= 65536 || raise(E_QUOTA, "Tool outcome exceeds 64 KiB.");
    value_bytes(result:content()) <= 65536 || raise(E_QUOTA, "Encoded tool outcome exceeds 64 KiB.");
    return result;
  endmethod

  method normalize owner: ARCH_WIZARD
    "Adapt explicit service envelopes and ordinary return values without guessing from string prefixes.";
    const {value} = args;
    if (typeof(value) == TYPE_FLYWEIGHT && value.delegate == $llm_tool_result)
      return $llm_tool_result:mk(value.status, value.value, value.code, value.wire);
    endif
    if (typeof(value) == TYPE_ERR)
      return $llm_tool_result:mk('failure, tostr(value), tostr(value));
    endif
    if (typeof(value) == TYPE_MAP && maphaskey(value, "ok"))
      typeof(value["ok"]) == TYPE_BOOL || (typeof(value["ok"]) == TYPE_INT && value["ok"] in {0, 1}) || raise(E_TYPE, "Tool ok must be boolean");
      const status = value["ok"] ? 'success | 'failure;
      return $llm_tool_result:mk(status, value, status == 'failure ? "tool_error" | "", value);
    endif
    return $llm_tool_result:mk('success, value);
  endmethod

  method is_success owner: ARCH_WIZARD
    "Return whether the operation completed successfully.";
    return this.status == 'success;
  endmethod

  method content owner: ARCH_WIZARD
    "Render the provider-facing result while preserving existing structured envelopes.";
    typeof(this.wire) == TYPE_MAP && return generate_json(this.wire);
    if (this.status == 'success && typeof(this.value) == TYPE_STR)
      return this.value;
    endif
    return generate_json(["ok" -> this.status == 'success, "status" -> tostr(this.status),
      "result" -> this.value, "code" -> this.code]);
  endmethod

  method message owner: ARCH_WIZARD
    "Build a tool reply with its provider call identity.";
    const {id, name} = args;
    return ["role" -> "tool", "tool_call_id" -> id, "name" -> name, "content" -> this:content()];
  endmethod
endobject
