object LLM_TASK [
  import_export_id -> "llm_task",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Task"
  parent: ROOT
  owner: ARCH_WIZARD

  property project (owner: ARCH_WIZARD, flags: "") = #-1;
  property task_id (owner: ARCH_WIZARD, flags: "") = 0;
  property parent_task_id (owner: ARCH_WIZARD, flags: "") = 0;
  property summary (owner: ARCH_WIZARD, flags: "") = "";
  property status (owner: ARCH_WIZARD, flags: "") = 'pending;
  property created_at (owner: ARCH_WIZARD, flags: "") = 0;
  property started_at (owner: ARCH_WIZARD, flags: "") = 0;
  property completed_at (owner: ARCH_WIZARD, flags: "") = 0;
  property result (owner: ARCH_WIZARD, flags: "") = "";
  property error (owner: ARCH_WIZARD, flags: "") = "";
  property findings (owner: ARCH_WIZARD, flags: "") = {};

  method check owner: ARCH_WIZARD
    "Authenticate task access and require current project membership, including after ownership changes.";
    const {task, principal} = args;
    typeof(task) == TYPE_OBJ && valid(task) && isa(task, $llm_task) || raise(E_TYPE);
    !is_clear_property(task, "project") && valid(task.project) || raise(E_PERM, "Task is no longer tracked.");
    $llm_project:check(task.project, principal);
    maphaskey(task.project.tasks, task.task_id) && task.project.tasks[task.task_id] == task
      || raise(E_PERM, "Task is no longer tracked.");
  endmethod

  method get_status owner: ARCH_WIZARD
    "Return a CoW task status snapshot to the project owner or wizard.";
    $llm_task:check(this, caller_perms());
    const children = this.project:children(this.task_id);
    return ["task_id" -> this.task_id, "description" -> this.summary, "status" -> this.status,
      "parent_task_id" -> this.parent_task_id, "created_at" -> this.created_at,
      "started_at" -> this.started_at, "completed_at" -> this.completed_at,
      "result" -> this.result, "error" -> this.error, "subtask_count" -> length(children),
      "finding_count" -> length(this.findings)];
  endmethod

  method _transition owner: ARCH_WIZARD
    "Apply a validated task transition; completed and failed tasks cannot be reopened.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    const {status, ?text = ""} = args;
    status in {'in_progress, 'blocked, 'completed, 'failed} || raise(E_INVARG);
    !(this.status in {'completed, 'failed}) || raise(E_INVARG, "Task is already finished.");
    $llm_project:text(text, 8192);
    this.status = status;
    if (status == 'in_progress)
      this.started_at == 0 && (this.started_at = time());
      this.error = "";
    elseif (status == 'completed)
      this.result = text;
      this.error = "";
      this.completed_at = time();
    else
      this.error = text;
      status == 'failed && (this.completed_at = time());
    endif
    return true;
  endmethod

  method mark_in_progress owner: ARCH_WIZARD
    "Set task status through the shared transition rules.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    set_task_perms(principal);
    return this:_transition('in_progress);
  endmethod

  method mark_complete owner: ARCH_WIZARD
    "Set task status through the shared transition rules.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    const {text} = args;
    set_task_perms(principal);
    return this:_transition('completed, text);
  endmethod

  method mark_failed owner: ARCH_WIZARD
    "Set task status through the shared transition rules.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    const {text} = args;
    set_task_perms(principal);
    return this:_transition('failed, text);
  endmethod

  method mark_blocked owner: ARCH_WIZARD
    "Set task status through the shared transition rules.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    const {text} = args;
    set_task_perms(principal);
    return this:_transition('blocked, text);
  endmethod

  method add_finding owner: ARCH_WIZARD
    "Add a distinct finding with task provenance; cap each task at 64 findings and 32 KiB.";
    $llm_task:check(this, caller_perms());
    const {subject, key, value} = args;
    $llm_project:text(subject, 256);
    $llm_project:text(key, 256);
    value_bytes(value) <= 8192 || raise(E_QUOTA, "Finding value exceeds 8 KiB.");
    $llm_value:validate_json(value);
    const finding = {this.task_id, subject, key, value};
    for existing in (this.findings)
      strcmp(toliteral(existing), toliteral(finding)) == 0 && return false;
    endfor
    const findings = {@this.findings, finding};
    length(findings) <= 64 && value_bytes(findings) <= 32768 || raise(E_QUOTA, "LLM task findings are full.");
    this.findings = findings;
    return true;
  endmethod

  method get_findings owner: ARCH_WIZARD
    "Return exact-subject findings with their task IDs; subject matching is case-sensitive.";
    $llm_task:check(this, caller_perms());
    const {subject} = args;
    $llm_project:text(subject, 256);
    return {finding for finding in (this.findings) if (strcmp(finding[2], subject) == 0)};
  endmethod

  method remove_finding owner: ARCH_WIZARD
    "Remove an exact finding; return false if absent.";
    $llm_task:check(this, caller_perms());
    const {subject, key, value} = args;
    const expected = toliteral({this.task_id, subject, key, value});
    const previous = length(this.findings);
    this.findings = {finding for finding in (this.findings) if (strcmp(toliteral(finding), expected) != 0)};
    return length(this.findings) < previous;
  endmethod

  method add_subtask owner: ARCH_WIZARD
    "Create one child in the same project, without retaining duplicate task references.";
    const principal = caller_perms();
    $llm_task:check(this, principal);
    const {description} = args;
    const project = this.project;
    const id = this.task_id;
    set_task_perms(principal);
    return project:create_task(description, id);
  endmethod

  method test_task_lifecycle owner: ARCH_WIZARD
    "Task records preserve progress, completion, timestamps, and parent identity.";
    const agent = $llm_agent:create(true);
    const project = agent:get_project();
    const task = project:create_task("Test task");
    const initial = task:get_status();
    $test_utils:assert_eq(initial["status"], 'pending, "New tasks are pending");
    task:mark_in_progress();
    const started = task:get_status()["started_at"];
    $test_utils:assert_true(started > 0, "Starting records a timestamp");
    task:mark_blocked("Waiting for source");
    task:mark_in_progress();
    $test_utils:assert_eq(task:get_status()["started_at"], started, "Resume preserves the start time");
    task:mark_complete("Success!");
    const completed = task:get_status();
    $test_utils:assert_eq(completed["status"], 'completed, "Completion is recorded");
    $test_utils:assert_eq(completed["result"], "Success!", "Completion retains the result");
    $test_utils:assert_true(completed["completed_at"] > 0, "Completion records a timestamp");
    return true;
  endmethod

  method test_findings owner: ARCH_WIZARD
    "Findings retain exact subject matching, deduplication, JSON values, and task provenance.";
    const agent = $llm_agent:create(true);
    const task = agent:get_project():create_task("Inspect database");
    task:add_finding("database", "tables", {"users", "posts"});
    $test_utils:assert_false(task:add_finding("database", "tables", {"users", "posts"}), "Repeated findings do not grow state");
    task:add_finding("Database", "tables", {"Users"});
    const findings = task:get_findings("database");
    $test_utils:assert_eq(length(findings), 1, "Subjects are case-sensitive");
    $test_utils:assert_eq(findings[1][1], task:get_status()["task_id"], "Findings retain task provenance");
    $test_utils:assert_true(task:remove_finding("database", "tables", {"users", "posts"}), "Owners can correct findings");
    $test_utils:assert_eq(task:get_findings("database"), {}, "Removal releases the finding");
    return true;
  endmethod

endobject
