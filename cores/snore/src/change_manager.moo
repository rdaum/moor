object CHANGE_MANAGER [
  import_export_id -> "change_manager"
]
  name: "Change Manager"
  parent: ROOT_CLASS
  owner: #2

  property default_package (owner: #2, flags: "") = "snore";
  property next_review (owner: #2, flags: "") = 1;
  property packages (owner: #2, flags: "") = [
    "snore" -> [
      "active" -> 0,
      "constants" -> [
        "BUILD_OPTIONS" -> BUILD_OPTIONS,
        "BUILDER" -> BUILDER,
        "BUILDER_FEATURE" -> BUILDER_FEATURE,
        "BUILDER_HELP" -> BUILDER_HELP,
        "BUILDING_UTILS" -> BUILDING_UTILS,
        "BUILTIN_FUNCTION_HELP" -> BUILTIN_FUNCTION_HELP,
        "BYTE_QUOTA_UTILS" -> BYTE_QUOTA_UTILS,
        "CHANGE_MANAGER" -> CHANGE_MANAGER,
        "CODE_UTILS" -> CODE_UTILS,
        "COMMAND_UTILS" -> COMMAND_UTILS,
        "CONTAINER" -> CONTAINER,
        "CONVERT_UTILS" -> CONVERT_UTILS,
        "CORE_HELP" -> CORE_HELP,
        "DEFAULT_GUEST" -> DEFAULT_GUEST,
        "DEFAULT_PLAYER" -> DEFAULT_PLAYER,
        "DEFAULT_PLAYER_HELP" -> DEFAULT_PLAYER_HELP,
        "DISPLAY_OPTIONS" -> DISPLAY_OPTIONS,
        "EDIT_OPTIONS" -> EDIT_OPTIONS,
        "EDITOR_HELP" -> EDITOR_HELP,
        "ERROR" -> ERROR,
        "EXIT" -> EXIT,
        "FEATURE" -> FEATURE,
        "FEATURE_WAREHOUSE" -> FEATURE_WAREHOUSE,
        "GARBAGE" -> GARBAGE,
        "GENDER_UTILS" -> GENDER_UTILS,
        "GENDERED_OBJECT" -> GENDERED_OBJECT,
        "GENERIC_EDITOR" -> GENERIC_EDITOR,
        "GENERIC_HELP" -> GENERIC_HELP,
        "GENERIC_OPTIONS" -> GENERIC_OPTIONS,
        "GENERIC_UTILS" -> GENERIC_UTILS,
        "GUEST" -> GUEST,
        "GUEST_LOG" -> GUEST_LOG,
        "HACKER" -> HACKER,
        "HELP" -> HELP,
        "HOUSEKEEPER" -> HOUSEKEEPER,
        "LAST_HUH" -> LAST_HUH,
        "LETTER" -> LETTER,
        "LIMBO" -> LIMBO,
        "LIST_EDITOR" -> LIST_EDITOR,
        "LIST_UTILS" -> LIST_UTILS,
        "LOCK_UTILS" -> LOCK_UTILS,
        "LOGIN" -> LOGIN,
        "MAIL_AGENT" -> MAIL_AGENT,
        "MAIL_EDITOR" -> MAIL_EDITOR,
        "MAIL_HELP" -> MAIL_HELP,
        "MAIL_OPTIONS" -> MAIL_OPTIONS,
        "MAIL_RECIPIENT" -> MAIL_RECIPIENT,
        "MAIL_RECIPIENT_CLASS" -> MAIL_RECIPIENT_CLASS,
        "MATCH_UTILS" -> MATCH_UTILS,
        "MATH_UTILS" -> MATH_UTILS,
        "MATRIX_UTILS" -> MATRIX_UTILS,
        "NEW_PLAYER_LOG" -> NEW_PLAYER_LOG,
        "NEW_PROG_LOG" -> NEW_PROG_LOG,
        "NEWS" -> NEWS,
        "NEWT_LOG" -> NEWT_LOG,
        "NO_ONE" -> NO_ONE,
        "NOTE" -> NOTE,
        "NOTE_EDITOR" -> NOTE_EDITOR,
        "OBJECT_QUOTA_UTILS" -> OBJECT_QUOTA_UTILS,
        "OBJECT_UTILS" -> OBJECT_UTILS,
        "PARANOID_DB" -> PARANOID_DB,
        "PASSWORD_VERIFIER" -> PASSWORD_VERIFIER,
        "PASTING_FEATURE" -> PASTING_FEATURE,
        "PERM_UTILS" -> PERM_UTILS,
        "PLAYER" -> PLAYER,
        "PLAYER_DB" -> PLAYER_DB,
        "PLAYER_START" -> PLAYER_START,
        "PROG" -> PROG,
        "PROG_HELP" -> PROG_HELP,
        "PROG_OPTIONS" -> PROG_OPTIONS,
        "PROGRAMMER_FEATURE" -> PROGRAMMER_FEATURE,
        "QUOTA_LOG" -> QUOTA_LOG,
        "RECYCLER" -> RECYCLER,
        "REGISTRATION_DB" -> REGISTRATION_DB,
        "ROOM" -> ROOM,
        "ROOT_CLASS" -> ROOT_CLASS,
        "SEQ_UTILS" -> SEQ_UTILS,
        "SERVER_OPTIONS" -> SERVER_OPTIONS,
        "SET_UTILS" -> SET_UTILS,
        "SITE_DB" -> SITE_DB,
        "SPELL" -> SPELL,
        "STAGE_TALK" -> STAGE_TALK,
        "STRING_UTILS" -> STRING_UTILS,
        "SYSOBJ" -> SYSOBJ,
        "THING" -> THING,
        "TIME_UTILS" -> TIME_UTILS,
        "UTILITY_FEATURE" -> UTILITY_FEATURE,
        "VERB_EDITOR" -> VERB_EDITOR,
        "VERB_HELP" -> VERB_HELP,
        "WIZ" -> WIZ,
        "WIZ_HELP" -> WIZ_HELP,
        "WIZ_UTILS" -> WIZ_UTILS,
        "WIZARD_FEATURE" -> WIZARD_FEATURE,
        "YOU" -> YOU
      ],
      "fields" -> {"program"},
      "generation" -> 1,
      "objects" -> {
        BUILD_OPTIONS,
        BUILDER,
        BUILDER_FEATURE,
        BUILDER_HELP,
        BUILDING_UTILS,
        BUILTIN_FUNCTION_HELP,
        BYTE_QUOTA_UTILS,
        CHANGE_MANAGER,
        CODE_UTILS,
        COMMAND_UTILS,
        CONTAINER,
        CONVERT_UTILS,
        CORE_HELP,
        DEFAULT_GUEST,
        DEFAULT_PLAYER,
        DEFAULT_PLAYER_HELP,
        DISPLAY_OPTIONS,
        EDIT_OPTIONS,
        EDITOR_HELP,
        ERROR,
        EXIT,
        FEATURE,
        FEATURE_WAREHOUSE,
        GARBAGE,
        GENDER_UTILS,
        GENDERED_OBJECT,
        GENERIC_EDITOR,
        GENERIC_HELP,
        GENERIC_OPTIONS,
        GENERIC_UTILS,
        GUEST,
        GUEST_LOG,
        HACKER,
        HELP,
        HOUSEKEEPER,
        LAST_HUH,
        LETTER,
        LIMBO,
        LIST_EDITOR,
        LIST_UTILS,
        LOCK_UTILS,
        LOGIN,
        MAIL_AGENT,
        MAIL_EDITOR,
        MAIL_HELP,
        MAIL_OPTIONS,
        MAIL_RECIPIENT,
        MAIL_RECIPIENT_CLASS,
        MATCH_UTILS,
        MATH_UTILS,
        MATRIX_UTILS,
        NEW_PLAYER_LOG,
        NEW_PROG_LOG,
        NEWS,
        NEWT_LOG,
        NO_ONE,
        NOTE,
        NOTE_EDITOR,
        OBJECT_QUOTA_UTILS,
        OBJECT_UTILS,
        PARANOID_DB,
        PASSWORD_VERIFIER,
        PASTING_FEATURE,
        PERM_UTILS,
        PLAYER,
        #2,
        #96,
        PLAYER_DB,
        PLAYER_START,
        PROG,
        PROG_HELP,
        PROG_OPTIONS,
        PROGRAMMER_FEATURE,
        QUOTA_LOG,
        RECYCLER,
        REGISTRATION_DB,
        ROOM,
        ROOT_CLASS,
        SEQ_UTILS,
        SERVER_OPTIONS,
        SET_UTILS,
        SITE_DB,
        SPELL,
        STAGE_TALK,
        STRING_UTILS,
        SYSOBJ,
        THING,
        TIME_UTILS,
        UTILITY_FEATURE,
        VERB_EDITOR,
        VERB_HELP,
        WIZ,
        WIZ_HELP,
        WIZ_UTILS,
        WIZARD_FEATURE,
        YOU
      },
      "schema" -> 1,
      "trusted_owners" -> {HACKER},
      "upstream" -> ""
    ]
  ];
  property pending (owner: #2, flags: "") = [];
  property receipts (owner: #2, flags: "") = [];

  method _error owner: #2
    "Raise a versioned service error.";
    caller == this || raise(E_PERM);
    const {code, message} = args;
    raise(E_INVARG, message, ["schema" -> 1, "code" -> code]);
  endmethod

  method _entry owner: #2
    "Authorize the invoking administrator without treating this verb's owner as the caller.";
    caller == this && this == $change_manager || raise(E_PERM);
    let {origin} = args;
    const actor = player;
    if (origin == #-1)
      "Top-level authenticated verb calls have no MOO caller; nested calls retain their original authority.";
      length(callers()) == 1 || raise(E_PERM);
      origin = actor;
    endif
    valid(actor) && valid(origin) || raise(E_PERM);
    const authority = actor;
    actor.wizard && origin == actor || raise(E_PERM);
    return {actor, authority};
  endmethod

  method _authorized owner: #2
    "Recheck persisted actor and authority after suspension or restart.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {actor, authority} = args;
    valid(actor) && valid(authority) && authority.wizard || raise(E_PERM);
    actor == authority && actor.wizard || raise(E_PERM);
    return true;
  endmethod

  method _get owner: #2
    "Read a pending record with owner and generation guards.";
    caller == this || raise(E_PERM);
    const {auth, id, ?generation = -1} = args;
    maphaskey(this.pending, id) || this:_error("missing_review", "No pending review with that ID.");
    const record = this.pending[id];
    auth[1] == record["actor"] || auth[1].wizard || raise(E_PERM);
    generation == -1 || generation == record["generation"] || this:_error("stale_generation", "Review changed; reload its status.");
    this:_authorized(@auth);
    return record;
  endmethod

  method _summary owner: #2
    "Return status without source, drafts, or program bodies.";
    caller == this || raise(E_PERM);
    const {record} = args;
    return ["schema" -> 1, "review_id" -> record["id"], "generation" -> record["generation"], "package" -> record["package"], "status" -> record["status"], "task" -> record["task"], "error" -> record["error"]];
  endmethod

  method capabilities owner: #2
    "Describe the versioned review API and its limits.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    return ["schema" -> 1, "operations" -> {"packages", "configure", "upstream", "stage", "status", "review", "inspection", "diagnostics", "details", "resolve", "apply", "refresh", "discard"}, "fields" -> {"program"}, "choices" -> {"incoming", "local", "edited", "defer"}, "transports" -> {"upload", "http"}, "authorization" -> "administrator", "default_package" -> this.default_package, "max_source_bytes" -> 4194304, "max_pending_bytes" -> 33554432, "max_packages" -> 32, "page_rows" -> 50, "max_detail_bytes" -> 524288, "max_page_bytes" -> 262144, "receipt_decisions" -> 50];
  endmethod

  method packages owner: #2
    "Return package bindings, policy, upstream settings, and active review IDs.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    return ["schema" -> 1, "packages" -> this.packages];
  endmethod

  method configure owner: #2
    "Create or conditionally replace explicit package bindings and program-only policy.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name, objects, constants, ?url = "", ?expected = 0, ?trusted_owners = {}} = args;
    typeof(name) == TYPE_STR && length(name) > 0 && length(name) <= 64 || raise(E_INVARG);
    typeof(objects) == TYPE_LIST && length(objects) > 0 && length(objects) <= 4096 || raise(E_INVARG);
    typeof(constants) == TYPE_MAP && value_bytes(constants) <= 1048576 || raise(E_INVARG);
    typeof(trusted_owners) == TYPE_LIST && length(trusted_owners) <= 128 || raise(E_INVARG);
    let owner_set = {};
    for owner in (trusted_owners)
      typeof(owner) == TYPE_OBJ && valid(owner) && !(owner in owner_set) || raise(E_INVARG);
      owner_set = {@owner_set, owner};
    endfor
    this:_url(url);
    let seen = {};
    for object in (objects)
      typeof(object) == TYPE_OBJ && valid(object) && !(object in seen) || raise(E_INVARG);
      seen = {@seen, object};
    endfor
    for other_name in (mapkeys(this.packages))
      if (other_name != name)
        for object in (this.packages[other_name]["objects"])
          !(object in seen) || this:_error("overlapping_package", "Another package already manages one of these objects.");
        endfor
      endif
    endfor
    let generation = 0;
    if (maphaskey(this.packages, name))
      const previous = this.packages[name];
      generation = previous["generation"];
      previous["active"] == 0 || this:_error("active_review", "Discard the active review before changing package settings.");
    else
      length(this.packages) < 32 || raise(E_QUOTA);
    endif
    generation == expected || this:_error("stale_generation", "Package settings changed.");
    const package = ["schema" -> 1, "generation" -> generation + 1, "objects" -> objects, "fields" -> {"program"}, "constants" -> constants, "trusted_owners" -> trusted_owners, "upstream" -> url, "active" -> 0];
    this.packages[name] = package;
    return package;
  endmethod

  method _url owner: #2
    "Accept a single HTTP text-bundle URL without embedded credentials.";
    caller == this || raise(E_PERM);
    const {url} = args;
    typeof(url) == TYPE_STR && length(url) <= 2048 || raise(E_INVARG);
    if (url != "")
      index(url, "http://") == 1 || index(url, "https://") == 1 && !index(url, "@") || raise(E_INVARG, "Expected an HTTP bundle URL without credentials.");
    endif
  endmethod

  method upstream owner: #2
    "Conditionally set the URL for one self-contained text bundle.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name, url, expected} = args;
    maphaskey(this.packages, name) || raise(E_INVARG);
    const package = this.packages[name];
    return this:configure(name, package["objects"], package["constants"], url, expected, package["trusted_owners"]);
  endmethod

  method _save_ready owner: #2
    "Validate exact staged input and save original evidence without applying changes.";
    caller == this || raise(E_PERM);
    let {record, sources, provenance} = args;
    value_bytes(sources) <= 4194304 || raise(E_QUOTA, "Source exceeds the service limit.");
    this:_authorized(record["actor"], record["authority"]);
    set_task_perms(record["authority"]);
    const package = this.packages[record["package"]];
    package["generation"] == record["package_generation"] && package["active"] == record["id"] || this:_error("stale_package", "Package settings changed during staging.");
    const report = preview_objdef_changes(sources, record["request"]);
    provenance["digest"] = report["source_digest"];
    record["sources"] = sources;
    record["report"] = report;
    record["provenance"] = provenance;
    record["status"] = "ready";
    record["error"] = [];
    record["task"] = 0;
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA, "Pending review storage is full.");
    this.pending[record["id"]] = record;
    return this:_summary(record);
  endmethod

  method stage owner: #2
    "Stage an uploaded source set, or start an HTTP fetch when sources are omitted.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name, ?sources = {}, ?operation = "update"} = args;
    operation in {"adopt", "update"} || raise(E_INVARG);
    maphaskey(this.packages, name) || this:_error("missing_package", "Configure package bindings first.");
    const package = this.packages[name];
    package["active"] == 0 || this:_error("active_review", "A review already exists. Refresh or discard it explicitly.");
    length(this.pending) < 8 || raise(E_QUOTA);
    typeof(sources) == TYPE_LIST || raise(E_TYPE);
    if (!sources && !package["upstream"])
      this:_error("missing_source", "Upload source units or configure an HTTP upstream.");
    endif
    const id = this.next_review;
    this.next_review = id + 1;
    let record = ["id" -> id, "generation" -> 1, "actor" -> auth[1], "authority" -> auth[2], "package" -> name, "package_generation" -> package["generation"], "status" -> "fetching", "task" -> 0, "error" -> [], "sources" -> {}, "report" -> [], "choices" -> [], "provenance" -> [], "request" -> ["schema" -> 1, "operation" -> operation, "objects" -> package["objects"], "fields" -> package["fields"], "constants" -> package["constants"], "trusted_owners" -> package["trusted_owners"]]];
    this.packages[name]["active"] = id;
    this.pending[id] = record;
    if (sources)
      try
        return this:_save_ready(record, sources, ["transport" -> "upload"]);
      except failure (ANY)
        record["status"] = "failed";
        record["error"] = typeof(failure[3]) == TYPE_MAP ? failure[3] | ["schema" -> 1, "code" -> "compile_failure", "message" -> tostr(failure[2])];
        this.pending[id] = record;
        return this:_summary(record);
      endtry
    endif
    const parent_task = task_id();
    fork fetch_task (0)
      this:_await_job(id, 1, parent_task);
      this:_fetch(id, 1);
    endfork
    record["task"] = fetch_task;
    this.pending[id] = record;
    return this:_summary(record);
  endmethod

  method _await_job owner: #2
    "Forks can start before the parent commits; wait for this job's persisted identity.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {id, generation, parent_task} = args;
    let parent_ended = false;
    for attempt in [1..1000]
      if (maphaskey(this.pending, id))
        const record = this.pending[id];
        if (record["generation"] == generation && record["task"] == task_id())
          return;
        endif
      endif
      !parent_ended || raise(E_INVARG, "Job was not committed by its parent.");
      parent_ended = !valid_task(parent_task);
      suspend(0.01);
    endfor
    raise(E_QUOTA, "Timed out waiting for the saved job.");
  endmethod

  method _job owner: #2
    "Validate a background task's saved identity, generation, package, and authority.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {id, generation, status} = args;
    maphaskey(this.pending, id) || raise(E_INVARG);
    const record = this.pending[id];
    record["generation"] == generation && record["status"] == status && record["task"] == task_id() || raise(E_PERM);
    this:_authorized(record["actor"], record["authority"]);
    set_task_perms(record["authority"]);
    const package = this.packages[record["package"]];
    package["active"] == id && package["generation"] == record["package_generation"] || raise(E_INVARG);
    return record;
  endmethod

  method _fetch owner: #2
    "Fetch one bounded UTF-8 bundle; recheck authorization and request identity after suspension.";
    caller == this || raise(E_PERM);
    const {id, generation} = args;
    let record = this:_job(id, generation, "fetching");
    set_task_perms(record["authority"]);
    try
      const url = this.packages[record["package"]]["upstream"];
      const response = worker_request("curl", {"GET", url, "", {}, ["max_bytes" -> 4194304, "strict_utf8" -> true, "include_url" -> true]}, ["timeout_seconds" -> 30.0]);
      record = this:_job(id, generation, "fetching");
      length(response) == 4 && response[1] == 200 || this:_error("http_failure", "HTTP bundle fetch did not return status 200.");
      let etag = "";
      for header in (response[2])
        if (header[1] == "etag")
          etag = header[2];
        endif
      endfor
      this:_save_ready(record, {["label" -> "http-bundle.moo", "text" -> response[3]]}, ["transport" -> "http", "url" -> response[4], "etag" -> etag]);
    except failure (ANY)
      if (maphaskey(this.pending, id) && this.pending[id]["generation"] == generation && this.pending[id]["task"] == task_id())
        record = this.pending[id];
        record["status"] = "failed";
        record["task"] = 0;
        record["error"] = typeof(failure[3]) == TYPE_MAP ? failure[3] | ["schema" -> 1, "code" -> "fetch_failed"];
        record["error"]["message"] = tostr(failure[2]);
        this.pending[id] = record;
      endif
    endtry
    if (maphaskey(this.pending, id) && this.pending[id]["generation"] == generation && this.pending[id]["status"] in {"ready", "failed"})
      `this:_notify_review(this.pending[id]) ! ANY';
    endif
  endmethod

  method status owner: #2
    "Return committed status and detect an interrupted fetch or apply task.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id} = args;
    if (maphaskey(this.receipts, id))
      const receipt = this.receipts[id];
      receipt["actor"] == auth[1] || auth[1].wizard || raise(E_PERM);
      return receipt;
    endif
    let record = this:_get(auth, id);
    if (record["status"] in {"fetching", "applying"} && !valid_task(record["task"]))
      const fetching = record["status"] == "fetching";
      record["status"] = fetching ? "failed" | "interrupted";
      record["task"] = 0;
      record["generation"] = record["generation"] + 1;
      record["error"] = ["schema" -> 1, "code" -> fetching ? "fetch_interrupted" | "interrupted", "message" -> fetching ? "Fetch ended without a result. Check worker availability, discard, and stage again." | "Task ended without a receipt. Refresh or discard this review."];
      this.pending[id] = record;
    endif
    let summary = this:_summary(record);
    if (maphaskey(record, "last_receipt"))
      summary["last_receipt"] = record["last_receipt"];
    endif
    return summary;
  endmethod

  method _review_decisions owner: #2
    "Count saved and default choices across the complete review, including pages not yet read.";
    caller == this || raise(E_PERM);
    const {record} = args;
    let selected = 0;
    let unresolved = 0;
    let blocked = 0;
    for row in (record["report"]["rows"])
      const saved = `record["choices"][row["id"]] ! E_RANGE => []';
      const kind = saved ? saved["choice"] | row["default"];
      if (!row["eligible"])
        blocked = blocked + (row["classification"] != "unchanged" ? 1 | 0);
      elseif (kind == "unresolved" || kind == "edited" && !maphaskey(saved, "validation"))
        unresolved = unresolved + 1;
      elseif (kind in {"incoming", "local", "edited"})
        selected = selected + 1;
      endif
    endfor
    return ["selected" -> selected, "unresolved" -> unresolved, "blocked" -> blocked];
  endmethod

  method review owner: #2
    "Return a bounded row page; the cursor binds review ID, generation, and offset.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?cursor = {}, ?classification = ""} = args;
    const record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    let offset = 1;
    if (cursor)
      length(cursor) == 3 && cursor[1] == id && cursor[2] == generation || this:_error("stale_cursor", "Cursor belongs to another review generation.");
      offset = cursor[3];
    endif
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    const rows = record["report"]["rows"];
    const names = this:_command_names();
    let page = {};
    let next_offset = offset;
    for i in [offset..length(rows)]
      next_offset = i + 1;
      let row = rows[i];
      if (classification == "changed" ? row["classification"] == "unchanged" | classification != "" && row["classification"] != classification)
        continue;
      endif
      const choice = `record["choices"][row["id"]] ! E_RANGE => []';
      row["label"] = this:_command_label(row["object"], names, row["names"][1]);
      row["choice"] = choice ? ["choice" -> choice["choice"], "validated" -> maphaskey(choice, "validation")] | [];
      if (value_bytes(page) + value_bytes(row) > 262144)
        length(page) > 0 || raise(E_QUOTA, "Row exceeds the page limit.");
        next_offset = i;
        break;
      endif
      page = {@page, row};
      if (length(page) == 50)
        break;
      endif
    endfor
    const next_cursor = next_offset <= length(rows) ? {id, generation, next_offset} | {};
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "total" -> length(rows), "rows" -> page, "cursor" -> next_cursor, "counts" -> record["report"]["counts"], "decision_counts" -> this:_review_decisions(record), "operation" -> record["request"]["operation"], "diagnostic_count" -> length(record["report"]["diagnostics"]), "diagnostics" -> record["report"]["diagnostics"][1..min(50, length(record["report"]["diagnostics"]))], "provenance" -> record["provenance"]];
  endmethod

  method diagnostics owner: #2
    "Return a bounded diagnostic page bound to the saved review generation.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?offset = 1, ?codes = {}} = args;
    const record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    typeof(offset) == TYPE_INT && offset >= 1 && typeof(codes) == TYPE_LIST || raise(E_INVARG);
    for code in (codes)
      typeof(code) == TYPE_STR || raise(E_INVARG);
    endfor
    const all = {item for item in (record["report"]["diagnostics"]) if (!codes || item["code"] in codes)};
    const names = this:_command_names();
    let page = {};
    for diagnostic in (all[offset..min(offset + 49, length(all))])
      if (maphaskey(diagnostic, "object"))
        const verb_name = diagnostic["code"] == "live_definition_unmatched" ? diagnostic["names"][1] | "";
        diagnostic["label"] = this:_command_label(diagnostic["object"], names, verb_name);
      endif
      page = {@page, diagnostic};
    endfor
    value_bytes(page) <= 262144 || raise(E_QUOTA, "Diagnostic page exceeds the service limit.");
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "total" -> length(all), "diagnostics" -> page, "next" -> offset + length(page) <= length(all) ? offset + length(page) | 0];
  endmethod

  method inspection owner: #2
    "Browse current local contents against the saved incoming source, without making update choices.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?offset = 1, ?classification = "", ?selected = "", ?expected = ""} = args;
    const record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    let request = record["request"];
    request["operation"] = "inspect";
    request["fields"] = {"object", "attribute", "property", "program"};
    request["details"] = selected ? {selected} | {};
    const report = preview_objdef_changes(record["sources"], request);
    !expected || expected == report["revision"] || this:_error("stale_inspection", "Local contents changed. Reload the review.");
    const all = {row for row in (report["rows"]) if ((!classification || row["classification"] == classification) && (!selected || row["id"] == selected))};
    const names = this:_command_names();
    let page = {};
    for row in (all[offset..min(offset + 49, length(all))])
      let name = row["name"];
      if (row["field"] == "program")
        name = name[index(name, ":") + 1..$];
        row["label"] = this:_command_label(row["object"], names, name);
      else
        row["label"] = this:_command_label(row["object"], names) + (name ? "." + name | "");
      endif
      page = {@page, row};
    endfor
    value_bytes(page) <= 524288 || raise(E_QUOTA, "Inspection page exceeds the service limit.");
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "rows" -> page, "revision" -> report["revision"], "counts" -> report["counts"], "total" -> length(all), "next" -> offset + length(page) <= length(all) ? offset + length(page) | 0];
  endmethod

  method details owner: #2
    "Return selected live/incoming text only while original review evidence still matches.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, row_id} = args;
    if (index(row_id, "inspect/") == 1)
      const result = this:inspection(id, generation, 1, "", row_id);
      length(result["rows"]) == 1 || this:_error("missing_row", "Inspection item no longer exists.");
      return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "row" -> result["rows"][1]];
    endif
    const record = this:_get(auth, id, generation);
    let request = record["request"];
    request["details"] = {row_id};
    const current = preview_objdef_changes(record["sources"], request);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before reading details.");
    for selected in (current["rows"])
      let row = selected;
      if (row["id"] == row_id)
        row["label"] = this:_command_label(row["object"], this:_command_names(), row["names"][1]);
        row["choice"] = `record["choices"][row_id] ! E_RANGE => []';
        value_bytes(row) <= 524288 || raise(E_QUOTA, "Selected details exceed the service limit.");
        return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "row" -> row];
      endif
    endfor
    this:_error("missing_row", "No such review row.");
  endmethod

  method resolve owner: #2
    "Save a conditional choice or edited draft, with native result validation.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, row_id, kind, ?program = ""} = args;
    let record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    let choice = ["choice" -> kind];
    if (kind == "edited")
      value_bytes(program) <= 524288 || raise(E_QUOTA);
      choice["program"] = program;
    endif
    record["choices"][row_id] = choice;
    const current = preview_objdef_changes(record["sources"], record["request"], record["choices"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before choosing.");
    for validation in (current["validation"])
      if (validation["id"] == row_id && validation["valid"])
        record["choices"][row_id]["validation"] = validation["validation"];
      endif
    endfor
    record["generation"] = generation + 1;
    record["status"] = "ready";
    record["error"] = [];
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA);
    this.pending[id] = record;
    return ["schema" -> 1, "review_id" -> id, "generation" -> record["generation"], "validation" -> current["validation"]];
  endmethod

  method refresh owner: #2
    "Create new evidence from saved input and discard all earlier choices.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    let record = this:_get(auth, id, generation);
    !(record["status"] in {"fetching", "applying"}) || this:_error("busy", "Task is still running.");
    length(record["sources"]) > 0 || this:_error("missing_source", "Discard this failed fetch and stage again.");
    record["choices"] = [];
    record["generation"] = generation + 1;
    return this:_save_ready(record, record["sources"], record["provenance"]);
  endmethod

  method apply owner: #2
    "Start one dedicated application task; success is reported only by a committed receipt.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    if (maphaskey(this.receipts, id))
      return this:status(id);
    endif
    let record = this:_get(auth, id);
    if (record["status"] == "applying")
      return this:_summary(record);
    endif
    record["generation"] == generation || this:_error("stale_generation", "Review choices changed.");
    record["status"] in {"ready", "partial"} || this:_error("not_ready", "Review is not ready.");
    record["generation"] = generation + 1;
    record["status"] = "applying";
    record["error"] = [];
    const parent_task = task_id();
    fork apply_task (0)
      this:_await_job(id, generation + 1, parent_task);
      this:_apply(id, generation + 1);
    endfork
    record["task"] = apply_task;
    this.pending[id] = record;
    return this:_summary(record);
  endmethod

  method _apply owner: #2
    "Commit content, baselines, and a source-free receipt together, or abort the task.";
    caller == this || raise(E_PERM);
    const {id, generation} = args;
    let record = this:_job(id, generation, "applying");
    set_task_perms(record["authority"]);
    let wrote = false;
    try
      const result = apply_objdef_changes(record["sources"], record["request"], record["report"]["evidence"], record["choices"]);
      wrote = true;
      let deferred = false;
      for diagnostic in (record["report"]["diagnostics"])
        if (diagnostic["code"] in {"missing_source", "unsupported_creation", "unsupported_or_ambiguous_definition"})
          deferred = true;
        endif
      endfor
      for row in (record["report"]["rows"])
        const kind = `record["choices"][row["id"]]["choice"] ! E_RANGE => row["default"]';
        if (kind == "defer" && (record["request"]["operation"] == "adopt" || row["classification"] in {"upstream", "conflict", "unbased"}))
          deferred = true;
        endif
      endfor
      record["generation"] = generation + 1;
      record["task"] = 0;
      record["choices"] = [];
      record["error"] = [];
      if (deferred)
        record["status"] = "partial";
        record["report"] = preview_objdef_changes(record["sources"], record["request"]);
        this.pending[id] = record;
      else
        record["status"] = "complete";
        this.pending = mapdelete(this.pending, id);
        this.packages[record["package"]]["active"] = 0;
      endif
      let receipt = ["schema" -> 1, "review_id" -> id, "generation" -> record["generation"], "package" -> record["package"], "status" -> record["status"], "task" -> 0, "error" -> []];
      receipt["actor"] = record["actor"];
      receipt["decision_count"] = length(result["decisions"]);
      receipt["decisions"] = result["decisions"][1..min(50, length(result["decisions"]))];
      receipt["source_digest"] = record["report"]["source_digest"];
      receipt["completed_at"] = time();
      if (deferred)
        this.pending[id]["last_receipt"] = receipt;
      else
        this.receipts[id] = receipt;
        if (length(this.receipts) > 64)
          this.receipts = mapdelete(this.receipts, mapkeys(this.receipts)[1]);
        endif
      endif
    except failure (ANY)
      if (wrote)
        rollback();
      endif
      record["status"] = "rejected";
      record["task"] = 0;
      record["generation"] = generation + 1;
      record["error"] = ["schema" -> 1, "code" -> "apply_rejected", "message" -> tostr(failure[2])];
      this.pending[id] = record;
    endtry
    `this:_notify_review(record) ! ANY';
  endmethod

  method discard owner: #2
    "Remove pending source and drafts with a generation check.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    const record = this:_get(auth, id, generation);
    record["status"] != "applying" || this:_error("busy", "Apply is running. Query status before discarding.");
    this.pending = mapdelete(this.pending, id);
    this.packages[record["package"]]["active"] = 0;
    return ["schema" -> 1, "review_id" -> id, "status" -> "discarded"];
  endmethod

  method _notify_review owner: #2
    "Deliver a terminal review result after the result task commits; output failure cannot undo it.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {record} = args;
    `record["notify"] ! E_RANGE => false' || return;
    const parent_task = task_id();
    fork delivery_task (0)
      let ended = false;
      for attempt in [1..1000]
        if (!valid_task(parent_task))
          ended = true;
          break;
        endif
        suspend(0.01);
      endfor
      ended || return;
      try
        const id = record["id"];
        const current = `this.pending[id] ! E_RANGE => `this.receipts[id] ! E_RANGE => []'';
        current && current["generation"] == record["generation"] && current["status"] == record["status"] || return;
        this:_authorized(record["actor"], record["authority"]);
        set_task_perms(record["authority"]);
        const lines = this:_command_status(this:_summary(record));
        record["actor"]:tell_lines(lines);
      except (ANY)
        "Notification is optional; the committed result remains available through status.";
      endtry
    endfork
  endmethod

  method _command_names owner: #2
    "Prefer system object references in command output.";
    caller == this || raise(E_PERM);
    let names = [];
    for name in (properties(#0))
      if (is_clear_property(#0, name))
        continue;
      endif
      const object = `#0.(name) ! E_PERM, E_PROPNF => false';
      if (typeof(object) == TYPE_OBJ && !maphaskey(names, object))
        names[object] = tostr("$", name);
      endif
    endfor
    return names;
  endmethod

  method _command_label owner: #2
    "Label an object or verb using its system reference when available.";
    caller == this || raise(E_PERM);
    const {object, names, ?verb_name = ""} = args;
    let label = `names[object] ! E_RANGE => tostr(object)';
    if (verb_name)
      label = tostr(label, ":", verb_name);
      return label;
    endif
    if (!maphaskey(names, object))
      const name = `tostr(object.name) ! ANY => ""';
      name && (label = tostr(label, " (", name[1..min(length(name), 80)], ")"));
    endif
    return label;
  endmethod


  method _command_item owner: #2
    "Return the display label for a review object or verb.";
    caller == this || raise(E_PERM);
    return this:_command_label(@args);
  endmethod
  method _command_table owner: #2
    "Render a small review table for text clients.";
    caller == this || raise(E_PERM);
    const {headers, rows} = args;
    let lines = {$string_utils:from_list(headers, " | ")};
    for row in (rows)
      lines = {@lines, $string_utils:from_list({tostr(cell) for cell in (row)}, " | ")};
    endfor
    return $string_utils:from_list(lines, "\n");
  endmethod

  method _command_overview owner: #2
    "Show the active review or the default package's fetch command.";
    caller == this || raise(E_PERM);
    const package = this.packages[this.default_package];
    if (package["active"])
      return this:_command_status(this:status(package["active"]));
    endif
    return {tostr(this.default_package, " changes"), "Check upstream for updates, compare the code, then choose what to apply.", "Check for updates: @changes fetch", "Command reference: help @changes"};
  endmethod

  method _command_status owner: #2
    "Show progress and the next useful action.";
    caller == this || raise(E_PERM);
    const {summary} = args;
    const id = summary["review_id"];
    const generation = summary["generation"];
    const status = summary["status"];
    let output = {tostr(summary["package"], " · Review ", id), tostr("Generation ", generation, ".")};
    if (status == "fetching")
      return {@output, "Fetching upstream code. You’ll get a message when it finishes.", tostr("@changes status ", id)};
    elseif (status == "ready")
      return {@output, "Upstream fetched. Ready to review.", tostr("@changes diff ", id)};
    elseif (status == "applying")
      return {@output, "Applying your choices. You’ll get a message when it finishes.", tostr("@changes status ", id)};
    elseif (status == "complete")
      return {@output, "Changes applied."};
    elseif (status == "partial")
      return {@output, "Changes applied. Some were skipped.", tostr("@changes diff ", id)};
    endif
    const error = summary["error"];
    const message = `error["message"] ! E_RANGE => `error["code"] ! E_RANGE => "No error details available."'';
    output = {@output, tostr(status == "failed" ? "Fetch failed: " | "Update stopped: ", message)};
    if (index(message, "No worker available for git"))
      output = {@output, "The server needs a Git worker. Start one, then fetch again."};
    elseif (index(message, "No worker available for curl"))
      output = {@output, "The server needs a curl worker. Start one, then fetch again."};
    endif
    if (status == "failed")
      return {@output, $string_utils:from_list({tostr("@changes discard ", id, " ", generation)}, "\n")};
    endif
    return {@output, $string_utils:from_list({tostr("@changes refresh ", id, " ", generation), tostr("@changes discard ", id, " ", generation)}, "\n")};
  endmethod

  method _command_diff owner: #2
    "Show program choices and local items absent from the fetched source.";
    caller == this || raise(E_PERM);
    const {record, offset} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG, "Page offset must be a positive number.");
    const id = record["id"];
    const generation = record["generation"];
    const rows = record["report"]["rows"];
    const counts = record["report"]["counts"];
    const names = this:_command_names();
    let output = {tostr(record["package"], " · Review ", id), tostr("Generation ", generation, ".")};
    const labels = ["upstream" -> {"upstream update", "upstream updates"}, "local" -> {"local edit", "local edits"}, "conflict" -> {"conflict", "conflicts"}, "converged" -> {"program already matches upstream", "programs already match upstream"}, "unbased" -> {"program without a baseline", "programs without a baseline"}, "incoming_only" -> {"new upstream verb", "new upstream verbs"}];
    let summary = "";
    for classification in ({"upstream", "local", "conflict", "converged", "unbased", "incoming_only"})
      const count = `counts[classification] ! E_RANGE => 0';
      if (count)
        summary = tostr(summary, summary ? "; " | "", count, " ", labels[classification][count == 1 ? 1 | 2]);
      endif
    endfor
    const unchanged = `counts["unchanged"] ! E_RANGE => 0';
    summary = tostr(summary, summary ? ". " | "", unchanged, " unchanged.");
    output = {@output, summary};
    let change_rows = {};
    let next_offset = 0;
    const choice_counts = this:_review_decisions(record);
    const selected = choice_counts["selected"];
    const unresolved = choice_counts["unresolved"];
    const descriptions = ["upstream" -> "Upstream edit", "local" -> "Local edit", "conflict" -> "Conflict", "converged" -> "Already matches", "unbased" -> "No baseline", "unchanged" -> "Unchanged", "incoming_only" -> "New upstream verb"];
    const decisions = ["incoming" -> "Use upstream", "local" -> "Keep local", "edited" -> "Use edited program", "defer" -> "Skip", "unresolved" -> "Choose"];
    const reasons = ["adoption_required" -> "Adopt a baseline first", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match", "change_group_required" -> "Declare the required change group first"];
    for i in [1..length(rows)]
      const row = rows[i];
      const choice = `record["choices"][row["id"]] ! E_RANGE => []';
      const kind = choice ? choice["choice"] | row["default"];
      if (row["classification"] == "unchanged" && kind == "defer" && !choice)
        continue;
      endif
      const invalid = kind == "edited" && !maphaskey(choice, "validation");
      if (i < offset || next_offset)
        continue;
      endif
      if (length(change_rows) == 20)
        next_offset = i;
        continue;
      endif
      let decision = decisions[kind];
      if (!row["eligible"])
        decision = $string_utils:from_list({`reasons[reason] ! E_RANGE => reason' for reason in (row["blockers"])}, "; ");
      elseif (invalid)
        decision = "Edited program has errors";
      elseif (kind == "incoming" && (record["request"]["operation"] == "adopt" || row["classification"] in {"unchanged", "converged"}))
        decision = "Record baseline";
      elseif (kind == "defer" && row["classification"] in {"local", "unchanged"})
        decision = "Keep local";
      endif
      const live_command = tostr("@changes source ", id, " ", generation, " ", i, " live");
      const incoming_command = tostr("@changes source ", id, " ", generation, " ", i, " incoming");
      let inspection = tostr(live_command, " / ", incoming_command);
      if (row["eligible"] && (kind == "unresolved" || invalid || kind == "defer" && row["classification"] in {"upstream", "conflict", "unbased"}))
        decision = tostr(decision, "; choose with @changes resolve ", id, " ", generation, " ", i, " incoming|local|defer");
      endif
      const displayed = {i, this:_command_item(row["object"], names, row["names"][1]), descriptions[row["classification"]], decision, inspection};
      value_bytes(change_rows) + value_bytes(displayed) <= 60000 || raise(E_QUOTA, "Review page is too large.");
      change_rows = {@change_rows, displayed};
    endfor
    if (change_rows)
      output = {@output, this:_command_table({"Row", "Verb", "Change", "On apply", "Read program"}, change_rows)};
    else
      output = {@output, offset == 1 ? "No program updates to apply." | "No more program changes on this page."};
    endif
    let local_rows = {};
    let local_total = 0;
    for diagnostic in (record["report"]["diagnostics"])
      if (!(diagnostic["code"] in {"missing_source", "live_definition_unmatched"}))
        continue;
      endif
      local_total = local_total + 1;
      if (length(local_rows) < 20)
        const object = diagnostic["object"];
        const verb_name = diagnostic["code"] == "live_definition_unmatched" ? diagnostic["names"][1] | "";
        local_rows = {@local_rows, {this:_command_item(object, names, verb_name), verb_name ? "Verb" | "Object"}};
      endif
    endfor
    if (local_rows)
      output = {@output, "Not in fetched source", "These local objects and verbs have no matching upstream definition. They’ll be kept.", this:_command_table({"Local item", "Kind"}, local_rows)};
      if (local_total > length(local_rows))
        output = {@output, tostr("Showing ", length(local_rows), " of ", local_total, ". See review details for the full list.")};
      endif
    endif
    let commands = {};
    if (next_offset)
      commands = {@commands, tostr("@changes diff ", id, " ", next_offset)};
    endif
    if (unresolved)
      output = {@output, tostr(unresolved, unresolved == 1 ? " program needs a choice before applying." | " programs need choices before applying.")};
    elseif (selected)
      commands = {@commands, tostr("@changes apply ", id, " ", generation)};
    endif
    commands = {@commands, tostr("@changes details ", id), tostr("@changes discard ", id, " ", generation)};
    output = {@output, $string_utils:from_list(commands, "\n")};
    return output;
  endmethod

  method _command_details owner: #2
    "Keep source provenance and checks out of the main review.";
    caller == this || raise(E_PERM);
    const {record, offset} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG, "Page offset must be a positive number.");
    const id = record["id"];
    const names = this:_command_names();
    let output = {tostr(record["package"], " · Review ", id, " details"), tostr("Generation ", record["generation"], ".")};
    const provenance = record["provenance"];
    let source = {};
    for field in ({"repository", "revision", "commit", "path", "url", "etag", "digest"})
      if (maphaskey(provenance, field))
        let value = provenance[field];
        if (field == "revision" && typeof(value) == TYPE_MAP)
          value = `value["ref"] ! E_RANGE => value["commit"]';
        endif
        source = {@source, {field, tostr(value)}};
      endif
    endfor
    if (source)
      output = {@output, "Source", this:_command_table({"Field", "Value"}, source)};
    endif
    let checks = {};
    const reasons = ["adoption_required" -> "Adopt a baseline first", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match"];
    for row in (record["report"]["rows"])
      if (!row["eligible"])
        const explanation = $string_utils:from_list({`reasons[reason] ! E_RANGE => reason' for reason in (row["blockers"])}, "; ");
        checks = {@checks, {this:_command_item(row["object"], names, row["names"][1]), tostr(explanation, row["classification"] == "unchanged" ? " (program unchanged)" | "")}};
      endif
    endfor
    let property_objects = 0;
    const diagnostic_labels = ["missing_source" -> "No upstream object definition", "live_definition_unmatched" -> "No matching upstream verb definition", "unsupported_creation" -> "New upstream object; creation isn’t supported", "unsupported_or_ambiguous_definition" -> "Upstream verb can’t be matched; definition changes aren’t supported", "definition_fields_unmanaged" -> "Owner/flags differ, or upstream includes verb metadata"];
    for diagnostic in (record["report"]["diagnostics"])
      if (diagnostic["code"] == "property_fields_unmanaged")
        property_objects = property_objects + 1;
        continue;
      endif
      const verb_name = maphaskey(diagnostic, "names") ? diagnostic["names"][1] | "";
      checks = {@checks, {this:_command_item(diagnostic["object"], names, verb_name), `diagnostic_labels[diagnostic["code"]] ! E_RANGE => diagnostic["code"]'}};
    endfor
    if (offset <= length(checks))
      const page = checks[offset..min(offset + 19, length(checks))];
      if (page)
        output = {@output, "Checks", this:_command_table({"Item", "Check"}, page)};
      endif
      if (offset + length(page) <= length(checks))
        output = {@output, $string_utils:from_list({tostr("@changes details ", id, " ", offset + length(page))}, "\n")};
      endif
    endif
    output = {@output, "Properties and object attributes aren’t compared or updated."};
    if (property_objects)
      output = {@output, tostr("The source contains property declarations on ", property_objects, " objects; this isn’t a count of property changes.")};
    endif
    return {@output, $string_utils:from_list({tostr("@changes diff ", id)}, "\n")};
  endmethod

  method _command_row owner: #2
    "Translate a displayed row number without changing the service's stable row IDs.";
    caller == this || raise(E_PERM);
    const {auth, id, generation, selector} = args;
    if (tostr(toint(selector)) != selector)
      return selector;
    endif
    const record = this:_get(auth, id, generation);
    const number = toint(selector);
    number >= 1 && number <= length(record["report"]["rows"]) || this:_error("missing_row", "No such review row number. Check @changes diff.");
    return record["report"]["rows"][number]["id"];
  endmethod

  method command owner: #2
    "Terminal wrapper for the versioned review service; writes require a displayed generation.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    let {words} = args;
    if (!words)
      return this:_command_overview();
    endif
    const action = words[1];
    action in {"packages", "package", "upstream", "fetch", "stage", "adopt", "status", "diff", "details", "source", "apply", "refresh", "discard", "resolve"} || raise(E_INVARG, tostr("Unknown @changes command: ", action, ". Use help @changes."));
    if (action in {"fetch", "stage", "adopt"} && length(words) == 1)
      words = {@words, this.default_package};
    elseif (action == "upstream" && length(words) == 2)
      words = {action, this.default_package, words[2]};
    endif
    if (action == "packages")
      let rows = {};
      for name in (mapkeys(this.packages))
        const package = this.packages[name];
        const upstream = package["upstream"];
        const source = typeof(upstream) == TYPE_STR ? (upstream ? upstream | "Not configured") | tostr(upstream["repository"], " · ", `upstream["revision"]["ref"] ! E_RANGE => upstream["revision"]["commit"]');
        const review = package["active"] ? tostr("@changes diff ", package["active"]) | "None";
        rows = {@rows, {name, length(package["objects"]), source, review}};
      endfor
      return {"Change packages", this:_command_table({"Package", "Objects", "Upstream", "Review"}, rows)};
    endif
    if (action == "package" && length(words) >= 3)
      let objects = {};
      let constants = [];
      for i in [3..length(words)]
        const object = toobj(words[i]);
        valid(object) || raise(E_INVARG, "Package targets must be valid object addresses.");
        objects = {@objects, object};
        const name = object_metadata(object, "import_export_id");
        if (typeof(name) == TYPE_STR && name != "")
          constants[name] = object;
        endif
      endfor
      const previous = `this.packages[words[2]]["generation"] ! E_RANGE => 0';
      const owners = `this.packages[words[2]]["trusted_owners"] ! E_RANGE => {}';
      const result = this:configure(words[2], objects, constants, "", previous, owners);
      return {tostr("Package ", words[2], " configured at generation ", result["generation"], ".")};
    endif
    if (action == "upstream" && length(words) == 3)
      const package = this.packages[words[2]];
      this:upstream(words[2], words[3], package["generation"]);
      return {"HTTP upstream configured."};
    endif
    if (action in {"fetch", "stage", "adopt"} && length(words) == 2)
      const result = this:stage(words[2], {}, action == "adopt" ? "adopt" | "update");
      this.pending[result["review_id"]]["notify"] = true;
      return this:_command_status(result);
    endif
    if (length(words) < 2)
      raise(E_INVARG, "Use help @changes for command syntax.");
    endif
    const id = toint(words[2]);
    if (action == "status")
      return this:_command_status(this:status(id));
    endif
    if (action in {"diff", "details"})
      length(words) in {2, 3} || raise(E_INVARG, "Use @changes diff ID [OFFSET] or @changes details ID [OFFSET].");
      const status = this:status(id);
      if (!(status["status"] in {"ready", "partial", "rejected"}))
        return this:_command_status(status);
      endif
      const offset = length(words) == 3 ? toint(words[3]) | 1;
      const record = this:_get(auth, id, status["generation"]);
      return action == "diff" ? this:_command_diff(record, offset) | this:_command_details(record, offset);
    endif
    if (length(words) < 3)
      if (action in {"apply", "refresh", "discard"})
        const current = this:_get(auth, id);
        raise(E_INVARG, tostr("Missing generation. Use @changes ", action, " ", id, " ", current["generation"], "."));
      endif
      raise(E_INVARG, "Use help @changes for command syntax.");
    endif
    const generation = toint(words[3]);
    if (action == "source" && length(words) in {5, 6})
      const detail = this:details(id, generation, this:_command_row(auth, id, generation, words[4]));
      words[5] in {"live", "incoming"} || raise(E_INVARG);
      const lines = explode(detail["row"][tostr(words[5], "_text")], "\n", true);
      const offset = length(words) == 6 ? toint(words[6]) | 1;
      offset >= 1 && offset <= length(lines) || raise(E_INVARG);
      let code = {};
      for i in [offset..min(offset + 49, length(lines))]
        value_bytes(code) + value_bytes(lines[i]) <= 60000 || raise(E_QUOTA, "Source page is too large.");
        code = {@code, tostr(i, ": ", lines[i])};
      endfor
      const names = this:_command_names();
      let output = {tostr(words[5] == "live" ? "Local program" | "Upstream program"), this:_command_item(detail["row"]["object"], names, detail["row"]["names"][1]), $string_utils:from_list(code, "\n")};
      if (offset + 50 <= length(lines))
        output = {@output, $string_utils:from_list({tostr("@changes source ", id, " ", generation, " ", words[4], " ", words[5], " ", offset + 50)}, "\n")};
      endif
      return {@output, $string_utils:from_list({tostr("@changes diff ", id)}, "\n")};
    endif
    if (action == "apply" && length(words) == 3)
      const result = this:apply(id, generation);
      if (maphaskey(this.pending, id))
        this.pending[id]["notify"] = true;
      endif
      return this:_command_status(result);
    elseif (action == "refresh" && length(words) == 3)
      return this:_command_status(this:refresh(id, generation));
    elseif (action == "discard" && length(words) == 3)
      this:discard(id, generation);
      return {tostr("Review ", id, " discarded. Fetch again when you want a new review.")};
    elseif (action == "resolve" && length(words) >= 5)
      let program = "";
      for i in [6..length(words)]
        program = tostr(program, i == 6 ? "" | " ", words[i]);
      endfor
      const row_id = this:_command_row(auth, id, generation, words[4]);
      const result = this:resolve(id, generation, row_id, words[5], program);
      const labels = ["incoming" -> "Use upstream", "local" -> "Keep local", "defer" -> "Skip", "edited" -> "Use edited program"];
      let output = {tostr("Review ", id, " · Generation ", result["generation"]), tostr(labels[words[5]], " saved for row ", words[4], ".")};
      for validation in (result["validation"])
        if (validation["id"] == row_id && !validation["valid"])
          output = {@output, tostr("Program error at line ", validation["line"], ": ", validation["message"])};
        endif
      endfor
      return {@output, $string_utils:from_list({tostr("@changes diff ", id)}, "\n")};
    endif
    raise(E_INVARG, "Use help @changes for command syntax.");
  endmethod
endobject
