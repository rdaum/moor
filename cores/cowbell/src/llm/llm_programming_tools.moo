object LLM_PROGRAMMING_TOOLS [
  import_export_id -> "llm_programming_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Programming Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Reusable programming operations for in-world and external agents.";

  method get_tools owner: ARCH_WIZARD
    "Return canonical programming schemas shared by rooms, external agents, and wearables.";
    const reference = ["type" -> "string", "minLength" -> 1, "maxLength" -> 256];
    const text = ["type" -> "string", "maxLength" -> 4096];
    const argument = ["type" -> "string", "enum" -> {"this", "any", "none"}];
    const permissions = ["type" -> "string", "maxLength" -> 4];
    const member = ["object" -> reference, "verb" -> reference];
    const property = ["object" -> reference, "property" -> reference];
    const value = ["description" -> "JSON value, used directly without evaluating MOO code."];
    const definitions = {
      {"add_verb", "Add an empty verb. Commands default to rd, methods to rxd.",
        ["object" -> reference, "verb" -> reference, "permissions" -> permissions, "dobj" -> argument, "prep" -> reference, "iobj" -> argument], {"object", "verb"}},
      {"program_verb", "Compile and replace a verb's code. Compiler errors leave its code unchanged.",
        ["object" -> reference, "verb" -> reference, "code" -> ["type" -> "string", "maxLength" -> 32768]], {"object", "verb", "code"}},
      {"delete_verb", "Delete a verb defined on the specified object.", member, {"object", "verb"}},
      {"set_verb_info", "Update supplied verb permissions, names, or owner.",
        ["object" -> reference, "verb" -> reference, "permissions" -> permissions, "names" -> reference, "owner" -> reference], {"object", "verb"}},
      {"set_verb_args", "Set the verb's direct object, preposition, and indirect object specifications.",
        ["object" -> reference, "verb" -> reference, "dobj" -> argument, "prep" -> reference, "iobj" -> argument], {"object", "verb", "dobj", "prep", "iobj"}},
      {"add_property", "Add a property with a JSON value (default 0) and flags (default rc).",
        ["object" -> reference, "property" -> reference, "value" -> value, "permissions" -> permissions], {"object", "property"}},
      {"set_property", "Set a property to the supplied JSON value.",
        ["object" -> reference, "property" -> reference, "value" -> value], {"object", "property", "value"}},
      {"delete_property", "Delete a property defined on the specified object.", property, {"object", "property"}},
      {"set_property_info", "Update supplied property permissions or owner.",
        ["object" -> reference, "property" -> reference, "permissions" -> permissions, "owner" -> reference], {"object", "property"}},
      {"grant_capability", "Grant building permissions on an area or room to a player.",
        ["target" -> reference, "category" -> ["type" -> "string", "enum" -> {"area", "room"}],
          "permissions" -> ["type" -> "array", "items" -> reference, "maxItems" -> 32, "minItems" -> 1],
          "grantee" -> reference], {"target", "category", "permissions", "grantee"}}
    };
    let tools = {};
    for definition in (definitions)
      const {name, description, fields, required} = definition;
      let properties = fields;
      properties["rationale"] = text;
      const schema = ["type" -> "object", "properties" -> properties,
        "required" -> required, "additionalProperties" -> false];
      tools = {@tools, $llm_agent_tool:mk(name, description, schema, this, name, 'mutation)};
    endfor
    return tools;
  endmethod

  method get_eval_tool owner: ARCH_WIZARD
    "Return the visor's opt-in evaluator; ordinary programming catalogs do not expose raw eval.";
    return $llm_agent_tool:mk("eval", "Execute MOO statements as the requester. Use return for results. Each execution requires its own confirmation.",
      ["type" -> "object", "properties" -> [
        "rationale" -> ["type" -> "string", "minLength" -> 1, "maxLength" -> 4096],
        "code" -> ["type" -> "string", "minLength" -> 1, "maxLength" -> 32768]],
        "required" -> {"rationale", "code"}, "additionalProperties" -> false], this, "evaluate", 'mutation);
  endmethod

  method evaluate owner: ARCH_WIZARD
    "Evaluate explicitly confirmed source as the programmer; never share approval across eval calls.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    this.session_bound && actor.programmer || raise(E_PERM, "Evaluation requires an active programmer interaction.");
    const code = request["code"];
    typeof(code) == TYPE_STR && length(code) > 0 && length(code) <= 32768 || raise(E_INVARG);
    const approval = this:_approve(actor, 'eval, actor, 'eval, "", request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    "Only a wizard activation can propagate a changed player into the next activation.";
    player = actor;
    return $llm_programming_tools:_evaluate_as(this, actor, code);
  endmethod

  method _evaluate_as owner: ARCH_WIZARD
    "Enter eval with both player and native permissions set to the captured programmer.";
    const {provider, actor, code} = args;
    caller == provider && caller_perms().wizard || raise(E_PERM);
    $llm_tool_provider:check(provider, actor, caller_perms());
    provider.session_bound && player == actor && actor.programmer || raise(E_PERM);
    set_task_perms(actor);
    let result;
    try
      result = eval(code, [], 1, 0);
    except error (ANY)
      return $llm_tool_result:mk('unknown, tostr(error[1]) + ": " + tostr(error[2]), "eval_failed");
    endtry
    if (!result[1])
      return $llm_tool_result:mk('failure, result[2], "compile_error");
    endif
    return "Result: " + toliteral(result[2]);
  endmethod

  method add_verb owner: ARCH_WIZARD
    "Add an empty verb under the captured actor after any interaction confirmation.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["verb"];
    const direct = `request["dobj"] ! E_RANGE => "this"';
    const prep = `request["prep"] ! E_RANGE => "none"';
    const indirect = `request["iobj"] ! E_RANGE => "none"';
    const defaults = direct == "none" && prep == "none" && indirect == "none" ? "rxd" | "rd";
    const flags = `request["permissions"] ! E_RANGE => defaults';
    const approval = this:_approve(actor, 'add_verb, target, 'verbs, "", request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    add_verb(target, {actor, flags, name}, {direct, prep, indirect});
    return "Added verb " + tostr(target) + ":" + name;
  endmethod

  method program_verb owner: ARCH_WIZARD
    "Compile code under the actor; report rejected source as a known failure.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["verb"];
    const lines = request["code"]:split("\n");
    const approval = this:_approve(actor, 'program_verb, target, 'verb, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    const errors = set_verb_code(target, name, lines);
    if (typeof(errors) == TYPE_LIST && length(errors))
      return $llm_tool_result:mk('failure, errors:join("\n"), "compile_error");
    endif
    return "Programmed " + tostr(target) + ":" + name;
  endmethod

  method delete_verb owner: ARCH_WIZARD
    "Delete a local verb with the actor's native authority.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["verb"];
    const approval = this:_approve(actor, 'delete_verb, target, 'verb, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    delete_verb(target, name);
    return "Deleted verb " + tostr(target) + ":" + name;
  endmethod

  method set_verb_info owner: ARCH_WIZARD
    "Update only supplied metadata and preserve all other verb fields.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["verb"];
    const owner = maphaskey(request, "owner")
      ? this:_resolve_object(request["owner"], actor) | #-1;
    let info = verb_info(target, name);
    let changed = false;
    for entry in ({{"permissions", 2}, {"names", 3}})
      if (maphaskey(request, entry[1]))
        info[entry[2]] = request[entry[1]];
        changed = true;
      endif
    endfor
    if (maphaskey(request, "owner"))
      info[1] = owner;
      valid(info[1]) || raise(E_INVARG);
      changed = true;
    endif
    changed || raise(E_INVARG, "Supply at least one metadata field.");
    const approval = this:_approve(actor, 'set_verb_info, target, 'verb, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    set_verb_info(target, name, info);
    return "Updated verb metadata on " + tostr(target) + ":" + name;
  endmethod

  method set_verb_args owner: ARCH_WIZARD
    "Set a local verb's argument specification with the actor's native authority.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["verb"];
    const specification = {request["dobj"], request["prep"], request["iobj"]};
    const approval = this:_approve(actor, 'set_verb_args, target, 'verb, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    set_verb_args(target, name, specification);
    return "Updated verb arguments on " + tostr(target) + ":" + name;
  endmethod

  method add_property owner: ARCH_WIZARD
    "Add a property using its supplied JSON value and permissions.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["property"];
    const value = `request["value"] ! E_RANGE => 0';
    const flags = `request["permissions"] ! E_RANGE => "rc"';
    const approval = this:_approve(actor, 'add_property, target, 'properties, "", request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    add_property(target, name, value, {actor, flags});
    return "Added property " + tostr(target) + "." + name;
  endmethod

  method set_property owner: ARCH_WIZARD
    "Set a property directly without evaluating source text.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["property"];
    const value = request["value"];
    const approval = this:_approve(actor, 'set_property, target, 'property, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    target.(name) = value;
    return "Set " + tostr(target) + "." + name;
  endmethod

  method delete_property owner: ARCH_WIZARD
    "Delete a local property with the actor's native authority.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["property"];
    const approval = this:_approve(actor, 'delete_property, target, 'property, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    delete_property(target, name);
    return "Deleted property " + tostr(target) + "." + name;
  endmethod

  method set_property_info owner: ARCH_WIZARD
    "Update only supplied property metadata fields.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["object"], actor);
    valid(target) || raise(E_INVARG, "Object no longer exists.");
    const name = request["property"];
    const owner = maphaskey(request, "owner")
      ? this:_resolve_object(request["owner"], actor) | #-1;
    let info = property_info(target, name);
    let changed = false;
    if (maphaskey(request, "permissions"))
      info[2] = request["permissions"];
      changed = true;
    endif
    if (maphaskey(request, "owner"))
      info[1] = owner;
      valid(info[1]) || raise(E_INVARG);
      changed = true;
    endif
    changed || raise(E_INVARG, "Supply at least one metadata field.");
    const approval = this:_approve(actor, 'set_property_info, target, 'property, name, request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    set_property_info(target, name, info);
    return "Updated property metadata on " + tostr(target) + "." + name;
  endmethod

  method grant_capability owner: ARCH_WIZARD
    "Grant explicit building permissions under the captured actor's authority.";
    const {request, actor} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    const target = this:_resolve_object(request["target"], actor);
    const grantee = this:_resolve_object(request["grantee"], actor);
    valid(target) && valid(grantee) || raise(E_INVARG);
    actor == target.owner || actor.wizard || raise(E_PERM);
    const category = request["category"]:to_symbol();
    const permissions = {p:to_symbol() for p in (request["permissions"])};
    const approval = this:_approve(actor, 'grant_capability, target, 'object, "", request);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    $root:grant_capability(target, permissions, grantee, category);
    return "Granted " + toliteral(permissions) + " on " + tostr(target) + " to " + tostr(grantee);
  endmethod

  method test_programming_compile_failure owner: ARCH_WIZARD
    "Rejected source produces a known failure and preserves the previous executable body.";
    const actor = $hacker;
    const target = create($thing, actor, 1);
    add_verb(target, {actor, "rxd", "probe"}, {"this", "none", "this"});
    set_verb_code(target, "probe", {"return 17;"});
    const result = this:program_verb(["object" -> target, "verb" -> "probe", "code" -> "if ("], actor);
    $test_utils:assert_eq(result.status, 'failure, "Compiler rejection is a known failure");
    $test_utils:assert_eq(result.code, "compile_error", "The failure identifies the compiler");
    $test_utils:assert_eq(target:probe(), 17, "Rejected code leaves the executable verb unchanged");
    return true;
  endmethod

  method test_programming_metadata_actor_permissions owner: ARCH_WIZARD
    "Advertised metadata tools update owned verbs and reject writes to foreign verbs.";
    const actor = $test_player;
    const owned = create($thing, actor, 2);
    const foreign = create($thing, $hacker, 2);
    try
      add_verb(owned, {actor, "rxd", "probe"}, {"this", "none", "this"});
      add_verb(foreign, {$hacker, "rxd", "probe"}, {"this", "none", "this"});
      this:set_verb_info(["object" -> tostr(owned), "verb" -> "probe", "permissions" -> "xd", "names" -> "renamed alias"], actor);
      this:set_verb_args(["object" -> tostr(owned), "verb" -> "renamed", "dobj" -> "any", "prep" -> "none", "iobj" -> "none"], actor);
      $test_utils:assert_eq(verb_info(owned, "renamed"), {actor, "xd", "renamed alias"}, "Metadata fields retain the actor owner");
      $test_utils:assert_eq(verb_args(owned, "renamed"), {"any", "none", "none"}, "Argument specification is updated");
      for operation in ({
        {"set_verb_info", ["object" -> tostr(foreign), "verb" -> "probe", "permissions" -> "rwxd"]},
        {"set_verb_args", ["object" -> tostr(foreign), "verb" -> "probe", "dobj" -> "any", "prep" -> "none", "iobj" -> "none"]}})
        $test_utils:assert_raises(E_PERM, this, operation[1], {operation[2], actor}, "Foreign metadata requires existing write authority");
      endfor
      $test_utils:assert_eq(verb_info(foreign, "probe"), {$hacker, "rxd", "probe"}, "Denied mutation leaves foreign metadata intact");
      $test_utils:assert_eq(verb_args(foreign, "probe"), {"this", "none", "this"}, "Denied mutation leaves foreign argument specification intact");
    finally
      recycle(foreign);
      recycle(owned);
    endtry
    return true;
  endmethod

endobject
