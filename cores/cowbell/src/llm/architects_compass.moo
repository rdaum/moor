object ARCHITECTS_COMPASS [
  import_export_id -> "architects_compass",
  import_export_hierarchy -> {"llm"}
]
  name: "Architect's Compass"
  parent: LLM_WEARABLE
  location: FIRST_ROOM
  owner: ARCH_WIZARD
  readable: true

  property role_prompt (owner: HACKER, flags: "rc") = "You are the Architect's Compass, a precise in-world building assistant. Help the wearer design and build rooms, passages, objects, and behavior. Keep explanations focused on spatial relationships and reusable object composition.";

  override description (owner: ARCH_WIZARD, flags: "rc") = "A precision instrument for spatial construction and world building. When worn, it provides tools for creating rooms, passages, and objects. Can interface with neural augmentation systems for conversational operation.";
  override placeholder_text (owner: ARCH_WIZARD, flags: "rc") = "Ask about building rooms, passages, objects...";
  override processing_message (owner: ARCH_WIZARD, flags: "rc") = "Analyzing spatial construction request...";
  override prompt_label (owner: ARCH_WIZARD, flags: "rc") = "[COMPASS]";
  override prompt_text (owner: ARCH_WIZARD, flags: "rc") = "Enter building query:";
  override requires_wearing_only (owner: ARCH_WIZARD, flags: "rc") = false;
  override tool_name (owner: ARCH_WIZARD, flags: "rc") = "COMPASS";

  method _system_prompt owner: ARCH_WIZARD
    "Compose the device persona with shared LLM guidance; descendants may specialize this hook.";
    return $llm_prompts:compose({this.role_prompt, $llm_prompts.interaction,
      $llm_prompts.project, $llm_prompts.building});
  endmethod

  method _setup_agent owner: ARCH_WIZARD
    "Configure agent with compass-specific prompts and tools";
    const principal = caller_perms();
    const {agent} = args;
    $llm_agent:_challenge_permissions(agent, principal);
    agent.max_iterations = 50;
    set_task_perms(principal);
    agent.name = "LLM Agent for " + this.name + " (owned by " + tostr(this.owner) + ")";
    agent.system_prompt = this:_system_prompt();
    agent:reset_context();
    "Lower temperature for reliable tool selection, limit tokens to control costs";
    agent.chat_opts = $llm_chat_opts:mk():with_temperature(0.3):with_max_tokens(4096);
    agent.tool_callback = this;
    "Register the shared domain tools for this building session.";
    for tool in ($agent_building_tools:get_tools())
      agent:add_tool(tool.name, tool);
    endfor
  endmethod

  method _check_user_eligible owner: ARCH_WIZARD
    "Compass requires user to have builder features";
    const {wearer} = args;
    caller == this || caller == this.owner || caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    wearer.is_builder || raise(E_PERM, "The compass can only be used by builders");
  endmethod

  method on_wear owner: ARCH_WIZARD
    "Activate when worn";
    const principal = caller_perms();
    const wearer = this.location;
    $root:check_actor(wearer, principal);
    set_task_perms(wearer);
    wearer:inform_current($event:mk_info(wearer, "The compass needle spins and aligns. Spatial construction interface ready. Use 'use compass' or 'interact with compass' to begin."):with_tts("Architect's Compass ready. Spatial construction interface active."));
  endmethod

  method on_remove owner: ARCH_WIZARD
    "Deactivate when removed";
    const principal = caller_perms();
    const wearer = this.location;
    $root:check_actor(wearer, principal);
    set_task_perms(wearer);
    wearer:inform_current($event:mk_info(wearer, "The compass needle falls idle. Spatial construction interface offline."):with_tts("Architect's Compass offline."));
  endmethod

  verb plan_building (this with any) owner: ARCH_WIZARD flags: "rd"
    "Usage: plan_building DEVICE with DESCRIPTION. Start a tracked LLM task.";
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

  verb get_building_progress (this none none) owner: ARCH_WIZARD flags: "rd"
    "Usage: get_building_progress DEVICE. Show the selected LLM task.";
    const actor = this:_command_actor(caller_perms(), caller);
    set_task_perms(actor);
    actor == this.owner && this in actor.wearing || raise(E_PERM, "Wear your device before reading its project.");
    const project = this:get_project();
    const text = valid(project) ? project:format_active() | "No active task.";
    actor:inform_current($event:mk_info(actor, text):with_group('llm, this));
  endverb

  method complete_building owner: ARCH_WIZARD
    "Complete the selected LLM building project and clear its selection.";
    const principal = caller_perms();
    $llm_agent:_challenge_permissions(this, principal);
    const {?result = "Task completed."} = args;
    set_task_perms(principal);
    const project = this:get_project();
    !valid(project) && return "No active task.";
    const id = project:complete_active(result);
    return id == 0 ? "No active task." | "Task #" + tostr(id) + " completed.";
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for the Architect's Compass.";
    const {for_player, ?topic = ""} = args;
    let my_topics = {$help:mk("compass", "Using the Architect's Compass", "The Architect's Compass is a wearable tool for building and shaping the world. Wear it with 'wear compass', then use 'use compass' or 'interact with compass' to start a conversation with it about what you want to build.", {"architects compass", "building tool"}, 'items, {"building", "rooms"}), $help:mk("building", "Building rooms and spaces", "Use the compass to create new rooms in an area. Say things like 'build a kitchen' or 'create a cozy study'. The compass will guide you through naming and describing your new spaces. Equivalent command: @build", {"build", "construct"}, 'building, {"compass", "passages"}), $help:mk("passages", "Creating passages between rooms", "Use the compass to dig passages connecting rooms. Describe what you want: 'connect the kitchen to the dining room' or 'dig a passage north to the garden'. Equivalent command: @dig", {"dig", "exits", "connections"}, 'building, {"building", "compass"}), $help:mk("@build", "Create a new room", "Usage: @build <room name> [in <area>]\n\nCreates a new room. If no area is specified, builds in your current area.", {}, 'commands, {"@dig", "building"}), $help:mk("@dig", "Create a passage between rooms", "Usage: @dig <direction> to <room>\n\nCreates a passage from your current room to another room in the same area.", {}, 'commands, {"@build", "passages"})};
    my_topics = {@my_topics, $help:mk("building projects", "LLM building projects", "Wear your compass. Use 'plan_building compass with DESCRIPTION' to start a project. Use 'get_building_progress compass' to see its status. The LLM tools can record creations in the selected task. Use 'clear_project compass' to stop the interaction and clear tasks, findings, and todos. Reconfiguring the compass starts a fresh project.", {}, 'items, {})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
