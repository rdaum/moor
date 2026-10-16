object LLM_RUNNER [
  import_export_id -> "llm_runner",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Runner"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property agent (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property enabled (owner: ARCH_WIZARD, flags: "rc") = true;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Runtime adapter that binds an agent to an event source/sink.";

  method attach_agent owner: ARCH_WIZARD
    "Attach a specific agent instance to this runner.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    const {agent_obj} = args;
    typeof(agent_obj) == TYPE_OBJ && valid(agent_obj) || raise(E_INVARG, "agent_obj must be valid object");
    isa(agent_obj, $llm_agent) || raise(E_TYPE);
    $llm_agent:_challenge_permissions(agent_obj, principal);
    set_task_perms(principal);
    this.agent = agent_obj;
    return this.agent;
  endmethod

  method run_once owner: ARCH_WIZARD
    "Run one prompt through attached agent.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    this.enabled || return "Runner disabled.";
    valid(this.agent) || raise(E_INVARG, "No agent attached");
    const {prompt, ?opts = false, ?request_principal = principal} = args;
    typeof(request_principal) == TYPE_OBJ && valid(request_principal) || raise(E_PERM);
    request_principal == principal || principal.wizard || raise(E_PERM);
    const agent = this.agent;
    isa(agent, $llm_agent) || raise(E_TYPE);
    $llm_agent:_challenge_permissions(agent, principal);
    set_task_perms(principal);
    return agent:send_message(prompt, opts, request_principal);
  endmethod

  method status owner: ARCH_WIZARD
    "Return runner status map.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    return ["enabled" -> this.enabled, "agent" -> this.agent, "agent_valid" -> valid(this.agent)];
  endmethod
endobject
