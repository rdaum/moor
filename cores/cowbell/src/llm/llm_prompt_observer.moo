object LLM_PROMPT_OBSERVER [
  import_export_id -> "llm_prompt_observer",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Prompt Observer"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property agent (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property enabled (owner: ARCH_WIZARD, flags: "rc") = true;
  property runner (owner: ARCH_WIZARD, flags: "rc") = #-1;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Room-facing observer adapter built on the shared LLM runtime.";

  method configure owner: ARCH_WIZARD
    "Replace idle collaborators with anonymous objects owned by the observer's owner.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    const local_runner = !is_clear_property(this, "runner") && valid(this.runner)
      && this.runner.owner == this.owner && isa(this.runner, $llm_runner);
    if (local_runner && valid(this.runner.agent) && this.runner.agent.owner == this.owner)
      !this.runner.agent.running || raise(E_INVARG, "Wait for the active response before reconfiguring");
    endif
    const agent = create($llm_agent, this.owner, 1);
    const runner = create($llm_runner, this.owner, 1);
    set_task_perms(this.owner);
    runner:attach_agent(agent);
    if (local_runner)
      this.runner.enabled = false;
      this.runner.agent = #-1;
    endif
    this.agent = agent;
    this.runner = runner;
    return agent;
  endmethod

  method respond_once owner: ARCH_WIZARD
    "Run one response as the caller; optional speech stays in the captured room.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    const {prompt, ?announce = false} = args;
    this.enabled || return "Observer disabled.";
    const destination = this.location;
    set_task_perms(principal);
    if (is_clear_property(this, "runner") || !valid(this.runner))
      this:configure();
    endif
    const runner = this.runner;
    const response = runner:run_once(prompt, false, principal);
    if (announce && this.runner == runner && typeof(response) == TYPE_STR
        && valid(destination) && this.location == destination && length(response) > 0)
      destination:announce($event:mk_say(this, this:name(), " says, \"", response, "\""));
    endif
    return response;
  endmethod

  method observer_status owner: ARCH_WIZARD
    "Return a compact observer diagnostics map.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    return ["enabled" -> this.enabled, "agent" -> this.agent, "runner" -> this.runner, "agent_valid" -> valid(this.agent), "runner_valid" -> valid(this.runner)];
  endmethod
endobject
