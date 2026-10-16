object LLM_CONVERSATION [
  import_export_id -> "llm_conversation",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Conversation"
  parent: ROOT
  owner: ARCH_WIZARD

  property subject (owner: ARCH_WIZARD, flags: "") = #-1;
  property messages (owner: ARCH_WIZARD, flags: "") = {};
  property revision (owner: ARCH_WIZARD, flags: "") = 0;

  method new owner: ARCH_WIZARD
    "Create independent anonymous transcript state bound to an owned object.";
    const principal = caller_perms();
    const {subject} = args;
    typeof(subject) == TYPE_OBJ && valid(subject) || raise(E_TYPE);
    principal == subject.owner || principal.wizard || raise(E_PERM);
    const conversation = create($llm_conversation, $arch_wizard, 1);
    conversation.subject = subject;
    conversation.messages = {};
    conversation.revision = 0;
    return conversation;
  endmethod

  method check owner: ARCH_WIZARD
    "Check the captured principal against the conversation's current owner.";
    const {conversation, principal} = args;
    typeof(conversation) == TYPE_OBJ && valid(conversation) && isa(conversation, $llm_conversation) || raise(E_TYPE);
    !is_clear_property(conversation, "subject") && valid(conversation.subject) || raise(E_PERM);
    typeof(principal) == TYPE_OBJ && valid(principal) || raise(E_PERM);
    principal == conversation.subject.owner || principal.wizard || raise(E_PERM);
  endmethod

  method validate owner: ARCH_WIZARD
    "Require bounded messages and complete tool exchanges before storing a transcript.";
    const {messages} = args;
    typeof(messages) == TYPE_LIST || raise(E_TYPE);
    length(messages) <= 256 && value_bytes(messages) <= 1048576
      || raise(E_QUOTA, "Conversation exceeds 256 messages or 1 MiB");
    $llm_value:validate_json(messages);
    let pending = {};
    for message in (messages)
      typeof(message) == TYPE_MAP && maphaskey(message, "role") || raise(E_INVARG, "Message needs a role");
      const role = message["role"];
      typeof(role) == TYPE_STR || raise(E_TYPE, "Message role must be text");
      {"system", "user", "assistant", "tool"}:find({known} => strcmp(role, known) == 0)
        || raise(E_INVARG, "Invalid message role");
      if (role == "tool")
        maphaskey(message, "tool_call_id") && typeof(message["tool_call_id"]) == TYPE_STR
          || raise(E_INVARG, "Tool response needs an ID");
        let matched = 0;
        for index in [1..length(pending)]
          if (strcmp(pending[index], message["tool_call_id"]) == 0)
            matched = index;
            break;
          endif
        endfor
        matched > 0 || raise(E_INVARG, "Orphaned or duplicate tool response");
        pending = listdelete(pending, matched);
      else
        length(pending) == 0 || raise(E_INVARG, "Missing tool responses");
      endif
      if (role == "assistant" && maphaskey(message, "tool_calls") && length(message["tool_calls"]) > 0)
        const response = $llm_response:mk(["choices" -> {["message" -> message]}]);
        response:is_valid() || raise(E_INVARG, "Invalid assistant tool calls");
        pending = {call["id"] for call in (message["tool_calls"])};
      else
        maphaskey(message, "content") && typeof(message["content"]) in {TYPE_STR, TYPE_LIST}
          || raise(E_TYPE, "Message needs text or structured content");
      endif
    endfor
    length(pending) == 0 || raise(E_INVARG, "Missing tool responses");
    return true;
  endmethod

  method snapshot owner: ARCH_WIZARD
    "Return an immutable CoW copy of the current transcript.";
    $llm_conversation:check(this, caller_perms());
    return this.messages;
  endmethod

  method replace owner: ARCH_WIZARD
    "Validate and replace the complete transcript atomically; every write advances its revision.";
    $llm_conversation:check(this, caller_perms());
    const {messages} = args;
    $llm_conversation:validate(messages);
    this.messages = messages;
    this.revision = this.revision + 1;
  endmethod

  method append owner: ARCH_WIZARD
    "Append one ordinary message without permitting orphaned tool messages.";
    const principal = caller_perms();
    $llm_conversation:check(this, principal);
    const {role, content} = args;
    const messages = {@this.messages, ["role" -> role, "content" -> content]};
    set_task_perms(principal);
    return this:replace(messages);
  endmethod

  method begin_exchange owner: ARCH_WIZARD
    "Record an assistant call and unknown outcomes together, before any tool effects.";
    const principal = caller_perms();
    $llm_conversation:check(this, principal);
    const {assistant} = args;
    const offset = length(this.messages) + 1;
    let pending = [];
    let placeholders = {};
    for call in (assistant["tool_calls"])
      const id = call["id"];
      !maphaskey(pending, id) || raise(E_INVARG, "Duplicate tool call ID");
      const outcome = $llm_tool_result:mk('unknown, "No outcome recorded. Inspect effects before retrying.", "pending");
      placeholders = {@placeholders, outcome:message(id, call["function"]["name"])};
      pending[id] = offset + length(placeholders);
    endfor
    const messages = {@this.messages, assistant, @placeholders};
    set_task_perms(principal);
    this:replace(messages);
    return pending;
  endmethod

  method record_tool owner: ARCH_WIZARD
    "Replace the matching journal slot without accepting a shifted or unrelated tool outcome.";
    const principal = caller_perms();
    $llm_conversation:check(this, principal);
    const {index, id, message} = args;
    typeof(index) == TYPE_INT && index >= 1 && index <= length(this.messages) || raise(E_INVARG, "Tool journal position changed");
    const previous = this.messages[index];
    maphaskey(previous, "tool_call_id") && strcmp(previous["tool_call_id"], id) == 0
      && maphaskey(message, "tool_call_id") && strcmp(message["tool_call_id"], id) == 0
      || raise(E_INVARG, "Tool journal identity changed");
    let messages = this.messages;
    messages[index] = message;
    set_task_perms(principal);
    this:replace(messages);
    return message;
  endmethod

  method needs_compaction owner: ARCH_WIZARD
    "Compact before retained messages approach the count or byte limit.";
    $llm_conversation:check(this, caller_perms());
    return length(this.messages) >= 192 || value_bytes(this.messages) >= 524288;
  endmethod

  method prepare_compaction owner: ARCH_WIZARD
    "Capture a revision and split complete turns; return false when no prefix can be summarized.";
    $llm_conversation:check(this, caller_perms());
    const {keep} = args;
    typeof(keep) == TYPE_INT && keep > 0 && keep < 256 || raise(E_INVARG, "Keep count must be between 1 and 255");
    const snapshot = this.messages;
    length(snapshot) <= keep + 1 && return false;
    const has_system = snapshot[1]["role"] == "system";
    const first = has_system ? 2 | 1;
    let split = length(snapshot) - keep + 1;
    while (split > first)
      const message = snapshot[split];
      const role = message["role"];
      if (role == "user" || role == "system"
          || (role == "assistant" && !maphaskey(message, "tool_calls")))
        break;
      endif
      split = split - 1;
    endwhile
    split <= first && return false;
    return ['revision -> this.revision, 'owner -> this.subject.owner,
      'prefix -> has_system ? {snapshot[1]} | {}, 'old -> snapshot[first..split - 1],
      'recent -> snapshot[split..$]];
  endmethod

  method apply_summary owner: ARCH_WIZARD
    "Commit a valid summary only if the captured revision and owner still match.";
    const principal = caller_perms();
    $llm_conversation:check(this, principal);
    const {plan, summary} = args;
    typeof(summary) == TYPE_STR && length(summary:trim()) > 0 || raise(E_INVARG, "Empty conversation summary");
    plan['owner] == this.subject.owner || raise(E_PERM, "Conversation ownership changed");
    plan['revision] == this.revision || return false;
    const messages = {@plan['prefix],
      ["role" -> "assistant", "content" -> "Previous conversation summary: " + summary], @plan['recent]};
    set_task_perms(principal);
    this:replace(messages);
    return true;
  endmethod
endobject
