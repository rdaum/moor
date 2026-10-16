object LLM_MEMORY [
  import_export_id -> "llm_memory",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Memory"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD

  property subject (owner: ARCH_WIZARD, flags: "") = #-1;
  property facts (owner: ARCH_WIZARD, flags: "") = #-1;

  method new owner: ARCH_WIZARD
    "Create anonymous bounded memory for an object under its current owner's authority.";
    const principal = caller_perms();
    const {subject} = args;
    typeof(subject) == TYPE_OBJ && valid(subject) || raise(E_TYPE);
    principal == subject || principal == subject.owner || principal.wizard || raise(E_PERM);
    const memory = create($llm_memory, $arch_wizard, 1);
    memory.subject = subject;
    memory.facts = create($relation, $arch_wizard, 1);
    "Object permission flags require integer values.";
    memory.facts.r = 0;
    return memory;
  endmethod

  method check owner: ARCH_WIZARD
    "Authorize the actual principal against the bound object and its current owner.";
    const {memory, principal} = args;
    typeof(memory) == TYPE_OBJ && valid(memory) && isa(memory, $llm_memory) || raise(E_TYPE);
    !is_clear_property(memory, "subject") && valid(memory.subject) || raise(E_PERM);
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    principal == memory.subject || principal == memory.subject.owner || principal.wizard || raise(E_PERM);
    return true;
  endmethod

  method snapshot owner: ARCH_WIZARD
    "Return immutable fact tuples: subject, text, and the oldest supporting timestamp.";
    $llm_memory:check(this, caller_perms());
    return $relation:tuples(this.facts);
  endmethod

  method _validate_text owner: ARCH_WIZARD
    "Validate bounded subject and fact text without rewriting their contents.";
    const {subject, fact} = args;
    typeof(subject) == TYPE_STR && typeof(fact) == TYPE_STR || raise(E_TYPE);
    length(subject:trim()) >= 2 && length(subject) <= 128 || raise(E_INVARG, "Subject must contain 2 to 128 characters");
    length(fact:trim()) >= 3 && length(fact) <= 1024 || raise(E_INVARG, "Fact must contain 3 to 1024 characters");
    strcmp(subject, fact) != 0 || raise(E_INVARG, "Subject and fact must differ");
  endmethod

  method remember owner: ARCH_WIZARD
    "Store a fact once; duplicates keep their original age. Reject overflow without discarding facts.";
    $llm_memory:check(this, caller_perms());
    const {subject, fact} = args;
    $llm_memory:_validate_text(subject, fact);
    const snapshot = $relation:tuples(this.facts);
    for tuple in (snapshot)
      if (strcmp(tuple[1], subject) == 0 && strcmp(tuple[2], fact) == 0)
        return false;
      endif
    endfor
    const added = {subject, fact, time()};
    length(snapshot) < 256 && value_bytes({@snapshot, added}) <= 65536
      || raise(E_QUOTA, "Memory is full; compact or forget facts before adding more");
    $relation:assert(added, this.facts);
    return true;
  endmethod

  method forget owner: ARCH_WIZARD
    "Remove one exact subject and fact pair under the current owner's authority.";
    $llm_memory:check(this, caller_perms());
    const {subject, fact} = args;
    $llm_memory:_validate_text(subject, fact);
    const snapshot = $relation:tuples(this.facts);
    const retained = {tuple for tuple in (snapshot)
      if (strcmp(tuple[1], subject) != 0 || strcmp(tuple[2], fact) != 0)};
    length(retained) == length(snapshot) && return false;
    return $relation:replace_snapshot(this.facts, snapshot, retained);
  endmethod

  method recall owner: ARCH_WIZARD
    "Select facts by exact subject, including their recorded ages.";
    $llm_memory:check(this, caller_perms());
    const {subject} = args;
    typeof(subject) == TYPE_STR || raise(E_TYPE);
    return {tuple for tuple in ($relation:tuples(this.facts)) if (strcmp(tuple[1], subject) == 0)};
  endmethod

  method summary owner: ARCH_WIZARD
    "Format bounded stored facts for conversation context.";
    $llm_memory:check(this, caller_perms());
    const tuples = $relation:tuples(this.facts);
    length(tuples) == 0 && return "";
    return "REMEMBERED FACTS:\n" + {tuple[1] + ": " + tuple[2] for tuple in (tuples)}:join("\n");
  endmethod

  method age owner: ARCH_WIZARD
    "Format an integer timestamp as elapsed seconds, minutes, hours, or days.";
    const {timestamp} = args;
    typeof(timestamp) == TYPE_INT || raise(E_TYPE);
    const elapsed = max(0, time() - timestamp);
    for scale in ({{86400, "day"}, {3600, "hour"}, {60, "minute"}, {1, "second"}})
      if (elapsed >= scale[1] || scale[1] == 1)
        const count = elapsed / scale[1];
        return tostr(count) + " " + scale[2] + (count == 1 ? " ago" | "s ago");
      endif
    endfor
  endmethod

  method apply_compaction owner: ARCH_WIZARD
    "Validate every proposed fact and source before exact snapshot replacement; preserve source ages.";
    $llm_memory:check(this, caller_perms());
    const {snapshot, proposed} = args;
    typeof(snapshot) == TYPE_LIST && typeof(proposed) == TYPE_LIST || raise(E_TYPE);
    length(proposed) > 0 && length(proposed) <= 256 || raise(E_INVARG, "Expected 1 to 256 facts");
    let replacement = {};
    let covered = {};
    for fact in (proposed)
      typeof(fact) == TYPE_MAP || raise(E_TYPE, "Each fact must be an object");
      maphaskey(fact, "subject") && maphaskey(fact, "fact") && maphaskey(fact, "sources")
        || raise(E_INVARG, "Each fact needs subject, fact, and sources");
      $llm_memory:_validate_text(fact["subject"], fact["fact"]);
      const sources = fact["sources"];
      typeof(sources) == TYPE_LIST && length(sources) > 0 || raise(E_TYPE, "Missing fact sources");
      let timestamp = false;
      for index in (sources)
        typeof(index) == TYPE_INT && index >= 1 && index <= length(snapshot)
          || raise(E_INVARG, "Invalid fact source index");
        const source = snapshot[index];
        typeof(source) == TYPE_LIST && length(source) == 3 && typeof(source[3]) == TYPE_INT
          || raise(E_TYPE, "Invalid source timestamp");
        timestamp = typeof(timestamp) == TYPE_INT ? min(timestamp, source[3]) | source[3];
        covered = setadd(covered, index);
      endfor
      replacement = {@replacement, {fact["subject"], fact["fact"], timestamp}};
    endfor
    length(covered) == length(snapshot) || raise(E_INVARG, "Compaction omitted source facts");
    value_bytes(replacement) <= 65536 || raise(E_QUOTA, "Compacted memory exceeds 64 KiB");
    return $relation:replace_snapshot(this.facts, snapshot, replacement);
  endmethod

  method compact owner: ARCH_WIZARD
    "Ask for a bounded proposal, then recheck authority and the fact snapshot after model I/O.";
    const principal = caller_perms();
    $llm_memory:check(this, principal);
    const {client} = args;
    const snapshot = $relation:tuples(this.facts);
    length(snapshot) >= 2 || raise(E_INVARG, "Need at least two facts");
    const prompt = "Consolidate these facts without inventing information. Keep conflicting claims explicit. "
      + "Return only a JSON array of objects with subject, fact, and sources fields. "
      + "Use one-based source indices. Include every source in at least one result. "
      + "Subjects have at most 128 characters; facts have at most 1024. "
      + "Tuples contain subject, fact, and original timestamp: " + toliteral(snapshot);
    const opts = $llm_chat_opts:mk():with_temperature(0.2);
    const response = $llm_response:mk($llm_provider:chat_as(client, principal, 'facts,
      {["role" -> "user", "content" -> prompt]}, opts, false, false, false));
    response:is_valid() || raise(E_INVARG, "Invalid compaction response");
    const proposed = parse_json(response:content());
    set_task_perms(principal);
    return this:apply_compaction(snapshot, proposed);
  endmethod

  method get_tools owner: ARCH_WIZARD
    "Return immutable memory descriptors bound to this memory instance.";
    $llm_memory:check(this, caller_perms());
    const subject = ["type" -> "string", "minLength" -> 2, "maxLength" -> 128];
    const fact = ["type" -> "string", "minLength" -> 3, "maxLength" -> 1024];
    const pair = ["type" -> "object", "properties" -> ["subject" -> subject, "fact" -> fact],
      "required" -> {"subject", "fact"}, "additionalProperties" -> false];
    return {
      $llm_agent_tool:mk("remember_fact", "Remember a fact about a person, place, or topic.", pair, this, "remember_fact", 'mutation),
      $llm_agent_tool:mk("forget_fact", "Forget one exact fact, for example before correcting it.", pair, this, "forget_fact", 'mutation),
      $llm_agent_tool:mk("recall_facts", "Recall facts about an exact subject with their recorded ages.",
        ["type" -> "object", "properties" -> ["subject" -> subject], "required" -> {"subject"},
          "additionalProperties" -> false], this, "recall_facts", 'inspection),
      $llm_agent_tool:mk("current_time", "Get the current date and time.",
        ["type" -> "object", "properties" -> [], "required" -> {}, "additionalProperties" -> false],
        this, "current_time", 'inspection)
    };
  endmethod

  method remember_fact owner: ARCH_WIZARD
    "Store a validated tool fact under the captured actor's authority.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    this:remember(parameters["subject"], parameters["fact"]);
    return "Successfully remembered about " + parameters["subject"] + ": " + parameters["fact"];
  endmethod

  method forget_fact owner: ARCH_WIZARD
    "Remove an exact fact under the captured actor's authority.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    return this:forget(parameters["subject"], parameters["fact"]) ? "Fact forgotten." | "Fact not found.";
  endmethod

  method recall_facts owner: ARCH_WIZARD
    "Format matching facts and ages for a tool response.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const subject = parameters["subject"];
    const tuples = this:recall(subject);
    length(tuples) == 0 && return "No facts remembered about " + subject + ".";
    return "Facts about " + subject + ":\n" + {" - " + tuple[2] + " (" + $llm_memory:age(tuple[3]) + ")"
      for tuple in (tuples)}:join("\n");
  endmethod

  method current_time owner: ARCH_WIZARD
    "Return current time under the memory tool authority contract.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_memory:check(this, actor);
    return ["current_time" -> ctime(), "timestamp" -> time()];
  endmethod
endobject
