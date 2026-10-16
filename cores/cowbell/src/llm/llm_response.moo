object LLM_RESPONSE [
  import_export_id -> "llm_response",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Response"
  parent: ROOT
  owner: ARCH_WIZARD

  method mk owner: ARCH_WIZARD
    "Wrap provider data without rewriting content or interpreting reasoning as an answer.";
    const {raw} = args;
    return <$llm_response, .raw = raw>;
  endmethod

  method is_valid owner: ARCH_WIZARD
    "Validate the first assistant message and the complete tool-call envelope.";
    const raw = this.raw;
    typeof(raw) == TYPE_MAP && maphaskey(raw, "choices") || return false;
    const choices = raw["choices"];
    typeof(choices) == TYPE_LIST && length(choices) > 0 || return false;
    const first = choices[1];
    typeof(first) == TYPE_MAP && maphaskey(first, "message") || return false;
    const message = first["message"];
    typeof(message) == TYPE_MAP || return false;
    if (maphaskey(message, "role") && message["role"] != "assistant")
      return false;
    endif
    const calls = maphaskey(message, "tool_calls") ? message["tool_calls"] | {};
    typeof(calls) == TYPE_LIST || return false;
    let seen = {};
    for call in (calls)
      typeof(call) == TYPE_MAP && maphaskey(call, "id") && maphaskey(call, "function") || return false;
      const id = call["id"];
      typeof(id) == TYPE_STR && length(id) > 0 && !(id in seen) || return false;
      seen = {@seen, id};
      const function = call["function"];
      typeof(function) == TYPE_MAP && maphaskey(function, "name") && maphaskey(function, "arguments") || return false;
      typeof(function["name"]) == TYPE_STR && length(function["name"]) > 0 || return false;
      typeof(function["arguments"]) in {TYPE_STR, TYPE_MAP} || return false;
      if (maphaskey(call, "type") && call["type"] != "function")
        return false;
      endif
    endfor
    length(calls) > 0 && return true;
    return maphaskey(message, "content") && typeof(message["content"]) == TYPE_STR;
  endmethod

  method message owner: ARCH_WIZARD
    "Return a reusable assistant envelope, retaining literal text and provider reasoning metadata.";
    this:is_valid() || return [];
    const source = this.raw["choices"][1]["message"];
    let message = ["role" -> "assistant"];
    if (maphaskey(source, "content") && typeof(source["content"]) == TYPE_STR)
      message["content"] = source["content"];
    endif
    for key in ({"reasoning", "reasoning_content", "reasoning_details"})
      if (maphaskey(source, key))
        message[key] = source[key];
      endif
    endfor
    if (maphaskey(source, "tool_calls") && length(source["tool_calls"]) > 0)
      let calls = {};
      for call in (source["tool_calls"])
        const function = call["function"];
        const parameters = function["arguments"];
        calls = {@calls, ["id" -> call["id"], "type" -> "function", "function" -> [
          "name" -> function["name"], "arguments" -> typeof(parameters) == TYPE_STR ? parameters | generate_json(parameters)]]};
      endfor
      message["tool_calls"] = calls;
    endif
    return message;
  endmethod

  method content owner: ARCH_WIZARD
    "Return only visible content. Reasoning and the literal word null are separate values.";
    this:is_valid() || return "";
    const message = this.raw["choices"][1]["message"];
    return maphaskey(message, "content") && typeof(message["content"]) == TYPE_STR ? message["content"] | "";
  endmethod

  method tool_calls owner: ARCH_WIZARD
    "Return validated tool calls or an empty list.";
    const message = this:message();
    return maphaskey(message, "tool_calls") ? message["tool_calls"] | {};
  endmethod

  method has_tool_calls owner: ARCH_WIZARD
    "Return whether the validated response requests tools.";
    return length(this:tool_calls()) > 0;
  endmethod

  method usage owner: ARCH_WIZARD
    "Return the provider usage map, if present.";
    const raw = this.raw;
    if (typeof(raw) == TYPE_MAP && maphaskey(raw, "usage") && typeof(raw["usage"]) == TYPE_MAP)
      return raw["usage"];
    endif
    return [];
  endmethod

  method reasoning owner: ARCH_WIZARD
    "Return reasoning metadata without exposing it as a final answer.";
    const message = this:message();
    return maphaskey(message, "reasoning_content") && typeof(message["reasoning_content"]) == TYPE_STR ? message["reasoning_content"] | "";
  endmethod

  method test_flyweight owner: ARCH_WIZARD
    "Provider normalization preserves literal content and rejects incomplete tool envelopes.";
    for text in ({"Hello!", "null", "null pointer", "NULL prefix"})
      const response = this:mk(["choices" -> {["message" -> ["content" -> text]]}]);
      response:is_valid() || raise(E_ASSERT);
      strcmp(response:content(), text) == 0 || raise(E_ASSERT);
      strcmp(response:message()["content"], text) == 0 || raise(E_ASSERT);
    endfor
    const reasoning = this:mk(["choices" -> {["message" -> ["reasoning_content" -> "private reasoning"]]}]);
    reasoning:content() == "" || raise(E_ASSERT, "Reasoning is not an answer");
    const invalid = this:mk(["choices" -> {["message" -> ["tool_calls" -> {["id" -> "incomplete"]}]]}]);
    !invalid:is_valid() || raise(E_ASSERT, "Incomplete tools are invalid");
    return true;
  endmethod
endobject
