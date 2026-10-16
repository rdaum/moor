object LLM_PROJECT [
  import_export_id -> "llm_project",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Project"
  parent: ROOT
  owner: ARCH_WIZARD

  property subject (owner: ARCH_WIZARD, flags: "") = #-1;
  property tasks (owner: ARCH_WIZARD, flags: "") = [];
  property todos (owner: ARCH_WIZARD, flags: "") = {};
  property next_task_id (owner: ARCH_WIZARD, flags: "") = 1;
  property next_todo_id (owner: ARCH_WIZARD, flags: "") = 1;
  property active_task_id (owner: ARCH_WIZARD, flags: "") = 0;

  method new owner: ARCH_WIZARD
    "Create independent anonymous project state for an owned subject.";
    const {subject} = args;
    $llm_agent:_challenge_permissions(subject, caller_perms());
    const project = create($llm_project, $arch_wizard, 1);
    project.subject = subject;
    project.tasks = [];
    project.todos = {};
    return project;
  endmethod

  method check owner: ARCH_WIZARD
    "Authenticate the actual principal against the project subject on every operation.";
    const {project, principal} = args;
    typeof(project) == TYPE_OBJ && valid(project) && isa(project, $llm_project) || raise(E_TYPE);
    !is_clear_property(project, "subject") && valid(project.subject) || raise(E_PERM);
    $llm_agent:_challenge_permissions(project.subject, principal);
  endmethod

  method text owner: ARCH_WIZARD
    "Validate bounded project text; limits count retained bytes.";
    const {value, limit} = args;
    typeof(value) == TYPE_STR || raise(E_TYPE);
    value_bytes(value) <= limit || raise(E_QUOTA, "LLM project text exceeds its limit.");
    return value;
  endmethod

  method add_todo owner: ARCH_WIZARD
    "Append a pending todo; retain at most 128 todos and 64 KiB.";
    $llm_project:check(this, caller_perms());
    const {content} = args;
    $llm_project:text(content, 4096);
    const id = this.next_todo_id;
    id <= 2147483647 || raise(E_QUOTA, "Todo IDs exhausted.");
    const todos = {@this.todos, ["id" -> id, "content" -> content, "status" -> 'pending]};
    length(todos) <= 128 && value_bytes(todos) <= 65536 || raise(E_QUOTA, "LLM todo list is full.");
    this.todos = todos;
    this.next_todo_id = id + 1;
    return id;
  endmethod

  method set_todos owner: ARCH_WIZARD
    "Validate a replacement completely before publishing its list and next ID.";
    $llm_project:check(this, caller_perms());
    const {items} = args;
    typeof(items) == TYPE_LIST || raise(E_TYPE);
    length(items) <= 128 && value_bytes(items) <= 65536 || raise(E_QUOTA, "LLM todo list is full.");
    let next = this.next_todo_id;
    let result = {};
    let ids = {};
    for item in (items)
      typeof(item) == TYPE_MAP || raise(E_TYPE);
      const content = $llm_project:text(item["content"], 4096);
      const status = item["status"];
      status in {'pending, 'in_progress, 'completed} || raise(E_INVARG, "Invalid todo status.");
      const id = maphaskey(item, "id") ? item["id"] | next;
      typeof(id) == TYPE_INT && id > 0 && id <= 2147483647 || raise(E_INVARG, "Todo IDs must be positive integers.");
      !(id in ids) || raise(E_INVARG, "Duplicate todo ID.");
      ids = {@ids, id};
      next = max(next, id + 1);
      result = {@result, ["id" -> id, "content" -> content, "status" -> status]};
    endfor
    value_bytes(result) <= 65536 || raise(E_QUOTA, "LLM todo list is full.");
    this.todos = result;
    this.next_todo_id = next;
    return true;
  endmethod

  method update_todo owner: ARCH_WIZARD
    "Update a known todo status without changing its identity or content.";
    $llm_project:check(this, caller_perms());
    const {id, status} = args;
    typeof(id) == TYPE_INT || raise(E_TYPE);
    status in {'pending, 'in_progress, 'completed} || raise(E_INVARG, "Invalid todo status.");
    let found = false;
    let result = {};
    for item in (this.todos)
      if (item["id"] == id)
        item["status"] = status;
        found = true;
      endif
      result = {@result, item};
    endfor
    found || raise(E_INVARG, "Todo not found.");
    this.todos = result;
    return true;
  endmethod

  method get_todos owner: ARCH_WIZARD
    "Return a CoW snapshot, optionally filtered by status.";
    $llm_project:check(this, caller_perms());
    const {?status = false} = args;
    status == false && return this.todos;
    status in {'pending, 'in_progress, 'completed} || raise(E_INVARG, "Invalid todo status.");
    return {item for item in (this.todos) if (item["status"] == status)};
  endmethod

  method remove_todo owner: ARCH_WIZARD
    "Remove one todo; return false when its ID is absent.";
    $llm_project:check(this, caller_perms());
    const {id} = args;
    typeof(id) == TYPE_INT || raise(E_TYPE);
    const previous = length(this.todos);
    this.todos = {item for item in (this.todos) if (item["id"] != id)};
    return length(this.todos) < previous;
  endmethod

  method clear_completed_todos owner: ARCH_WIZARD
    "Release completed todo records.";
    $llm_project:check(this, caller_perms());
    this.todos = {item for item in (this.todos) if (item["status"] != 'completed)};
    return true;
  endmethod

  method format_todos owner: ARCH_WIZARD
    "Render the private todo snapshot as literal text.";
    $llm_project:check(this, caller_perms());
    length(this.todos) == 0 && return "No todos.";
    let lines = {};
    for todo in (this.todos)
      const prefix = todo["status"] == 'completed ? "[x]" | todo["status"] == 'in_progress ? "[>]" | "[ ]";
      lines = {@lines, prefix + " " + todo["content"]};
    endfor
    return lines:join("\n");
  endmethod

  method create_task owner: ARCH_WIZARD
    "Create a bounded anonymous task; an optional parent must belong to this project.";
    $llm_project:check(this, caller_perms());
    const {description, ?parent_id = 0} = args;
    $llm_project:text(description, 4096);
    typeof(parent_id) == TYPE_INT || raise(E_TYPE);
    parent_id == 0 || maphaskey(this.tasks, parent_id) || raise(E_INVARG, "Parent task not found.");
    if (parent_id != 0)
      !(this.tasks[parent_id].status in {'completed, 'failed}) || raise(E_INVARG, "Parent task is finished.");
    endif
    length(this.tasks) < 64 || raise(E_QUOTA, "LLM project has 64 tasks; remove finished work first.");
    const id = this.next_task_id;
    id <= 2147483647 || raise(E_QUOTA, "Task IDs exhausted.");
    const task = create($llm_task, $arch_wizard, 1);
    task.project = this;
    task.task_id = id;
    task.parent_task_id = parent_id;
    task.summary = description;
    task.created_at = time();
    task.status = 'pending;
    task.findings = {};
    this.tasks[id] = task;
    this.next_task_id = id + 1;
    return task;
  endmethod

  method start_task owner: ARCH_WIZARD
    "Create and select a top-level task for a wearable frontend.";
    const principal = caller_perms();
    $llm_project:check(this, principal);
    const task = this:create_task(@args);
    task:mark_in_progress();
    const id = task:get_status()["task_id"];
    this.active_task_id = id;
    return task;
  endmethod

  method get_task owner: ARCH_WIZARD
    "Return a tracked task or #-1 when absent.";
    $llm_project:check(this, caller_perms());
    const {id} = args;
    typeof(id) == TYPE_INT || raise(E_TYPE);
    return `this.tasks[id] ! E_RANGE => #-1';
  endmethod

  method active_task owner: ARCH_WIZARD
    "Return the selected task or #-1 when none remains.";
    $llm_project:check(this, caller_perms());
    return `this.tasks[this.active_task_id] ! E_RANGE => #-1';
  endmethod

  method complete_active owner: ARCH_WIZARD
    "Complete the selected task and clear the selection; return its ID or zero.";
    $llm_project:check(this, caller_perms());
    const {?result = "Task completed."} = args;
    !maphaskey(this.tasks, this.active_task_id) && return 0;
    const id = this.active_task_id;
    this.tasks[id]:mark_complete(result);
    this.active_task_id = 0;
    return id;
  endmethod

  method get_task_status owner: ARCH_WIZARD
    "Return status snapshots for all retained tasks in ID order.";
    $llm_project:check(this, caller_perms());
    return {this.tasks[id]:get_status() for id in (sort(mapkeys(this.tasks)))};
  endmethod

  method children owner: ARCH_WIZARD
    "Return direct child IDs for a tracked parent.";
    $llm_project:check(this, caller_perms());
    const {id} = args;
    maphaskey(this.tasks, id) || raise(E_INVARG, "Task not found.");
    return {child for child in (sort(mapkeys(this.tasks))) if (this.tasks[child].parent_task_id == id)};
  endmethod

  method remove_task owner: ARCH_WIZARD
    "Remove a task and its descendants, release findings, and revoke retained task handles.";
    $llm_project:check(this, caller_perms());
    const {id} = args;
    typeof(id) == TYPE_INT || raise(E_TYPE);
    !maphaskey(this.tasks, id) && return false;
    let removed = {id};
    for candidate in (sort(mapkeys(this.tasks)))
      this.tasks[candidate].parent_task_id in removed && (removed = setadd(removed, candidate));
    endfor
    for removed_id in (removed)
      const task = this.tasks[removed_id];
      task.findings = {};
      task.result = "";
      task.error = "";
      task.summary = "";
      task.project = #-1;
      this.tasks = mapdelete(this.tasks, removed_id);
    endfor
    this.active_task_id in removed && (this.active_task_id = 0);
    return true;
  endmethod

  method clear owner: ARCH_WIZARD
    "Release all project work and revoke task handles; IDs are never reused.";
    $llm_project:check(this, caller_perms());
    for id in (mapkeys(this.tasks))
      this:remove_task(id);
    endfor
    this.todos = {};
    this.active_task_id = 0;
    return true;
  endmethod

  method format_active owner: ARCH_WIZARD
    "Render the selected task as literal text; absent selection returns a short message.";
    $llm_project:check(this, caller_perms());
    !maphaskey(this.tasks, this.active_task_id) && return "No active task.";
    const status = this.tasks[this.active_task_id]:get_status();
    let lines = {"Task #" + tostr(status["task_id"]) + ": " + status["description"],
      "Status: " + tostr(status["status"]), "Findings: " + tostr(status["finding_count"]),
      "Subtasks: " + tostr(status["subtask_count"])};
    status["status"] == 'completed && (lines = {@lines, "Result: " + status["result"]});
    status["status"] in {'blocked, 'failed} && (lines = {@lines, "Reason: " + status["error"]});
    return lines:join("\n");
  endmethod

  method test_todo_lifecycle owner: HACKER
    "Test basic todo operations.";
    const subject = $llm_agent:create(true);
    const agent = subject:get_project();
    "Add todos";
    const id1 = agent:add_todo("First task");
    const id2 = agent:add_todo("Second task");
    let todos = agent:get_todos();
    length(todos) != 2 && raise(E_ASSERT, "Should have 2 todos");
    todos[1]["status"] != 'pending && raise(E_ASSERT, "New todo should be pending");
    "Update status";
    agent:update_todo(id1, 'in_progress);
    todos = agent:get_todos();
    todos[1]["status"] != 'in_progress && raise(E_ASSERT, "Should be in_progress");
    "Complete and clear";
    agent:update_todo(id1, 'completed);
    agent:clear_completed_todos();
    todos = agent:get_todos();
    length(todos) != 1 && raise(E_ASSERT, "Should have 1 todo after clear");
    todos[1]["id"] != id2 && raise(E_ASSERT, "Remaining todo should be id2");
    "Remove";
    agent:remove_todo(id2);
    todos = agent:get_todos();
    length(todos) != 0 && raise(E_ASSERT, "Should have 0 todos");
    return true;
  endmethod

  method test_todo_filter owner: HACKER
    "Test todo filtering by status.";
    const subject = $llm_agent:create(true);
    const agent = subject:get_project();
    agent:add_todo("Pending 1");
    const id2 = agent:add_todo("In progress");
    agent:add_todo("Pending 2");
    agent:update_todo(id2, 'in_progress);
    const pending = agent:get_todos('pending);
    length(pending) != 2 && raise(E_ASSERT, "Should have 2 pending");
    const in_prog = agent:get_todos('in_progress);
    length(in_prog) != 1 && raise(E_ASSERT, "Should have 1 in_progress");
    return true;
  endmethod

  method test_set_todos owner: HACKER
    "Test replacing entire todo list.";
    const subject = $llm_agent:create(true);
    const agent = subject:get_project();
    agent:set_todos({["content" -> "Task A", "status" -> 'pending], ["content" -> "Task B", "status" -> 'in_progress], ["content" -> "Task C", "status" -> 'completed]});
    const todos = agent:get_todos();
    length(todos) != 3 && raise(E_ASSERT, "Should have 3 todos");
    todos[2]["status"] != 'in_progress && raise(E_ASSERT, "Second should be in_progress");
    return true;
  endmethod

endobject
