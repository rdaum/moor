object DATA_VISOR [
  import_export_id -> "data_visor",
  import_export_hierarchy -> {"llm"}
]
  name: "Data Visor"
  parent: LLM_WEARABLE
  owner: HACKER
  fertile: true
  readable: true

  property role_prompt (owner: HACKER, flags: "rc") = "You are the Data Visor, a concise heads-up display for inspecting and programming this MOO. Present concrete findings and readable source. Preserve the wearer's intent and use the available programming tools.";

  override description (owner: HACKER, flags: "rc") = "A sleek augmented reality visor that displays real-time MOO database information. When worn, it provides a heads-up display for inspecting objects, code, and system internals.";
  override placeholder_text (owner: HACKER, flags: "rc") = "Ask about objects, code, or database structure...";
  override processing_message (owner: HACKER, flags: "rc") = "Analyzing request, accessing neural pathways...";
  override prompt_label (owner: HACKER, flags: "rc") = "[INTERFACE]";
  override tool_name (owner: HACKER, flags: "rc") = "VISOR";

  method _system_prompt owner: ARCH_WIZARD
    "Compose the device persona with shared LLM guidance; descendants may specialize this hook.";
    return $llm_prompts:compose({this.role_prompt, $llm_prompts.interaction,
      $llm_prompts.project, $llm_prompts.programming});
  endmethod

  method _setup_agent owner: ARCH_WIZARD
    "Configure agent with visor-specific prompts and tools";
    const principal = caller_perms();
    const {agent} = args;
    $llm_agent:_challenge_permissions(agent, principal);
    agent.max_iterations = 30;
    set_task_perms(principal);
    agent.name = "LLM Agent for " + this.name + " (owned by " + tostr(this.owner) + ")";
    agent.system_prompt = this:_system_prompt();
    agent:reset_context();
    "Lower temperature for reliable tool selection";
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3):with_max_tokens(4096);
    agent.tool_callback = this;
    "Register authoring tools from parent class (doc_lookup, message tools, rule tools)";
    for tool in ($llm_inspection_tools:get_tools())
      agent:add_tool(tool.name, tool);
    endfor
    this:_register_authoring_tools(agent);
    "Register present_verb_code tool";
    const present_verb_code_tool = $llm_agent_tool:mk("present_verb_code", "PREFERRED: Present formatted verb code to the user with syntax highlighting and metadata table. Use this instead of get_verb_code when showing code to the user.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "show_line_numbers" -> ["type" -> "boolean", "description" -> "Include line numbers (default: true)"]], "required" -> {"object", "verb"}], this, "present_verb_code", 'presentation);
    agent:add_tool("present_verb_code", present_verb_code_tool);
    "Register present_verb_code_range tool";
    const present_verb_code_range_tool = $llm_agent_tool:mk("present_verb_code_range", "PREFERRED: Present a specific range of verb code to the user with syntax highlighting. Use this to show focused code regions.", ["type" -> "object", "properties" -> ["object" -> ["type" -> "string", "description" -> "The object (e.g. '#1', '$login', or 'here')"], "verb" -> ["type" -> "string", "description" -> "The verb name"], "start_line" -> ["type" -> "integer", "description" -> "First line to show (1-indexed)", "minimum" -> 1], "end_line" -> ["type" -> "integer", "description" -> "Last line to show (inclusive)", "minimum" -> 1], "context_lines" -> ["type" -> "integer", "description" -> "Additional context lines before/after (default: 0)", "minimum" -> 0, "maximum" -> 1000]], "required" -> {"object", "verb", "start_line", "end_line"}], this, "present_verb_code_range", 'presentation);
    agent:add_tool("present_verb_code_range", present_verb_code_range_tool);
    for tool in ($llm_programming_tools:get_tools())
      if (tool.name != "grant_capability")
        agent:add_tool(tool.name, tool);
      endif
    endfor
    const eval_tool = $llm_programming_tools:get_eval_tool();
    agent:add_tool(eval_tool.name, eval_tool);
    for tool in ($llm_building_tools:get_tools())
      if (tool.name in {"create_object", "recycle_object"})
        agent:add_tool(tool.name, tool);
      endif
    endfor
    "Register architect's compass building tools if available";
    this:_register_compass_tools_if_available(agent, this.owner);
  endmethod

  method _find_architects_compass owner: ARCH_WIZARD
    "Find architect's compass in wearer's inventory or worn items";
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(caller_perms());
    const {wearer} = args;
    !valid(wearer) && return #-1;
    "Check worn items";
    for item in (`wearer.wearing ! ANY => {}')
      if (valid(item) && item.owner == wearer && isa(item, $architects_compass))
        return item;
      endif
    endfor
    "Check inventory";
    for item in (wearer.contents)
      if (valid(item) && item.owner == wearer && isa(item, $architects_compass))
        return item;
      endif
    endfor
    return #-1;
  endmethod

  method _register_compass_tools_if_available owner: ARCH_WIZARD
    "Add selected shared building tools when the actor owns an available compass.";
    const principal = caller_perms();
    const {agent, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(actor);
    const compass = this:_find_architects_compass(actor);
    valid(compass) && compass.owner == actor || return;
    const names = {"build_room", "dig_passage", "remove_passage", "set_passage_description",
      "rename_object", "describe_object", "move_object", "grant_capability"};
    const catalog = $llm_tool_catalog:compose($llm_building_tools:get_tools(), $llm_programming_tools:get_tools());
    for tool in (catalog)
      if (tool.name in names)
        agent:add_tool(tool.name, tool);
      endif
    endfor
  endmethod

  method _tool_present_verb_code owner: ARCH_WIZARD
    "Present a bounded source snapshot on the captured wearable connection.";
    const {request, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const source = $llm_verb_source:read(actor, request["object"], request["verb"]);
    const document = source:document(`request["show_line_numbers"] ! E_RANGE => true');
    $llm_wearable_session:emit_for_device(this, actor, $event:mk_eval_result(actor, "", document));
    return "Code presented to user: " + source:signature();
  endmethod

  method _tool_present_verb_code_range owner: ARCH_WIZARD
    "Present a bounded source range with context on the captured wearable connection.";
    const {request, actor} = args;
    $llm_wearable:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const source = $llm_verb_source:read(actor, request["object"], request["verb"],
      request["start_line"], request["end_line"], `request["context_lines"] ! E_RANGE => 0');
    $llm_wearable_session:emit_for_device(this, actor, $event:mk_eval_result(actor, "", source:document()));
    return "Code range presented to user: " + source:signature() + " lines "
      + tostr(source.first) + "-" + tostr(source.last);
  endmethod

  method _check_user_eligible owner: HACKER
    "Visor requires .programmer to use";
    const {wearer} = args;
    wearer.programmer || raise(E_PERM, "The person wearing the visor is not a programmer, and not able to use its functions");
  endmethod

  method on_wear owner: ARCH_WIZARD
    "Initialize and activate the HUD when worn";
    const principal = caller_perms();
    const wearer = this.location;
    $root:check_actor(wearer, principal);
    set_task_perms(wearer);
    wearer:inform_current($event:mk_info(wearer, "The visor's interface flickers to life as you adjust it over your eyes. A luminescent display materializes in the corner of your vision - cascading lines of data flow past in electric blue" + " and green" + ". The world around you shimmers momentarily as the augmented reality overlay synchronizes with your neural patterns."):with_tts("The visor's interface flickers to life as you adjust it over your eyes. A luminescent display materializes in the corner of your vision - cascading lines of data flow past in electric blue and green. The world around you shimmers momentarily as the augmented reality overlay synchronizes with your neural patterns."));
    wearer:inform_current($event:mk_info(wearer, "[BOOT] Neural link established. Augmented reality overlay: " + "ONLINE"):with_presentation_hint('inset):with_group('llm, this):with_tts("Neural link established. Augmented reality overlay online."));
    wearer:inform_current($event:mk_info(wearer, "[READY] Database inspection interface active. Commands: use/interact, reset"):with_presentation_hint('inset):with_group('llm, this):with_tts("Database inspection interface active. Commands: use, interact, or reset."));
  endmethod

  method on_remove owner: ARCH_WIZARD
    "Deactivate the HUD when removed";
    const principal = caller_perms();
    const wearer = this.location;
    $root:check_actor(wearer, principal);
    set_task_perms(wearer);
    wearer:inform_current($event:mk_info(wearer, "[SHUTDOWN] Neural link severed. Augmented reality overlay: " + "OFFLINE"):with_presentation_hint('inset):with_group('llm, this):with_tts("Neural link severed. Augmented reality overlay offline."));
    wearer:inform_current($event:mk_info(wearer, "The luminescent display flickers and dims, data streams dissolving into static. The augmented overlay fades from your peripheral vision like phosphor afterimages. As the neural link disconnects, you hear a faint electronic hiss - then silence. The world returns to its unaugmented state."));
  endmethod

  verb plan_investigation (this with any) owner: ARCH_WIZARD flags: "rd"
    "Usage: plan_investigation DEVICE with DESCRIPTION. Start a tracked LLM task.";
    const actor = this:_command_actor(caller_perms(), caller);
    set_task_perms(actor);
    actor == this.owner && this in actor.wearing || raise(E_PERM, "Wear your device before planning.");
    this:_check_user_eligible(actor);
    if (is_clear_property(this, "agent") || !valid(this.agent) || this.agent.owner != this.owner)
      this:configure();
    endif
    const project = this:get_project();
    project:start_task(iobjstr);
    actor:inform_current($event:mk_info(actor, project:format_active()):with_group('llm, this));
  endverb

  verb get_investigation_status (this none none) owner: ARCH_WIZARD flags: "rd"
    "Usage: get_investigation_status DEVICE. Show the selected LLM task.";
    const actor = this:_command_actor(caller_perms(), caller);
    set_task_perms(actor);
    actor == this.owner && this in actor.wearing || raise(E_PERM, "Wear your device before reading its project.");
    const project = this:get_project();
    const text = valid(project) ? project:format_active() | "No active task.";
    actor:inform_current($event:mk_info(actor, text):with_group('llm, this));
  endverb

  method complete_investigation owner: ARCH_WIZARD
    "Complete the selected LLM investigation and clear its selection.";
    const principal = caller_perms();
    $llm_agent:_challenge_permissions(this, principal);
    const {?result = "Task completed."} = args;
    set_task_perms(principal);
    const project = this:get_project();
    !valid(project) && return "No active task.";
    const id = project:complete_active(result);
    return id == 0 ? "No active task." | "Task #" + tostr(id) + " completed.";
  endmethod

  method test_mutation_tool_actor_permissions owner: ARCH_WIZARD
    "Mutation tools retain ordinary ownership authority without fabricated target grants.";
    let add_prop_result = 0;
    let add_verb_result = 0;
    let args_result = 0;
    let code_result = 0;
    let delete_prop_result = 0;
    let delete_verb_result = 0;
    let prop_perms_result = 0;
    let recycle_result = 0;
    let set_prop_result = 0;
    let verb_perms_result = 0;
    const actor = $hacker;
    const previous_wearing = actor.wearing;
    let session = #-1;
    const visor = $data_visor:create(true);
    const target = $thing:create(0);
    const recycle_target = $thing:create(0);
    try
      visor.owner = actor;
      actor.wearing = {@previous_wearing, visor};
      session = $llm_wearable_session:open(visor, actor, 0);
      add_verb(session, {$arch_wizard, "rxd", "read_input"}, {"this", "none", "this"});
      set_verb_code(session, "read_input", {"return \"yes\";"});
      const provider = visor.agent.tools["add_verb"].target_obj;
      target.name = "Data Visor Mutation Probe";
      target.owner = actor;
      target.r = 0;
      recycle_target.name = "Data Visor Recycle Probe";
      recycle_target.owner = actor;
      recycle_target.r = 0;
      add_prop_result = provider:add_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "value" -> "initial", "rationale" -> "test add", "permissions" -> ""], actor);
      $test_utils:assert_true(index(add_prop_result, "Added property") > 0, "add_property should use the actor's ownership");
      set_prop_result = provider:set_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "value" -> "updated"], actor);
      $test_utils:assert_true(index(set_prop_result, "Set ") > 0, "set_property should use the actor's ownership");
      set_task_perms($arch_wizard);
      $test_utils:assert_eq(target.visor_mut_prop, "updated", "set_property should update the private target");
      prop_perms_result = provider:set_property_info(["object" -> tostr(target), "property" -> "visor_mut_prop", "permissions" -> "rc", "owner" -> tostr(actor)], actor);
      $test_utils:assert_true(index(prop_perms_result, "Updated property metadata") > 0, "set_property_info should use the actor's ownership");
      delete_prop_result = provider:delete_property(["object" -> tostr(target), "property" -> "visor_mut_prop", "rationale" -> "test delete"], actor);
      $test_utils:assert_true(index(delete_prop_result, "Deleted property") > 0, "delete_property should use the actor's ownership");
      set_task_perms($arch_wizard);
      $test_utils:assert_false("visor_mut_prop" in properties(target), "delete_property should remove the private property");
      add_verb_result = provider:add_verb(["object" -> tostr(target), "verb" -> "visor_mut_verb", "rationale" -> "test add", "permissions" -> "xd"], actor);
      $test_utils:assert_true(index(add_verb_result, "Added verb") > 0, "add_verb should use the actor's ownership");
      code_result = provider:program_verb(["object" -> tostr(target), "verb" -> "visor_mut_verb", "rationale" -> "test code", "code" -> "return \"updated\";"], actor);
      $test_utils:assert_true(index(code_result, "Programmed ") > 0, "program_verb should use the actor's ownership");
      args_result = provider:set_verb_args(["object" -> tostr(target), "verb" -> "visor_mut_verb", "dobj" -> "this", "prep" -> "none", "iobj" -> "this"], actor);
      $test_utils:assert_true(index(args_result, "Updated verb arguments") > 0, "set_verb_args should use the actor's ownership");
      verb_perms_result = provider:set_verb_info(["object" -> tostr(target), "verb" -> "visor_mut_verb", "permissions" -> "rxd", "owner" -> tostr(actor)], actor);
      $test_utils:assert_true(index(verb_perms_result, "Updated verb metadata") > 0, "set_verb_info should use the actor's ownership");
      delete_verb_result = provider:delete_verb(["object" -> tostr(target), "verb" -> "visor_mut_verb", "rationale" -> "test delete"], actor);
      $test_utils:assert_true(index(delete_verb_result, "Deleted verb") > 0, "delete_verb should use the actor's native authority");
      set_task_perms($arch_wizard);
      $test_utils:assert_false("visor_mut_verb" in verbs(target), "delete_verb should remove the private verb");
      recycle_result = visor.agent.tools["recycle_object"].target_obj:recycle_object(["object" -> tostr(recycle_target), "rationale" -> "test recycle"], actor);
      $test_utils:assert_true(index(recycle_result, "Recycled") > 0, "recycle_object should recycle under wearer permissions");
      set_task_perms($arch_wizard);
      $test_utils:assert_false(valid(recycle_target), "recycle_object should destroy the target");
    finally
      set_task_perms($arch_wizard);
      valid(recycle_target) && !is_anonymous(recycle_target) && recycle(recycle_target);
      valid(target) && !is_anonymous(target) && recycle(target);
      valid(visor) && !is_anonymous(visor) && recycle(visor);
      valid(session) && session:close();
      actor.wearing = previous_wearing;
    endtry
    return true;
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for the Data Visor.";
    const {for_player, ?topic = ""} = args;
    let my_topics = {$help:mk("visor", "Using the Data Visor", "The Data Visor is a wearable tool for inspecting and programming objects. Wear it with 'wear visor', then use 'use visor' or 'interact with visor' to start a conversation about what you want to inspect or code.", {"data visor", "programming tool"}, 'items, {"programming", "inspect"}), $help:mk("inspect", "Inspecting objects", "Use the visor to examine objects in detail. Ask it to 'inspect the door' or 'show me the properties of the chair'. It can reveal verbs, properties, and the internal structure of objects.", {"examine", "look at code"}, 'programming, {"visor", "programming"}), $help:mk("@examine", "Examine an object's structure", "Usage: @examine <object>\n\nShows detailed information about an object including its parent, owner, properties, and verbs.", {"@exam"}, 'commands, {"inspect", "visor"}), $help:mk("@program", "Edit a verb's code", "Usage: @program <object>:<verb>\n\nOpens the verb editor to modify an object's verb code. Requires programmer permissions.", {"@prog"}, 'commands, {"programming", "visor"})};
    my_topics = {@my_topics, $help:mk("investigations", "LLM investigation tasks", "Wear your visor. Use 'plan_investigation visor with DESCRIPTION' to start an investigation. Use 'get_investigation_status visor' to see its status. The LLM tools can record findings in the selected task. Use 'clear_project visor' to stop the interaction and clear tasks, findings, and todos. Reconfiguring the visor starts a fresh project.", {}, 'items, {})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

endobject
