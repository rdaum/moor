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
        "CHANGE_TOOLS" -> CHANGE_TOOLS,
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
      "fields" -> {"program", "property", "object"},
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
        CHANGE_TOOLS,
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
  property terminal_contexts (owner: ARCH_WIZARD, flags: "") = [];
  property terminal_drafts (owner: ARCH_WIZARD, flags: "") = [];
  property next_terminal_draft (owner: ARCH_WIZARD, flags: "") = 1;

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
    return ["schema" -> 1, "review_id" -> record["id"], "generation" -> record["generation"], "package" -> record["package"], "status" -> record["status"], "task" -> record["task"], "error" -> record["error"], "provenance" -> record["provenance"], "recovery_count" -> length(`record["recovery"] ! E_RANGE => []')];
  endmethod

  method capabilities owner: ARCH_WIZARD
    "Describe the versioned review API and its limits.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    return ["schema" -> 1, "operations" -> {"packages", "history", "configure", "propose_objects", "upstream", "stage", "status", "review", "inspection", "diagnostics", "details", "base_source", "set_group", "initialize_property", "resolve_group", "resolve", "prepare_apply", "apply", "refresh", "recovery", "result", "finish", "discard"}, "fields" -> {"program", "property", "object"}, "choices" -> {"incoming", "local", "edited", "defer"}, "transports" -> {"upload", "http", "git"}, "git_authorization" -> "wizard", "authorization" -> "administrator", "default_package" -> this.default_package, "max_source_bytes" -> 4194304, "max_pending_bytes" -> 33554432, "max_packages" -> 32, "page_rows" -> 50, "max_detail_bytes" -> 524288, "max_page_bytes" -> 262144, "receipt_decisions" -> 50, "retained_reviews" -> 64, "max_receipt_bytes" -> 33554432];
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
    const {name, ?offset = 1} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    maphaskey(this.packages, name) || raise(E_INVARG);
    let reviews = {};
    for id in (mapkeys(this.receipts))
      const receipt = this.receipts[id];
      if (receipt["package"] == name && (receipt["actor"] == auth[1] || auth[1].wizard))
        reviews = {@reviews, ["review_id" -> id, "generation" -> receipt["generation"], "status" -> receipt["status"], "completed_at" -> receipt["completed_at"], "decision_count" -> receipt["decision_count"], "actor" -> receipt["actor"], "provenance" -> receipt["provenance"]]};
      endif
    endfor
    const page = reviews[offset..min(offset + 49, length(reviews))];
    return ["schema" -> 1, "package" -> name, "total" -> length(reviews), "reviews" -> page, "next" -> offset + length(page) <= length(reviews) ? offset + length(page) | 0];
  endmethod

  method configure owner: ARCH_WIZARD
    "Create or conditionally replace explicit package bindings and review policy.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {name, objects, constants, ?url = "", ?expected = 0, ?trusted_owners = {}} = args;
    typeof(name) == TYPE_STR && length(name) > 0 && length(name) <= 64 || raise(E_INVARG);
    typeof(objects) == TYPE_LIST && length(objects) <= 4096 || raise(E_INVARG);
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
    const package = ["schema" -> 1, "generation" -> generation + 1, "objects" -> objects, "fields" -> {"program", "property", "object"}, "constants" -> constants, "trusted_owners" -> trusted_owners, "upstream" -> upstream, "active" -> 0];
    let configured = package;
    if (maphaskey(this.packages, name))
      configured["policy"] = `this.packages[name]["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
      configured["members"] = `this.packages[name]["members"] ! E_RANGE => []';
    endif
    this.packages[name] = configured;
    return configured;
  endmethod

  method propose_objects owner: ARCH_WIZARD
    "Bind proposed objects before preview; initial contents apply only at absent addresses.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    auth[1].wizard || raise(E_PERM);
    const {name, expected, bindings, ?complete = false} = args;
    typeof(bindings) == TYPE_MAP && length(bindings) <= 4096 && complete in {true, false, 0, 1} || raise(E_INVARG);
    let package = this.packages[name];
    package["active"] == 0 || this:_error("active_review", "Discard the active review before changing object bindings.");
    package["generation"] == expected || this:_error("stale_generation", "Package settings changed.");
    let policy = `package["policy"] ! E_RANGE => ["schema" -> 1, "groups" -> {}]';
    const previous = `package["proposed_bindings"] ! E_RANGE => []';
    const old_ids = mapvalues(previous);
    let objects = {object for object in (package["objects"]) if (!(object in old_ids))};
    let constants = package["constants"];
    for constant in (mapkeys(previous))
      constants = mapdelete(constants, constant);
    endfor
    let creations = {entry for entry in (`policy["creations"] ! E_RANGE => {}') if (maphaskey(entry, "installed"))};
    for constant in (mapkeys(bindings))
      typeof(constant) == TYPE_STR && match(constant, "^[A-Za-z_][A-Za-z0-9_]*$") || raise(E_INVARG, "Use objdef constant names for new object bindings.");
      const object = bindings[constant];
      typeof(object) == TYPE_OBJ && `object == toobj(toint(object)) ! ANY => false' && toint(object) >= 0 && !(object in objects) || raise(E_INVARG, "Choose distinct numeric object IDs for proposed objects.");
      !maphaskey(constants, constant) || this:_error("existing_binding", "This constant already binds an existing object. Choose a new constant name.");
      for other in (mapkeys(this.packages))
        other == name || !(object in this.packages[other]["objects"]) || this:_error("overlapping_package", "Another package already manages this object ID.");
      endfor
      constants[constant] = object;
      objects = {@objects, object};
      creations = {@creations, ["object" -> object, "mode" -> "absent"]};
    endfor
    length(objects) <= 4096 || raise(E_INVARG);
    policy["creations"] = creations;
    policy["complete"] = complete ? objects | {};
    package["policy"] = policy;
    package["objects"] = objects;
    package["constants"] = constants;
    package["proposed_bindings"] = bindings;
    package["generation"] = expected + 1;
    this.packages[name] = package;
    return package;
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
    record = this:_reconcile_choices(record, report);
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
    record["request"]["fields"] = {"program", "property", "object"};
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
      return this:_receipt_page(receipt, 1);
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
      summary["last_receipt"] = this:_receipt_page(record["last_receipt"], 1);
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
      if (this:_needs_review(record, row["id"]))
        unresolved = unresolved + 1;
      elseif (!row["eligible"])
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
      row["label"] = row["field"] == "object" ? this:_command_label(row["object"], names) | row["field"] == "property" ? this:_command_label(row["object"], names) + "." + row["names"][1] | this:_command_label(row["object"], names, row["names"][1]);
      row["choice"] = choice ? ["choice" -> choice["choice"], "validated" -> maphaskey(choice, "validation")] | [];
      row["needs_review"] = this:_needs_review(record, row["id"]);
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
    let groups = {};
    let row_index = [];
    for indexed in (rows)
      row_index[indexed["id"]] = indexed;
    endfor
    for group in (record["report"]["groups"])
      let selected = 0;
      let needs_review = false;
      for member in (group["members"])
        const member_row = row_index[member];
        const decision = `record["choices"][member]["choice"] ! E_RANGE => member_row["default"]';
        if (decision in {"incoming", "local", "edited"})
          selected = selected + 1;
        endif
        needs_review = needs_review || this:_needs_review(record, member) || decision == "unresolved" || (decision == "edited" && !maphaskey(record["choices"][member], "validation"));
      endfor
      const state = !group["complete"] ? "blocked" | needs_review ? "needs_review" | selected == 0 ? "deferred" | selected == length(group["members"]) ? "selected" | "partial";
      groups = {@groups, ["name" -> group["name"], "requires" -> group["requires"], "complete" -> group["complete"], "count" -> length(group["members"]), "selected" -> selected, "status" -> state]};
    endfor
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
    let bundled = {};
    for update in (record["report"]["rows"])
      if (`update["operation"] ! E_RANGE => ""' == "add_object")
        bundled = {@bundled, update["object"]};
      endif
      if (`update["operation"] ! E_RANGE => ""' in {"add_verb", "add_property", "retain_property"})
        managed[tostr(update["object"], "/", update["field"], "/", update["names"][1])] = true;
      endif
    endfor
    let inspected = {};
    let counts = report["counts"];
    for inspected_row in (report["rows"])
      const member = inspected_row["field"] == "program" ? inspected_row["name"][index(inspected_row["name"], ":") + 1..$] | inspected_row["name"];
      if (inspected_row["object"] in bundled || maphaskey(managed, tostr(inspected_row["object"], "/", inspected_row["field"], "/", member)))
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
        row["label"] = row["field"] == "object" ? this:_command_label(row["object"], this:_command_names()) | row["field"] == "property" ? this:_command_label(row["object"], this:_command_names()) + "." + row["names"][1] | this:_command_label(row["object"], this:_command_names(), row["names"][1]);
        row["base_fetch_available"] = auth[1].wizard && `row["base_source"]["transport"] == "git" ! E_RANGE => false';
        row["choice"] = `record["choices"][row_id] ! E_RANGE => []';
        row["recovery"] = `record["recovery"][row_id] ! E_RANGE => []';
        row["needs_review"] = this:_needs_review(record, row_id);
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
    "Declare changes that must apply together while preserving saved decisions.";
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

  method _needs_review owner: ARCH_WIZARD
    "Distinguish changed decisions from drafts saved when their group was deliberately deferred.";
    caller == this || raise(E_PERM);
    const {record, id} = args;
    const saved = `record["recovery"][id] ! E_RANGE => []';
    return length(saved) > 0 && `saved["needs_review"] ! E_RANGE => true';
  endmethod

  method _reconcile_choices owner: ARCH_WIZARD
    "Retain decisions whose member identity and live revision still match; keep other drafts for recovery.";
    caller == this || raise(E_PERM);
    let {record, report} = args;
    let choices = [];
    let recovery = `record["recovery"] ! E_RANGE => []';
    let bases = [];
    const previous = `record["report"]["rows"] ! E_RANGE => {}';
    let members = [];
    for current in (report["rows"])
      const candidates = `members[current["member"]] ! E_RANGE => {}';
      members[current["member"]] = {@candidates, current};
    endfor
    for old in (previous)
      const id = old["id"];
      const matches = `members[old["member"]] ! E_RANGE => {}';
      const same = length(matches) == 1 && matches[1]["id"] == id;
      if (same && matches[1]["base"] == old["base"] && maphaskey(`record["base_sources"] ! E_RANGE => []', id))
        bases[id] = record["base_sources"][id];
      endif
      if (!maphaskey(record["choices"], id))
        continue;
      endif
      let choice = record["choices"][id];
      if (maphaskey(recovery, id))
        if (!same || `matches[1]["revision"] != old["revision"] ! E_RANGE => true')
          recovery[id]["needs_review"] = true;
        endif
        same && (choices[id] = ["choice" -> "defer"]);
        continue;
      endif
      const unchanged = same && `matches[1]["revision"] == old["revision"] ! E_RANGE => false';
      if (unchanged && choice["choice"] in matches[1]["choices"])
        if (maphaskey(choice, "validation"))
          choice = mapdelete(choice, "validation");
        endif
        choices[id] = choice;
      else
        recovery[id] = ["id" -> id, "member" -> old["member"], "label" -> this:_member_label(old), "choice" -> choice, "reason" -> same ? "The contents or permissions changed. Check this saved decision again." | "This member is missing or has a different identity. Its saved decision was not attached to another member."];
        same && (choices[id] = ["choice" -> "defer"]);
      endif
    endfor
    record["choices"] = choices;
    record["recovery"] = recovery;
    record["base_sources"] = bases;
    const validated = preview_objdef_changes(record["sources"], record["request"], choices);
    for validation in (validated["validation"])
      if (validation["valid"])
        record["choices"][validation["id"]]["validation"] = validation["validation"];
      endif
    endfor
    record["report"] = report;
    record["group_validation"] = validated["group_validation"];
    return record;
  endmethod

  method recovery owner: ARCH_WIZARD
    "Read saved decisions that need another check, including drafts for members no longer present.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?offset = 1} = args;
    const record = this:_get(auth, id, generation);
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    const saved = `record["recovery"] ! E_RANGE => []';
    const keys = mapkeys(saved);
    let page = {};
    let next = offset;
    for i in [offset..length(keys)]
      const item = saved[keys[i]];
      if (length(page) && (value_bytes(page) + value_bytes(item) > 524288 || length(page) >= 50))
        break;
      endif
      page = {@page, item};
      next = i + 1;
    endfor
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "total" -> length(keys), "items" -> page, "next" -> next <= length(keys) ? next | 0];
  endmethod

  method _save_policy owner: ARCH_WIZARD
    "Save administrator policy while preserving decisions and recoverable drafts.";
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
    record = this:_reconcile_choices(record, report);
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
    let row_index = [];
    for indexed in (record["report"]["rows"])
      row_index[indexed["id"]] = indexed;
    endfor
    for row_id in (groups[1]["members"])
      const selected = row_index[row_id];
      if (kind == "incoming" && this:_needs_review(record, row_id))
        continue;
      endif
      if (kind == "incoming" && `record["recovery"][row_id]["group_deferred"] ! E_RANGE => false')
        record["choices"][row_id] = record["recovery"][row_id]["choice"];
        record["recovery"] = mapdelete(record["recovery"], row_id);
        if (maphaskey(`record["group_choices"] ! E_RANGE => []', row_id))
          record["group_choices"] = mapdelete(record["group_choices"], row_id);
        endif
        continue;
      endif
      if (kind == "incoming" && maphaskey(record["choices"], row_id) && !maphaskey(`record["group_choices"] ! E_RANGE => []', row_id))
        continue;
      endif
      if (kind == "defer" && `record["choices"][row_id]["choice"] != "defer" ! E_RANGE => false' && !maphaskey(`record["recovery"] ! E_RANGE => []', row_id))
        let saved = `record["recovery"] ! E_RANGE => []';
        saved[row_id] = ["id" -> row_id, "member" -> selected["member"], "label" -> this:_member_label(selected), "choice" -> record["choices"][row_id], "reason" -> "This group was deferred. Its previous decision is saved here.", "needs_review" -> false, "group_deferred" -> true];
        record["recovery"] = saved;
      endif
      record["choices"][row_id] = ["choice" -> kind == "incoming" && `selected["operation"] ! E_RANGE => ""' in {"retain_property", "retain_object"} ? "local" | kind];
      let group_choices = `record["group_choices"] ! E_RANGE => []';
      group_choices[row_id] = name;
      record["group_choices"] = group_choices;
    endfor
    const current = preview_objdef_changes(record["sources"], record["request"], record["choices"]);
    current["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before choosing.");
    for validation in (current["validation"])
      if (validation["valid"])
        record["choices"][validation["id"]]["validation"] = validation["validation"];
      endif
    endfor
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
    if (maphaskey(`record["group_choices"] ! E_RANGE => []', row_id))
      record["group_choices"] = mapdelete(record["group_choices"], row_id);
    endif
    if (maphaskey(`record["recovery"] ! E_RANGE => []', row_id))
      record["recovery"] = mapdelete(record["recovery"], row_id);
    endif
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
    "Compare saved input again while preserving decisions and drafts for review.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    let record = this:_get(auth, id, generation);
    !(record["status"] in {"fetching", "applying"}) || this:_error("busy", "Task is still running.");
    length(record["sources"]) > 0 || this:_error("missing_source", "Discard this failed fetch and stage again.");
    record["generation"] = generation + 1;
    return this:_save_ready(record, record["sources"], record["provenance"]);
  endmethod

  method apply owner: ARCH_WIZARD
    "Start one dedicated application task; success is reported only by a committed receipt.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?prepared = []} = args;
    if (maphaskey(this.receipts, id))
      return this:status(id);
    endif
    let record = this:_get(auth, id);
    if (generation in `record["last_receipt"]["request_generations"] ! E_RANGE => {}')
      return this:status(id);
    endif
    if (record["status"] == "applying")
      return this:_summary(record);
    endif
    record["generation"] == generation || this:_error("stale_generation", "Review choices changed.");
    record["status"] in {"ready", "partial"} || this:_error("not_ready", "Review is not ready.");
    !prepared || prepared == this:_apply_guard(record) || this:_error("stale_preparation", "The prepared selection changed. Read its effects again.");
    for row in (record["report"]["rows"])
      !this:_needs_review(record, row["id"]) || this:_error("decision_needs_review", "Check the saved decisions affected by changed contents before applying.");
    endfor
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

  method _apply_guard owner: ARCH_WIZARD
    "Bind an apply summary to the review selection, policy, source, and live evidence.";
    caller == this || raise(E_PERM);
    const {record} = args;
    return ["review_id" -> record["id"], "generation" -> record["generation"], "package_generation" -> record["package_generation"], "evidence" -> record["report"]["evidence"]];
  endmethod

  method _effects owner: ARCH_WIZARD
    "Describe every member's outcome, including defaults on pages the administrator has not opened.";
    caller == this || raise(E_PERM);
    const {record, validation} = args;
    let effects = {};
    const names = this:_command_names();
    let checked = [];
    for item in (validation)
      checked[item["id"]] = item;
    endfor
    for row in (record["report"]["rows"])
      const saved = `record["choices"][row["id"]] ! E_RANGE => []';
      const kind = saved ? saved["choice"] | row["default"];
      const operation = `row["operation"] ! E_RANGE => "program"';
      let effect = "deferred";
      if (!row["eligible"])
        effect = row["classification"] == "unchanged" ? "retained" | "blocked";
      elseif (kind in {"incoming", "local", "edited"})
        if (operation in {"retain_property", "retain_object"})
          effect = "retained";
        elseif (record["request"]["operation"] == "adopt")
          effect = "baseline";
        elseif (operation in {"add_object", "add_property", "add_verb"})
          effect = operation;
        elseif (kind == "local" || (kind == "incoming" && row["incoming"] == row["live"]))
          effect = "baseline";
        elseif (kind == "edited")
          const check = `checked[row["id"]] ! E_RANGE => []';
          effect = length(check) && check["valid"] && check["fingerprint"] == row["live"] ? "baseline" | "program";
        else
          effect = "program";
        endif
      elseif (row["classification"] in {"unchanged", "local"} && kind == "defer")
        effect = "retained";
      endif
      const label = row["field"] == "object" ? this:_command_label(row["object"], names) | row["field"] == "property" ? this:_command_label(row["object"], names) + "." + row["names"][1] | this:_command_label(row["object"], names, row["names"][1]);
      effects = {@effects, ["id" -> row["id"], "member" -> row["member"], "label" -> label, "field" -> row["field"], "choice" -> kind, "defaulted" -> !saved, "effect" -> effect]};
    endfor
    return effects;
  endmethod

  method prepare_apply owner: ARCH_WIZARD
    "Read exact effects before apply. Counts cover all members; the list is paginated.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?offset = 1} = args;
    const record = this:_get(auth, id, generation);
    record["status"] in {"ready", "partial", "rejected"} || this:_error("not_ready", "Review is not ready.");
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    const report = preview_objdef_changes(record["sources"], record["request"], record["choices"]);
    report["evidence"] == record["report"]["evidence"] || this:_error("stale_review", "Live state changed. Refresh before preparing an update.");
    const effects = this:_effects(record, report["validation"]);
    let counts = ["program" -> 0, "add_verb" -> 0, "add_property" -> 0, "add_object" -> 0, "baseline" -> 0, "retained" -> 0, "deferred" -> 0, "blocked" -> 0];
    for effect in (effects)
      counts[effect["effect"]] = counts[effect["effect"]] + 1;
    endfor
    const decisions = this:_review_decisions(record);
    const page = effects[offset..min(offset + 49, length(effects))];
    value_bytes(page) <= 262144 || raise(E_QUOTA, "Effect page exceeds the service limit.");
    return ["schema" -> 1, "review_id" -> id, "generation" -> generation, "prepared" -> this:_apply_guard(record), "counts" -> counts, "can_apply" -> decisions["selected"] > 0 && decisions["unresolved"] == 0, "decision_counts" -> decisions, "total" -> length(effects), "effects" -> page, "next" -> offset + length(page) <= length(effects) ? offset + length(page) | 0, "provenance" -> record["provenance"]];
  endmethod

  method _apply owner: ARCH_WIZARD
    "Commit content, baselines, and a source-free receipt together, or abort the task.";
    caller == this || raise(E_PERM);
    const {id, generation} = args;
    let record = this:_job(id, generation, "applying");
    set_task_perms(record["authority"]);
    let wrote = false;
    try
      const preview = preview_objdef_changes(record["sources"], record["request"], record["choices"]);
      const effects = this:_effects(record, preview["validation"]);
      const result = apply_objdef_changes(record["sources"], record["request"], record["report"]["evidence"], record["choices"]);
      wrote = true;
      let members = `this.packages[record["package"]]["members"] ! E_RANGE => []';
      for created in (result["created"])
        members[created["member"]] = created;
      endfor
      this.packages[record["package"]]["members"] = members;
      let policy = record["request"]["policy"];
      let creations = `policy["creations"] ! E_RANGE => {}';
      let proposals = `this.packages[record["package"]]["proposed_bindings"] ! E_RANGE => []';
      let bindings_changed = false;
      for installed in (result["created"])
        if (installed["field"] == "object")
          bindings_changed = true;
          for i in [1..length(creations)]
            if (creations[i]["object"] == installed["object"])
              creations[i]["installed"] = installed["creation"];
            endif
          endfor
          for constant in (mapkeys(proposals))
            if (proposals[constant] == installed["object"])
              proposals = mapdelete(proposals, constant);
            endif
          endfor
        endif
      endfor
      if (bindings_changed)
        policy["creations"] = creations;
        record["request"]["policy"] = policy;
        this.packages[record["package"]]["policy"] = policy;
        this.packages[record["package"]]["proposed_bindings"] = proposals;
        this.packages[record["package"]]["generation"] = this.packages[record["package"]]["generation"] + 1;
        record["package_generation"] = this.packages[record["package"]]["generation"];
      endif
      let deferred = false;
      for diagnostic in (record["report"]["diagnostics"])
        if (diagnostic["code"] in {"missing_source", "unsupported_creation", "unsupported_or_ambiguous_definition", "outside_managed_scope", "unsupported_removal"})
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
      receipt["request_generations"] = {@`record["last_receipt"]["request_generations"] ! E_RANGE => {}', generation - 1};
      receipt["provenance"] = record["provenance"];
      let labels = [];
      for effect in (effects)
        labels[effect["id"]] = effect;
      endfor
      let decisions = `record["last_receipt"]["decisions"] ! E_RANGE => {}';
      for decision in (result["decisions"])
        const effect = labels[decision["id"]];
        decision["label"] = effect["label"];
        decision["member"] = effect["member"];
        decision["field"] = effect["field"];
        decision["effect"] = effect["effect"];
        decision["applied_generation"] = generation;
        decisions = {@decisions, decision};
      endfor
      receipt["decision_count"] = length(decisions);
      receipt["decisions"] = decisions;
      receipt["source_digest"] = record["report"]["source_digest"];
      receipt["completed_at"] = time();
      if (deferred)
        this.pending[id]["last_receipt"] = receipt;
        value_bytes(this.pending) <= 33554432 || raise(E_QUOTA, "Pending review storage is full.");
      else
        this:_store_receipt(receipt);
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
    return this:_close_review(record, "discarded");
  endmethod

  method finish owner: ARCH_WIZARD
    "Finish a review without accepting deferred baselines or undoing committed changes.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation} = args;
    if (maphaskey(this.receipts, id))
      return this:status(id);
    endif
    const record = this:_get(auth, id, generation);
    !(record["status"] in {"fetching", "applying"}) || this:_error("busy", "Wait for the running operation before finishing.");
    return this:_close_review(record, "finished");
  endmethod

  method _close_review owner: ARCH_WIZARD
    "Keep earlier committed effects in history when closing a partial or abandoned review.";
    caller == this || raise(E_PERM);
    const {record, state} = args;
    let receipt = `record["last_receipt"] ! E_RANGE => ["schema" -> 1, "review_id" -> record["id"], "package" -> record["package"], "actor" -> record["actor"], "provenance" -> record["provenance"], "decision_count" -> 0, "decisions" -> {}]';
    receipt["generation"] = record["generation"] + 1;
    receipt["status"] = state;
    receipt["task"] = 0;
    receipt["error"] = [];
    receipt["completed_at"] = time();
    if (state != "discarded" || receipt["decision_count"] > 0)
      this:_store_receipt(receipt);
    endif
    this.pending = mapdelete(this.pending, record["id"]);
    this.packages[record["package"]]["active"] = 0;
    return this:_receipt_page(receipt, 1);
  endmethod

  method _store_receipt owner: ARCH_WIZARD
    "Retain whole results, evicting older reviews at 64 reviews or 32 MiB, never truncating decisions.";
    caller == this || raise(E_PERM);
    const {receipt} = args;
    value_bytes(receipt) <= 33554432 || raise(E_QUOTA, "The complete result exceeds receipt storage. Finish this review before another apply.");
    this.receipts[receipt["review_id"]] = receipt;
    while (length(this.receipts) > 64 || value_bytes(this.receipts) > 33554432)
      let oldest = 0;
      for id in (mapkeys(this.receipts))
        if (id != receipt["review_id"] && (!oldest || this.receipts[id]["completed_at"] < this.receipts[oldest]["completed_at"]))
          oldest = id;
        endif
      endfor
      oldest || raise(E_QUOTA);
      this.receipts = mapdelete(this.receipts, oldest);
    endwhile
  endmethod

  method _receipt_page owner: ARCH_WIZARD
    "Bound returned result bodies while retaining all decisions in storage.";
    caller == this || raise(E_PERM);
    let {receipt, offset} = args;
    typeof(offset) == TYPE_INT && offset >= 1 || raise(E_INVARG);
    const all = receipt["decisions"];
    receipt["decisions"] = {};
    for i in [offset..min(offset + 49, length(all))]
      if (value_bytes(receipt) + value_bytes(all[i]) > 262000)
        length(receipt["decisions"]) || raise(E_QUOTA, "One decision exceeds the result page limit.");
        break;
      endif
      receipt["decisions"] = {@receipt["decisions"], all[i]};
    endfor
    receipt["next_decision"] = offset + length(receipt["decisions"]) <= length(all) ? offset + length(receipt["decisions"]) | 0;
    value_bytes(receipt) <= 262144 || raise(E_QUOTA, "Result page exceeds the service limit.");
    return receipt;
  endmethod

  method result owner: ARCH_WIZARD
    "Read a generation-bound page of committed decisions, including earlier partial applications.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    const {id, generation, ?offset = 1} = args;
    let receipt = [];
    if (maphaskey(this.receipts, id))
      receipt = this.receipts[id];
      receipt["actor"] == auth[1] || auth[1].wizard || raise(E_PERM);
    else
      const record = this:_get(auth, id, generation);
      receipt = `record["last_receipt"] ! E_RANGE => []';
      length(receipt) || this:_error("no_result", "This review has not applied any changes.");
      receipt["generation"] = record["generation"];
    endif
    receipt["generation"] == generation || this:_error("stale_generation", "The result changed. Read its status again.");
    return this:_receipt_page(receipt, offset);
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
        $format.paragraph:inline($help_utils:command_usage("@changes diff", player, {"any", "any", "any"}), " — Review changes."),
        $format.paragraph:inline($help_utils:command_usage("@changes preview", player, {"any", "any", "any"}), " — Read what applying your choices will do."),
        $format.paragraph:inline($help_utils:command_usage("@changes status", player, {"any", "any", "any"}), " — Check progress.")}),
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
    elseif (status in {"finished", "discarded"})
      return {@output, $format.paragraph:mk("Review closed. Earlier committed changes remain installed. Deferred contents and baselines stay unchanged."), $format.list:actions({this:_command_action(tostr("@changes history ", summary["package"]), "Read update history"), this:_command_action(tostr("@changes fetch ", summary["package"]), "Check for another update")})};
    elseif (status == "partial")
      return {@output, $format.paragraph:mk("Changes applied. Some were skipped."), $format.list:actions({this:_command_review_link(summary, "Review remaining changes")})};
    endif
    const error = summary["error"];
    const message = `error["message"] ! E_RANGE => `error["code"] ! E_RANGE => "No error details available."'';
    output = {@output, $format.paragraph:mk(tostr(status == "failed" ? "Fetch failed: " | "Update stopped: ", message))};
    if (index(message, "No worker available for git"))
      output = {@output, $format.paragraph:mk("The server needs a Git worker. Start one, then use @changes status and @changes retry.")};
    elseif (index(message, "No worker available for curl"))
      output = {@output, $format.paragraph:mk("The server needs a curl worker. Start one, then use @changes status and @changes retry.")};
    endif
    if (status == "failed")
      return {@output, $format.list:actions({this:_command_action("@changes status", "Read the current failure"), this:_command_action("@changes retry", "Retry after fixing the source or worker"), this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard failed review")})};
    endif
    return {@output, $format.list:actions({this:_command_action(tostr("@changes refresh ", id, " ", generation), "Refresh review"), this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard review")})};
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
    let displayed_rows = [];
    let next_offset = 0;
    const choice_counts = this:_review_decisions(record);
    const selected = choice_counts["selected"];
    const unresolved = choice_counts["unresolved"];
    const descriptions = ["upstream" -> "Upstream edit", "local" -> "Local edit", "conflict" -> "Conflict", "converged" -> "Already matches", "unbased" -> "No baseline", "unchanged" -> "Unchanged", "incoming_only" -> "New member"];
    const decisions = ["incoming" -> "Use upstream", "local" -> "Keep local", "edited" -> "Use edited program", "defer" -> "Skip", "unresolved" -> "Choose"];
    const reasons = ["adoption_required" -> "Accepted upstream history is unknown; review history repair before adoption", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match", "change_group_required" -> "Declare the required change group first", "change_group_member_missing" -> "A required group member is missing", "verb_name_collision" -> "A local or inherited verb uses a conflicting name", "initialization_required" -> "Review the initial value, then use @changes initialize", "initial_value_required" -> "Supply an explicit initial value", "inherited_effects_require_approval" -> "Explicitly permit descendants to inherit this property", "property_name_collision" -> "An existing property conflicts with this addition", "object_address_occupied" -> "Object ID is occupied; discard, choose another binding, and fetch again", "object_binding_changed" -> "Object no longer matches its creation receipt", "creation_requires_wizard" -> "Object creation requires wizard authority", "invalid_object_parent" -> "A parent is missing or inheritance contains a cycle", "invalid_object_location" -> "A location is missing or containment contains a cycle", "missing_inherited_property" -> "An override names a property no parent defines"];
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
      const label = row["field"] == "object" ? this:_command_label(row["object"], names) | row["field"] == "property" ? this:_command_label(row["object"], names) + "." + row["names"][1] | this:_command_label(row["object"], names, row["names"][1]);
      const link = this:_command_review_link(record, label, row["id"]);
      const displayed = $format.paragraph:inline(tostr(i, ". "), link, " — ", descriptions[row["classification"]], ". ", decision, ".");
      value_bytes(change_rows) + value_bytes(displayed) <= 60000 || raise(E_QUOTA, "Review page is too large.");
      change_rows = {@change_rows, displayed};
      displayed_rows[tostr(i)] = row["id"];
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
      commands = {@commands, this:_command_action("@changes preview", "Read planned effects")};
    endif
    commands = {@commands, this:_command_action(tostr("@changes details ", id), "Upstream source and limitations"), this:_command_action(tostr("@changes discard ", id, " ", generation), "Discard review")};
    output = {@output, $format.list:actions(commands)};
    this:_terminal_remember(record, displayed_rows, next_offset);
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
    const reasons = ["adoption_required" -> "Accepted upstream history is unknown; review history repair before adoption", "untrusted_target_authority" -> "Ownership or permissions block upgrades", "object_identity_mismatch" -> "Object identity doesn’t match", "change_group_required" -> "Declare the required change group first", "change_group_member_missing" -> "A required group member is missing", "verb_name_collision" -> "A local or inherited verb uses a conflicting name", "initialization_required" -> "Review the initial value, then use @changes initialize", "initial_value_required" -> "Supply an explicit initial value", "inherited_effects_require_approval" -> "Explicitly permit descendants to inherit this property", "property_name_collision" -> "An existing property conflicts with this addition", "object_address_occupied" -> "Object ID is occupied; discard, choose another binding, and fetch again", "object_binding_changed" -> "Object no longer matches its creation receipt", "creation_requires_wizard" -> "Object creation requires wizard authority", "invalid_object_parent" -> "A parent is missing or inheritance contains a cycle", "invalid_object_location" -> "A location is missing or containment contains a cycle", "missing_inherited_property" -> "An override names a property no parent defines"];
    for row in (record["report"]["rows"])
      if (!row["eligible"])
        const explanation = {`reasons[reason] ! E_RANGE => reason' for reason in (row["blockers"])}:join("; ");
        checks = {@checks, {this:_command_item(row["object"], names, row["names"][1]), tostr(explanation, row["classification"] == "unchanged" ? " (member unchanged)" | "")}};
      endif
    endfor
    let property_objects = 0;
    const diagnostic_labels = ["missing_source" -> "No upstream object definition", "live_definition_unmatched" -> "No matching upstream verb definition", "unsupported_creation" -> "Declare a proposed object binding before reviewing creation", "outside_managed_scope" -> "Object is outside this package scope", "unsupported_removal" -> "Complete source omits a member; removal is unsupported", "unsupported_or_ambiguous_definition" -> "Upstream verb can’t be matched; definition changes aren’t supported", "definition_fields_unmanaged" -> "Owner/flags differ, or upstream includes verb metadata"];
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
    output = {@output, $format.paragraph:mk("Apply updates selected programs and creates approved objects, verbs, and properties. Existing property values and object attributes stay unchanged.")};
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

  method _member_label owner: ARCH_WIZARD
    "Retain a readable member name even when the member disappears after refresh.";
    caller == this || raise(E_PERM);
    const {row} = args;
    const object = this:_command_label(row["object"], this:_command_names());
    return row["field"] == "object" ? object | object + (row["field"] == "property" ? "." | ":") + row["names"][1];
  endmethod

  method _terminal_key owner: ARCH_WIZARD
    "Keep displayed choices separate for each actor and connection.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    return tostr(player, "/", `connection() ! E_INVARG => player');
  endmethod

  method _terminal_context owner: ARCH_WIZARD
    "Read only the review version displayed on this connection.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const context = `this.terminal_contexts[this:_terminal_key()] ! E_RANGE => []';
    context || this:_error("no_displayed_review", "Open a review with @changes diff first. Use @changes packages to find another package.");
    return context;
  endmethod

  method _terminal_remember owner: ARCH_WIZARD
    "Record displayed row identities. Merely reading status must not rebind old row numbers.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {record, ?displayed = [], ?next = -1} = args;
    const key = this:_terminal_key();
    let context = `this.terminal_contexts[key] ! E_RANGE => []';
    const id = `record["id"] ! E_RANGE => record["review_id"]';
    if (!context || context["id"] != id || context["generation"] != record["generation"])
      context = ["id" -> id, "generation" -> record["generation"], "rows" -> [], "prepared" -> [], "next" -> 0];
    endif
    for number in (mapkeys(displayed))
      context["rows"][number] = displayed[number];
    endfor
    next >= 0 && (context["next"] = next);
    context["updated"] = time();
    let contexts = this.terminal_contexts;
    contexts[key] = context;
    value_bytes(context) <= 8388608 || this:_error("review_too_large", "This terminal review exceeds 8 MiB. Use explicit review and member IDs.");
    while (length(contexts) > 128 || value_bytes(contexts) > 8388608)
      let oldest = "";
      for candidate in (mapkeys(contexts))
        if (candidate != key && (!oldest || contexts[candidate]["updated"] < contexts[oldest]["updated"]))
          oldest = candidate;
        endif
      endfor
      contexts = mapdelete(contexts, oldest);
    endwhile
    this.terminal_contexts = contexts;
  endmethod

  method _terminal_row owner: ARCH_WIZARD
    "Resolve a shortcut against a displayed stable row, never a fresh row position.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {auth, context, selector} = args;
    try
      this:_get(auth, context["id"], context["generation"]);
    except failure (E_INVARG)
      this:_error(`failure[3]["code"] ! E_RANGE => "stale_generation"', "This review changed. Use @changes diff to check the current list before choosing again.");
    endtry
    const number = toint(selector);
    tostr(number) == selector && maphaskey(context["rows"], selector) || this:_error("row_not_displayed", "That row is not on a page you have opened. Use @changes diff or @changes next first.");
    return context["rows"][selector];
  endmethod

  method _terminal_effect_names owner: ARCH_WIZARD
    "Use the same concrete effect descriptions for plans and committed results.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    return ["program" -> "Replace program", "add_verb" -> "Add verb", "add_property" -> "Create property with its initial value", "add_object" -> "Create object", "baseline" -> "Acknowledge upstream; keep live contents", "retained" -> "Keep live contents", "deferred" -> "Decide later; keep contents and baseline", "blocked" -> "Cannot apply"];
  endmethod

  method _terminal_preview owner: ARCH_WIZARD
    "Display the complete counts and one page of effects, and remember the exact prepared handle.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {context, offset} = args;
    const prepared = this:prepare_apply(context["id"], context["generation"], offset);
    const labels = this:_terminal_effect_names();
    let counts = {};
    for kind in (mapkeys(prepared["counts"]))
      const count = prepared["counts"][kind];
      count && (counts = {@counts, tostr(labels[kind], ": ", count)});
    endfor
    let effects = {};
    for item in (prepared["effects"])
      effects = {@effects, tostr(item["label"], " — ", labels[item["effect"]], item["defaulted"] ? " (automatic choice)" | "", ".")};
    endfor
    let output = {$format.title:mk("What applying will do", 3), $format.list:mk(counts), $format.paragraph:mk("Counts cover the whole review. Opening another page does not change the selection."), $format.list:mk(effects)};
    this.terminal_contexts[this:_terminal_key()]["prepared"] = prepared["can_apply"] ? prepared["prepared"] | [];
    if (prepared["next"])
      output = {@output, $format.list:actions({this:_command_action(tostr("@changes preview ", prepared["next"]), "More planned effects")})};
    endif
    if (prepared["can_apply"])
      output = {@output, $format.list:actions({this:_command_action("@changes apply", "Apply these choices")})};
    else
      output = {@output, $format.paragraph:mk("There is nothing ready to apply, or a saved choice needs attention. Use @changes diff to review it.")};
    endif
    return output;
  endmethod

  method _terminal_result owner: ARCH_WIZARD
    "Show committed effects without confusing pending decisions with installed changes.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {id, offset} = args;
    const status = this:status(id);
    const result = this:result(id, status["generation"], offset);
    const labels = this:_terminal_effect_names();
    let lines = {};
    for item in (result["decisions"])
      lines = {@lines, tostr(item["label"], " — ", labels[item["effect"]], ".")};
    endfor
    let output = {$format.title:mk(tostr("Review ", id, " results"), 3), $format.paragraph:mk(tostr(result["decision_count"], " recorded decisions. Status: ", result["status"], ".")), $format.list:mk(lines)};
    if (result["next_decision"])
      output = {@output, $format.list:actions({this:_command_action(tostr("@changes result ", id, " ", result["next_decision"]), "More recorded decisions")})};
    endif
    return output;
  endmethod

  method _terminal_compare owner: ARCH_WIZARD
    "Show a paginated local-to-upstream comparison with explicit line numbers and literal source.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {auth, context, selector, offset} = args;
    offset >= 1 || raise(E_INVARG, "The comparison offset must be positive.");
    const row = this:details(context["id"], context["generation"], this:_terminal_row(auth, context, selector))["row"];
    const local = explode(row["live_text"], "\n", true);
    const incoming = explode(row["incoming_text"], "\n", true);
    let start = 1;
    while (start <= min(length(local), length(incoming)) && local[start] == incoming[start])
      start = start + 1;
    endwhile
    let old_end = length(local);
    let new_end = length(incoming);
    while (old_end >= start && new_end >= start && local[old_end] == incoming[new_end])
      old_end = old_end - 1;
      new_end = new_end - 1;
    endwhile
    const old_count = max(0, old_end - start + 1);
    const total = old_count + max(0, new_end - start + 1);
    let lines = {};
    let next = offset;
    for position in [offset..min(offset + 39, total)]
      const removed = position <= old_count;
      const number = removed ? start + position - 1 | start + position - old_count - 1;
      const line = tostr(removed ? "- local " | "+ upstream ", number, ": ", removed ? local[number] | incoming[number]);
      if (value_bytes(lines) + value_bytes(line) > 60000)
        length(lines) || raise(E_QUOTA, "One source line exceeds the terminal page limit.");
        break;
      endif
      lines = {@lines, line};
      next = position + 1;
    endfor
    let output = {$format.title:mk(row["label"], 3), $format.paragraph:mk(total ? "Minus lines are local code; plus lines are upstream code. Matching leading and trailing lines are omitted." | "Local and upstream contents match."), $format.code:mk(lines, "moo")};
    if (next <= total)
      output = {@output, $format.list:actions({this:_command_action(tostr("@changes compare ", selector, " ", next), "More comparison")})};
    endif
    return {@output, $format.paragraph:mk(tostr("Use @changes incoming ", selector, " to use upstream; @changes keep ", selector, " to keep your code and acknowledge upstream; or @changes later ", selector, " to leave both code and baseline unchanged."))};
  endmethod

  method _terminal_source owner: ARCH_WIZARD
    "Page literal source without requiring a browser or clickable actions.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {title, lines, offset, command} = args;
    offset >= 1 || raise(E_INVARG, "The source offset must be positive.");
    let page = {};
    let next = offset;
    for i in [offset..min(offset + 39, length(lines))]
      const line = tostr(i, ": ", lines[i]);
      if (value_bytes(page) + value_bytes(line) > 60000)
        length(page) || raise(E_QUOTA, "One source line exceeds the terminal page limit.");
        break;
      endif
      page = {@page, line};
      next = i + 1;
    endfor
    let output = {$format.title:mk(title, 3), $format.code:mk(page, "moo")};
    if (next <= length(lines))
      output = {@output, $format.paragraph:mk(tostr("More source: ", command, " ", next))};
    endif
    return output;
  endmethod

  method _terminal_store_draft owner: ARCH_WIZARD
    "Persist editor input within a bounded store before the next read commits the task.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {draft} = args;
    value_bytes(draft["lines"]) <= 524288 || raise(E_QUOTA, "The draft exceeds 512 KiB. Previous saved input remains available.");
    const id = draft["id"];
    maphaskey(this.terminal_drafts, id) || length(this.terminal_drafts) < 128 || raise(E_QUOTA, "The draft store is full. Use @changes drafts and @changes forget-draft ID to remove old drafts.");
    const old = `this.terminal_drafts[id] ! E_RANGE => []';
    value_bytes(this.terminal_drafts) - value_bytes(old) + value_bytes(draft) <= 8388608 || raise(E_QUOTA, "Draft storage exceeds 8 MiB. Remove an old draft before saving more text.");
    this.terminal_drafts[id] = draft;
  endmethod

  method _terminal_draft owner: ARCH_WIZARD
    "Only a draft's original writer can read its terminal buffer.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {id} = args;
    const draft = `this.terminal_drafts[id] ! E_RANGE => []';
    draft && draft["actor"] == player || this:_error("missing_draft", "No saved draft with that ID belongs to you. Use @changes drafts.");
    return draft;
  endmethod

  method _terminal_edit owner: ARCH_WIZARD
    "Collect recoverable multiline input, then validate a choice against the displayed review version.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {auth, context, selector} = args;
    const row_id = this:_terminal_row(auth, context, selector);
    const row = this:details(context["id"], context["generation"], row_id)["row"];
    row["field"] == "program" && "edited" in row["choices"] || this:_error("not_editable", "This item does not accept an edited program. Read its explanation in @changes diff.");
    let draft = [];
    for entry in (mapvalues(this.terminal_drafts))
      if (entry["actor"] == auth[1] && entry["review"] == context["id"] && entry["row"] == row_id)
        draft = entry;
        break;
      endif
    endfor
    const resumed = length(draft) > 0;
    if (resumed)
      if (draft["task"] && valid_task(draft["task"]))
        const connected = {entry[1] for entry in (connections(auth[1]))};
        if (`draft["connection"] in connected ! E_RANGE => true')
          this:_error("draft_busy", "This draft is open on another connection. Finish or cancel that editor first.");
        endif
        `kill_task(draft["task"]) ! E_INVARG';
      endif
    else
      let program = `row["choice"]["program"] ! E_RANGE => `row["recovery"]["choice"]["program"] ! E_RANGE => ""'';
      const lines = typeof(program) == TYPE_LIST ? program | program ? explode(program, "\n", true) | {};
      draft = ["id" -> this.next_terminal_draft, "actor" -> auth[1], "review" -> context["id"], "row" -> row_id, "revision" -> row["revision"], "lines" -> lines];
      this.next_terminal_draft = this.next_terminal_draft + 1;
    endif
    const changed = draft["revision"] != row["revision"];
    draft["generation"] = context["generation"];
    draft["revision"] = row["revision"];
    draft["label"] = row["label"];
    draft["task"] = task_id();
    draft["connection"] = connection();
    this:_terminal_store_draft(draft);
    const instructions = tostr("Editing draft ", draft["id"], " for ", row["label"], ". ", length(draft["lines"]), " saved lines. New lines append. Use .show to read, .clear to replace all text, .delete NUMBER to remove a line, . to validate and save the choice, or @abort to leave the editor and keep the draft. Prefix a literal dot with another dot.");
    player:inform_current($event:mk_info(player, instructions));
    if (changed)
      player:inform_current($event:mk_info(player, "The reviewed contents changed since this draft was written. Check the code before saving it against this review."));
    endif
    try
      while (true)
        const line = read(draft["connection"]);
        this:_authorized(@auth);
        this:_terminal_draft(draft["id"])["task"] == task_id() || raise(E_INVARG, "This editor was replaced by another session.");
        if (line == ".")
          break;
        elseif (line == "@abort")
          draft["task"] = 0;
          this:_terminal_store_draft(draft);
          return {$format.paragraph:mk(tostr("Editing cancelled. Draft ", draft["id"], " is saved. Live code and the review choice are unchanged. Resume with @changes edit ", selector, "; read it with @changes draft ", draft["id"], "."))};
        elseif (line == ".clear")
          draft["lines"] = {};
        elseif (line == ".show" || index(line, ".show ") == 1)
          const offset = line == ".show" ? 1 | toint(line[7..$]);
          const output = this:_terminal_source(tostr("Draft ", draft["id"]), draft["lines"], offset, ".show");
          player:inform_current($event:mk_info(player, $format.block:mk(@output)));
          continue;
        elseif (index(line, ".delete ") == 1)
          const number = toint(line[9..$]);
          if (number < 1 || number > length(draft["lines"]))
            player:inform_current($event:mk_info(player, "There is no line with that number. Use .show to read the draft."));
            continue;
          endif
          draft["lines"] = listdelete(draft["lines"], number);
        else
          draft["lines"] = {@draft["lines"], length(line) && line[1] == "." ? line[2..$] | line};
        endif
        this:_terminal_store_draft(draft);
      endwhile
    except failure (ANY)
      "The last successful read already committed the saved input.";
      draft = this:_terminal_draft(draft["id"]);
      draft["task"] == task_id() || return {};
      draft["task"] = 0;
      this:_terminal_store_draft(draft);
      return {$format.paragraph:mk(tostr("Editing stopped: ", failure[2], ". Draft ", draft["id"], " remains saved. Use @changes drafts after reconnecting."))};
    endtry
    draft["task"] = 0;
    this:_terminal_store_draft(draft);
    try
      const result = this:resolve(context["id"], context["generation"], row_id, "edited", draft["lines"]:join("\n"));
      let updated = context;
      updated["generation"] = result["generation"];
      updated["prepared"] = [];
      this.terminal_contexts[this:_terminal_key()] = updated;
      let output = {$format.paragraph:mk(tostr("Draft ", draft["id"], " saved as the edited choice for ", row["label"], ". Live code is unchanged."))};
      let valid = true;
      for diagnostic in (result["validation"])
        if (diagnostic["id"] == row_id && !diagnostic["valid"])
          valid = false;
          output = {@output, $format.paragraph:mk(tostr("Program error at line ", diagnostic["line"], ": ", diagnostic["message"]))};
        endif
      endfor
      return {@output, $format.paragraph:mk(valid ? "The edited program compiles. Use @changes preview to inspect its effects before applying." | tostr("The draft needs correction. Use @changes edit ", selector, " to continue."))};
    except failure (ANY)
      return {$format.paragraph:mk(tostr("The choice was not saved: ", failure[2], ". Draft ", draft["id"], " remains available. Use @changes diff to read the current review, then reopen the matching item. Live code is unchanged."))};
    endtry
  endmethod

  method command owner: ARCH_WIZARD
    "Offer terminal shortcuts while preserving the version and member identities actually displayed.";
    const auth = this:_entry(caller_perms());
    set_task_perms(auth[2]);
    let {words} = args;
    const action = words ? words[1] | "";
    if (action in {"review", "check"})
      words[1] = action == "review" ? "diff" | "fetch";
    endif
    if (words && words[1] in {"diff", "status", "details"} && length(words) == 1)
      const context = `this.terminal_contexts[this:_terminal_key()] ! E_RANGE => []';
      const id = context ? context["id"] | this.packages[this.default_package]["active"];
      id || this:_error("no_active_review", "There is no active review. Use @changes fetch to check for updates.");
      words = {words[1], tostr(id)};
    elseif (action == "next")
      const context = this:_terminal_context();
      this:_get(auth, context["id"], context["generation"]);
      context["next"] || this:_error("last_page", "There are no more review pages.");
      words = {"diff", tostr(context["id"]), tostr(context["next"])};
    elseif (action in {"incoming", "keep", "later"} && length(words) == 2)
      const context = this:_terminal_context();
      const row = this:_terminal_row(auth, context, words[2]);
      const choices = ["incoming" -> "incoming", "keep" -> "local", "later" -> "defer"];
      words = {"resolve", tostr(context["id"]), tostr(context["generation"]), row, choices[action]};
    elseif (action == "resolve" && length(words) == 3)
      const context = this:_terminal_context();
      const row = this:_terminal_row(auth, context, words[2]);
      words = {action, tostr(context["id"]), tostr(context["generation"]), row, words[3]};
    elseif (action in {"select-group", "skip-group"} && length(words) == 2)
      const context = this:_terminal_context();
      words = {action, tostr(context["id"]), tostr(context["generation"]), words[2]};
    elseif (action in {"initialize", "base"} && length(words) in {2, 3})
      const context = this:_terminal_context();
      const row = this:_terminal_row(auth, context, words[2]);
      words = {action, tostr(context["id"]), tostr(context["generation"]), row, @words[3..$]};
    elseif (action == "source" && length(words) in {3, 4})
      const context = this:_terminal_context();
      const row = this:_terminal_row(auth, context, words[2]);
      words = {action, tostr(context["id"]), tostr(context["generation"]), row, @words[3..$]};
    elseif (action == "group" && length(words) >= 3 && tostr(toint(words[2])) != words[2])
      const context = this:_terminal_context();
      let members = {};
      for selector in (words[3..$])
        members = {@members, index(selector, "requires=") == 1 ? selector | this:_terminal_row(auth, context, selector)};
      endfor
      words = {action, tostr(context["id"]), tostr(context["generation"]), words[2], @members};
    elseif (action in {"refresh", "discard", "finish"} && length(words) == 1)
      const context = this:_terminal_context();
      words = {action, tostr(context["id"]), tostr(context["generation"])};
    endif
    if (action == "retry" && length(words) == 1)
      const context = this:_terminal_context();
      const record = this:_get(auth, context["id"], context["generation"]);
      record["status"] == "failed" || this:_error("not_failed", "Only a failed fetch can be retried. Use @changes status to read its result.");
      const package = record["package"];
      this:discard(context["id"], context["generation"]);
      const result = this:stage(package);
      this:_terminal_remember(result);
      return this:_command_progress(result);
    endif
    if (action == "edit" && length(words) == 2)
      return this:_terminal_edit(auth, this:_terminal_context(), words[2]);
    elseif (action == "drafts" && length(words) == 1)
      let drafts = {};
      for draft in (mapvalues(this.terminal_drafts))
        if (draft["actor"] == auth[1])
          drafts = {@drafts, tostr("Draft ", draft["id"], ": ", draft["label"], "; review ", draft["review"], "; ", length(draft["lines"]), " lines. Read @changes draft ", draft["id"], ".")};
        endif
      endfor
      return {$format.title:mk("Your saved drafts", 3), $format.list:mk(drafts), $format.paragraph:mk("To resume a draft, open its review with @changes diff ID, then use @changes edit ROW for the matching item. Drafts for missing members remain readable. Remove an old buffer with @changes forget-draft ID.")};
    elseif (action == "draft" && length(words) in {2, 3})
      const draft = this:_terminal_draft(toint(words[2]));
      return this:_terminal_source(tostr("Draft ", draft["id"], " for ", draft["label"]), draft["lines"], length(words) == 3 ? toint(words[3]) | 1, tostr("@changes draft ", draft["id"]));
    elseif (action == "forget-draft" && length(words) == 2)
      const draft = this:_terminal_draft(toint(words[2]));
      if (draft["task"] && valid_task(draft["task"]))
        const connected = {entry[1] for entry in (connections(auth[1]))};
        !(draft["connection"] in connected) || this:_error("draft_busy", "Close this draft's editor before removing it.");
        `kill_task(draft["task"]) ! E_INVARG';
      endif
      this.terminal_drafts = mapdelete(this.terminal_drafts, draft["id"]);
      return {$format.paragraph:mk("Draft buffer removed. Saved review choices and live code are unchanged.")};
    elseif (action == "recovery" && length(words) in {1, 2})
      const context = this:_terminal_context();
      const offset = length(words) == 2 ? toint(words[2]) | 1;
      const page = this:recovery(context["id"], context["generation"], offset);
      let items = {};
      for i in [1..length(page["items"])]
        const item = page["items"][i];
        items = {@items, tostr(offset + i - 1, ". ", `item["label"] ! E_RANGE => item["member"]', ": ", item["reason"], maphaskey(item["choice"], "program") ? tostr(" Read its saved code with @changes recovery-source ", offset + i - 1, ".") | "")};
      endfor
      let output = {$format.title:mk("Saved decisions that need another check", 3), $format.list:mk(items), $format.paragraph:mk("Unsubmitted terminal drafts are listed separately by @changes drafts.")};
      if (page["next"])
        output = {@output, $format.paragraph:mk(tostr("More saved decisions: @changes recovery ", page["next"]))};
      endif
      return output;
    elseif (action == "recovery-source" && length(words) in {2, 3})
      const context = this:_terminal_context();
      const page = this:recovery(context["id"], context["generation"], toint(words[2]));
      page["items"] || this:_error("missing_draft", "No saved decision at that position. Use @changes recovery.");
      const item = page["items"][1];
      const program = `item["choice"]["program"] ! E_RANGE => ""';
      const lines = typeof(program) == TYPE_LIST ? program | explode(program, "\n", true);
      return this:_terminal_source("Recovered edited program", lines, length(words) == 3 ? toint(words[3]) | 1, tostr("@changes recovery-source ", words[2]));
    endif
    if (action == "preview" && length(words) in {1, 2})
      return this:_terminal_preview(this:_terminal_context(), length(words) == 2 ? toint(words[2]) | 1);
    elseif (action == "apply" && length(words) == 1)
      const context = this:_terminal_context();
      context["prepared"] || this:_error("not_prepared", "Use @changes preview to read what will happen before applying.");
      return this:_command_progress(this:apply(context["id"], context["generation"], context["prepared"]));
    elseif (action == "compare" && length(words) in {2, 3})
      return this:_terminal_compare(auth, this:_terminal_context(), words[2], length(words) == 3 ? toint(words[3]) | 1);
    elseif (action == "history" && length(words) in {1, 2, 3})
      const name = length(words) >= 2 ? words[2] | this.default_package;
      const history = this:history(name, length(words) == 3 ? toint(words[3]) | 1);
      let lines = {};
      for item in (history["reviews"])
        lines = {@lines, tostr("Review ", item["review_id"], ": ", item["status"], "; ", item["decision_count"], " decisions; by ", item["actor"], ". Read @changes result ", item["review_id"], ".")};
      endfor
      let output = {$format.title:mk(name + " update history", 3), $format.list:mk(lines), $format.paragraph:mk("The service retains up to 64 complete review records within 32 MiB. Old records are removed as whole reviews. There is no general rollback command.")};
      if (history["next"])
        output = {@output, $format.list:actions({this:_command_action(tostr("@changes history ", name, " ", history["next"]), "More history")})};
      endif
      return output;
    elseif (action == "result" && length(words) in {1, 2, 3})
      const id = length(words) >= 2 ? toint(words[2]) | this:_terminal_context()["id"];
      return this:_terminal_result(id, length(words) == 3 ? toint(words[3]) | 1);
    elseif (action == "finish" && length(words) == 3)
      const summary = this:finish(toint(words[2]), toint(words[3]));
      this:_terminal_remember(summary);
      return this:_command_status(summary);
    elseif (action == "remove-group" && length(words) == 2)
      const context = this:_terminal_context();
      const result = this:set_group(context["id"], context["generation"], words[2], {});
      this:_terminal_remember(result);
      return {$format.paragraph:mk("Group removed. Saved decisions are retained. Use @changes diff to check the affected items.")};
    endif
    const previous = `this.terminal_contexts[this:_terminal_key()] ! E_RANGE => []';
    const output = this:_command_dispatch(auth, words);
    if (words && words[1] in {"resolve", "base"} && previous && previous["id"] == toint(words[2]) && previous["generation"] == toint(words[3]))
      "Saving a decision or fetching accepted source preserves displayed member identities.";
      let context = previous;
      context["generation"] = this.pending[context["id"]]["generation"];
      context["prepared"] = [];
      this.terminal_contexts[this:_terminal_key()] = context;
    elseif (words && words[1] in {"group", "select-group", "skip-group", "initialize", "refresh"})
      this:_terminal_remember(this:status(toint(words[2])));
    elseif (words && words[1] == "status")
      this:_terminal_remember(this:status(toint(words[2])));
    endif
    return output;
  endmethod

  method _command_dispatch owner: ARCH_WIZARD
    "Run explicit versioned terminal commands after authorization.";
    caller == this || raise(E_PERM);
    let {auth, words} = args;
    set_task_perms(auth[2]);
    if (!words)
      return this:_command_overview();
    endif
    const action = words[1];
    action in {"packages", "package", "upstream", "fetch", "stage", "adopt", "status", "diff", "details", "source", "base", "apply", "refresh", "discard", "resolve", "group", "select-group", "skip-group", "initialize", "bindings"} || raise(E_INVARG, tostr("Unknown @changes command: ", action, ". Use help @changes."));
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
        const complete = `package["policy"]["complete"] ! E_RANGE => {}';
        const scope = length(complete) == length(package["objects"]) && length(complete) > 0 ? "Complete" | complete ? "Mixed" | "Partial";
        rows = {@rows, {name, package["generation"], length(package["objects"]), scope, source, review}};
      endfor
      return {$format.title:mk("Change packages", 3), $format.table:mk({"Package", "Generation", "Objects", "Scope", "Upstream", "Review"}, rows)};
    endif
    if (action == "package" && length(words) >= 2)
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
      this:_terminal_remember(result);
      return this:_command_progress(result);
    endif
    if (action == "bindings" && length(words) >= 3)
      let bindings = [];
      let complete = false;
      for word in (words[4..$])
        if (word == "complete")
          complete = true;
        else
          const pair = explode(word, "=", true);
          length(pair) == 2 && length(pair[1]) > 0 && index(pair[2], "#") == 1 || raise(E_INVARG, "Use NAME=#ID for each proposed binding.");
          bindings[pair[1]] = toobj(pair[2][2..$]);
        endif
      endfor
      this:propose_objects(words[2], toint(words[3]), bindings, complete);
      return {"Proposed object bindings saved. Fetch or upload source to review their initial contents."};
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
      offset >= 1 && offset <= max(1, length(lines)) || raise(E_INVARG);
      let code = {};
      for i in [offset..min(offset + 49, length(lines))]
        value_bytes(code) + value_bytes(lines[i]) <= 60000 || raise(E_QUOTA, "Source page is too large.");
        code = {@code, tostr(i, ": ", lines[i])};
      endfor
      let output = {$format.title:mk(tostr(words[5] == "live" ? "Local contents" | words[5] == "base" ? "Accepted baseline" | "Upstream contents"), 3), $format.paragraph:mk(this:_command_review_link(detail, detail["row"]["label"], detail["row"]["id"])), $format.code:mk(code, "moo")};
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
      let output = {$format.title:mk(tostr("Review ", id, " · Generation ", result["generation"]), 3), $format.paragraph:mk(tostr(labels[words[5]], " saved for ", this:details(id, result["generation"], row_id)["row"]["label"], "."))};
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
