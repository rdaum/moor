object LLM_PRESENTER [
  import_export_id -> "llm_presenter",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Presenter"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD

  property recipient (owner: ARCH_WIZARD, flags: "") = #-1;
  property destination (owner: ARCH_WIZARD, flags: "") = 0;
  property group (owner: ARCH_WIZARD, flags: "") = #-1;
  property active (owner: ARCH_WIZARD, flags: "") = false;

  method new owner: ARCH_WIZARD
    "Create an anonymous output destination for one authenticated request.";
    const principal = caller_perms();
    const {actor, destination, group} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    (typeof(destination) == TYPE_OBJ || destination == 0) && typeof(group) == TYPE_OBJ || raise(E_TYPE);
    const presenter = create($llm_presenter, $arch_wizard, 1);
    presenter.recipient = actor;
    presenter.destination = destination;
    presenter.group = group;
    presenter.active = true;
    return presenter;
  endmethod

  method check owner: ARCH_WIZARD
    "Validate the captured actor and open destination without invoking supplied behavior.";
    const principal = caller_perms();
    const {presenter, actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    typeof(presenter) == TYPE_OBJ && valid(presenter) && isa(presenter, $llm_presenter) || raise(E_TYPE);
    actor == presenter.recipient || raise(E_PERM);
    presenter.active || raise(E_INVARG, "Presentation is closed.");
    return true;
  endmethod

  method send owner: ARCH_WIZARD
    "Send bounded structured output only to the captured connection, never to the room.";
    const principal = caller_perms();
    const {presenter, actor, content} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    $llm_presenter:check(presenter, actor);
    value_bytes(content) <= 65536 || raise(E_QUOTA, "Presentation exceeds 64 KiB.");
    const destination = presenter.destination;
    typeof(destination) == TYPE_OBJ || return false;
    const event = $event:mk_info(actor, content):with_audience('utility):with_group('llm, presenter.group);
    set_task_perms(actor);
    actor:inform_connection(destination, event);
    return true;
  endmethod

  method send_event owner: ARCH_WIZARD
    "Deliver a bounded canonical event through the captured destination, including rewrite metadata.";
    const {presenter, actor, event} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_presenter:check(presenter, actor);
    typeof(event) == TYPE_FLYWEIGHT && event.delegate == $event && event.actor == actor || raise(E_INVARG);
    value_bytes(event) <= 65536 || raise(E_QUOTA, "Presentation exceeds 64 KiB.");
    const destination = presenter.destination;
    typeof(destination) == TYPE_OBJ || return false;
    set_task_perms(actor);
    return actor:inform_connection(destination, event:with_audience('utility));
  endmethod

  method close owner: ARCH_WIZARD
    "Close the destination and release references; retained tool descriptors cannot send more output.";
    const principal = caller_perms();
    principal == this.recipient || principal.wizard || raise(E_PERM);
    this.active = false;
    this.recipient = #-1;
    this.destination = 0;
    this.group = #-1;
  endmethod

  method get_tools owner: ARCH_WIZARD
    "Return immutable presentation tools bound to this destination.";
    set_task_perms(caller_perms());
    const title = ["type" -> "string", "maxLength" -> 256];
    const text = ["type" -> "string", "maxLength" -> 16384];
    return {
      $llm_agent_tool:mk("present_code", "Present source code to the requester without interpreting its contents.",
        ["type" -> "object", "properties" -> ["title" -> title, "code" -> text,
          "language" -> ["type" -> "string", "maxLength" -> 32]],
          "required" -> {"code"}, "additionalProperties" -> false], this, "present_code", 'presentation),
      $llm_agent_tool:mk("show_verb", "Read verb source and show a preview to the requester. Include the reason for reading it.",
        ["type" -> "object", "properties" -> ["object" -> title, "verb" -> title, "reason" -> text],
          "required" -> {"object", "verb", "reason"}, "additionalProperties" -> false], this, "show_verb", 'presentation),
      $llm_agent_tool:mk("present_report", "Present a prose report to the requester.",
        ["type" -> "object", "properties" -> ["title" -> title, "content" -> text],
          "required" -> {"content"}, "additionalProperties" -> false], this, "present_report", 'presentation),
      $llm_agent_tool:mk("present_table", "Present a table to the requester.",
        ["type" -> "object", "properties" -> ["title" -> title,
          "headers" -> ["type" -> "array", "items" -> title, "maxItems" -> 16],
          "rows" -> ["type" -> "array", "maxItems" -> 64,
            "items" -> ["type" -> "array", "maxItems" -> 16]]],
          "required" -> {"headers", "rows"}, "additionalProperties" -> false], this, "present_table", 'presentation)
    };
  endmethod

  method present_code owner: ARCH_WIZARD
    "Format source as an opaque code block for the captured requester.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    $llm_presenter:check(this, actor);
    const title = `parameters["title"] ! E_RANGE => ""';
    const language = `parameters["language"] ! E_RANGE => "moo"';
    const blocks = title ? {$format.title:mk(title)} | {};
    $llm_presenter:send(this, actor, $format.block:mk(@blocks, $format.code:mk(parameters["code"], language)));
    return "Code presented.";
  endmethod

  method present_report owner: ARCH_WIZARD
    "Format a validated prose report without decoding or coercing its text.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    $llm_presenter:check(this, actor);
    const title = `parameters["title"] ! E_RANGE => ""';
    const content = parameters["content"];
    const blocks = title ? {$format.title:mk(title)} | {};
    $llm_presenter:send(this, actor, $format.block:mk(@blocks, content));
    return "Report presented.";
  endmethod

  method present_table owner: ARCH_WIZARD
    "Format table values as text while preserving their row and column order.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    $llm_presenter:check(this, actor);
    const title = `parameters["title"] ! E_RANGE => ""';
    const headers = parameters["headers"];
    typeof(headers) == TYPE_LIST && typeof(parameters["rows"]) == TYPE_LIST || raise(E_TYPE);
    for candidate in (parameters["rows"])
      typeof(candidate) == TYPE_LIST || raise(E_TYPE, "Each row must be a list.");
    endfor
    const rows = {{typeof(cell) == TYPE_STR ? cell | tostr(cell) for cell in (row)} for row in (parameters["rows"])};
    const blocks = title ? {$format.title:mk(title)} | {};
    $llm_presenter:send(this, actor, $format.block:mk(@blocks, $format.table:mk(headers, rows)));
    return "Table presented: " + tostr(length(rows)) + " rows.";
  endmethod

  method show_verb owner: ARCH_WIZARD
    "Read one source snapshot and send a twelve-line preview to the captured connection.";
    const {parameters, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    $llm_presenter:check(this, actor);
    const source = $llm_verb_source:read(actor, parameters["object"], parameters["verb"]);
    const preview = source.lines[1..min(12, source.total)];
    const title = source:signature() + " [" + source.flags + "] " + source.argspec:join(" ");
    const omitted = source.total > 12 ? tostr(source.total - 12) + " more lines." | "";
    const document = $format.block:mk($format.title:mk(title), parameters["reason"],
      $format.code:mk(preview, 'moo, 1), omitted);
    $llm_presenter:send(this, actor, document);
    return ["code" -> source.lines:join("\n"), "flags" -> source.flags,
      "argspec" -> source.argspec:join(" "), "total_lines" -> source.total];
  endmethod
endobject
