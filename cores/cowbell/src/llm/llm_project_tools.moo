object LLM_PROJECT_TOOLS [
  import_export_id -> "llm_project_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Project Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  method get_tools owner: ARCH_WIZARD
    "Return one bounded project catalog for wearable interactions, with explicit effect classes.";
    const text = ["type" -> "string", "maxLength" -> 4096];
    const label = ["type" -> "string", "maxLength" -> 256];
    const todo = ["type" -> "object", "properties" -> ["content" -> text,
      "status" -> ["type" -> "string", "enum" -> {"pending", "in_progress", "completed"}]],
      "required" -> {"content", "status"}, "additionalProperties" -> false];
    "Definitions contain name, description, fields, required fields, and effect class.";
    const definitions = {
      {"create_task", "Start and select a task for this device's project.",
        ["description" -> text], {"description"}, 'mutation},
      {"record_finding", "Record a finding in the selected task. Subjects and keys are case-sensitive.",
        ["subject" -> label, "key" -> label, "value" -> text], {"subject", "key", "value"}, 'mutation},
      {"get_findings", "Read the selected task's findings for an exact subject.",
        ["subject" -> label], {"subject"}, 'inspection},
      {"task_status", "Read the selected task's status and findings count.", [], {}, 'inspection},
      {"complete_task", "Complete the selected task with a result summary.",
        ["result" -> ["type" -> "string", "maxLength" -> 8192]], {"result"}, 'mutation},
      {"todo_write", "Replace this device's project todo list.",
        ["todos" -> ["type" -> "array", "items" -> todo, "maxItems" -> 128]], {"todos"}, 'mutation},
      {"get_todos", "Read this device's project todo list.", [], {}, 'inspection}
    };
    let tools = {};
    for definition in (definitions)
      const {name, description, fields, required, effect} = definition;
      const schema = ["type" -> "object", "properties" -> fields,
        "required" -> required, "additionalProperties" -> false];
      tools = {@tools, $llm_agent_tool:mk(name, description, schema, this, name, effect)};
    endfor
    return tools;
  endmethod

  method _project owner: ARCH_WIZARD
    "Resolve the captured agent's project only for a live, authenticated interaction.";
    const {actor, principal} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    $llm_tool_provider:check(this, actor, principal);
    this.session_bound || raise(E_PERM, "Project tools require a wearable interaction.");
    const agent = this.bound_session.agent;
    set_task_perms(actor);
    return agent:get_project();
  endmethod

  method create_task owner: ARCH_WIZARD
    "Create and select a bounded task in this interaction's project.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    const task = project:start_task(parameters["description"]);
    const status = task:get_status();
    return "Task #" + tostr(status["task_id"]) + " started: " + status["description"];
  endmethod

  method record_finding owner: ARCH_WIZARD
    "Record one bounded finding in the selected task.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    const task = project:active_task();
    valid(task) || raise(E_INVARG, "No active task; create one first.");
    task:add_finding(parameters["subject"], parameters["key"], parameters["value"]);
    return "Finding recorded.";
  endmethod

  method get_findings owner: ARCH_WIZARD
    "Return the selected task's exact-subject findings as JSON.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    const task = project:active_task();
    valid(task) || raise(E_INVARG, "No active task; create one first.");
    return generate_json(task:get_findings(parameters["subject"]));
  endmethod

  method task_status owner: ARCH_WIZARD
    "Format the selected task with the shared project renderer.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    return project:format_active();
  endmethod

  method complete_task owner: ARCH_WIZARD
    "Complete the selected task and retain its result in the project.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    const id = project:complete_active(parameters["result"]);
    return id == 0 ? "No active task." | "Task #" + tostr(id) + " completed.";
  endmethod

  method todo_write owner: ARCH_WIZARD
    "Validate and replace the captured project's todo list.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    const statuses = ["pending" -> 'pending, "in_progress" -> 'in_progress, "completed" -> 'completed];
    const items = parameters["todos"];
    typeof(items) == TYPE_LIST && length(items) <= 128 || raise(E_INVARG);
    let todos = {};
    for item in (items)
      const content = $llm_project:text(item["content"], 4096);
      const status = item["status"];
      maphaskey(statuses, status) || raise(E_INVARG, "Invalid todo status.");
      todos = {@todos, ["content" -> content, "status" -> statuses[status]]};
    endfor
    set_task_perms(actor);
    project:set_todos(todos);
    return tostr(length(todos)) + " todos set:\n" + project:format_todos();
  endmethod

  method get_todos owner: ARCH_WIZARD
    "Format the captured project's todo list.";
    const {parameters, actor} = args;
    const project = this:_project(actor, caller_perms());
    set_task_perms(actor);
    return project:format_todos();
  endmethod
endobject
