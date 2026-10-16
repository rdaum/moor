object LLM_CODE_SEARCH [
  import_export_id -> "llm_code_search",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Code Search"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  method get_tools owner: ARCH_WIZARD
    "Describe literal source search with bounded pages and an explicit continuation cursor.";
    const schema = ["type" -> "object", "properties" -> [
      "pattern" -> ["type" -> "string", "minLength" -> 1, "maxLength" -> 128,
        "description" -> "Case-insensitive literal text, not a regular expression"],
      "object" -> ["type" -> "string", "minLength" -> 1, "maxLength" -> 256,
        "description" -> "Optional object; otherwise search local scope, or global scope for a wizard"],
      "limit" -> ["type" -> "integer", "minimum" -> 1, "maximum" -> 50,
        "description" -> "Maximum matches in this page; default 20"],
      "cursor" -> ["type" -> "array", "minItems" -> 3, "maxItems" -> 3,
        "items" -> ["type" -> "integer", "minimum" -> 1],
        "description" -> "Copy a nonempty next_cursor with the same pattern and scope"]],
      "required" -> {"pattern"}, "additionalProperties" -> false];
    return {$llm_agent_tool:mk("grep",
      "Search readable local verb definitions. Follow next_cursor until empty and report omissions. "
        + "Pages read the current world; restart after object or verb changes.",
      schema, $llm_code_search, "search", 'inspection)};
  endmethod

  method _scope owner: ARCH_WIZARD
    "Capture sorted object identities with actor authority; native enumeration materializes the scope.";
    const {actor, spec} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    if (spec != "")
      const target = $llm_tool_provider:_resolve_object(spec, actor);
      valid(target) || raise(E_INVARG, "Search object does not exist.");
      return {{target}, "object " + tostr(target)};
    endif
    actor.wizard && return {sort(objects()), "global"};
    const room = actor.location;
    let candidates = {actor, @actor.contents};
    if (valid(room))
      candidates = {@candidates, room, @room.contents};
    endif
    let unique = [];
    for candidate in (candidates)
      valid(candidate) && (unique[candidate] = true);
    endfor
    return {sort(mapkeys(unique)), "local"};
  endmethod

  method _snippet owner: ARCH_WIZARD
    "Capture at most 200 source characters around the complete matched literal.";
    const {line, position, pattern_length} = args;
    const first = max(1, position - (200 - pattern_length) / 2);
    const last = min(length(line), first + 199);
    return (first > 1 ? "..." | "") + line[first..last] + (last < length(line) ? "..." | "");
  endmethod

  method _page owner: ARCH_WIZARD
    "Return bounded JSON data; cursor fields are one-based object, verb, and line positions.";
    let {result, cursor} = args;
    result["next_cursor"] = cursor;
    value_bytes(result) <= 65536 || raise(E_QUOTA, "Search result exceeds 64 KiB.");
    return result;
  endmethod

  method search owner: ARCH_WIZARD
    "Search one bounded page under actor permissions without yielding between native reads.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    const values = $llm_tool_schema:arguments(request, $llm_code_search:get_tools()[1].parameters);
    const pattern = values["pattern"];
    const limit = `values["limit"] ! E_RANGE => 20';
    const spec = `values["object"] ! E_RANGE => ""';
    let {object_index, verb_index, line_index} = `values["cursor"] ! E_RANGE => {1, 1, 1}';
    set_task_perms(actor);
    const {scope, label} = $llm_code_search:_scope(actor, spec);
    object_index <= length(scope) + 1 || raise(E_INVARG, "Search cursor is outside the current scope.");
    let result = ["scope" -> label, "matches" -> {}, "omitted_lines" -> {},
      "unreadable_objects" -> 0, "unreadable_verbs" -> 0,
      "scanned" -> ["objects" -> 0, "verbs" -> 0, "lines" -> 0, "bytes" -> 0]];
    while (object_index <= length(scope))
      if (result["scanned"]["objects"] >= 64)
        return $llm_code_search:_page(result, {object_index, verb_index, line_index});
      endif
      const target = scope[object_index];
      result["scanned"]["objects"] = result["scanned"]["objects"] + 1;
      let names = {};
      try
        names = verbs(target);
      except error (E_PERM)
        spec != "" && raise(error[1], error[2]);
        result["unreadable_objects"] = result["unreadable_objects"] + 1;
      endtry
      while (verb_index <= length(names))
        if (result["scanned"]["verbs"] >= 128)
          return $llm_code_search:_page(result, {object_index, verb_index, line_index});
        endif
        result["scanned"]["verbs"] = result["scanned"]["verbs"] + 1;
        let lines = {};
        try
          lines = verb_code(target, verb_index);
        except (E_PERM)
          result["unreadable_verbs"] = result["unreadable_verbs"] + 1;
        endtry
        while (line_index <= length(lines))
          if (result["scanned"]["lines"] >= 512 || length(result["matches"]) >= limit
              || length(result["omitted_lines"]) >= 16)
            return $llm_code_search:_page(result, {object_index, verb_index, line_index});
          endif
          const line = lines[line_index];
          const bytes = value_bytes(line);
          if (bytes > 262144)
            result["omitted_lines"] = {@result["omitted_lines"],
              ["object" -> tostr(target), "verb_index" -> verb_index, "line" -> line_index,
                "reason" -> "Line exceeds 256 KiB."]};
          else
            if (result["scanned"]["bytes"] + bytes > 262144)
              return $llm_code_search:_page(result, {object_index, verb_index, line_index});
            endif
            result["scanned"]["bytes"] = result["scanned"]["bytes"] + bytes;
            const position = index(line, pattern, false);
            if (position > 0)
              const aliases = verb_info(target, verb_index)[3];
              const match = ["object" -> tostr(target), "verb_index" -> verb_index,
                "names" -> aliases[1..min(length(aliases), 128)],
                "line" -> line_index, "text" -> $llm_code_search:_snippet(line, position, length(pattern))];
              const matches = {@result["matches"], match};
              if (value_bytes(matches) > 49152)
                return $llm_code_search:_page(result, {object_index, verb_index, line_index});
              endif
              result["matches"] = matches;
            endif
          endif
          result["scanned"]["lines"] = result["scanned"]["lines"] + 1;
          line_index = line_index + 1;
        endwhile
        verb_index = verb_index + 1;
        line_index = 1;
      endwhile
      object_index = object_index + 1;
      verb_index = 1;
      line_index = 1;
    endwhile
    return $llm_code_search:_page(result, {});
  endmethod
endobject
