object #90032
  name: "LLM Suggestion Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_llm_suggestion_scenarios";

  method fixture owner: ARCH_WIZARD
    "Install a deterministic suggestion client for isolated session tests; only a wizard may configure it.";
    caller_perms().wizard || raise(E_PERM);
    const client = create($root, #90100, 1);
    add_property(client, "requests", {}, {#90100, ""});
    add_property(client, "principals", {}, {#90100, ""});
    add_property(client, "purposes", {}, {#90100, ""});
    add_property(client, "blocked", false, {#90100, ""});
    add_property(client, "response", "Try @doc properties.", {#90100, ""});
    add_verb(client, {#90100, "rxd", "simple_query"}, {"this", "none", "this"});
    set_verb_code(client, "simple_query", {
      "this.requests = {@this.requests, args[1]};",
      "this.principals = {@this.principals, caller_perms()};",
      "this.purposes = {@this.purposes, args[2]};",
      "while (this.blocked) suspend(0.1); endwhile",
      "return this.response;"});
    $player.suggestions_llm_client = client;
    #90101.authoring_features = $prog_features;
    return client;
  endmethod

  method command_fixture owner: ARCH_WIZARD
    "Configure controlled command metadata and callbacks for connected LLM suggestion tests.";
    caller_perms().wizard || raise(E_PERM);
    const client = this:fixture();
    client.response = "{\"candidates\":[\"get probe\",\"press 7\",\"nonexistent command\"],\"note\":\"Try these.\"}";
    const room = create($room, #90102, 1);
    room.r = 1;
    room.name = "Command Test Room";
    add_property(room, "principal", #-1, {#90102, ""});
    add_property(room, "blocked", false, {#90102, ""});
    add_verb(room, {#90102, "rxd", "command_hints"}, {"this", "none", "this"});
    set_verb_code(room, "command_hints", {
      "this.principal = caller_perms(); while (this.blocked) suspend(0.1); endwhile",
      "return {[\"command\" -> \"press 7\", \"description\" -> \"Select floor seven.\"]};"});
    const item = create($thing, #90102, 1);
    item.r = 1;
    item.name = "Command Probe";
    item.aliases = {"probe"};
    move(item, room);
    add_property(#90100, "command_fixture", {#90102.location, room, item}, {#90100, ""});
    move(#90102, room);
    return client;
  endmethod

  method close_command_fixture owner: ARCH_WIZARD
    "Restore the player's location and release anonymous command fixture references.";
    caller_perms().wizard || raise(E_PERM);
    const {original, room, item} = #90100.command_fixture;
    $llm_suggestion:cancel_all(#90102);
    move(#90102, original);
    delete_property(#90100, "command_fixture");
    $player.suggestions_llm_client = #-1;
    #90102.llm_assist_contexts = {};
    return true;
  endmethod

  method help_fixture owner: ARCH_WIZARD
    "Install a readable help provider that records incoming authority and can suspend or change its catalog.";
    caller_perms().wizard || raise(E_PERM);
    const client = this:fixture();
    client.response = "{\"topics\":[\"alias\",\"unlisted\",\"probe\"],\"note\":\"A short note.\"}";
    const provider = create($root, #90102, 1);
    provider.r = 1;
    for field in ({"principal", "entered"})
      add_property(provider, field, 0, {#90102, ""});
    endfor
    add_property(provider, "blocked", false, {#90102, ""});
    add_property(provider, "summary", "A test topic.", {#90102, ""});
    add_property(provider, "topic_name", "probe", {#90102, ""});
    add_verb(provider, {#90102, "rxd", "help_topics"}, {"this", "none", "this"});
    set_verb_code(provider, "help_topics", {
      "this.principal = caller_perms(); this.entered = this.entered + 1;",
      "while (this.blocked) suspend(0.1); endwhile",
      "if (args[2] != \"\") return 0; endif",
      "return {$help:mk(this.topic_name, this.summary, \"Private body is not model context.\", {\"alias\"})};"});
    add_property(#90102, "help_probe", provider, {#90102, ""});
    add_verb(#90102, {#90102, "rxd", "help_environment"}, {"this", "none", "this"});
    set_verb_code(#90102, "help_environment", {"return {this.help_probe};"});
    return client;
  endmethod

  method wait_requests owner: ARCH_WIZARD
    "Wait for a fake request barrier without assuming transaction scheduling order.";
    caller_perms().wizard || raise(E_PERM);
    const {client, count} = args;
    for attempt in [1..40]
      length(client.requests) >= count && return true;
      suspend(0.1);
    endfor
    raise(E_ASSERT, "Suggestion did not reach the fake provider.");
  endmethod

  method wait_closed owner: ARCH_WIZARD
    "Wait for job retirement and verify its task and retained-reference cleanup.";
    caller_perms().wizard || raise(E_PERM);
    const {job, ?tasks = {job.worker_task, job.monitor_task}} = args;
    const {worker, monitor} = tasks;
    for attempt in [1..40]
      if (job.closed && !valid_task(worker) && !valid_task(monitor))
        $test_utils:assert_false(valid(job.requester) || valid(job.client) || valid(job.presenter), "Closed suggestions release collaborators");
        $test_utils:assert_eq(job.request, [], "Closed suggestions release input");
        $test_utils:assert_eq(job.input_snapshot, [], "Closed suggestions release metadata");
        return job.state;
      endif
      suspend(0.1);
    endfor
    raise(E_ASSERT, "Suggestion did not release its owned tasks.");
  endmethod

  method _as_player owner: RUNTIME_PLAYER
    "Exercise the real entry under ordinary player authority.";
    caller == this && this == #90032 || raise(E_PERM);
    const {target, name, values} = args;
    return target:(name)(@values);
  endmethod

  method _as_programmer owner: RUNTIME_PROGRAMMER
    "Exercise native metadata reads as their ordinary programmer owner.";
    caller == this && this == #90032 || raise(E_PERM);
    const {target, name, values} = args;
    return target:(name)(@values);
  endmethod

  method _read_assist_contexts owner: RUNTIME_PROGRAMMER
    "Read another player's retained opt-in state under ordinary programmer permissions.";
    caller == this && this == #90032 || raise(E_PERM);
    return #90102.llm_assist_contexts;
  endmethod

  method test_command_metadata_respects_native_permissions owner: ARCH_WIZARD
    "Command metadata cannot expose a foreign private object or private ancestor definitions.";
    const ancestor = create($root, #90101, 2);
    const target = create(ancestor, #90102, 2);
    try
      add_verb(ancestor, {#90101, "rxd", "secret_command"}, {"this", "none", "none"});
      let denied = false;
      try
        this:_as_player($llm_command_context, "command_specs", {#90102, ancestor});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "Native read denial is preserved");
      const specs = this:_as_player($llm_command_context, "command_specs", {#90102, target});
      $test_utils:assert_false(index(toliteral(specs), "secret_command") > 0, "Private ancestors contribute no names");
    finally
      recycle(target);
      recycle(ancestor);
    endtry
    return true;
  endmethod

  method test_command_context_visible_state_and_authentication owner: ARCH_WIZARD
    "State collection preserves readable container state and authenticates the caller before any target hook.";
    const target = create($container, #90102, 1);
    target.open = false;
    target.locked = true;
    const info = this:_as_player($llm_command_context, "object_info", {#90102, target});
    $test_utils:assert_eq(info["state"], "closed and locked", "Container state remains available");
    let denied = false;
    try
      this:_as_player($llm_command_context, "object_info", {#90101, target});
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "A supplied actor cannot lend authority to object context");
    return true;
  endmethod

  method test_command_context_custom_exits owner: ARCH_WIZARD
    "Custom exit commands use the maintained provider API, retain requester authority, and never execute travel.";
    const room = create($room, #90102, 1);
    const provider = create($root, #90102, 2);
    try
      add_property(provider, "principal", #-1, {#90102, ""});
      add_property(provider, "used", false, {#90102, ""});
      add_property(provider, "available", true, {#90102, ""});
      add_verb(provider, {#90102, "rxd", "travel_options"}, {"this", "none", "this"});
      set_verb_code(provider, "travel_options", {
        "this.principal = caller_perms();",
        "return {['id -> \"archway\", 'label -> \"archway\", 'command -> \"enter archway\", 'available -> this.available]};"});
      add_verb(provider, {#90102, "rxd", "use_exit"}, {"this", "none", "this"});
      set_verb_code(provider, "use_exit", {"this.used = true; return #-1;"});
      $room:register_exit_provider(room, provider);
      const offered = this:_as_player($llm_command_context, "room_commands", {#90102, room});
      $test_utils:assert_true("enter archway" in offered['commands], "Custom commands enter context");
      $test_utils:assert_eq(provider.principal, #90102, "Exit providers receive requester authority");
      $test_utils:assert_false(provider.used, "Context collection never executes travel");
      provider.available = false;
      const unavailable = this:_as_player($llm_command_context, "room_commands", {#90102, room});
      $test_utils:assert_false("enter archway" in unavailable['commands], "Unavailable exits cannot validate candidates");
    finally
      $room:unregister_exit_provider(room, provider);
      recycle(provider);
    endtry
    return true;
  endmethod

  method test_command_object_metadata_and_scope owner: ARCH_WIZARD
    "Command context preserves inherited syntax, names, aliases, and the shipped player environment.";
    const target = create($thing, #90102, 1);
    target.name = "Command Probe";
    target.aliases = {"probe"};
    const info = this:_as_player($llm_command_context, "object_info", {#90102, target});
    $test_utils:assert_eq(info["name"], "Command Probe", "Object names survive extraction");
    $test_utils:assert_eq(info["aliases"], {"probe"}, "Object aliases survive extraction");
    $test_utils:assert_true(index(toliteral(info["commands"]), "<this>") > 0, "Inherited target signatures remain available");
    const snapshot = this:_as_player($llm_command_context, "snapshot", {#90102});
    const repeated = this:_as_player($llm_command_context, "snapshot", {#90102});
    $test_utils:assert_true(snapshot == repeated && strcmp(toliteral(snapshot), toliteral(repeated)) == 0,
      "Unchanged context is stable across native player enumeration order");
    $test_utils:assert_true(value_bytes(generate_json(snapshot['data])) <= 24576, "Shipped context fits model limits");
    $test_utils:assert_true(this:_as_player($llm_command_context, "accepts", {#90102, snapshot, "look"}),
      "Native command syntax is accepted");
    $test_utils:assert_false(this:_as_player($llm_command_context, "accepts", {#90102, snapshot, "nonsuch_command_xyz"}),
      "Invented verbs are rejected");
    $test_utils:assert_false(this:_as_player($llm_command_context, "accepts", {#90102, snapshot, "look\nquit"}),
      "Multiple input lines cannot become one command link");
    return true;
  endmethod

  method test_assist_contexts_are_private owner: ARCH_WIZARD
    "A foreign programmer cannot inspect retained LLM input or its connection.";
    let denied = false;
    try
      this:_read_assist_contexts();
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Saved LLM inputs are private");
    denied = false;
    try
      this:_as_player($llm_assist_context, "prune", {#90101});
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Context helpers authenticate the actual actor");
    return true;
  endmethod

  method test_help_environment_callbacks_use_requester owner: ARCH_WIZARD
    "Administrative feature probes cannot receive wizard authority during LLM help context collection.";
    const features = #90102.admin_features;
    const provider = create($root, #90102, 1);
    add_property(provider, "principal", #-1, {#90102, ""});
    add_verb(provider, {#90102, "rxd", "is_elevated"}, {"this", "none", "this"});
    set_verb_code(provider, "is_elevated", {"this.principal = caller_perms(); return false;"});
    try
      #90102.admin_features = {provider};
      this:_as_player(#90102, "help_environment", {});
      $test_utils:assert_eq(provider.principal, #90102, "Feature probes retain requester authority");
    finally
      #90102.admin_features = features;
    endtry
    return true;
  endmethod

  method test_help_lookup_callbacks_use_requester owner: ARCH_WIZARD
    "Targeted help tools and ordinary help enumeration must not lend their helper's wizard authority.";
    const features = #90102.features;
    const provider = create($root, #90102, 1);
    provider.r = 1;
    add_property(provider, "principal", #-1, {#90102, ""});
    add_verb(provider, {#90102, "rxd", "help_topics"}, {"this", "none", "this"});
    set_verb_code(provider, "help_topics", {
      "this.principal = caller_perms();",
      "if (args[2] == \"\") return {}; endif return 0;"});
    try
      #90102.features = {provider};
      this:_as_player($llm_help_tools, "help_lookup", {["topic" -> "missing_probe_topic"], #90102});
      $test_utils:assert_eq(provider.principal, #90102, "Targeted LLM help lookup uses the actor");
      provider.principal = #-1;
      this:_as_player(#90102, "_collect_help_topics", {});
      $test_utils:assert_eq(provider.principal, #90102, "Ordinary enumeration uses the actor");
    finally
      #90102.features = features;
    endtry
    return true;
  endmethod

  method test_help_catalog_shipped_environment owner: ARCH_WIZARD
    "The ordinary player's shipped help environment fits the shared catalog and model context bounds.";
    const environment = this:_as_player(#90102, "help_environment", {});
    const catalog = this:_as_player($llm_help_tools, "catalog", {#90102, environment});
    $test_utils:assert_true(length(catalog) > 0, "The shipped catalog contains topics");
    const topics = {["name" -> entry['name], "summary" -> entry['summary], "aliases" -> entry['aliases]]
      for entry in (catalog)};
    $test_utils:assert_true(value_bytes(generate_json(["query" -> "movement", "topics" -> topics])) <= 24576,
      "The ordinary environment fits the suggestion context limit");
    return true;
  endmethod

  method test_help_catalog_authentication_and_bounds owner: ARCH_WIZARD
    "Catalogs authenticate the incoming actor and reject oversized provider collections before callbacks.";
    let denied = false;
    try
      this:_as_player($llm_help_tools, "catalog", {#90101, {}});
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "A help catalog cannot borrow another actor's authority");
    let limited = false;
    try
      this:_as_player($llm_help_tools, "catalog", {#90102, {#-1 for index in [1..129]}});
    except (E_QUOTA)
      limited = true;
    endtry
    $test_utils:assert_true(limited, "Provider admission is bounded before dispatch");
    return true;
  endmethod

  method test_help_catalog_rejects_malformed_topics owner: ARCH_WIZARD
    "Help providers cannot inject arbitrary flyweights or oversized topic lists into LLM context.";
    const provider = create($root, #90102, 1);
    provider.r = 1;
    add_property(provider, "reply", {<$root>}, {#90102, ""});
    add_verb(provider, {#90102, "rxd", "help_topics"}, {"this", "none", "this"});
    set_verb_code(provider, "help_topics", {"return this.reply;"});
    let malformed = false;
    try
      this:_as_player($llm_help_tools, "catalog", {#90102, {provider}});
    except (E_TYPE)
      malformed = true;
    endtry
    $test_utils:assert_true(malformed, "Only canonical help values enter the catalog");
    const topic = $help:mk("probe", "Summary.", "Body.");
    provider.reply = {topic for index in [1..257]};
    let limited = false;
    try
      this:_as_player($llm_help_tools, "catalog", {#90102, {provider}});
    except (E_QUOTA)
      limited = true;
    endtry
    $test_utils:assert_true(limited, "Topic lists are bounded before iteration");
    provider.r = 0;
    $test_utils:assert_eq(this:_as_player($llm_help_tools, "catalog", {#90102, {provider}}), {},
      "Private providers do not enter the public help catalog");
    return true;
  endmethod

  method test_doc_metadata_uses_native_permissions owner: ARCH_WIZARD
    "Foreign private object names remain unavailable; owned names are bounded and readable.";
    const target = create($root, #90101, 2);
    try
      add_verb(target, {#90101, "", "private_guide_metadata"}, {"this", "none", "this"});
      let denied = false;
      try
        this:_as_player($llm_doc_suggestion, "_names", {target, 'verb, 64});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "The helper does not lend wizard authority to metadata collection");
      const names = this:_as_programmer($llm_doc_suggestion, "_names", {target, 'verb, 1});
      $test_utils:assert_eq(names, {"private_guide_metadata"}, "Owners retain their native metadata access");
    finally
      recycle(target);
    endtry
    return true;
  endmethod

  method test_doc_admission_rejects_spoofed_actor owner: ARCH_WIZARD
    "A caller cannot create another programmer's suggestion by supplying their identity.";
    const request = ['kind -> 'builtin, 'spec -> "missing", 'target -> #-1, 'member -> ""];
    let denied = false;
    try
      this:_as_player($llm_doc_suggestion, "start", {#90101, #-1001, request});
    except (E_PERM)
      denied = true;
    endtry
    $test_utils:assert_true(denied, "Actor authentication precedes connection and provider work");
    return true;
  endmethod
  method test_suggestion_event_rejects_foreign_actor owner: ARCH_WIZARD
    "Structured placeholder delivery cannot bypass the presenter's actor authentication.";
    const presenter = $llm_presenter:new(#90101, 0, $prog_features);
    try
      let denied = false;
      try
        this:_as_player($llm_presenter, "send_event",
          {presenter, #90101, $event:mk_info(#90101, "forged")});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "A caller cannot lend another actor's identity to a presenter");
    finally
      presenter:close();
    endtry
    return true;
  endmethod
endobject
