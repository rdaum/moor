object LLM_CHAT_OPTS [
  import_export_id -> "llm_chat_opts",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Chat Options"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for LLM chat request options. Builder pattern: $llm_chat_opts:mk():with_temperature(0.3):with_tool_choice('required)";

  method mk owner: HACKER
    "Create an empty chat options flyweight";
    return <this>;
  endmethod

  method with_temperature owner: HACKER
    "Set sampling temperature (0-2, lower = more deterministic)";
    const {temp} = args;
    typeof(temp) in {TYPE_INT, TYPE_FLOAT} || raise(E_TYPE, "temperature must be a number");
    let slots = flyslots(this);
    slots['temperature] = temp;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_max_tokens owner: HACKER
    "Set maximum output tokens";
    const {max} = args;
    typeof(max) == TYPE_INT || raise(E_TYPE, "max_tokens must be an integer");
    let slots = flyslots(this);
    slots['max_tokens] = max;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_tool_choice owner: HACKER
    "Set tool choice: 'auto, 'none, 'required, or [\"type\" -> \"function\", \"function\" -> [\"name\" -> \"fn\"]].";
    const {choice} = args;
    let slots = flyslots(this);
    slots['tool_choice] = choice;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_top_p owner: HACKER
    "Set nucleus sampling threshold (0-1)";
    const {p} = args;
    typeof(p) in {TYPE_INT, TYPE_FLOAT} || raise(E_TYPE, "top_p must be a number");
    let slots = flyslots(this);
    slots['top_p] = p;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_frequency_penalty owner: HACKER
    "Set frequency penalty (-2 to 2, penalizes repeated tokens)";
    const {penalty} = args;
    typeof(penalty) in {TYPE_INT, TYPE_FLOAT} || raise(E_TYPE, "frequency_penalty must be a number");
    let slots = flyslots(this);
    slots['frequency_penalty] = penalty;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_presence_penalty owner: HACKER
    "Set presence penalty (-2 to 2, encourages new topics)";
    const {penalty} = args;
    typeof(penalty) in {TYPE_INT, TYPE_FLOAT} || raise(E_TYPE, "presence_penalty must be a number");
    let slots = flyslots(this);
    slots['presence_penalty] = penalty;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_stop owner: HACKER
    "Set stop sequences (string or list of strings)";
    const {stop} = args;
    let slots = flyslots(this);
    slots['stop] = stop;
    return toflyweight(this.delegate, slots);
  endmethod

  method with_json_mode owner: HACKER
    "Enable JSON output mode";
    let slots = flyslots(this);
    slots['response_format] = ["type" -> "json_object"];
    return toflyweight(this.delegate, slots);
  endmethod

endobject
