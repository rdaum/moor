object SYSOBJ [
  import_export_id -> "sysobj"
]
  name: "System Object"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property startup_hooks (owner: ARCH_WIZARD, flags: "r") = {FIRST_ROOM};

  property actor (owner: HACKER, flags: "r") = ACTOR;
  property admin_features (owner: ARCH_WIZARD, flags: "rc") = ADMIN_FEATURES;
  property agent_building_tools (owner: HACKER, flags: "r") = AGENT_BUILDING_TOOLS;
  property agent_room (owner: ARCH_WIZARD, flags: "rc") = AGENT_ROOM;
  property agentic (owner: ARCH_WIZARD, flags: "rc") = AGENTIC;
  property ambiguous_match (owner: HACKER, flags: "r") = #-2;
  property arch_wizard (owner: HACKER, flags: "r") = ARCH_WIZARD;
  property architects_compass (owner: HACKER, flags: "r") = ARCHITECTS_COMPASS;
  property area (owner: HACKER, flags: "r") = AREA;
  property bg_ticks (owner: HACKER, flags: "r") = 300000;
  property brass_key (owner: HACKER, flags: "r") = BRASS_KEY;
  property builder_features (owner: HACKER, flags: "r") = BUILDER_FEATURES;
  property builder_prototypes (owner: HACKER, flags: "r") = {
    ROOM,
    THING,
    WEARABLE,
    CONTAINER,
    SITTABLE,
    ACTOR,
    NOTE,
    AREA,
    CONSUMABLE,
    FOOD,
    DRINK
  };
  property cat_kibble (owner: HACKER, flags: "r") = CAT_KIBBLE;
  property change_manager (owner: ARCH_WIZARD, flags: "r") = CHANGE_MANAGER;
  property change_tools (owner: ARCH_WIZARD, flags: "r") = CHANGE_TOOLS;
  property consumable (owner: ARCH_WIZARD, flags: "r") = CONSUMABLE;
  property container (owner: HACKER, flags: "r") = CONTAINER;
  property core_version (owner: ARCH_WIZARD, flags: "rc") = "0.3.0";
  property couch (owner: HACKER, flags: "r") = COUCH;
  property data_visor (owner: HACKER, flags: "r") = DATA_VISOR;
  property dm (owner: HACKER, flags: "r") = DM;
  property document (owner: HACKER, flags: "r") = DOCUMENT;
  property document_node (owner: HACKER, flags: "r") = DOCUMENT_NODE;
  property document_renderer (owner: HACKER, flags: "r") = DOCUMENT_RENDERER;
  property document_resolver (owner: HACKER, flags: "r") = DOCUMENT_RESOLVER;
  property document_schema (owner: HACKER, flags: "r") = DOCUMENT_SCHEMA;
  property document_template (owner: HACKER, flags: "r") = DOCUMENT_TEMPLATE;
  property drink (owner: ARCH_WIZARD, flags: "r") = DRINK;
  property dump_interval (owner: ARCH_WIZARD, flags: "r") = 3600;
  property event (owner: HACKER, flags: "r") = EVENT;
  property event_receiver (owner: HACKER, flags: "r") = EVENT_RECEIVER;
  property examination (owner: HACKER, flags: "r") = EXAMINATION;
  property failed_match (owner: HACKER, flags: "r") = #-3;
  property fg_ticks (owner: HACKER, flags: "r") = 600000;
  property first_area (owner: HACKER, flags: "r") = FIRST_AREA;
  property first_room (owner: HACKER, flags: "r") = FIRST_ROOM;
  property food (owner: ARCH_WIZARD, flags: "r") = FOOD;
  property format (owner: HACKER, flags: "r") = FORMAT;
  property git (owner: ARCH_WIZARD, flags: "r") = GIT;
  property git_entry (owner: ARCH_WIZARD, flags: "r") = GIT_ENTRY;
  property git_repository (owner: ARCH_WIZARD, flags: "r") = GIT_REPOSITORY;
  property git_snapshot (owner: ARCH_WIZARD, flags: "r") = GIT_SNAPSHOT;
  property grant_utils (owner: HACKER, flags: "r") = GRANT_UTILS;
  property hacker (owner: HACKER, flags: "r") = HACKER;
  property help (owner: HACKER, flags: "r") = HELP;
  property help_topics (owner: ARCH_WIZARD, flags: "r") = HELP_TOPICS;
  property help_utils (owner: HACKER, flags: "r") = HELP_UTILS;
  property henri (owner: HACKER, flags: "r") = HENRI;
  property housekeeping (owner: HACKER, flags: "r") = HOUSEKEEPING;
  property int_proto (owner: HACKER, flags: "r") = INT_PROTO;
  property kibble_cupboard (owner: HACKER, flags: "r") = KIBBLE_CUPBOARD;
  property letter (owner: HACKER, flags: "r") = LETTER;
  property list_proto (owner: HACKER, flags: "r") = LIST_PROTO;
  property llm_agent (owner: HACKER, flags: "r") = LLM_AGENT;
  property llm_agent_tool (owner: HACKER, flags: "r") = LLM_AGENT_TOOL;
  property llm_chat_opts (owner: HACKER, flags: "r") = LLM_CHAT_OPTS;
  property llm_client (owner: HACKER, flags: "r") = LLM_CLIENT;
  property llm_response (owner: ARCH_WIZARD, flags: "rc") = LLM_RESPONSE;
  property llm_room_observer (owner: HACKER, flags: "r") = LLM_ROOM_OBSERVER;
  property llm_task (owner: HACKER, flags: "r") = LLM_TASK;
  property llm_wearable (owner: HACKER, flags: "r") = LLM_WEARABLE;
  property local (owner: HACKER, flags: "r") = #-1;
  property login (owner: HACKER, flags: "r") = LOGIN;
  property look (owner: HACKER, flags: "r") = LOOK;
  property mail_features (owner: HACKER, flags: "r") = MAIL_FEATURES;
  property mail_room (owner: HACKER, flags: "r") = MAIL_ROOM;
  property mailbox (owner: HACKER, flags: "r") = MAILBOX;
  property match (owner: HACKER, flags: "r") = MATCH;
  property mr_welcome (owner: HACKER, flags: "r") = MR_WELCOME;
  property msg_bag (owner: HACKER, flags: "r") = MSG_BAG;
  property note (owner: HACKER, flags: "r") = NOTE;
  property nothing (owner: HACKER, flags: "r") = #-1;
  property obj_utils (owner: HACKER, flags: "r") = OBJ_UTILS;
  property passage (owner: HACKER, flags: "r") = PASSAGE;
  property password (owner: HACKER, flags: "r") = PASSWORD;
  property player (owner: HACKER, flags: "r") = PLAYER;
  property player_activity (owner: ARCH_WIZARD, flags: "rc") = PLAYER_ACTIVITY;
  property prog_features (owner: HACKER, flags: "r") = PROG_FEATURES;
  property prog_utils (owner: HACKER, flags: "r") = PROG_UTILS;
  property pronouns (owner: HACKER, flags: "r") = PRONOUNS;
  property property (owner: HACKER, flags: "r") = PROPERTY;
  property prototype_box (owner: HACKER, flags: "r") = PROTOTYPE_BOX;
  property reaction (owner: HACKER, flags: "r") = REACTION;
  property relation (owner: HACKER, flags: "r") = RELATION;
  property rlm_agent (owner: ARCH_WIZARD, flags: "r") = RLM_AGENT;
  property room (owner: HACKER, flags: "r") = ROOM;
  property root (owner: HACKER, flags: "r") = ROOT;
  property rule (owner: HACKER, flags: "r") = RULE;
  property rule_engine (owner: HACKER, flags: "r") = RULE_ENGINE;
  property server_options (owner: ARCH_WIZARD, flags: "r") = SERVER_OPTIONS;
  property sittable (owner: HACKER, flags: "r") = SITTABLE;
  property social_features (owner: HACKER, flags: "r") = SOCIAL_FEATURES;
  property str_proto (owner: HACKER, flags: "r") = STR_PROTO;
  property sub (owner: HACKER, flags: "r") = SUB;
  property sub_utils (owner: HACKER, flags: "r") = SUB_UTILS;
  property sym_proto (owner: HACKER, flags: "r") = SYM_PROTO;
  property sysobj (owner: HACKER, flags: "r") = SYSOBJ;
  property test_player (owner: HACKER, flags: "r") = TEST_PLAYER;
  property test_utils (owner: HACKER, flags: "r") = TEST_UTILS;
  property thing (owner: HACKER, flags: "r") = THING;
  property url_utils (owner: HACKER, flags: "r") = URL_UTILS;
  property verb (owner: HACKER, flags: "r") = VERB;
  property wearable (owner: HACKER, flags: "r") = WEARABLE;
  property wiz_features (owner: HACKER, flags: "r") = WIZ_FEATURES;

  override description (owner: ARCH_WIZARD, flags: "rc") = "System object containing global properties and core server event handlers.";

  method do_oauth_login owner: ARCH_WIZARD
    "Daemon entry for verified OAuth identities; require a root call for an unauthenticated connection.";
    !callers() && `toint(player) ! E_TYPE, E_INVARG => 0' < 0 && caller == player || raise(E_PERM);
    connection_name(player);
    const {operation, @parameters} = args;
    operation in {"oauth2_check", "oauth2_create", "oauth2_connect"} || raise(E_INVARG);
    return $login:(operation)(@parameters);
  endmethod

  method do_login_command owner: ARCH_WIZARD
    "...This code should only be run as a server task, but we'll let wizards poke at it...";
    callers() && !caller_perms().wizard && return E_PERM;
    args = $login:parse_command(@args);
    return $login:(args[1])(@listdelete(args, 1));
  endmethod

  method "user_created user_connected" owner: ARCH_WIZARD
    "Called by the server when a user connects (or reconnects, or is created).";
    callers() && !caller_perms().wizard && return E_PERM;
    user = args[1];
    if (user < #0)
      return;
    endif
    "Don't trust verb name alone -- old clients may call user_created on reconnect.";
    "Verify by checking whether the player has connected before.";
    last_conn = `user.last_connected ! E_PROPNF => 0';
    is_new_player = verb == "user_created" && !last_conn;
    "Existing players with another connection are already awake.";
    "New players still need setup when HTTP authentication overlaps the first WebSocket.";
    if (length(connections(user)) > 1 && !is_new_player)
      set_task_perms(user);
      `user:emit_room_look(user.location) ! ANY';
      return;
    endif
    "First connection. Decide whether to announce.";
    "Use the last connection time to avoid repeat announcements during short reconnects.";
    quiet_period = `$login.connection_quiet_period ! E_PROPNF => 7200';
    time_since_last = last_conn > 0 ? time() - last_conn | quiet_period + 1;
    should_announce = is_new_player || time_since_last > quiet_period;
    "Set up new players before dropping perms (mailbox, welcome letter).";
    if (is_new_player)
      `$login:setup_new_player(user) ! E_VERBNF';
    endif
    "Drop to user perms for confunc.";
    set_task_perms(user);
    `user.location:confunc(user, is_new_player, should_announce) ! E_INVIND, E_VERBNF';
    `user:anyconfunc() ! E_VERBNF';
    "Greet new players after room confunc.";
    if (is_new_player)
      `$login:greet_new_player(user) ! E_VERBNF';
    endif
    "Player confunc handles DM/mail notifications, then updates last_connected.";
    `user:confunc() ! E_VERBNF';
  endmethod

  method "user_disconnected user_client_disconnected" owner: ARCH_WIZARD
    "Called when a user disconnects (only when last connection is removed).";
    callers() && !caller_perms().wizard && return E_PERM;
    user = args[1];
    if (user < #0)
      return;
    endif
    "Guard: don't run disfunc if the user still has active connections.";
    length(connections(user)) > 0 && return;
    "Set last_disconnected before calling disfunc so it reads consistently.";
    `user.last_disconnected = time() ! E_PROPNF, E_PERM';
    set_task_perms(user);
    "Room disfunc and player disfunc run inline, no fork.";
    `user.location:disfunc(user) ! E_INVIND, E_VERBNF';
    `user:disfunc() ! E_VERBNF';
  endmethod

  method user_reconnected owner: ARCH_WIZARD
    "Record a server reconnect without room output. Wizard callers may invoke this for administration.";
    callers() && !caller_perms().wizard && return E_PERM;
    const {user} = args;
    if (user < #0)
      return;
    endif
    `user.last_connected = time() ! E_PROPNF, E_PERM';
  endmethod

  method do_command owner: ARCH_WIZARD
    "Custom command handler which is capable of handling ambiguous object matches by attempting to find matching verb candidates.";
    "...This code should only be run as a server task, but we'll let wizards poke at it...";
    callers() && !caller_perms().wizard && return E_PERM;
    "Just choose to ignore empty commands...";
    length(args) == 0 && return true;
    command = argstr;
    "Handle 'player message shortcut for DMs";
    if (command && command[1] == "'")
      rest = command[2..$]:trim();
      if (rest && length(rest) > 0)
        parts = rest:split(" ");
        if (length(parts) >= 1)
          command = "dm " + rest;
        endif
      endif
    endif
    set_task_perms(player);
    env = player:match_environment(command, ['complex -> true]);
    "Run the parts that need wizard permissions";
    "We let this throw otherwise errors in commands would not propagate.";
    return this:_command_handler(command, env);
  endmethod

  method _command_handler owner: ARCH_WIZARD
    "The wizard-permissioned portion of the custom command handler";
    caller == this || raise(E_PERM);
    {command, match_env} = args;
    "Parse command using match environment (all visible objects for dobj/iobj matching)";
    try
      pc = parse_command(command, match_env, true, 0.3);
    except e (ANY)
      "parse_command failed - report to programmers, friendly message to others";
      if (player.programmer || player.wizard)
        traceback = {"Parse error: " + toliteral(e[2]) + ":"};
        for tb in (e[4])
          traceback = {@traceback, tostr("... called from ", tb[4], ":", tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6])};
        endfor
        traceback = {@traceback, "(End of traceback)"};
        traceback = {@traceback, " [when called with " + toliteral(args) + "]"};
        player:inform_current($event:mk_error(player, $format.code:mk(traceback)):with_metadata('command_exception, true));
      else
        player:inform_current($event:mk_do_not_understand(player, "I don't understand that."):with_audience('utility):with_metadata('command_exception, true));
      endif
      return true;
    endtry
    "Treat `go home` as the dedicated `home` command.";
    if (typeof(pc) == TYPE_MAP && `pc['verb] ! E_RANGE => ""' == "go")
      go_target = `pc['dobjstr] ! E_RANGE => ""';
      if (typeof(go_target) == TYPE_STR && go_target:trim():lowercase() == "home")
        pc['verb] = "home";
        pc['argstr] = "";
        pc['args] = {};
        pc['dobj] = $nothing;
        pc['dobjstr] = "";
        pc['iobj] = $nothing;
        pc['iobjstr] = "";
        pc['prep] = -1;
        pc['prepstr] = "";
        pc['ambiguous_dobj] = {};
        pc['ambiguous_iobj] = {};
      endif
    endif
    "Get command environment (only player and location for primary verb searching)";
    command_env = player:command_environment();
    if (pc['dobj] == $ambiguous_match)
      dobj_candidates = pc['ambiguous_dobj];
    else
      dobj_candidates = {pc['dobj]};
    endif
    if (pc['iobj] == $ambiguous_match)
      iobj_candidates = pc['ambiguous_iobj];
    else
      iobj_candidates = {pc['iobj]};
    endif
    for dobj in (dobj_candidates)
      for iobj in (iobj_candidates)
        test_pc = pc;
        test_pc['dobj] = dobj;
        test_pc['iobj] = iobj;
        vm_matches = find_command_verb(test_pc, command_env);
        if (vm_matches)
          for m in (vm_matches)
            {target, verbspec} = m;
            {def, flags, verbnames, v} = verbspec;
            try
              suspend(0);
              dispatch_command_verb(target, v, test_pc);
              return true;
            except e (ANY)
              "Command verb threw an error - report it to the player";
              if (player.programmer || player.wizard)
                traceback = {"Command failed: " + toliteral(e[2]) + ":"};
                for tb in (e[4])
                  traceback = {@traceback, tostr("... called from ", tb[4], ":", tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6])};
                endfor
                traceback = {@traceback, "(End of traceback)"};
                player:inform_current($event:mk_error(player, $format.code:mk(traceback)):with_metadata('command_exception, true));
              else
                player:inform_current($event:mk_error(player, "Something went wrong while processing your command. If this keeps happening, please let a wizard know what you were trying to do."):with_metadata('command_exception, true));
              endif
              return true;
            endtry
          endfor
        endif
      endfor
    endfor
    "Dispatch any unmatched action out to the room for potential special handling (furniture, passages, etc.)";
    set_task_perms(player);
    suspend(0);
    player.location:maybe_handle_command(pc) && return true;
    "No verb matches found - try LLM suggestion";
    try
      suspend(0);
      player:suggest_command_alternatives(pc) && return true;
    except e (ANY)
      if (player.programmer || player.wizard)
        traceback = {"Command failed: " + toliteral(e[2]) + ":"};
        for tb in (e[4])
          traceback = {@traceback, tostr("... called from ", tb[4], ":", tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6])};
        endfor
        traceback = {@traceback, "(End of traceback)"};
        player:inform_current($event:mk_error(player, $format.code:mk(traceback)):with_metadata('command_exception, true));
      else
        player:inform_current($event:mk_error(player, "Something went wrong while processing your command. If this keeps happening, please let a wizard know what you were trying to do."):with_metadata('command_exception, true));
      endif
    endtry
    player:inform_current($event:mk_do_not_understand(player, "I don't know how to do that."):with_audience('utility));
    return true;
  endmethod

  method bf_recycle owner: ARCH_WIZARD
    "Intercept recycle() to enforce permission checking through :destroy.";
    "Allows direct recycle() if: wizard, object not rooted in #1, or called from :destroy.";
    {target} = args;
    "If not an object, let builtin raise the appropriate error";
    if (typeof(target) != TYPE_OBJ)
      return recycle(target);
    endif
    "Check if we're being called directly from $root:destroy before downgrading, so runtime grants installed by :destroy are preserved.";
    stack = callers();
    if (length(stack))
      frame = stack[1];
      if (frame[2] == "destroy" && frame[4] == $root && (frame[1] == target || (typeof(frame[1]) == TYPE_FLYWEIGHT && frame[1].delegate == target)))
        return recycle(target);
      endif
    endif
    "IMPORTANT: Run as the caller, so that the right permissions are applied...";
    set_task_perms(caller_perms());
    "Wizards can recycle anything";
    if (caller_perms().wizard)
      return recycle(target);
    endif
    "Objects not rooted in $root don't have :destroy, allow direct recycle";
    if (!isa(target, $root))
      return recycle(target);
    endif
    "Not authorized - must go through :destroy for permission checking";
    raise(E_PERM);
  endmethod

  method list_builder_prototypes owner: HACKER
    "Return list of builder prototypes with descriptions.";
    result = {};
    seen = [];
    for proto in (this.builder_prototypes)
      if (typeof(proto) != TYPE_OBJ || !valid(proto) || maphaskey(seen, proto))
        continue;
      endif
      seen[proto] = 1;
      proto_name = proto.name;
      for prop in (properties(this))
        value = this.(prop);
        if (typeof(value) == TYPE_OBJ && value == proto)
          proto_name = "$" + tostr(prop);
          break;
        endif
      endfor
      desc = proto.description;
      result = {@result, ["object" -> toliteral(proto), "name" -> proto_name, "description" -> desc]};
    endfor
    return result;
  endmethod

  method test_list_builder_prototypes owner: HACKER
    "Builder prototype catalog should expose sysobj names and descriptions.";
    prototypes = this:list_builder_prototypes();
    found_room = false;
    found_thing = false;
    for proto_info in (prototypes)
      if (proto_info["name"] == "$room")
        found_room = true;
        $test_utils:assert_eq(proto_info["object"], toliteral($room), "room prototype should include object literal");
        $test_utils:assert_eq(proto_info["description"], $room.description, "room prototype should include real description");
      elseif (proto_info["name"] == "$thing")
        found_thing = true;
        $test_utils:assert_eq(proto_info["object"], toliteral($thing), "thing prototype should include object literal");
        $test_utils:assert_eq(proto_info["description"], $thing.description, "thing prototype should include real description");
      endif
    endfor
    $test_utils:assert_true(found_room, "builder prototypes should include $room: " + toliteral(prototypes));
    $test_utils:assert_true(found_thing, "builder prototypes should include $thing: " + toliteral(prototypes));
    return true;
  endmethod

  method server_started owner: ARCH_WIZARD
    "Called on server start to kick off initial state after being out of existence for a bit...";
    "...This code should only be run as a server task, but we'll let wizards poke at it...";
    callers() && !caller_perms().wizard && return E_PERM;
    server_log("Core starting...");
    for object in (this.startup_hooks)
      object:initialize();
    endfor
    "Issue capability for $login to create players";
    player_class = $login.default_player_class;
    $login.player_setup_capability = $player:issue_capability(player_class, {'create_child, 'make_player}, 0, $arch_wizard);
    server_log("Issued player creation capability to $login");
  endmethod

  method handle_uncaught_error owner: ARCH_WIZARD
    "Called when there's an uncaught error in a task...";
    if (callers() && !caller_perms().wizard)
      server_log("Illegal call to `handle_uncaught_error` from " + toliteral(callers()));
      return;
    endif
    {code, msg, value, stack, traceback} = args;
    server_log("Uncaught error: " + toliteral(code) + "(" + toliteral(msg) + ") (value: " + toliteral(value) + ")\n" + toliteral(traceback));
    "Let the player object handle it if it wants to";
    return `player:(verb)(@args) ! ANY';
  endmethod

  method external_agent_tools owner: ARCH_WIZARD
    "Return tool definitions for external AI agents (MCP/Claude Code etc.)";
    "Returns list of maps with: name, description, input_schema, target_obj, target_verb";
    "Tools are executed as the authenticated player, not as wizard";
    callers() && !caller_perms().programmer && return E_PERM;
    tools = {};
    "Building tools from $agent_building_tools";
    for tool in ({@$agent_building_tools:get_tools(), @$change_tools:get_tools()})
      tools = {@tools, tool:to_mcp_schema()};
    endfor
    return tools;
  endmethod

  method external_agent_resources owner: ARCH_WIZARD
    "Return resource definitions for external AI agents (MCP/Claude Code etc.)";
    "Returns list of maps with: uri, name, description, mimeType, content";
    "Resources are read-only context that agents can browse";
    callers() && !caller_perms().programmer && return E_PERM;
    resources = {};
    "Building guide from $agent_building_tools";
    resources = {@resources, ["uri" -> "moor://building-guide", "name" -> "Building Guide", "description" -> "Instructions for using building tools to create rooms, objects, and configure behaviors.", "mimeType" -> "text/plain", "content" -> $agent_building_tools.guide]};
    resources = {@resources, ["uri" -> "moor://changes-guide", "name" -> "Change management", "description" -> "Review upstream updates, preserve local code, and apply guarded changes.", "mimeType" -> "text/plain", "content" -> $change_tools.guide]};
    return resources;
  endmethod

  method _log owner: ARCH_WIZARD
    callers() && !caller_perms().wizard && return E_PERM;
    server_log(@args);
  endmethod
endobject
