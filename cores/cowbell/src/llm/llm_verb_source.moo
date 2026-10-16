object LLM_VERB_SOURCE [
  import_export_id -> "llm_verb_source",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Verb Source"
  parent: ROOT
  owner: ARCH_WIZARD

  method metadata owner: ARCH_WIZARD
    "Capture native verb metadata under actor authority without invoking target behavior.";
    const {actor, spec, name} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    typeof(name) == TYPE_STR && length(name) > 0 && length(name) <= 256 || raise(E_INVARG, "Invalid verb name.");
    set_task_perms(actor);
    const target = $llm_tool_provider:_resolve_object(spec, actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    for definer in ({target, @ancestors(target)})
      try
        const info = verb_info(definer, name);
        const argspec = verb_args(definer, name);
        const snapshot = <$llm_verb_source, .target = target, .definer = definer, .name = name,
          .owner = info[1], .flags = info[2], .names = info[3], .argspec = argspec>;
        value_bytes(snapshot) <= 65536 || raise(E_QUOTA, "Verb metadata exceeds 64 KiB.");
        return snapshot;
      except (E_VERBNF)
        continue;
      endtry
    endfor
    raise(E_VERBNF, "Verb not found: " + name);
  endmethod

  method read owner: ARCH_WIZARD
    "Capture source or a range. End zero means the last line; context expands a valid range.";
    const {actor, spec, name, ?start = 1, ?end = 0, ?context = 0} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    typeof(start) == TYPE_INT && typeof(end) == TYPE_INT && typeof(context) == TYPE_INT || raise(E_TYPE);
    start >= 1 && (end == 0 || end >= start) && context >= 0 && context <= 1000
      || raise(E_INVARG, "Invalid source range.");
    set_task_perms(actor);
    const metadata = $llm_verb_source:metadata(actor, spec, name);
    const source = verb_code(metadata.definer, name, false, true);
    const count = length(source);
    const first = max(1, start - context);
    const last = end == 0 ? count | min(count, end + context);
    if (!(count == 0 && start == 1 && end == 0))
      first <= last || raise(E_INVARG, "Source range is outside the verb.");
    endif
    let slots = flyslots(metadata);
    slots['lines] = count == 0 ? {} | source[first..last];
    slots['first] = first;
    slots['last] = last;
    slots['total] = count;
    const snapshot = toflyweight($llm_verb_source, slots);
    value_bytes(snapshot) <= 65536 || raise(E_QUOTA, "Source selection exceeds 64 KiB; request fewer lines.");
    return snapshot;
  endmethod

  method signature owner: ARCH_WIZARD
    "Name the captured definer and requested verb alias without reading live metadata.";
    return tostr(this.definer) + ":" + this.name;
  endmethod

  method metadata_text owner: ARCH_WIZARD
    "Render captured verb metadata for an LLM inspection result.";
    return {"Verb: " + this:signature(), "Owner: " + tostr(this.owner), "Flags: " + this.flags,
      "Args: " + this.argspec:join(" "), "Defined on: " + tostr(this.definer)}:join("\n");
  endmethod

  method text owner: ARCH_WIZARD
    "Render captured source and metadata for an LLM inspection result.";
    return {"Verb: " + this:signature(), "Flags: " + this.flags + "  Argspec: " + this.argspec:join(" "),
      "Code (" + tostr(this.total) + " lines):", "---", this.lines:join("\n"), "---"}:join("\n");
  endmethod

  method numbered_text owner: ARCH_WIZARD
    "Render a selected source range with its original line numbers.";
    return {tostr(this.first + i - 1) + ": " + this.lines[i] for i in [1..length(this.lines)]}:join("\n");
  endmethod

  method document owner: ARCH_WIZARD
    "Build structured metadata and opaque source; renderers supply the optional number gutter.";
    const {?show_line_numbers = true} = args;
    typeof(show_line_numbers) == TYPE_BOOL || raise(E_TYPE);
    const range = this.total == 0 ? "0 lines"
      | "Lines " + tostr(this.first) + "-" + tostr(this.last) + " of " + tostr(this.total);
    const table = $format.table:mk({"Verb", "Args", "Owner", "Flags", "Range"},
      {{this:signature(), this.argspec:join(" "), tostr(this.owner), this.flags, range}});
    return $format.block:mk(table, $format.code:mk(this.lines, 'moo, show_line_numbers ? this.first | false));
  endmethod
endobject
