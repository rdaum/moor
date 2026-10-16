object LLM_HELP_TOOLS [
  import_export_id -> "llm_help_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Help Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Reusable help operations for in-world and external agents.";

  method get_tools owner: ARCH_WIZARD
    "Return the domain's immutable tool descriptors with explicit effect classifications.";
    const target_obj = this;
    let tools = {};
    tools = {@tools, $llm_agent_tool:mk("doc_lookup", "Read developer documentation for an object, verb, or property. Use formats: obj, obj:verb, obj.property.", ["type" -> "object", "properties" -> ["target" -> ["type" -> "string", "description" -> "Object/verb/property reference, e.g., '$sub_utils', '#61:drop_msg', '#61.get_msg'"]], "required" -> {"target"}], target_obj, "doc_lookup", 'inspection)};
    tools = {@tools, $llm_agent_tool:mk("help_lookup", "Look up a help topic to get information about commands and features. Pass empty string to list all available topics.", ["type" -> "object", "properties" -> ["topic" -> ["type" -> "string", "description" -> "Help topic to look up (e.g., 'building', 'passages', '@build'). Pass empty string to list all."]], "required" -> {"topic"}], target_obj, "help_lookup", 'inspection)};
    return tools;
  endmethod

  method doc_lookup owner: ARCH_WIZARD
    "Tool: Fetch developer documentation for object/verb/property";
    let alias_name = 0;
    let doc_text = 0;
    let item_name = 0;
    let object_str = 0;
    let parsed = 0;
    let selector = 0;
    let selectors = 0;
    let target_obj = 0;
    let title = 0;
    let type = 0;
    let verb_location = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const target_spec = args_map["target"];
    set_task_perms(actor);
    "Handle special alias cases";
    let alias_obj = false;
    if (typeof(target_spec) == TYPE_STR)
      alias_name = target_spec:starts_with("$") ? target_spec[2..$] | target_spec;
      alias_name == "sub_utils" && (alias_obj = $sub_utils);
      alias_name == "sub" && (alias_obj = $sub);
    endif
    if (alias_obj)
      type = 'object;
      target_obj = alias_obj;
      item_name = "";
    else
      parsed = $prog_utils:parse_target_spec(target_spec);
      parsed || raise(E_INVARG, "Invalid format. Use object, object:verb, or object.property");
      object_str = parsed['object_str];
      selectors = parsed['selectors];
      "Determine type and item_name from selectors";
      if (length(selectors) > 0)
        selector = selectors[1];
        type = selector['kind];
        item_name = selector['item_name];
      else
        type = 'object;
        item_name = "";
      endif
      target_obj = $match:match_object(object_str, actor);
      typeof(target_obj) == TYPE_OBJ || raise(E_INVARG, "Object not found");
      valid(target_obj) || raise(E_INVARG, "Object no longer exists");
    endif
    "Fetch docs based on type";
    if (type == 'object)
      doc_text = $help_utils:get_object_documentation(target_obj);
      title = "Documentation for " + tostr(target_obj);
    elseif (type == 'verb)
      verb_location = target_obj:find_verb_definer(item_name);
      verb_location == #-1 && raise(E_INVARG, "Verb '" + tostr(item_name) + "' not found on " + tostr(target_obj));
      doc_text = $help_utils:extract_verb_documentation(verb_location, item_name);
      title = "Documentation for " + tostr(target_obj) + ":" + tostr(item_name);
    elseif (type == 'property)
      doc_text = $help_utils:property_documentation(target_obj, item_name);
      title = "Documentation for " + tostr(target_obj) + "." + tostr(item_name);
    else
      raise(E_INVARG, "Unknown target type");
    endif
    const doc_body = typeof(doc_text) == TYPE_LIST ? doc_text:join("\n") | doc_text;
    return title + "\n\n" + (doc_body ? doc_body | "(No documentation available)");
  endmethod

  method help_lookup owner: ARCH_WIZARD
    "Tool: Look up a help topic";
    let all_topics = 0;
    let result = 0;
    const {args_map, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const topic = args_map["topic"];
    typeof(topic) != TYPE_STR && raise(E_INVARG, "Error: topic must be a string.");
    if (!is_player(actor))
      const room = actor.location;
      valid(room) || raise(E_INVARG, "Actor is not in a room.");
      return $llm_help_tools:lookup_environment(topic, actor,
        {$help_topics, $social_features, $builder_features, $prog_features, room, @room.contents});
    endif
    "If empty topic, list available topics";
    if (topic == "")
      all_topics = $llm_help_tools:catalog(actor, actor:help_environment());
      result = {"Available help topics:"};
      for t in (all_topics)
        result = {@result, "  " + t['name] + " - " + t['summary]};
      endfor
      return result:join("\n");
    endif
    "Search for specific topic";
    const found = actor:find_help_topic(topic);
    if (typeof(found) == TYPE_INT)
      return "No help found for: " + topic;
    endif
    "Return structured help";
    return "Topic: " + found.name + "\n\n" + found.summary + "\n\n" + found.content + (found.see_also ? "\n\nSee also: " + found.see_also:join(", ") | "");
  endmethod

  method catalog owner: ARCH_WIZARD
    "Collect bounded help metadata under the actor, preserving exact readable provider identities.";
    const {actor, environment} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    typeof(environment) == TYPE_LIST && length(environment) <= 128 || raise(E_QUOTA, "Too many help providers.");
    let seen = {};
    let names = {};
    let result = {};
    for source in (environment)
      if (typeof(source) != TYPE_OBJ || !valid(source) || source in seen)
        continue;
      endif
      seen = {@seen, source};
      if (!source.r || !respond_to(source, "help_topics"))
        continue;
      endif
      const topics = source:help_topics(actor, "");
      typeof(topics) == TYPE_LIST && length(topics) <= 256 || raise(E_QUOTA, "Invalid help topic list.");
      for topic in (topics)
        typeof(topic) == TYPE_FLYWEIGHT && topic.delegate == $help || raise(E_TYPE, "Invalid help topic.");
        const name = topic.name;
        const summary = topic.summary;
        const aliases = topic.aliases;
        typeof(name) == TYPE_STR && length(name) > 0 && length(name) <= 256
          && typeof(summary) == TYPE_STR && length(summary) <= 2048
          && typeof(aliases) == TYPE_LIST && length(aliases) <= 16 || raise(E_INVARG, "Invalid help metadata.");
        for alias in (aliases)
          typeof(alias) == TYPE_STR && length(alias) <= 256 || raise(E_INVARG, "Invalid help alias.");
        endfor
        if (!(name:lowercase() in names))
          names = {@names, name:lowercase()};
          result = {@result, ['name -> name, 'summary -> summary, 'aliases -> aliases, 'provider -> source]};
          length(result) <= 256 && value_bytes(result) <= 65536 || raise(E_QUOTA, "Help catalog is too large.");
        endif
      endfor
    endfor
    return result;
  endmethod

  method lookup_environment owner: ARCH_WIZARD
    "Search bounded readable help providers as the actor; provider failures remain explicit failures.";
    const {topic, actor, environment} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    length(environment) <= 128 || raise(E_QUOTA, "Too many help providers.");
    if (topic == "")
      const catalog = $llm_help_tools:catalog(actor, environment);
      return {"Available help topics:", @{entry['name] + " - " + entry['summary] for entry in (catalog)}}:join("\n");
    endif
    let seen = [];
    for source in (environment)
      if (!valid(source) || maphaskey(seen, source))
        continue;
      endif
      seen[source] = true;
      if (!source.r || !respond_to(source, "help_topics"))
        continue;
      endif
      const found = source:help_topics(actor, topic);
      if (typeof(found) == TYPE_FLYWEIGHT && found.delegate == $help)
        const text = "Topic: " + found.name + "\n\n" + found.summary + "\n\n" + found.content
          + (found.see_also ? "\n\nSee also: " + found.see_also:join(", ") | "");
        value_bytes(text) <= 65536 || raise(E_QUOTA, "Help topic is too large.");
        return text;
      endif
    endfor
    return "No help found for: " + topic;
  endmethod

endobject
