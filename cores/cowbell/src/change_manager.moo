object CHANGE_MANAGER [
  import_export_id -> "change_manager"
]
  name: "Change Manager"
  parent: ROOT
  owner: ARCH_WIZARD

  property default_package (owner: ARCH_WIZARD, flags: "") = "cowbell";
  property next_review (owner: ARCH_WIZARD, flags: "") = 1;
  property packages (owner: ARCH_WIZARD, flags: "") = [
    "cowbell" -> [
      "active" -> 0,
      "constants" -> [
        "ACTOR" -> ACTOR,
        "ADMIN_FEATURES" -> ADMIN_FEATURES,
        "ADMIN_HELP_TOPICS" -> ADMIN_HELP_TOPICS,
        "AGENT_BUILDING_TOOLS" -> AGENT_BUILDING_TOOLS,
        "AGENT_ROOM" -> AGENT_ROOM,
        "AGENTIC" -> AGENTIC,
        "AGENTIC_AGENT" -> AGENTIC_AGENT,
        "AGENTIC_CODING_ROOM" -> AGENTIC_CODING_ROOM,
        "AGENTIC_EVENT_QUEUE" -> AGENTIC_EVENT_QUEUE,
        "AGENTIC_LOOP" -> AGENTIC_LOOP,
        "AGENTIC_ROOM_OBSERVER" -> AGENTIC_ROOM_OBSERVER,
        "AGENTIC_RUNNER" -> AGENTIC_RUNNER,
        "AGENTIC_TOOL" -> AGENTIC_TOOL,
        "ANSI" -> ANSI,
        "ARCH_WIZARD" -> ARCH_WIZARD,
        "ARCH_WIZARD_MAILBOX" -> ARCH_WIZARD_MAILBOX,
        "ARCHITECTS_COMPASS" -> ARCHITECTS_COMPASS,
        "AREA" -> AREA,
        "BRASS_KEY" -> BRASS_KEY,
        "BUILDER_FEATURES" -> BUILDER_FEATURES,
        "BUILDER_HELP_TOPICS" -> BUILDER_HELP_TOPICS,
        "CAT_KIBBLE" -> CAT_KIBBLE,
        "CHANGE_MANAGER" -> CHANGE_MANAGER,
        "CONSUMABLE" -> CONSUMABLE,
        "CONTAINER" -> CONTAINER,
        "COUCH" -> COUCH,
        "DATA_VISOR" -> DATA_VISOR,
        "DM" -> DM,
        "DRINK" -> DRINK,
        "EVENT" -> EVENT,
        "EVENT_RECEIVER" -> EVENT_RECEIVER,
        "EXAMINATION" -> EXAMINATION,
        "FIRST_AREA" -> FIRST_AREA,
        "FIRST_AREA_PASSAGES" -> FIRST_AREA_PASSAGES,
        "FIRST_ROOM" -> FIRST_ROOM,
        "FOOD" -> FOOD,
        "FORMAT" -> FORMAT,
        "FORMAT_ANNOTATION" -> FORMAT_ANNOTATION,
        "FORMAT_BLOCK" -> FORMAT_BLOCK,
        "FORMAT_CODE" -> FORMAT_CODE,
        "FORMAT_DEFLIST" -> FORMAT_DEFLIST,
        "FORMAT_LINK" -> FORMAT_LINK,
        "FORMAT_LIST" -> FORMAT_LIST,
        "FORMAT_PARAGRAPH" -> FORMAT_PARAGRAPH,
        "FORMAT_TABLE" -> FORMAT_TABLE,
        "FORMAT_TEXT" -> FORMAT_TEXT,
        "FORMAT_TITLE" -> FORMAT_TITLE,
        "GIT" -> GIT,
        "GIT_ENTRY" -> GIT_ENTRY,
        "GIT_REPOSITORY" -> GIT_REPOSITORY,
        "GIT_SNAPSHOT" -> GIT_SNAPSHOT,
        "GRANT_UTILS" -> GRANT_UTILS,
        "HACKER" -> HACKER,
        "HELP" -> HELP,
        "HELP_SOURCE" -> HELP_SOURCE,
        "HELP_TOPICS" -> HELP_TOPICS,
        "HELP_UTILS" -> HELP_UTILS,
        "HENRI" -> HENRI,
        "HOUSEKEEPING" -> HOUSEKEEPING,
        "HOUSEKEEPING_SWEEP_MSGS" -> HOUSEKEEPING_SWEEP_MSGS,
        "HTML" -> HTML,
        "INT_PROTO" -> INT_PROTO,
        "KIBBLE_CAN_1" -> KIBBLE_CAN_1,
        "KIBBLE_CAN_2" -> KIBBLE_CAN_2,
        "KIBBLE_CAN_3" -> KIBBLE_CAN_3,
        "KIBBLE_CUPBOARD" -> KIBBLE_CUPBOARD,
        "LETTER" -> LETTER,
        "LIST_PROTO" -> LIST_PROTO,
        "LLM_AGENT" -> LLM_AGENT,
        "LLM_AGENT_TOOL" -> LLM_AGENT_TOOL,
        "LLM_CHAT_OPTS" -> LLM_CHAT_OPTS,
        "LLM_CLIENT" -> LLM_CLIENT,
        "LLM_RESPONSE" -> LLM_RESPONSE,
        "LLM_ROOM_OBSERVER" -> LLM_ROOM_OBSERVER,
        "LLM_TASK" -> LLM_TASK,
        "LLM_WEARABLE" -> LLM_WEARABLE,
        "LOGIN" -> LOGIN,
        "LOOK" -> LOOK,
        "MAIL_FEATURES" -> MAIL_FEATURES,
        "MAIL_ROOM" -> MAIL_ROOM,
        "MAILBOX" -> MAILBOX,
        "MATCH" -> MATCH,
        "MR_WELCOME" -> MR_WELCOME,
        "MSG_BAG" -> MSG_BAG,
        "NOTE" -> NOTE,
        "OBJ_UTILS" -> OBJ_UTILS,
        "PASSAGE" -> PASSAGE,
        "PASSWORD" -> PASSWORD,
        "PLAYER" -> PLAYER,
        "PLAYER_ACTIVITY" -> PLAYER_ACTIVITY,
        "PROG_FEATURES" -> PROG_FEATURES,
        "PROG_HELP_TOPICS" -> PROG_HELP_TOPICS,
        "PROG_UTILS" -> PROG_UTILS,
        "PRONOUNS" -> PRONOUNS,
        "PROPERTY" -> PROPERTY,
        "PROTOTYPE_BOX" -> PROTOTYPE_BOX,
        "REACTION" -> REACTION,
        "RELATION" -> RELATION,
        "RLM_AGENT" -> RLM_AGENT,
        "ROOM" -> ROOM,
        "ROOT" -> ROOT,
        "RULE" -> RULE,
        "RULE_ENGINE" -> RULE_ENGINE,
        "RULE_TEST" -> RULE_TEST,
        "SERVER_OPTIONS" -> SERVER_OPTIONS,
        "SITTABLE" -> SITTABLE,
        "SOCIAL_FEATURES" -> SOCIAL_FEATURES,
        "STR_PROTO" -> STR_PROTO,
        "SUB" -> SUB,
        "SUB_UTILS" -> SUB_UTILS,
        "SYM_PROTO" -> SYM_PROTO,
        "SYSOBJ" -> SYSOBJ,
        "TEST_PLAYER" -> TEST_PLAYER,
        "TEST_UTILS" -> TEST_UTILS,
        "THING" -> THING,
        "URL_UTILS" -> URL_UTILS,
        "VERB" -> VERB,
        "WEARABLE" -> WEARABLE,
        "WIZ_FEATURES" -> WIZ_FEATURES,
        "WIZARD_HELP_TOPICS" -> WIZARD_HELP_TOPICS
      ],
      "fields" -> {"program", "property"},
      "generation" -> 1,
      "objects" -> {
        ACTOR,
        AGENTIC,
        AGENTIC_AGENT,
        AGENTIC_CODING_ROOM,
        AGENTIC_EVENT_QUEUE,
        AGENTIC_LOOP,
        AGENTIC_ROOM_OBSERVER,
        AGENTIC_RUNNER,
        AGENTIC_TOOL,
        ARCH_WIZARD,
        LOGIN,
        PASSWORD,
        CHANGE_MANAGER,
        EVENT,
        EVENT_RECEIVER,
        MSG_BAG,
        SUB,
        SUB_UTILS,
        EXAMINATION,
        ADMIN_FEATURES,
        BUILDER_FEATURES,
        MAIL_FEATURES,
        PROG_FEATURES,
        SOCIAL_FEATURES,
        WIZ_FEATURES,
        ANSI,
        FORMAT,
        FORMAT_ANNOTATION,
        FORMAT_BLOCK,
        FORMAT_CODE,
        FORMAT_DEFLIST,
        FORMAT_LINK,
        FORMAT_LIST,
        FORMAT_PARAGRAPH,
        FORMAT_TABLE,
        FORMAT_TEXT,
        FORMAT_TITLE,
        GIT,
        GIT_REPOSITORY,
        GIT_SNAPSHOT,
        GIT_ENTRY,
        HTML,
        HACKER,
        ADMIN_HELP_TOPICS,
        BUILDER_HELP_TOPICS,
        HELP,
        HELP_SOURCE,
        HELP_TOPICS,
        HELP_UTILS,
        PROG_HELP_TOPICS,
        WIZARD_HELP_TOPICS,
        ARCH_WIZARD_MAILBOX,
        BRASS_KEY,
        CAT_KIBBLE,
        COUCH,
        FIRST_AREA,
        FIRST_AREA_PASSAGES,
        FIRST_ROOM,
        HENRI,
        HOUSEKEEPING,
        HOUSEKEEPING_SWEEP_MSGS,
        KIBBLE_CAN_1,
        KIBBLE_CAN_2,
        KIBBLE_CAN_3,
        KIBBLE_CUPBOARD,
        MAIL_ROOM,
        PROTOTYPE_BOX,
        TEST_PLAYER,
        CONSUMABLE,
        CONTAINER,
        DRINK,
        FOOD,
        NOTE,
        SITTABLE,
        THING,
        WEARABLE,
        AGENT_BUILDING_TOOLS,
        AGENT_ROOM,
        ARCHITECTS_COMPASS,
        DATA_VISOR,
        LLM_AGENT,
        LLM_AGENT_TOOL,
        LLM_CHAT_OPTS,
        LLM_CLIENT,
        LLM_RESPONSE,
        LLM_ROOM_OBSERVER,
        LLM_TASK,
        LLM_WEARABLE,
        MR_WELCOME,
        RLM_AGENT,
        LOOK,
        DM,
        LETTER,
        MAILBOX,
        PLAYER,
        PLAYER_ACTIVITY,
        PRONOUNS,
        RELATION,
        ROOT,
        REACTION,
        RULE,
        RULE_ENGINE,
        RULE_TEST,
        SERVER_OPTIONS,
        SYSOBJ,
        INT_PROTO,
        LIST_PROTO,
        PROPERTY,
        STR_PROTO,
        SYM_PROTO,
        VERB,
        GRANT_UTILS,
        MATCH,
        OBJ_UTILS,
        PROG_UTILS,
        TEST_UTILS,
        URL_UTILS,
        AREA,
        PASSAGE,
        ROOM
      },
      "schema" -> 1,
      "trusted_owners" -> {HACKER},
      "upstream" -> [
        "path" -> "cores/cowbell/src",
        "repository" -> "https://github.com/timbran-project/moor.git",
        "revision" -> ["ref" -> "refs/heads/main"],
        "transport" -> "git"
      ]
    ]
  ];
  property pending (owner: ARCH_WIZARD, flags: "") = [];
  property receipts (owner: ARCH_WIZARD, flags: "") = [];

  method _error owner: ARCH_WIZARD
    "Raise a versioned service error.";
    caller == this || raise(E_PERM);
    const {code, message} = args;
    raise(E_INVARG, message, ["schema" -> 1, "code" -> code]);
  endmethod

  method _entry owner: ARCH_WIZARD
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
    let authority = actor;
    if (!actor.wizard)
      $admin_features in actor.admin_features || raise(E_PERM);
      authority = $admin_features:_resolve_delegate(actor);
      $admin_features:_is_allowed_verb(actor, $wiz_features, "@changes") || raise(E_PERM);
    endif
    origin in {actor, authority} && valid(authority) && authority.wizard || raise(E_PERM);
    return {actor, authority};
  endmethod

  method _authorized owner: ARCH_WIZARD
    "Recheck persisted actor and authority after suspension or restart.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {actor, authority} = args;
    valid(actor) && valid(authority) && authority.wizard || raise(E_PERM);
    if (!actor.wizard)
      $admin_features in actor.admin_features || raise(E_PERM);
      $admin_features:_resolve_delegate(actor) == authority || raise(E_PERM);
      $admin_features:_is_allowed_verb(actor, $wiz_features, "@changes") || raise(E_PERM);
    else
      actor == authority || raise(E_PERM);
    endif
    return true;
  endmethod

  method _get owner: ARCH_WIZARD
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

  method _summary owner: ARCH_WIZARD
    "Return status without source, drafts, or program bodies.";
    caller == this || raise(E_PERM);
    const {record} = args;
    return ["schema" -> 1, "review_id" -> record["id"], "generation" -> record["generation"], "package" -> record["package"], "status" -> record["status"], "task" -> record["task"], "error" -> record["error"], "provenance" -> record["provenance"]];
  endmethod

  method capabilities owner: ARCH_WIZARD
    "Describe the versioned review API and its limits.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    return ["schema" -> 1, "operations" -> {"packages", "history", "configure", "upstream", "stage", "status", "review", "inspection", "diagnostics", "details", "base_source", "set_group", "initialize_property", "resolve_group", "resolve", "apply", "refresh", "discard"}, "fields" -> {"program", "property"}, "choices" -> {"incoming", "local", "edited", "defer"}, "transports" -> {"upload", "http", "git"}, "git_authorization" -> "wizard", "authorization" -> "administrator", "default_package" -> this.default_package, "max_source_bytes" -> 4194304, "max_pending_bytes" -> 33554432, "max_packages" -> 32, "page_rows" -> 50, "max_detail_bytes" -> 524288, "max_page_bytes" -> 262144, "receipt_decisions" -> 50];
  endmethod

  method packages owner: ARCH_WIZARD
    "Return package bindings, policy, upstream settings, and active review IDs.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    return ["schema" -> 1, "packages" -> this.packages];
  endmethod

  method history owner: ARCH_WIZARD
    "List retained completion summaries without source or decision bodies.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name} = args;
    maphaskey(this.packages, name) || raise(E_INVARG);
    let reviews = {};
    for id in (mapkeys(this.receipts))
      const receipt = this.receipts[id];
      if (receipt["package"] == name && (receipt["actor"] == auth[1] || auth[1].wizard))
        reviews = {@reviews, ["review_id" -> id, "generation" -> receipt["generation"], "status" -> receipt["status"], "completed_at" -> receipt["completed_at"], "decision_count" -> receipt["decision_count"]]};
      endif
    endfor
    return ["schema" -> 1, "package" -> name, "reviews" -> reviews];
  endmethod

  method configure owner: ARCH_WIZARD
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
    const upstream = this:_upstream(url);
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
    const package = ["schema" -> 1, "generation" -> generation + 1, "objects" -> objects, "fields" -> {"program", "property"}, "constants" -> constants, "trusted_owners" -> trusted_owners, "upstream" -> upstream, "active" -> 0];
    let configured = package;
    if (maphaskey(this.packages, name))
      configured["policy"] = `this.packages[name]["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
      configured["members"] = `this.packages[name]["members"] ! E_RANGE => []';
    endif
    this.packages[name] = configured;
    return configured;
  endmethod

  method _url owner: ARCH_WIZARD
    "Accept a single HTTP text-bundle URL without embedded credentials.";
    caller == this || raise(E_PERM);
    const {url} = args;
    typeof(url) == TYPE_STR && length(url) <= 2048 || raise(E_INVARG);
    if (url != "")
      index(url, "http://") == 1 || index(url, "https://") == 1 && !index(url, "@") || raise(E_INVARG, "Expected an HTTP bundle URL without credentials.");
    endif
  endmethod

  method upstream owner: ARCH_WIZARD
    "Conditionally set an HTTP bundle URL or a Git repository source map.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name, url, expected} = args;
    maphaskey(this.packages, name) || raise(E_INVARG);
    const package = this.packages[name];
    return this:configure(name, package["objects"], package["constants"], url, expected, package["trusted_owners"]);
  endmethod

  method _keys owner: ARCH_WIZARD
    "Normalize source-setting keys and reject duplicates or unknown fields.";
    caller == this || raise(E_PERM);
    const {input, allowed} = args;
    typeof(input) == TYPE_MAP || raise(E_TYPE);
    let result = [];
    for key in (mapkeys(input))
      typeof(key) in {TYPE_STR, TYPE_SYM} || raise(E_TYPE);
      const name = tostr(key);
      name in allowed && !maphaskey(result, name) || raise(E_INVARG, "Unknown or duplicate upstream field.");
      result[name] = input[key];
    endfor
    return result;
  endmethod

  method _upstream owner: ARCH_WIZARD
    "Validate HTTP bundle URLs and Git source descriptors without network access.";
    caller == this || raise(E_PERM);
    const {source} = args;
    if (typeof(source) == TYPE_STR)
      this:_url(source);
      return source;
    endif
    const spec = this:_keys(source, {"transport", "repository", "revision", "path"});
    for key in ({"transport", "repository", "revision"})
      maphaskey(spec, key) || raise(E_INVARG, "Missing upstream field: " + key);
    endfor
    spec["transport"] == "git" || raise(E_INVARG, "Unknown upstream transport.");
    const url = spec["repository"];
    this:_url(url);
    url != "" && !index(url, "?") && !index(url, "#") || raise(E_INVARG, "Expected a Git HTTP(S) URL without a query or fragment.");
    const revision = this:_keys(spec["revision"], {"ref", "commit"});
    length(revision) == 1 || raise(E_INVARG, "Specify one full Git ref or commit.");
    if (maphaskey(revision, "commit"))
      $git:oid(revision["commit"]);
    else
      const ref = revision["ref"];
      typeof(ref) == TYPE_STR || raise(E_TYPE);
      index(ref, "refs/", 1) == 1 && length(ref) > 5 && length(ref) <= 1024 || raise(E_INVARG, "Use a full Git ref, such as refs/heads/main.");
    endif
    const path = `spec["path"] ! E_RANGE => ""';
    typeof(path) == TYPE_STR && length(path) <= 4096 || raise(E_INVARG);
    !index(path, "\\") || raise(E_INVARG, "Git paths use forward slashes.");
    if (path != "")
      for part in (explode(path, "/", true))
        !(part in {"", ".", ".."}) || raise(E_INVARG, "Git paths must be relative without dot segments.");
      endfor
    endif
    return ["transport" -> "git", "repository" -> url, "revision" -> revision, "path" -> path];
  endmethod

  method _git_sources owner: ARCH_WIZARD
    "Convert local snapshot files to objdef units. Package constants remain the authoritative bindings.";
    caller == this || raise(E_PERM);
    const {snapshot} = args;
    snapshot.complete || raise(E_INVARG, "Git staging requires a complete snapshot.");
    let sources = {};
    let bytes = 0;
    for entry in (snapshot.entries)
      if (entry.kind in {'symlink, 'submodule})
        this:_error("unsupported_git_entry", "Git source trees cannot contain symlinks or submodules: " + entry.path);
      endif
      if (entry.kind != 'file || length(entry.path) < 4)
        continue;
      endif
      if (strcmp(entry.path[length(entry.path) - 3..$], ".moo") != 0)
        continue;
      endif
      const parts = explode(entry.path, "/", true);
      if (strcmp(parts[$], "constants.moo") == 0)
        continue;
      endif
      let text = "";
      try
        text = binary_to_str(entry.content);
      except failure (E_INVARG)
        this:_error("invalid_git_utf8", "Git objdef source is not UTF-8: " + entry.path);
      endtry
      const unit = ["label" -> entry.path, "text" -> text];
      bytes = bytes + value_bytes(unit);
      bytes <= 4194304 || raise(E_QUOTA, "Source exceeds the service limit.");
      sources = {@sources, unit};
    endfor
    length(sources) > 0 || this:_error("missing_source", "Git subtree contains no objdef object files.");
    return sources;
  endmethod

  method _save_ready owner: ARCH_WIZARD
    "Validate exact staged input and save original evidence without applying changes.";
    caller == this || raise(E_PERM);
    let {record, sources, provenance} = args;
    value_bytes(sources) <= 4194304 || raise(E_QUOTA, "Source exceeds the service limit.");
    this:_authorized(record["actor"], record["authority"]);
    set_task_perms(record["authority"]);
    const package = this.packages[record["package"]];
    package["generation"] == record["package_generation"] && package["active"] == record["id"] || this:_error("stale_package", "Package settings changed during staging.");
    record["request"]["provenance"] = maphaskey(provenance, "digest") ? mapdelete(provenance, "digest") | provenance;
    const report = preview_objdef_changes(sources, record["request"]);
    provenance["digest"] = report["source_digest"];
    record["sources"] = sources;
    record["report"] = report;
    record["group_validation"] = report["group_validation"];
    record["provenance"] = provenance;
    record["status"] = "ready";
    record["error"] = [];
    record["task"] = 0;
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA, "Pending review storage is full.");
    this.pending[record["id"]] = record;
    return this:_summary(record);
  endmethod

  method stage owner: ARCH_WIZARD
    "Stage uploaded source, or fetch the configured upstream. Git fetches require an actual wizard.";
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
      this:_error("missing_source", "Upload source units or configure an upstream.");
    endif
    if (!sources && typeof(package["upstream"]) == TYPE_MAP)
      auth[1].wizard || raise(E_PERM, "Git staging requires wizard authority.");
    endif
    const id = this.next_review;
    this.next_review = id + 1;
    let record = ["id" -> id, "generation" -> 1, "actor" -> auth[1], "authority" -> auth[2], "package" -> name, "package_generation" -> package["generation"], "upstream" -> package["upstream"], "status" -> "fetching", "task" -> 0, "error" -> [], "sources" -> {}, "report" -> [], "choices" -> [], "provenance" -> [], "request" -> ["schema" -> 1, "operation" -> operation, "objects" -> package["objects"], "fields" -> package["fields"], "constants" -> package["constants"], "trusted_owners" -> package["trusted_owners"]]];
    record["request"]["fields"] = {"program", "property"};
    record["request"]["policy"] = `package["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
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

  method _await_job owner: ARCH_WIZARD
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

  method _job owner: ARCH_WIZARD
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

  method _fetch owner: ARCH_WIZARD
    "Fetch saved upstream settings; recheck authority and job identity before saving any source.";
    caller == this || raise(E_PERM);
    const {id, generation} = args;
    let record = this:_job(id, generation, "fetching");
    set_task_perms(record["authority"]);
    try
      const upstream = `record["upstream"] ! E_RANGE => this.packages[record["package"]]["upstream"]';
      if (typeof(upstream) == TYPE_MAP)
        record["actor"].wizard || raise(E_PERM, "Git staging requires wizard authority.");
        const repository = $git:repository(upstream["repository"], ['max_entries -> 4096, 'max_file_bytes -> 4194304, 'max_total_bytes -> 4194304]);
        const snapshot = repository:snapshot(upstream["revision"], upstream["path"]);
        record = this:_job(id, generation, "fetching");
        record["actor"].wizard || raise(E_PERM, "Git staging requires wizard authority.");
        const sources = this:_git_sources(snapshot);
        this:_save_ready(record, sources, ["transport" -> "git", "repository" -> upstream["repository"], "revision" -> upstream["revision"], "commit" -> snapshot.commit, "tree" -> snapshot.tree, "path" -> snapshot.path]);
        `this:_notify_review(this.pending[id]) ! ANY';
        return;
      endif
      const url = upstream;
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

  method status owner: ARCH_WIZARD
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

  method _review_decisions owner: ARCH_WIZARD
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
    unresolved = unresolved + length(`record["group_validation"] ! E_RANGE => {}');
    return ["selected" -> selected, "unresolved" -> unresolved, "blocked" -> blocked];
  endmethod

  method review owner: ARCH_WIZARD
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
      row["label"] = row["field"] == "property" ? this:_command_label(row["object"], names) + "." + row["names"][1] | this:_command_label(row["object"], names, row["names"][1]);
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
    const groups = {["name" -> group["name"], "requires" -> group["requires"], "complete" -> group["complete"], "count" -> length(group["members"])] for group in (record["report"]["groups"])};
    const next_cursor = next_offset <= length(rows) ? {id, generation, next_offset} | {};
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "total" -> length(rows), "rows" -> page, "cursor" -> next_cursor, "counts" -> record["report"]["counts"], "decision_counts" -> this:_review_decisions(record), "groups" -> groups, "group_validation" -> `record["group_validation"] ! E_RANGE => {}', "operation" -> record["request"]["operation"], "diagnostic_count" -> length(record["report"]["diagnostics"]), "diagnostics" -> record["report"]["diagnostics"][1..min(50, length(record["report"]["diagnostics"]))], "provenance" -> record["provenance"]];
  endmethod

  method diagnostics owner: ARCH_WIZARD
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

  method inspection owner: ARCH_WIZARD
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
    let managed = [];
    for update in (record["report"]["rows"])
      if (`update["operation"] ! E_RANGE => ""' in {"add_verb", "add_property", "retain_property"})
        managed[tostr(update["object"], "/", update["field"], "/", update["names"][1])] = true;
      endif
    endfor
    let inspected = {};
    let counts = report["counts"];
    for inspected_row in (report["rows"])
      const member = inspected_row["field"] == "program" ? inspected_row["name"][index(inspected_row["name"], ":") + 1..$] | inspected_row["name"];
      if (maphaskey(managed, tostr(inspected_row["object"], "/", inspected_row["field"], "/", member)))
        counts[inspected_row["classification"]] = counts[inspected_row["classification"]] - 1;
      else
        inspected = {@inspected, inspected_row};
      endif
    endfor
    const all = {row for row in (inspected) if ((!classification || row["classification"] == classification) && (!selected || row["id"] == selected))};
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
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "rows" -> page, "revision" -> report["revision"], "counts" -> counts, "total" -> length(all), "next" -> offset + length(page) <= length(all) ? offset + length(page) | 0];
  endmethod

  method details owner: ARCH_WIZARD
    "Return selected programs and verified historical text while original evidence still matches.";
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
    const bases = `record["base_sources"] ! E_RANGE => []';
    if (maphaskey(bases, row_id))
      request["base_sources"] = bases[row_id];
    endif
    const current = preview_objdef_changes(record["sources"], request, record["choices"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before reading details.");
    for selected in (current["rows"])
      let row = selected;
      if (row["id"] == row_id)
        row["label"] = row["field"] == "property" ? this:_command_label(row["object"], this:_command_names()) + "." + row["names"][1] | this:_command_label(row["object"], this:_command_names(), row["names"][1]);
        row["base_fetch_available"] = auth[1].wizard && `row["base_source"]["transport"] == "git" ! E_RANGE => false';
        row["choice"] = `record["choices"][row_id] ! E_RANGE => []';
        if (row["choice"])
          row["choice"]["validated"] = maphaskey(row["choice"], "validation");
        endif
        row["validation"] = {item for item in (current["validation"]) if (item["id"] == row_id && !item["valid"])};
        value_bytes(row) <= 524288 || raise(E_QUOTA, "Selected details exceed the service limit.");
        return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "row" -> row];
      endif
    endfor
    this:_error("missing_row", "No such review row.");
  endmethod

  method base_source owner: ARCH_WIZARD
    "Verify uploaded or pinned Git source against one member's baseline and retain it only in this review.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    let {id, generation, row_id, ?sources = {}} = args;
    let record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    typeof(sources) == TYPE_LIST || raise(E_TYPE);
    const row = this:details(id, generation, row_id)["row"];
    typeof(row["base"]) == TYPE_STR || this:_error("missing_base", "This member has no accepted baseline.");
    if (!sources)
      auth[1].wizard || raise(E_PERM, "Git retrieval requires wizard authority.");
      const origin = `row["base_source"] ! E_RANGE => []';
      `origin["transport"] == "git" ! E_RANGE => false' || this:_error("missing_base_source", "Upload the objdefs for this member's accepted baseline.");
      const upstream = this:_upstream(["transport" -> "git", "repository" -> origin["repository"], "revision" -> ["commit" -> origin["commit"]], "path" -> origin["path"]]);
      const repository = $git:repository(upstream["repository"], ['max_entries -> 4096, 'max_file_bytes -> 4194304, 'max_total_bytes -> 4194304]);
      const snapshot = repository:snapshot(upstream["revision"], upstream["path"]);
      record = this:_get(auth, id, generation);
      auth[1].wizard || raise(E_PERM, "Git retrieval requires wizard authority.");
      sources = this:_git_sources(snapshot);
    endif
    value_bytes(sources) <= 4194304 || raise(E_QUOTA, "Historical source exceeds the service limit.");
    const package = this.packages[record["package"]];
    package["generation"] == record["package_generation"] && package["active"] == id || this:_error("stale_package", "Package settings changed during retrieval.");
    let request = record["request"];
    request["details"] = {row_id};
    request["base_sources"] = sources;
    const verified = preview_objdef_changes(record["sources"], request);
    verified["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before loading historical source.");
    let bases = `record["base_sources"] ! E_RANGE => []';
    bases[row_id] = sources;
    record["base_sources"] = bases;
    record["generation"] = generation + 1;
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA, "Pending review storage is full.");
    this.pending[id] = record;
    return this:details(id, generation + 1, row_id);
  endmethod

  method set_group owner: ARCH_WIZARD
    "Declare changes that must apply together; replace review evidence and clear earlier choices.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, name, members, ?requires = {}} = args;
    let record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    typeof(name) == TYPE_STR && typeof(members) == TYPE_LIST && typeof(requires) == TYPE_LIST || raise(E_TYPE);
    const current = preview_objdef_changes(record["sources"], record["request"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before changing groups.");
    let policy = `record["request"]["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
    let groups = {group for group in (policy["groups"]) if (group["name"] != name)};
    if (members)
      groups = {@groups, ["name" -> name, "members" -> members, "requires" -> requires]};
    endif
    policy["groups"] = groups;
    record["request"]["policy"] = policy;
    return this:_save_policy(record, policy, generation);
  endmethod

  method initialize_property owner: ARCH_WIZARD
    "Permit the reviewed initial value only when absent; descendants require an explicit choice.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, row_id, ?descendants = false} = args;
    let record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    descendants in {false, true, 0, 1} || raise(E_TYPE);
    const current = preview_objdef_changes(record["sources"], record["request"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before changing groups.");
    let policy = `record["request"]["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
    const rows = {row for row in (current["rows"]) if (row["id"] == row_id)};
    length(rows) == 1 && `rows[1]["operation"] == "add_property" ! E_RANGE => false' || this:_error("not_an_addition", "Choose a proposed property addition.");
    const member = rows[1]["member"];
    let initialization = {entry for entry in (`policy["initialization"] ! E_RANGE => {}') if (entry["member"] != member)};
    initialization = {@initialization, ["member" -> member, "mode" -> "absent", "descendants" -> descendants]};
    policy["initialization"] = initialization;
    record["request"]["policy"] = policy;
    return this:_save_policy(record, policy, generation);
  endmethod

  method _save_policy owner: ARCH_WIZARD
    "Save an already validated administrator policy edit and invalidate earlier choices.";
    caller == this || raise(E_PERM);
    let {record, policy, generation} = args;
    set_task_perms(record["authority"]);
    const id = record["id"];
    const report = preview_objdef_changes(record["sources"], record["request"]);
    let package = this.packages[record["package"]];
    package["generation"] == record["package_generation"] && package["active"] == id || this:_error("stale_package", "Package settings changed.");
    package["policy"] = policy;
    package["generation"] = package["generation"] + 1;
    record["package_generation"] = package["generation"];
    record["generation"] = generation + 1;
    record["report"] = report;
    record["choices"] = [];
    record["base_sources"] = [];
    record["group_validation"] = report["group_validation"];
    record["status"] = "ready";
    record["error"] = [];
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA);
    this.packages[record["package"]] = package;
    this.pending[id] = record;
    return this:_summary(record);
  endmethod

  method resolve_group owner: ARCH_WIZARD
    "Select every required change in a group, or skip the whole group.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, name, kind} = args;
    kind in {"incoming", "defer"} || raise(E_INVARG);
    let record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    const groups = {group for group in (record["report"]["groups"]) if (group["name"] == name)};
    length(groups) == 1 || this:_error("missing_group", "No change group has that name.");
    kind == "defer" || groups[1]["complete"] || this:_error("incomplete_group", "Some required changes are missing or blocked. Resolve those problems before selecting this group.");
    for row_id in (groups[1]["members"])
      const selected = {row for row in (record["report"]["rows"]) if (row["id"] == row_id)}[1];
      record["choices"][row_id] = ["choice" -> kind == "incoming" && `selected["operation"] ! E_RANGE => ""' == "retain_property" ? "local" | kind];
    endfor
    const current = preview_objdef_changes(record["sources"], record["request"], record["choices"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before choosing.");
    record["group_validation"] = current["group_validation"];
    record["generation"] = generation + 1;
    record["status"] = "ready";
    record["error"] = [];
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA);
    this.pending[id] = record;
    return this:_summary(record);
  endmethod

  method resolve owner: ARCH_WIZARD
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
    record["group_validation"] = current["group_validation"];
    value_bytes(this.pending) + value_bytes(record) <= 33554432 || raise(E_QUOTA);
    this.pending[id] = record;
    return ["schema" -> 1, "review_id" -> id, "generation" -> record["generation"], "validation" -> current["validation"]];
  endmethod

  method refresh owner: ARCH_WIZARD
    "Create new evidence from saved input and discard all earlier choices.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    let record = this:_get(auth, id, generation);
    !(record["status"] in {"fetching", "applying"}) || this:_error("busy", "Task is still running.");
    length(record["sources"]) > 0 || this:_error("missing_source", "Discard this failed fetch and stage again.");
    record["choices"] = [];
    record["base_sources"] = [];
    record["generation"] = generation + 1;
    return this:_save_ready(record, record["sources"], record["provenance"]);
  endmethod

  method apply owner: ARCH_WIZARD
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

  method _apply owner: ARCH_WIZARD
    "Commit content, baselines, and a source-free receipt together, or abort the task.";
    caller == this || raise(E_PERM);
    const {id, generation} = args;
    let record = this:_job(id, generation, "applying");
    set_task_perms(record["authority"]);
    let wrote = false;
    try
      const result = apply_objdef_changes(record["sources"], record["request"], record["report"]["evidence"], record["choices"]);
      wrote = true;
      let members = `this.packages[record["package"]]["members"] ! E_RANGE => []';
      for created in (result["created"])
        members[created["member"]] = created;
      endfor
      this.packages[record["package"]]["members"] = members;
      let deferred = false;
      for diagnostic in (record["report"]["diagnostics"])
        if (diagnostic["code"] in {"missing_source", "unsupported_creation", "unsupported_or_ambiguous_definition"})
          deferred = true;
        endif
      endfor
      for row in (record["report"]["rows"])
        const kind = `record["choices"][row["id"]]["choice"] ! E_RANGE => row["default"]';
        if (kind == "defer" && (record["request"]["operation"] == "adopt" || row["classification"] in {"upstream", "conflict", "unbased", "incoming_only"}))
          deferred = true;
        endif
      endfor
      record["generation"] = generation + 1;
      record["task"] = 0;
      record["choices"] = [];
      record["base_sources"] = [];
      record["error"] = [];
      if (deferred)
        record["status"] = "partial";
        record["report"] = preview_objdef_changes(record["sources"], record["request"]);
        record["group_validation"] = record["report"]["group_validation"];
        this.pending[id] = record;
      else
        record["status"] = "complete";
        this.pending = mapdelete(this.pending, id);
        this.packages[record["package"]]["active"] = 0;
      endif
      let receipt = ["schema" -> 1, "review_id" -> id, "generation" -> record["generation"], "package" -> record["package"], "status" -> record["status"], "task" -> 0, "error" -> []];
      receipt["actor"] = record["actor"];
      receipt["provenance"] = record["provenance"];
      receipt["decision_count"] = length(result["decisions"]);
      let labels = [];
      for row in (record["report"]["rows"])
        labels[row["id"]] = tostr(row["object"], ":", row["names"][1]);
      endfor
      let decisions = {};
      for decision in (result["decisions"][1..min(50, length(result["decisions"]))])
        decision["label"] = labels[decision["id"]];
        decisions = {@decisions, decision};
      endfor
      receipt["decisions"] = decisions;
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

  method discard owner: ARCH_WIZARD
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

  method _notify_review owner: ARCH_WIZARD
    "Deliver a terminal review result after the result task commits; output failure cannot undo it.";
    caller == this && this == $change_manager || raise(E_PERM);
    const {record} = args;
    const notification = `record["notify"] ! E_RANGE => []';
    typeof(notification) == TYPE_MAP && length(notification) || return;
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
        const event = $event:mk_info(record["actor"], $format.block:mk(@lines)):with_presentation_hint('inset);
        record["actor"]:rewrite_event(notification["rewrite_id"], event, notification["connection"]);
      except (ANY)
        "Notification is optional; the committed result remains available through status.";
      endtry
    endfork
  endmethod

  method _command_names owner: ARCH_WIZARD
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
    for name in (properties($format))
      if (is_clear_property($format, name))
        continue;
      endif
      const object = `$format.(name) ! E_PERM, E_PROPNF => false';
      if (typeof(object) == TYPE_OBJ && !maphaskey(names, object))
        names[object] = tostr("$format.", name);
      endif
    endfor
    return names;
  endmethod

  method _command_label owner: ARCH_WIZARD
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


  method _command_item owner: ARCH_WIZARD
    "Label an object or verb and attach its inspection action when the object exists.";
    caller == this || raise(E_PERM);
    const {object, names, ?verb_name = ""} = args;
    const label = this:_command_label(@args);
    valid(object) || return label;
    return verb_name ? $format.annotation:verb(object, verb_name, label) | $format.annotation:mk(label, ["kind" -> "object", "ref" -> $url_utils:to_curie_str(object), "objectKind" -> `object:reference_kind() ! E_VERBNF => "object"']);
  endmethod
  method _command_action owner: ARCH_WIZARD
    "Compose the visible action label and command separately from its interaction.";
    caller == this || raise(E_PERM);
    const {command, ?label = command, ?descriptor = ["kind" -> "command", "command" -> command]} = args;
    const code = $format.code:inline(command);
    const content = label == command ? code | $format.paragraph:inline($format.text:mk(label), "\n", code);
    const annotation = $format.annotation:mk(content, descriptor);
    return descriptor["kind"] == "command" && label != command ? annotation:with_action(["label" -> label]) | annotation;
  endmethod

  method _command_review_link owner: ARCH_WIZARD
    "Open the review directly in Meadow; show the same authored command in plain output.";
    caller == this || raise(E_PERM);
    const {record, label, ?row = ""} = args;
    const id = maphaskey(record, "id") ? record["id"] | record["review_id"];
    let descriptor = ["kind" -> "change", "provider" -> $url_utils:to_curie_str(this), "review" -> id, "generation" -> record["generation"]];
    row && (descriptor["row"] = row);
    return row ? $format.annotation:mk($format.code:inline(label), descriptor) | this:_command_action(tostr("@changes diff ", id), label, descriptor);
  endmethod

  method _command_overview owner: ARCH_WIZARD
    "Show the default package's active review or fetch action, with command and help links.";
    caller == this || raise(E_PERM);
    const package = this.packages[this.default_package];
    let output = {$format.title:mk(tostr(this.default_package, " changes"), 3)};
    const active = package["active"];
    if (active)
      const current = this:status(active);
      output = this:_command_status(current);
    else
      output = {@output, $format.paragraph:mk("Check upstream for updates, compare the code, then choose what to apply."), $format.list:actions({this:_command_action("@changes fetch", "Check for updates")})};
    endif
    return {@output,
      $format.list:mk({
        $format.paragraph:inline($help_utils:command_usage("@changes diff <review ID>", player, {"any", "any", "any"}), " — Review changes."),
        $format.paragraph:inline($help_utils:command_usage("@changes apply <review ID> <generation>", player, {"any", "any", "any"}), " — Apply your choices."),
        $format.paragraph:inline($help_utils:command_usage("@changes status <review ID>", player, {"any", "any", "any"}), " — Check progress.")}),
      $format.list:actions({this:_command_action("help @changes", "Command reference", ["kind" -> "help", "provider" -> $url_utils:to_curie_str($admin_features.help_source), "topic" -> "@changes"])})};
  endmethod

  method _command_progress owner: ARCH_WIZARD
    "Send a replaceable status to the initiating connection; keep command results as formatting fragments.";
    caller == this || raise(E_PERM);
    const {summary} = args;
    const lines = this:_command_status(summary);
    summary["status"] in {"fetching", "applying"} || return lines;
    const destination = `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return lines;
    const id = summary["review_id"];
    const record = this.pending[id];
    const previous = `record["notify"] ! E_RANGE => []';
    if (typeof(previous) == TYPE_MAP && `previous["task"] ! E_RANGE => 0' == record["task"])
      return lines;
    endif
    const rewrite_id = uuid();
    this.pending[id]["notify"] = ["rewrite_id" -> rewrite_id, "connection" -> destination, "task" -> record["task"]];
    const fallback = tostr("Still waiting. Check progress with @changes status ", id, ".");
    const event = $event:mk_info(record["actor"], $format.block:mk(@lines)):with_rewritable(rewrite_id, 300, fallback):with_presentation_hint('processing);
    record["actor"]:inform_connection(destination, event);
    return {};
  endmethod

  method _command_status owner: ARCH_WIZARD
    "Show progress and the next useful action.";
    caller == this || raise(E_PERM);
    const {summary} = args;
    const id = summary["review_id"];
    const generation = summary["generation"];
    const status = summary["status"];
    let output = {$format.title:mk(tostr(summary["package"], " · Review ", id, " · Generation ", generation), 3)};
    if (status == "fetching")
      return {@output, $format.paragraph:mk(tostr("Checking ", summary["package"], " for updates…"))};
    elseif (status == "ready")
      return {@output, $format.list:actions({this:_command_review_link(summary, "Review changes")})};
    elseif (status == "applying")
      return {@output, $format.paragraph:mk(tostr("Applying changes to ", summary["package"], "…"))};
    elseif (status == "complete")
      return {@output, $format.paragraph:mk("Changes applied.")};
    elseif (status == "partial")
      return {@output, $format.paragraph:mk("Changes applied. Some were skipped."), $format.list:actions({this:_command_review_link(summary, "Review remaining changes")})};
    endif
    const error = summary["error"];
    const message = `error["message"] ! E_RANGE => `error["code"] ! E_RANGE => "No error details available."'';
    output = {@output, $format.paragraph:mk(tostr(status == "failed" ? "Fetch failed: " | "Update stopped: ", message))};
    if (index(message, "No worker available for git"))
      output = {@output, $format.paragraph:mk("The server needs a Git worker. Start one, then fetch again.")};
    elseif (index(message, "No worker available for curl"))
      output = {@output, $format.paragraph:mk("The server needs a curl worker. Start one, then fetch again.")};
    endif
    if (status == "failed")
      return {@output, $format.list:actions({this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard failed review")})};
    endif
    return {@output, $format.list:actions({this:_command_action(tostr("@changes refresh ", id, " ", generation), "Refresh review (clears choices)"), this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard review")})};
  endmethod

  method _command_diff owner: ARCH_WIZARD
    "Show program choices and local items absent from the fetched source.";
    caller == this || raise(E_PERM);
    const {record, offset} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG, "Page offset must be a positive number.");
    const id = record["id"];
    const generation = record["generation"];
    const rows = record["report"]["rows"];
    const counts = record["report"]["counts"];
    const names = this:_command_names();
    let output = {$format.title:mk(tostr(record["package"], " · Review ", id, " · Generation ", generation), 3)};
    const labels = ["upstream" -> {"upstream update", "upstream updates"}, "local" -> {"local edit", "local edits"}, "conflict" -> {"conflict", "conflicts"}, "converged" -> {"program already matches upstream", "programs already match upstream"}, "unbased" -> {"program without a baseline", "programs without a baseline"}, "incoming_only" -> {"new member", "new members"}];
    let summary = "";
    for classification in ({"upstream", "local", "conflict", "converged", "unbased", "incoming_only"})
      const count = `counts[classification] ! E_RANGE => 0';
      if (count)
        summary = tostr(summary, summary ? "; " | "", count, " ", labels[classification][count == 1 ? 1 | 2]);
      endif
    endfor
    const unchanged = `counts["unchanged"] ! E_RANGE => 0';
    summary = tostr(summary, summary ? ". " | "", unchanged, " members match the accepted baseline and upstream.");
    output = {@output, $format.paragraph:mk(summary)};
    for group in (record["report"]["groups"])
      const dependency = group["requires"] ? tostr(" Requires: ", toliteral(group["requires"]), ".") | "";
      output = {@output, $format.paragraph:mk(tostr("Group ", group["name"], ": ", length(group["members"]), " changes must apply together.", dependency))};
    endfor
    for problem in (`record["group_validation"] ! E_RANGE => {}')
      output = {@output, $format.paragraph:mk(problem["message"])};
    endfor
    let change_rows = {};
    let next_offset = 0;
    const choice_counts = this:_review_decisions(record);
    const selected = choice_counts["selected"];
    const unresolved = choice_counts["unresolved"];
    const descriptions = ["upstream" -> "Upstream edit", "local" -> "Local edit", "conflict" -> "Conflict", "converged" -> "Already matches", "unbased" -> "No baseline", "unchanged" -> "Unchanged", "incoming_only" -> "New member"];
    const decisions = ["incoming" -> "Use upstream", "local" -> "Keep local", "edited" -> "Use edited program", "defer" -> "Skip", "unresolved" -> "Choose"];
    const reasons = ["adoption_required" -> "Adopt a baseline first", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match", "change_group_required" -> "Declare the required change group first", "change_group_member_missing" -> "A required group member is missing", "verb_name_collision" -> "A local or inherited verb uses a conflicting name", "initialization_required" -> "Review the initial value, then use @changes initialize", "initial_value_required" -> "Supply an explicit initial value", "inherited_effects_require_approval" -> "Explicitly permit descendants to inherit this property", "property_name_collision" -> "An existing property conflicts with this addition"];
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
        decision = {`reasons[reason] ! E_RANGE => reason' for reason in (row["blockers"])}:join("; ");
      elseif (invalid)
        decision = "Edited program has errors";
      elseif (kind == "incoming" && (record["request"]["operation"] == "adopt" || row["classification"] in {"unchanged", "converged"}))
        decision = "Record baseline";
      elseif (kind == "defer" && row["classification"] in {"local", "unchanged"})
        decision = "Keep local";
      endif
      const label = row["field"] == "property" ? this:_command_label(row["object"], names) + "." + row["names"][1] | this:_command_label(row["object"], names, row["names"][1]);
      const link = this:_command_review_link(record, label, row["id"]);
      const displayed = $format.paragraph:inline(tostr(i, ". "), link, " — ", descriptions[row["classification"]], ". ", decision, ".");
      value_bytes(change_rows) + value_bytes(displayed) <= 60000 || raise(E_QUOTA, "Review page is too large.");
      change_rows = {@change_rows, displayed};
    endfor
    if (change_rows)
      output = {@output, $format.list:mk(change_rows)};
    else
      output = {@output, $format.paragraph:mk(offset == 1 ? "No selected changes to apply." | "No more changes on this page.")};
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
        local_rows = {@local_rows, this:_command_label(object, names, verb_name)};
      endif
    endfor
    if (local_rows)
      output = {@output, $format.title:mk("Only in this MOO", 4), $format.list:mk(local_rows), $format.paragraph:mk("Absent from the fetched source. These will be kept.")};
      if (local_total > length(local_rows))
        output = {@output, $format.paragraph:mk(tostr("Showing ", length(local_rows), " of ", local_total, ". See review details for the full list."))};
      endif
    endif
    let commands = {this:_command_review_link(record, "Open code review")};
    if (next_offset)
      commands = {@commands, this:_command_action(tostr("@changes diff ", id, " ", next_offset), "Next page")};
    endif
    if (unresolved)
      output = {@output, $format.paragraph:mk(tostr(unresolved, unresolved == 1 ? " member or group needs a choice before applying." | " members or groups need choices before applying."))};
    elseif (selected)
      commands = {@commands, this:_command_action(tostr("@changes apply ", id, " ", generation), "Apply choices")};
    endif
    commands = {@commands, this:_command_action(tostr("@changes details ", id), "Source and checks"), this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard review")};
    output = {@output, $format.list:actions(commands)};
    return output;
  endmethod

  method _command_details owner: ARCH_WIZARD
    "Keep source provenance and checks out of the main review.";
    caller == this || raise(E_PERM);
    const {record, offset} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG, "Page offset must be a positive number.");
    const id = record["id"];
    const names = this:_command_names();
    let output = {$format.title:mk(tostr(record["package"], " · Review ", id, " details"), 3), $format.paragraph:mk(tostr("Generation ", record["generation"], "."))};
    const provenance = record["provenance"];
    let source = {};
    for field in ({"repository", "revision", "commit", "path", "url", "etag", "digest"})
      if (maphaskey(provenance, field))
        let value = provenance[field];
        if (field == "revision" && typeof(value) == TYPE_MAP)
          value = `value["ref"] ! E_RANGE => value["commit"]';
        endif
        source = {@source, {field, field in {"repository", "url"} ? $format.link:external(value) | tostr(value)}};
      endif
    endfor
    if (source)
      output = {@output, $format.title:mk("Source", 4), $format.table:mk({"Field", "Value"}, source)};
    endif
    let checks = {};
    const reasons = ["adoption_required" -> "Adopt a baseline first", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match", "change_group_required" -> "Declare the required change group first", "change_group_member_missing" -> "A required group member is missing", "verb_name_collision" -> "A local or inherited verb uses a conflicting name", "initialization_required" -> "Review the initial value, then use @changes initialize", "initial_value_required" -> "Supply an explicit initial value", "inherited_effects_require_approval" -> "Explicitly permit descendants to inherit this property", "property_name_collision" -> "An existing property conflicts with this addition"];
    for row in (record["report"]["rows"])
      if (!row["eligible"])
        const explanation = {`reasons[reason] ! E_RANGE => reason' for reason in (row["blockers"])}:join("; ");
        checks = {@checks, {this:_command_item(row["object"], names, row["names"][1]), tostr(explanation, row["classification"] == "unchanged" ? " (member unchanged)" | "")}};
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
        output = {@output, $format.title:mk("Checks", 4), $format.table:mk({"Item", "Check"}, page)};
      endif
      if (offset + length(page) <= length(checks))
        output = {@output, $format.list:actions({this:_command_action(tostr("@changes details ", id, " ", offset + length(page)), "More checks")})};
      endif
    endif
    output = {@output, $format.paragraph:mk("Apply updates selected programs and creates approved verbs and properties. Existing property values and object attributes stay unchanged.")};
    if (property_objects)
      output = {@output, $format.paragraph:mk(tostr("The source contains property declarations on ", property_objects, " objects; this isn’t a count of property changes."))};
    endif
    return {@output, $format.list:actions({this:_command_action(tostr("@changes diff ", id), "Back to review")})};
  endmethod

  method _command_row owner: ARCH_WIZARD
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

  method command owner: ARCH_WIZARD
    "Terminal wrapper for the versioned review service; writes require a displayed generation.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    let {words} = args;
    if (!words)
      return this:_command_overview();
    endif
    const action = words[1];
    action in {"packages", "package", "upstream", "fetch", "stage", "adopt", "status", "diff", "details", "source", "base", "apply", "refresh", "discard", "resolve", "group", "select-group", "skip-group", "initialize"} || raise(E_INVARG, tostr("Unknown @changes command: ", action, ". Use help @changes."));
    if (action == "upstream" && length(words) >= 2)
      const git_index = words[2] == "git" ? 2 | 3;
      if (length(words) >= git_index && words[git_index] == "git")
        length(words) in {git_index + 2, git_index + 3} || raise(E_INVARG, "Expected Git repository, full ref or commit, and optional path.");
        const name = git_index == 2 ? this.default_package | words[2];
        const revision = words[git_index + 2];
        const key = index(revision, "sha1:", 1) == 1 ? "commit" | "ref";
        const path = length(words) == git_index + 3 ? words[git_index + 3] | "";
        this:upstream(name, ["transport" -> "git", "repository" -> words[git_index + 1], "revision" -> [key -> revision], "path" -> path], this.packages[name]["generation"]);
        return {"Git upstream configured."};
      endif
    endif
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
        const review = package["active"] ? this:_command_action(tostr("@changes diff ", package["active"]), tostr("Review ", package["active"])) | "None";
        rows = {@rows, {name, length(package["objects"]), source, review}};
      endfor
      return {$format.title:mk("Change packages", 3), $format.table:mk({"Package", "Objects", "Upstream", "Review"}, rows)};
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
      return this:_command_progress(result);
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
    if (action == "initialize" && length(words) in {4, 5})
      length(words) == 4 || words[5] == "descendants" || raise(E_INVARG);
      return this:_command_status(this:initialize_property(id, generation, this:_command_row(auth, id, generation, words[4]), length(words) == 5));
    endif
    if (action == "group" && length(words) >= 5)
      let members = {};
      let requires = {};
      const record = this:_get(auth, id, generation);
      for word in (words[5..$])
        if (index(word, "requires=") == 1)
          requires = explode(word[10..$], ",", false);
        else
          const row_id = this:_command_row(auth, id, generation, word);
          for row in (record["report"]["rows"])
            if (row["id"] == row_id)
              members = {@members, row["member"]};
            endif
          endfor
        endif
      endfor
      return this:_command_status(this:set_group(id, generation, words[4], members, requires));
    endif
    if (action in {"select-group", "skip-group"} && length(words) == 4)
      return this:_command_status(this:resolve_group(id, generation, words[4], action == "select-group" ? "incoming" | "defer"));
    endif
    if (action == "base" && length(words) == 4)
      this:base_source(id, generation, this:_command_row(auth, id, generation, words[4]));
      return this:_command_status(this:status(id));
    endif
    if (action == "source" && length(words) in {5, 6})
      const detail = this:details(id, generation, this:_command_row(auth, id, generation, words[4]));
      words[5] in {"live", "incoming", "base"} || raise(E_INVARG);
      if (words[5] == "base")
        detail["row"]["base_text_available"] || this:_error("missing_base_source", "Load accepted source before reading it.");
      endif
      const lines = explode(detail["row"][tostr(words[5], "_text")], "\n", true);
      const offset = length(words) == 6 ? toint(words[6]) | 1;
      offset >= 1 && offset <= length(lines) || raise(E_INVARG);
      let code = {};
      for i in [offset..min(offset + 49, length(lines))]
        value_bytes(code) + value_bytes(lines[i]) <= 60000 || raise(E_QUOTA, "Source page is too large.");
        code = {@code, tostr(i, ": ", lines[i])};
      endfor
      const names = this:_command_names();
      let output = {$format.title:mk(tostr(words[5] == "live" ? "Local contents" | words[5] == "base" ? "Accepted baseline" | "Upstream contents"), 3), $format.paragraph:mk(this:_command_review_link(detail, this:_command_label(detail["row"]["object"], names, detail["row"]["names"][1]), detail["row"]["id"])), $format.code:mk(code, "moo")};
      if (offset == 1 && words[5] == "incoming" && maphaskey(detail["row"], "definition_text"))
        output = {$format.paragraph:mk("Apply creates this verb with the following definition. Flags: r permits reading, x permits calls from programs, and d enables error reporting. Arguments control command matching."), $format.code:mk(explode(detail["row"]["definition_text"], "\n", true)), @output};
      endif
      if (offset + 50 <= length(lines))
        output = {@output, $format.list:actions({this:_command_action(tostr("@changes source ", id, " ", generation, " ", words[4], " ", words[5], " ", offset + 50), "More source")})};
      endif
      return {@output, $format.list:actions({this:_command_action(tostr("@changes diff ", id), "Back to review")})};
    endif
    if (action == "apply" && length(words) == 3)
      const result = this:apply(id, generation);
      return this:_command_progress(result);
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
      let output = {$format.title:mk(tostr("Review ", id, " · Generation ", result["generation"]), 3), $format.paragraph:mk(tostr(labels[words[5]], " saved for row ", words[4], "."))};
      for validation in (result["validation"])
        if (validation["id"] == row_id && !validation["valid"])
          output = {@output, $format.paragraph:mk(tostr("Program error at line ", validation["line"], ": ", validation["message"]))};
        endif
      endfor
      return {@output, $format.list:actions({this:_command_action(tostr("@changes diff ", id), "Review choices")})};
    endif
    raise(E_INVARG, "Use help @changes for command syntax.");
  endmethod
endobject
