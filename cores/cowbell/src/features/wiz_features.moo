object WIZ_FEATURES [
  import_export_id -> "wiz_features",
  import_export_hierarchy -> {"features"}
]
  name: "Wizard Features"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property help_source (owner: ARCH_WIZARD, flags: "rc") = WIZARD_HELP_TOPICS;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Provides wizard-only administrative verbs (@programmer, @builder, @llm-*, etc.) for wizards.";

  verb "@announce" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Broadcast a message to all connected players.";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    msg = argstr;
    if (!msg || msg == "")
      player:inform_current($event:mk_error(player, "Usage: @announce <message>"):with_audience('utility));
      return;
    endif
    msg = "\u201C" + msg + "\u201D";
    title = $format.title:mk("Announcement from " + player:name());
    content = $format.block:mk(title, msg);
    event = $event:mk_info(player, content):with_audience('utility):with_presentation_hint('inset);
    for p in (connected_players())
      `p:tell(event) ! E_VERBNF => p:tell(event)';
    endfor
    player:inform_current($event:mk_info(player, "Announcement sent to " + tostr(length(connected_players())) + " connection(s)."):with_audience('utility));
  endverb

  verb "@programmer" (any none none) owner: ARCH_WIZARD flags: "d"
    "Grant or upgrade a player to programmer status";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr || dobjstr == "")
      raise(E_INVARG, "Usage: @programmer <player>");
    endif
    target = `$match:match_player(dobjstr) ! E_INVARG => $nothing';
    if (!valid(target))
      raise(E_INVARG, "No player found matching '" + dobjstr + "'");
    endif
    if (!is_player(target))
      raise(E_INVARG, tostr(target) + " is not a player.");
    endif
    "Check current status";
    if (target.authoring_features == $prog_features)
      player:inform_current($event:mk_error(player, target:name() + " is already a programmer."));
      return;
    endif
    is_upgrade = target.authoring_features == $builder_features;
    "Check if player has a description (skip for upgrades)";
    if (!is_upgrade)
      desc = target:description();
      if (!desc || desc == "")
        "Get pronouns for proper grammar";
        pronouns = `target:pronouns() ! E_VERBNF => $pronouns:mk('they, 'them, 'their, 'theirs, 'themself, false)';
        possessive = pronouns.possessive;
        question = "Grant " + target:name() + " programmer bit despite " + possessive + " lack of description?";
        metadata = {{"input_type", "yes_no"}, {"prompt", question}};
        response = player:read_with_prompt(metadata);
        typeof(response) == TYPE_STR || (response = tostr(response));
        response = response:trim():lowercase();
        if (!(response == "yes" || response == "y"))
          player:inform_current($event:mk_error(player, "Programmer bit not granted."));
          return;
        endif
      endif
    endif
    "Set features and flags";
    target.authoring_features = $prog_features;
    target.programmer = true;
    target.is_builder = true;
    "Handle tools";
    owner_name = target:name();
    if (is_upgrade)
      "Already has compass from builder, just add visor";
      visor = create($data_visor, target);
      visor.owner = target;
      visor.name = owner_name + "'s " + $data_visor.name;
      visor.aliases = $data_visor.aliases;
      visor:moveto(target);
      "Announce upgrade";
      if (valid(target.location))
        event = $event:mk_info(target, $sub:nc(), " ", $sub:verb_have(), " been upgraded to programmer privileges."):with_this(target.location);
        target.location:announce(event);
        tools_event = $event:mk_info(target, $sub:nc(), " ", $sub:verb_have(), " been granted a Data Visor."):with_this(target.location);
        target.location:announce(tools_event);
      endif
      target:tell($event:mk_info(target, "You have been upgraded to programmer. A Data Visor has been added to your inventory for code editing and advanced features."));
      player:inform_current($event:mk_info(player, "You upgraded ", target:name(), " to programmer privileges."));
    else
      "Fresh grant - create both tools";
      compass = create($architects_compass, target);
      compass.owner = target;
      compass.name = owner_name + "'s " + $architects_compass.name;
      compass.aliases = $architects_compass.aliases;
      compass:moveto(target);
      visor = create($data_visor, target);
      visor.owner = target;
      visor.name = owner_name + "'s " + $data_visor.name;
      visor.aliases = $data_visor.aliases;
      visor:moveto(target);
      "Announce fresh grant";
      if (valid(target.location))
        event = $event:mk_info(target, $sub:nc(), " ", $sub:verb_have(), " been granted programmer and builder privileges."):with_this(target.location);
        target.location:announce(event);
        tools_event = $event:mk_info(target, $sub:nc(), " ", $sub:verb_have(), " been granted an Architect's Compass and a Data Visor."):with_this(target.location);
        target.location:announce(tools_event);
      endif
      target:tell($event:mk_info(target, "In your inventory there are now an Architect's Compass and a Data Visor - powerful instruments bonded to you alone. Wear them to activate their capabilities: the Compass for building and spatial construction, the Visor for analyzing, writing code, creating objects, adding properties, and shaping the world's logic. Guard them carefully, as they grant significant power over the world."));
      player:inform_current($event:mk_info(player, "You granted ", target:name(), " programmer and builder privileges."));
    endif
  endverb

  verb "@builder" (any none none) owner: ARCH_WIZARD flags: "d"
    "Grant builder status or downgrade from programmer";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!valid(dobj))
      raise(E_INVARG, "Usage: @builder <player>");
    endif
    if (!is_player(dobj))
      raise(E_INVARG, tostr(dobj) + " is not a player.");
    endif
    "Check current status";
    if (dobj.authoring_features == $builder_features)
      player:inform_current($event:mk_error(player, dobj:name() + " is already a builder."));
      return;
    endif
    is_downgrade = dobj.authoring_features == $prog_features;
    "Confirm downgrade if necessary";
    if (is_downgrade)
      question = "Downgrade " + dobj:name() + " from programmer to builder? This will remove their Data Visor and programmer flag.";
      metadata = {{"input_type", "yes_no"}, {"prompt", question}};
      response = player:read_with_prompt(metadata);
      typeof(response) == TYPE_STR || (response = tostr(response));
      response = response:trim():lowercase();
      if (!(response == "yes" || response == "y"))
        player:inform_current($event:mk_error(player, "Downgrade cancelled."));
        return;
      endif
    endif
    "Check if player has a description (skip for downgrades)";
    if (!is_downgrade)
      desc = dobj:description();
      if (!desc || desc == "")
        pronouns = `dobj:pronouns() ! E_VERBNF => $pronouns:mk('they, 'them, 'their, 'theirs, 'themself, false)';
        possessive = pronouns.possessive;
        question = "Grant " + dobj:name() + " builder status despite " + possessive + " lack of description?";
        metadata = {{"input_type", "yes_no"}, {"prompt", question}};
        response = player:read_with_prompt(metadata);
        typeof(response) == TYPE_STR || (response = tostr(response));
        response = response:trim():lowercase();
        if (!(response == "yes" || response == "y"))
          player:inform_current($event:mk_error(player, "Builder status not granted."));
          return;
        endif
      endif
    endif
    "Set features and flags";
    dobj.authoring_features = $builder_features;
    dobj.is_builder = true;
    "Handle tools and flags";
    owner_name = dobj:name();
    if (is_downgrade)
      dobj.programmer = false;
      for item in (dobj.contents)
        if (valid(item) && isa(item, $data_visor))
          item:destroy();
          break;
        endif
      endfor
      if (valid(dobj.location))
        event = $event:mk_info(dobj, $sub:nc(), " ", $sub:verb_have(), " been downgraded to builder privileges."):with_this(dobj.location);
        dobj.location:announce(event);
      endif
      dobj:tell($event:mk_info(dobj, "You have been downgraded to builder. Your Data Visor has been removed, but you retain your Architect's Compass."));
      player:inform_current($event:mk_info(player, "You downgraded ", dobj:name(), " to builder privileges."));
    else
      compass = create($architects_compass, dobj);
      compass.owner = dobj;
      compass.name = owner_name + "'s " + $architects_compass.name;
      compass.aliases = $architects_compass.aliases;
      compass:moveto(dobj);
      if (valid(dobj.location))
        event = $event:mk_info(dobj, $sub:nc(), " ", $sub:verb_have(), " been granted builder privileges."):with_this(dobj.location);
        dobj.location:announce(event);
        tools_event = $event:mk_info(dobj, $sub:nc(), " ", $sub:verb_have(), " been granted an Architect's Compass."):with_this(dobj.location);
        dobj.location:announce(tools_event);
      endif
      dobj:tell($event:mk_info(dobj, "In your inventory there is now an Architect's Compass - a powerful instrument bonded to you alone. Wear it to activate its capabilities for building and spatial construction. Guard it carefully, as it grants significant power over the world."));
      player:inform_current($event:mk_info(player, "You granted ", dobj:name(), " builder privileges."));
    endif
  endverb

  verb "@reconfigure-tools" (none none none) owner: ARCH_WIZARD flags: "d"
    "Reconfigure all Architect's Compasses and Data Visors in the database";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    compasses = {};
    visors = {};
    for o in (descendants($architects_compass))
      if (valid(o) && o != $architects_compass)
        compasses = {@compasses, o};
      endif
    endfor
    for o in (descendants($data_visor))
      if (valid(o) && o != $data_visor)
        visors = {@visors, o};
      endif
    endfor
    total = length(compasses) + length(visors);
    if (total == 0)
      player:inform_current($event:mk_info(player, "No tool instances found to reconfigure."));
      return;
    endif
    question = "Reconfigure " + tostr(length(compasses)) + " compass(es) and " + tostr(length(visors)) + " visor(s)?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Reconfiguration cancelled."));
      return;
    endif
    compass_count = 0;
    for compass in (compasses)
      try
        compass:reconfigure();
        compass_count = compass_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to reconfigure " + tostr(compass) + ": " + toliteral(e)));
      endtry
    endfor
    visor_count = 0;
    for visor in (visors)
      try
        visor:reconfigure();
        visor_count = visor_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to reconfigure " + tostr(visor) + ": " + toliteral(e)));
      endtry
    endfor
    player:inform_current($event:mk_info(player, "Reconfigured " + tostr(compass_count) + " compass(es) and " + tostr(visor_count) + " visor(s)."));
  endverb

  verb "@shutdown" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Dump the database, announce, and shut down the server with countdown.";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    msg = argstr;
    if (!msg)
      msg = "Server is shutting down.";
    endif
    delay_minutes = 2;
    parts = msg:split(" ");
    if (length(parts) >= 2 && parts[1] == "in")
      possible_delay = toint(parts[2]);
      if (typeof(possible_delay) == TYPE_INT && possible_delay > 0)
        delay_minutes = possible_delay;
        remaining = length(parts) >= 3 ? parts[3..$] | {};
        msg = remaining ? remaining:join(" ") | "Server is shutting down.";
      endif
    endif
    question = "Shut down the server in " + tostr(delay_minutes) + " minute(s)?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Shutdown cancelled."):with_audience('utility));
      return;
    endif
    announce_times = {};
    delay = delay_minutes;
    if (delay > 0)
      while (delay > 0)
        announce_times = {@announce_times, delay * 60};
        delay = delay / 2;
      endwhile
      announce_times = {@announce_times, 30, 10};
    else
      announce_times = {0};
    endif
    for i in [1..length(announce_times)]
      seconds = announce_times[i];
      base_msg = $format.code:mk("** Server will shut down in " + tostr(seconds) + " second(s): " + msg + " **");
      event = $event:mk_info(player, $format.title:mk("Shutdown ..."), base_msg):with_audience('utility):with_presentation_hint('inset);
      for p in (connected_players())
        `p:tell(event) ! E_VERBNF => p:tell(event)';
      endfor
      next_delay = i < length(announce_times) ? announce_times[i] - announce_times[i + 1] | 0;
      if (next_delay > 0)
        suspend(next_delay);
      endif
    endfor
    final_msg = $format.code:mk("## Server shutdown: " + msg + " ##");
    final_event = $event:mk_info(player, final_msg):with_audience('utility):with_presentation_hint('inset);
    for p in (connected_players())
      `p:tell(final_event) ! E_VERBNF => p:tell(final_event)';
      `boot_player(p) ! ANY';
    endfor
    suspend(0);
    dump_database();
    shutdown(msg);
  endverb

  verb "@llm-budget" (any none none) owner: ARCH_WIZARD flags: "d"
    "View a player's LLM token budget and usage";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    "Try to resolve target player";
    target = dobj;
    if (!valid(target) && dobjstr)
      target = $match:match_player(dobjstr, player);
    endif
    if (!valid(target))
      raise(E_INVARG, "Usage: @llm-budget <player>");
    endif
    if (!is_player(target))
      raise(E_INVARG, tostr(target) + " is not a player.");
    endif
    "Get budget and usage information";
    budget = target.llm_token_budget;
    used = target.llm_tokens_used;
    usage_log = target.llm_usage_log;
    percent_used = used * 100 / budget;
    "Build budget info as table";
    title_obj = $format.title:mk("LLM Token Budget for " + target:name() + " (" + tostr(target) + ")");
    budget_rows = {{"Budget", tostr(budget) + " tokens"}, {"Used", tostr(used) + " tokens"}, {"Remaining", tostr(budget - used) + " tokens"}, {"Usage", tostr(percent_used) + "%"}};
    budget_table = $format.table:mk({"Property", "Value"}, budget_rows);
    "Build content blocks";
    content_blocks = {title_obj, budget_table};
    "Add usage log if available";
    if (length(usage_log) > 0)
      usage_rows = {};
      start_idx = length(usage_log) > 5 ? length(usage_log) - 4 | 1;
      for i in [start_idx..length(usage_log)]
        entry = usage_log[i];
        timestamp = `entry["timestamp"] ! ANY => 0';
        tokens = `entry["tokens"] ! ANY => 0';
        time_str = ctime(timestamp);
        usage_rows = {@usage_rows, {time_str, tostr(tokens) + " tokens"}};
      endfor
      usage_title = $format.title:mk("Recent usage (last 5 calls)");
      usage_table = $format.table:mk({"Time", "Tokens"}, usage_rows);
      content_blocks = {@content_blocks, usage_title, usage_table};
    endif
    "Send formatted output";
    content = $format.block:mk(@content_blocks);
    player:inform_current($event:mk_info(player, content):with_audience('utility));
  endverb

  verb "@llm-set-budget" (any at any) owner: ARCH_WIZARD flags: "d"
    "Set a player's LLM token budget";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    target = dobj;
    if (!valid(target) && dobjstr)
      target = $match:match_object(dobjstr, player);
    endif
    if (!valid(target))
      raise(E_INVARG, "Usage: @llm-set-budget <player> to <budget>");
    endif
    if (!is_player(target))
      raise(E_INVARG, tostr(target) + " is not a player.");
    endif
    if (!iobjstr)
      raise(E_INVARG, "Usage: @llm-set-budget <player> to <budget>");
    endif
    new_budget = tonum(iobjstr);
    typeof(new_budget) == TYPE_INT || raise(E_INVARG, "Budget must be a number.");
    new_budget > 0 || raise(E_INVARG, "Budget must be positive.");
    old_budget = target.llm_token_budget;
    used = target.llm_tokens_used;
    question = "Set " + target:name() + "'s LLM token budget to " + tostr(new_budget) + " (currently " + tostr(old_budget) + ", " + tostr(used) + " used)?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Budget change cancelled."));
      return;
    endif
    target.llm_token_budget = new_budget;
    player:inform_current($event:mk_info(player, "Set " + target:name() + "'s LLM token budget to " + tostr(new_budget) + " tokens."));
  endverb

  verb "@llm-reset-usage" (any none none) owner: ARCH_WIZARD flags: "d"
    "Reset a player's LLM token usage counter";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    target = dobj;
    if (!valid(target) && dobjstr)
      target = $match:match_object(dobjstr, player);
    endif
    if (!valid(target))
      raise(E_INVARG, "Usage: @llm-reset-usage <player>");
    endif
    if (!is_player(target))
      raise(E_INVARG, tostr(target) + " is not a player.");
    endif
    used = target.llm_tokens_used;
    budget = target.llm_token_budget;
    question = "Reset " + target:name() + "'s LLM token usage from " + tostr(used) + " to 0 (budget: " + tostr(budget) + ")?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Usage reset cancelled."));
      return;
    endif
    target.llm_tokens_used = 0;
    target.llm_usage_log = {};
    player:inform_current($event:mk_info(player, "Reset " + target:name() + "'s LLM token usage to 0 and cleared usage log."));
  endverb

  verb "@reissue-tools" (none none none) owner: ARCH_WIZARD flags: "rd"
    this:_challenge_command_perms();
    player.wizard || raise(E_PERM, "Only wizards can reissue tools.");
    "Destroy all existing visors and compasses, then reissue them to all programmers and builders";
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    compasses = {};
    visors = {};
    for o in (descendants($architects_compass))
      if (valid(o) && o != $architects_compass)
        compasses = {@compasses, o};
      endif
    endfor
    for o in (descendants($data_visor))
      if (valid(o) && o != $data_visor)
        visors = {@visors, o};
      endif
    endfor
    compass_recipients = {};
    for p in (players())
      if (valid(p) && p.is_builder && p != $hacker)
        compass_recipients = {@compass_recipients, p};
      endif
    endfor
    visor_recipients = {};
    for p in (players())
      if (valid(p) && p.programmer && p != $hacker)
        visor_recipients = {@visor_recipients, p};
      endif
    endfor
    question = "Destroy " + tostr(length(compasses)) + " compass(es) and " + tostr(length(visors)) + " visor(s), then reissue " + tostr(length(compass_recipients)) + " compass(es) and " + tostr(length(visor_recipients)) + " visor(s)?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Tool reissue cancelled."));
      return;
    endif
    compass_count = 0;
    for compass in (compasses)
      try
        compass:destroy();
        compass_count = compass_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to destroy " + tostr(compass) + ": " + toliteral(e)));
      endtry
    endfor
    visor_count = 0;
    for visor in (visors)
      try
        visor:destroy();
        visor_count = visor_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to destroy " + tostr(visor) + ": " + toliteral(e)));
      endtry
    endfor
    player:inform_current($event:mk_info(player, "Destroyed " + tostr(compass_count) + " compass(es) and " + tostr(visor_count) + " visor(s)."));
    compass_count = 0;
    for recipient in (compass_recipients)
      try
        owner_name = recipient:name();
        compass = create($architects_compass, recipient);
        compass.owner = recipient;
        compass.name = owner_name + "'s " + $architects_compass.name;
        compass.aliases = $architects_compass.aliases;
        compass:moveto(recipient);
        compass_count = compass_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to issue compass to " + tostr(recipient) + ": " + toliteral(e)));
      endtry
    endfor
    visor_count = 0;
    for recipient in (visor_recipients)
      try
        owner_name = recipient:name();
        visor = create($data_visor, recipient);
        visor.owner = recipient;
        visor.name = owner_name + "'s " + $data_visor.name;
        visor.aliases = $data_visor.aliases;
        visor:moveto(recipient);
        visor_count = visor_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to issue visor to " + tostr(recipient) + ": " + toliteral(e)));
      endtry
    endfor
    player:inform_current($event:mk_info(player, "Issued " + tostr(compass_count) + " compass(es) and " + tostr(visor_count) + " visor(s)."));
  endverb

  verb "@chown" (any at any) owner: ARCH_WIZARD flags: "rd"
    "Change ownership of objects, properties, or verbs";
    "Usage: @chown <object> to <new_owner>";
    "Usage: @chown <object>.<property> to <new_owner>";
    "Usage: @chown <object>:<verb> to <new_owner>";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@chown TARGET to NEW_OWNER")));
      return;
    endif
    target_spec = dobjstr:trim();
    owner_str = iobjstr:trim();
    "Match the new owner";
    try
      new_owner = $match:match_object(owner_str, player);
      typeof(new_owner) != TYPE_OBJ && raise(E_INVARG, "Owner must be an object.");
      !valid(new_owner) && raise(E_INVARG, "Owner object no longer exists.");
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find owner: " + e[2]));
      return;
    endtry
    "Parse the target specification";
    parsed = $prog_utils:parse_target_spec(target_spec);
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid target reference. Use 'object', 'object.property', or 'object:verb'"));
      return;
    endif
    object_str = parsed['object_str];
    if (parsed['type] == 'object)
      type = 'object;
      item_name = "";
    else
      selector = parsed['selectors][1];
      type = selector['kind];
      item_name = selector['item_name];
      if (!item_name)
        type = 'object;
      endif
      if (selector['inherited])
        player:inform_current($event:mk_error(player, "@chown only works on direct object properties and verbs, not inherited ones."));
        return;
      endif
    endif
    "Match the target object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find object: " + e[2]));
      return;
    endtry
    "Dispatch based on type";
    if (type == 'property)
      try
        metadata = $prog_utils:get_property_metadata(target_obj, item_name);
        current_perms = metadata:perms();
        metadata:set_perms(new_owner, current_perms);
        player:inform_current($event:mk_info(player, "Property ." + item_name + " on " + tostr(target_obj) + " now owned by " + tostr(new_owner) + "."));
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error changing property owner: " + e[2]));
      endtry
    elseif (type == 'verb)
      try
        metadata = $prog_utils:get_verb_metadata(target_obj, item_name);
        current_perms = metadata:flags();
        metadata:set_perms(new_owner, current_perms);
        player:inform_current($event:mk_info(player, "Verb :" + item_name + " on " + tostr(target_obj) + " now owned by " + tostr(new_owner) + "."));
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error changing verb owner: " + e[2]));
      endtry
    elseif (type == 'object)
      try
        target_obj:set_owner(new_owner);
        player:inform_current($event:mk_info(player, "Object " + tostr(target_obj) + " now owned by " + tostr(new_owner) + "."));
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error changing object owner: " + e[2]));
      endtry
    else
      "Inherited references not supported for @chown";
      player:inform_current($event:mk_error(player, "@chown only works on direct object properties and verbs, not inherited ones."));
    endif
  endverb

  verb "@llm-config" (any none none) owner: ARCH_WIZARD flags: "rd"
    "View or configure LLM client settings (endpoint, model, key)";
    "Usage: @llm-config                  - show current settings";
    "       @llm-config endpoint <url>   - set API endpoint";
    "       @llm-config model <name>     - set default model";
    "       @llm-config key              - set API key (prompts securely)";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    parts = argstr ? argstr:split(" ") | {};
    cmd = length(parts) > 0 ? parts[1] | "";
    if (cmd == "")
      "Show current configuration";
      key_status = $llm_client:is_configured() ? "configured" | "NOT SET";
      title = $format.title:mk("LLM Client Configuration");
      rows = {{"Endpoint", $llm_client.api_endpoint}, {"Model", $llm_client.model}, {"API Key", key_status}};
      table = $format.table:mk({"Setting", "Value"}, rows);
      content = $format.block:mk(title, table);
      player:inform_current($event:mk_info(player, content):with_audience('utility));
      return;
    elseif (cmd == "endpoint")
      length(parts) < 2 && raise(E_INVARG, "Usage: @llm-config endpoint <url>");
      new_endpoint = parts[2..$]:join(" ");
      !new_endpoint:starts_with("http") && raise(E_INVARG, "Endpoint must be a valid URL");
      old_endpoint = $llm_client.api_endpoint;
      $llm_client.api_endpoint = new_endpoint;
      player:inform_current($event:mk_info(player, "LLM endpoint changed from " + old_endpoint + " to " + new_endpoint));
    elseif (cmd == "model")
      length(parts) < 2 && raise(E_INVARG, "Usage: @llm-config model <name>");
      new_model = parts[2..$]:join(" ");
      old_model = $llm_client.model;
      $llm_client.model = new_model;
      player:inform_current($event:mk_info(player, "LLM model changed from " + old_model + " to " + new_model));
    elseif (cmd == "key")
      "Prompt for key securely";
      metadata = {{"input_type", "password"}, {"prompt", "Enter API key:"}};
      new_key = player:read_with_prompt(metadata);
      if (!new_key || new_key == "")
        player:inform_current($event:mk_error(player, "API key not changed."));
        return;
      endif
      $llm_client:set_api_key(new_key);
      player:inform_current($event:mk_info(player, "LLM API key has been set."));
    else
      raise(E_INVARG, "Unknown subcommand. Use: endpoint, model, or key");
    endif
  endverb

  verb "@llm-reset-agents" (none none none) owner: ARCH_WIZARD flags: "rd"
    "Reconfigure all LLM agents to pick up new client configuration";
    this:_challenge_command_perms();
    wearables = {};
    for proto in ({$data_visor, $architects_compass})
      for o in (descendants(proto))
        if (valid(o))
          wearables = {@wearables, o};
        endif
      endfor
    endfor
    observers = {};
    for o in (descendants($llm_room_observer))
      if (valid(o))
        observers = {@observers, o};
      endif
    endfor
    total = length(wearables) + length(observers);
    if (total == 0)
      player:inform_current($event:mk_info(player, "No LLM agent instances found to reconfigure."));
      return;
    endif
    question = "Reconfigure " + tostr(length(wearables)) + " wearable(s) and " + tostr(length(observers)) + " observer(s)?";
    metadata = {{"input_type", "yes_no"}, {"prompt", question}};
    response = player:read_with_prompt(metadata);
    typeof(response) == TYPE_STR || (response = tostr(response));
    response = response:trim():lowercase();
    if (!(response == "yes" || response == "y"))
      player:inform_current($event:mk_error(player, "Reconfiguration cancelled."));
      return;
    endif
    wearable_count = 0;
    for wearable in (wearables)
      try
        wearable:reconfigure();
        wearable_count = wearable_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to reconfigure " + tostr(wearable) + ": " + toliteral(e)));
      endtry
    endfor
    observer_count = 0;
    for observer in (observers)
      try
        observer:reconfigure();
        observer_count = observer_count + 1;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Failed to reconfigure " + tostr(observer) + ": " + toliteral(e)));
      endtry
    endfor
    player:inform_current($event:mk_info(player, "Reconfigured " + tostr(wearable_count) + " wearable(s) and " + tostr(observer_count) + " observer(s)."));
  endverb

  method _challenge_command_perms owner: HACKER flags: "xd"
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for wizard commands via configured help source.";
    {for_player, ?topic = ""} = args;
    sources = {};
    primary = this.help_source;
    if (valid(primary))
      sources = {@sources, primary};
    endif
    include_admin = true;
    admin_features = $sysobj.admin_features;
    if (typeof(for_player) == TYPE_OBJ && valid(for_player) && valid(admin_features))
      player_admin_features = for_player.admin_features;
      typeof(player_admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
      if (is_member(admin_features, player_admin_features))
        include_admin = false;
      endif
    endif
    if (include_admin && valid(admin_features))
      admin_source = admin_features.help_source;
      if (valid(admin_source) && !is_member(admin_source, sources))
        sources = {@sources, admin_source};
      endif
    endif
    aggregated = {};
    for source in (sources)
      result = `source:help_topics(for_player, topic) ! ANY => 0';
      if (typeof(result) == TYPE_INT)
        continue;
      endif
      if (topic == "")
        if (typeof(result) == TYPE_LIST)
          aggregated = {@aggregated, @result};
        else
          aggregated = {@aggregated, result};
        endif
      else
        return result;
      endif
    endfor
    if (topic == "" && length(aggregated) > 0)
      return aggregated;
    endif
    verb_help = `$help_utils:verb_help_from_hint(this, topic, 'administration) ! ANY => 0';
    typeof(verb_help) != TYPE_INT && return verb_help;
    return 0;
  endmethod

  verb "@sueval" (any none none) owner: ARCH_WIZARD flags: "d"
    "HINT: <expression> -- Evaluate a MOO expression with wizard permissions.";
    this:_challenge_command_perms();
    if (!argstr || argstr:trim() == "")
      player:inform_current($event:mk_error(player, "Usage: @sueval <expression>"):with_audience('utility));
      return;
    endif
    "If invoked via @sudo, keep delegated perms; otherwise, use caller perms.";
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    try
      answer = eval("return " + argstr + ";", ['me -> player, 'here -> player.location], 1, 2);
      if (answer[1])
        result = answer[2];
        "Format the result";
        {result_type, result_content} = this:_format_eval_result(result);
        if (result_type == 'deflist)
          "Object result - show as definition list";
          result_event = $event:mk_eval_result(player, result_content):with_presentation_hint('inset):with_group('eval);
        else
          "Simple result - show in code block with =>";
          code = $format.code:mk("=> " + result_content, 'moo);
          result_event = $event:mk_eval_result(player, code):with_group('eval);
        endif
      else
        error_content = answer[2];
        error_text = error_content:join("\n");
        result_event = $event:mk_eval_error(player, $format.code:mk(error_text)):with_group('eval);
      endif
    except id (ANY)
      traceback = {"Eval failed: " + toliteral(id[1]) + " " + toliteral(id[2]) + ":"};
      for tb in (id[4])
        target = toliteral(tb[4]) + ":";
        if (tb[4] == #-1)
          target = "builtin function ";
        endif
        traceback = {@traceback, tostr("... called from ", target, tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6])};
      endfor
      traceback = {@traceback, "(End of traceback)"};
      result_event = $event:mk_eval_exception(player, $format.code:mk(traceback)):with_group('eval);
    endtry
    player:inform_current(result_event);
  endverb

  verb "@wizard" (any none none) owner: ARCH_WIZARD flags: "rd"
    "Grant wizard status to a player";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr || dobjstr == "")
      raise(E_INVARG, "Usage: @wizard <player>");
    endif
    target = `$match:match_player(dobjstr) ! E_INVARG => $nothing';
    if (!valid(target))
      raise(E_INVARG, "No player found matching '" + dobjstr + "'");
    endif
    if (!is_player(target))
      raise(E_INVARG, tostr(target) + " is not a player.");
    endif
    if (target.wizard)
      player:inform_current($event:mk_error(player, target:name() + " is already a wizard."));
      return;
    endif
    was_programmer = target.authoring_features == $prog_features;
    was_builder = target.authoring_features == $builder_features;
    "Ensure programmer/builder flags and tools";
    target.authoring_features = $prog_features;
    target.programmer = true;
    target.is_builder = true;
    owner_name = target:name();
    if (!was_programmer)
      if (was_builder)
        visor = create($data_visor, target);
        visor.owner = target;
        visor.name = owner_name + "'s " + $data_visor.name;
        visor.aliases = $data_visor.aliases;
        visor:moveto(target);
      else
        compass = create($architects_compass, target);
        compass.owner = target;
        compass.name = owner_name + "'s " + $architects_compass.name;
        compass.aliases = $architects_compass.aliases;
        compass:moveto(target);
        visor = create($data_visor, target);
        visor.owner = target;
        visor.name = owner_name + "'s " + $data_visor.name;
        visor.aliases = $data_visor.aliases;
        visor:moveto(target);
      endif
    endif
    "Ensure admin and wizard feature objects are wired";
    admin_features = target.admin_features;
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    admin_feat_obj = $admin_features;
    if (valid(admin_feat_obj) && !is_member(admin_feat_obj, admin_features))
      admin_features = {@admin_features, admin_feat_obj};
    endif
    if (!is_member(this, admin_features))
      admin_features = {@admin_features, this};
    endif
    target.admin_features = admin_features;
    target.wizard = true;
    if (valid(target.location))
      event = $event:mk_info(target, $sub:nc(), " ", $sub:verb_have(), " been granted wizard privileges."):with_this(target.location);
      target.location:announce(event);
    endif
    target:tell($event:mk_info(target, "You have been granted wizard privileges. Your command set now includes programmer, admin, and wizard features."));
    player:inform_current($event:mk_info(player, "You granted ", target:name(), " wizard privileges."));
  endverb

  verb "@audit-cprops" (any none none) owner: ARCH_WIZARD flags: "rd"
    "Audit c-flagged property owners against object owners.";
    "Usage: @audit-cprops [--repair|-r] [OBJECT]";
    "With no OBJECT, scans all objects visible to current task permissions.";
    this:_challenge_command_perms();
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    start_time = ftime();
    input = argstr ? argstr:trim() | "";
    repair = false;
    target_spec = "";
    if (input)
      for tok in (input:words())
        if (tok == "--repair" || tok == "-r")
          repair = true;
        elseif (tok:starts_with("-"))
          player:inform_current($event:mk_error(player, "Unknown option: " + tok));
          return;
        elseif (target_spec == "")
          target_spec = tok;
        else
          target_spec = target_spec + " " + tok;
        endif
      endfor
    endif
    if (target_spec != "")
      try
        target = $match:match_object(target_spec, player);
      except e (ANY)
        player:inform_current($event:mk_error(player, "Could not resolve object: " + e[2]));
        return;
      endtry
      scan_objects = {target};
    else
      scan_objects = objects();
    endif
    if (!scan_objects)
      player:inform_current($event:mk_info(player, "No objects available to scan."));
      return;
    endif
    mismatches = {};
    scanned_props = 0;
    repaired_count = 0;
    failed_repairs = 0;
    obj_count = 0;
    for obj in (scan_objects)
      obj_count = obj_count + 1;
      if (obj_count % 25 == 0)
        suspend_if_needed();
      endif
      prop_names = obj:all_properties();
      for prop_name in (prop_names)
        info = property_info(obj, prop_name);
        if (!(typeof(info) == TYPE_LIST && length(info) >= 2))
          continue;
        endif
        perms = info[2];
        if (!(typeof(perms) == TYPE_STR && index(perms, "c")))
          continue;
        endif
        scanned_props = scanned_props + 1;
        prop_owner = info[1];
        if (prop_owner != obj.owner)
          action = "mismatch";
          if (repair)
            try
              set_property_info(obj, prop_name, {obj.owner, perms});
              repaired_count = repaired_count + 1;
              action = "repaired";
            except e (ANY)
              failed_repairs = failed_repairs + 1;
              action = "failed: " + toliteral(e);
            endtry
          endif
          obj_name = `obj.name ! ANY => "<unnamed>"';
          if (typeof(obj_name) != TYPE_STR || obj_name == "")
            obj_name = "<unnamed>";
          endif
          obj_owner_name = valid(obj.owner) ? obj.owner.name | "Recycled";
          prop_owner_name = valid(prop_owner) ? prop_owner.name | "Recycled";
          mismatches = {@mismatches, {obj, obj_name, prop_name, obj.owner, prop_owner, obj_owner_name, prop_owner_name, action}};
        endif
      endfor
    endfor
    elapsed = ftime() - start_time;
    if (!mismatches)
      scope = length(scan_objects) == 1 ? tostr(" on ", scan_objects[1]) | "";
      if (repair)
        player:inform_current($event:mk_info(player, tostr("No mismatched c-properties found", scope, ". Nothing repaired. Scanned ", length(scan_objects), " object(s), ", scanned_props, " c-property entries. Time: ", elapsed, "s")));
      else
        player:inform_current($event:mk_info(player, tostr("No mismatched c-properties found", scope, ". Scanned ", length(scan_objects), " object(s), ", scanned_props, " c-property entries. Time: ", elapsed, "s")));
      endif
      return;
    endif
    if (repair)
      headers = {"Object", "Property", "Object Owner", "Property Owner", "Action"};
    else
      headers = {"Object", "Property", "Object Owner", "Property Owner"};
    endif
    rows = {};
    row_count = 0;
    for m in (mismatches)
      row_count = row_count + 1;
      if (row_count % 50 == 0)
        suspend_if_needed();
      endif
      {obj, obj_name, prop_name, obj_owner, prop_owner, obj_owner_name, prop_owner_name, action} = m;
      obj_label = tostr(obj) + " (" + obj_name + ")";
      base = {obj_label, "." + prop_name, obj_owner_name + " (" + tostr(obj_owner) + ")", prop_owner_name + " (" + tostr(prop_owner) + ")"};
      if (repair)
        rows = {@rows, {@base, action}};
      else
        rows = {@rows, base};
      endif
    endfor
    summary = tostr("Found ", length(mismatches), " mismatched c-properties across ", length(scan_objects), " object(s). Scanned ", scanned_props, " c-property entries.");
    if (repair)
      summary = summary + tostr(" Repaired ", repaired_count, "; failed ", failed_repairs, ".");
    endif
    content = $format.block:mk(summary, $format.table:mk(headers, rows));
    player:inform_current($event:mk_info(player, content));
    player:inform_current($event:mk_info(player, tostr("Time: ", elapsed, "s")));
  endverb

  verb "@changes" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Review and apply program updates. Use help @changes for commands.";
    this == $wiz_features || raise(E_PERM);
    this:_challenge_command_perms();
    const authority = player.wizard ? player | $admin_features:_resolve_delegate(player);
    set_task_perms(authority);
    let lines;
    try
      lines = $change_manager:command(args);
    except failure (E_INVARG)
      lines = {$format.title:mk("Changes", 3), $format.paragraph:mk(failure[2])};
      if (typeof(failure[3]) == TYPE_MAP && `failure[3]["code"] ! E_RANGE => ""' == "missing_review")
        lines = {@lines, $format.list:actions({$format.annotation:command("@changes packages", $format.paragraph:inline("Find active reviews", "\n", $format.code:inline("@changes packages")))})};
      elseif (typeof(failure[3]) == TYPE_MAP && `failure[3]["code"] ! E_RANGE => ""' == "active_review" && maphaskey(failure[3], "review_id"))
        const details = failure[3];
        const ready = details["status"] in {"ready", "partial", "rejected"};
        const label = ready ? "Open existing review" | "Check existing review";
        const content = $format.paragraph:inline(label, "\n", $format.code:inline(details["command"]));
        const link = ready ? $format.annotation:mk(content, ["kind" -> "change", "provider" -> $url_utils:to_curie_str($change_manager), "review" -> details["review_id"], "generation" -> details["generation"]]) | $format.annotation:command(details["command"], content);
        lines = {@lines, $format.list:actions({link})};
      endif
    endtry
    length(lines) || return;
    player:inform_current($event:mk_info(player, $format.block:mk(@lines)):with_audience('utility):with_presentation_hint('inset));
  endverb
endobject
