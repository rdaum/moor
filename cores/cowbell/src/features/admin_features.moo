object ADMIN_FEATURES [
  import_export_id -> "admin_features",
  import_export_hierarchy -> {"features"}
]
  name: "Admin Features"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property help_source (owner: ARCH_WIZARD, flags: "rc") = ADMIN_HELP_TOPICS;
  property sudo_active (owner: ARCH_WIZARD, flags: "rc") = [];
  property sudo_allowed (owner: ARCH_WIZARD, flags: "rc") = [];
  property sudo_delegates (owner: ARCH_WIZARD, flags: "rc") = [];
  property sudo_log (owner: ARCH_WIZARD, flags: "rc") = {};
  property sudo_log_limit (owner: ARCH_WIZARD, flags: "rc") = 200;
  property sudo_require_confirm (owner: ARCH_WIZARD, flags: "rc") = 1;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Provides delegated admin command elevation (@sudo) with explicit grants and per-command allowlists.";

  method _challenge_command_perms owner: ARCH_WIZARD
    caller == player || caller == #-1 || caller_perms() == player || caller_perms().wizard || raise(E_PERM);
  endmethod

  method _is_allowed_verb owner: ARCH_WIZARD
    {subject, target, verb_name} = args;
    typeof(subject) == TYPE_OBJ || return false;
    typeof(verb_name) == TYPE_STR || return false;
    needle = verb_name:trim():lowercase();
    if (!needle)
      return false;
    endif
    target_key = "";
    if (typeof(target) == TYPE_OBJ && valid(target))
      target_key = tostr(target):lowercase();
    endif
    allowed_map = this.sudo_allowed;
    typeof(allowed_map) == TYPE_MAP || return false;
    if (!maphaskey(allowed_map, subject))
      return false;
    endif
    allowed = allowed_map[subject];
    typeof(allowed) == TYPE_LIST || return false;
    for raw in (allowed)
      if (typeof(raw) != TYPE_STR)
        continue;
      endif
      token = raw:trim():lowercase();
      if (!token)
        continue;
      endif
      if (token == "*" || token == needle)
        return true;
      endif
      split_at = index(token, "::");
      if (!split_at)
        continue;
      endif
      obj_part = token[1..split_at - 1];
      verb_part = token[split_at + 2..$];
      if (obj_part && verb_part == needle && target_key && obj_part == target_key)
        return true;
      endif
    endfor
    return false;
  endmethod

  verb "@sudo-revoke" (any none none) owner: ARCH_WIZARD flags: "d"
    "HINT: <player> -- Revoke sudo delegation and allowlist for a player.";
    "Usage: @sudo-revoke <player>";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr)
      raise(E_INVARG, "Usage: @sudo-revoke <player>");
    endif
    target = `dobj ! E_INVIND => $nothing';
    if (!valid(target))
      target = $match:match_player(dobjstr, player);
    endif
    if (!valid(target) || !is_player(target))
      raise(E_INVARG, "Target must be a valid player.");
    endif
    delegates = this.sudo_delegates;
    typeof(delegates) == TYPE_MAP || (delegates = []);
    old_delegate = maphaskey(delegates, target) ? delegates[target] | $nothing;
    if (maphaskey(delegates, target))
      delegates = mapdelete(delegates, target);
    endif
    this.sudo_delegates = delegates;
    allowed_map = this.sudo_allowed;
    typeof(allowed_map) == TYPE_MAP || (allowed_map = []);
    old_allow = maphaskey(allowed_map, target) ? allowed_map[target] | {};
    if (maphaskey(allowed_map, target))
      allowed_map = mapdelete(allowed_map, target);
    endif
    this.sudo_allowed = allowed_map;
    active = this.sudo_active;
    if (typeof(active) == TYPE_MAP)
      for tid in (mapkeys(active))
        entry = active[tid];
        if (typeof(entry) == TYPE_MAP && entry["subject"] == target)
          active = mapdelete(active, tid);
        endif
      endfor
      this.sudo_active = active;
    endif
    admin_features = target.admin_features;
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    filtered = {};
    for feat in (admin_features)
      if (feat != this)
        filtered = {@filtered, feat};
      endif
    endfor
    target.admin_features = filtered;
    this:_append_log(["kind" -> "revoke", "actor" -> player, "subject" -> target, "delegate" -> old_delegate, "allow" -> old_allow]);
    player:inform_current($event:mk_info(player, "Revoked sudo delegation for " + target:name() + "."));
    if (target != player)
      target:tell($event:mk_info(target, player:name() + " revoked your sudo access."));
    endif
  endverb

  method _resolve_delegate owner: ARCH_WIZARD
    {subject} = args;
    typeof(subject) == TYPE_OBJ || return $nothing;
    if (subject.wizard)
      return subject;
    endif
    delegates = this.sudo_delegates;
    typeof(delegates) == TYPE_MAP || return $nothing;
    if (!maphaskey(delegates, subject))
      return $nothing;
    endif
    delegate = delegates[subject];
    typeof(delegate) == TYPE_OBJ || return $nothing;
    valid(delegate) || return $nothing;
    return delegate;
  endmethod

  verb "@sudo-allow" (any any any) owner: ARCH_WIZARD flags: "d"
    "HINT: <player> to <verb|obj::verb,...> -- Set per-player sudo allowlist.";
    "Usage: @sudo-allow <player> to <verb|obj::verb,...>";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr || !iobjstr)
      raise(E_INVARG, "Usage: @sudo-allow <player> to <verb|obj::verb,...>");
    endif
    target = `dobj ! E_INVIND => $nothing';
    if (!valid(target))
      target = $match:match_player(dobjstr, player);
    endif
    if (!valid(target) || !is_player(target))
      raise(E_INVARG, "Target must be a valid player.");
    endif
    verbs = {};
    seen = [];
    for raw in (iobjstr:split(","))
      if (typeof(raw) != TYPE_STR)
        continue;
      endif
      token = raw:trim():lowercase();
      if (!token)
        continue;
      endif
      split_at = index(token, "::");
      if (split_at)
        obj_part = token[1..split_at - 1]:trim();
        verb_part = token[split_at + 2..$]:trim();
        if (!obj_part || !verb_part)
          raise(E_INVARG, "Malformed allowlist token: " + raw);
        endif
        resolved_obj = `toobj(obj_part) ! E_INVARG => $nothing';
        if (valid(resolved_obj))
          obj_part = tostr(resolved_obj):lowercase();
        endif
        token = obj_part + "::" + verb_part;
      endif
      if (!maphaskey(seen, token))
        verbs = {@verbs, token};
        seen[token] = 1;
      endif
    endfor
    if (!verbs)
      raise(E_INVARG, "Provide at least one verb or obj::verb token.");
    endif
    allowed_map = this.sudo_allowed;
    typeof(allowed_map) == TYPE_MAP || (allowed_map = []);
    allowed_map[target] = verbs;
    this.sudo_allowed = allowed_map;
    this:_append_log(["kind" -> "allow", "actor" -> player, "subject" -> target, "allow" -> verbs]);
    player:inform_current($event:mk_info(player, "Updated sudo allowlist for " + target:name() + ": " + verbs:join(", ") + "."));
  endverb

  verb "@sudo-grant" (any any any) owner: ARCH_WIZARD flags: "d"
    "HINT: <player> as <wizard_player> -- Grant sudo delegation and seed an allowlist for a player.";
    "Usage: @sudo-grant <player> as <wizard_player>";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (!dobjstr || !iobjstr)
      raise(E_INVARG, "Usage: @sudo-grant <player> as <wizard_player>");
    endif
    prepstr == "as" || raise(E_INVARG, "Use `as`: @sudo-grant <player> as <wizard_player>");
    target = `dobj ! E_INVIND => $nothing';
    if (!valid(target))
      target = $match:match_player(dobjstr, player);
    endif
    delegate = `iobj ! E_INVIND => $nothing';
    if (!valid(delegate))
      delegate = $match:match_player(iobjstr, player);
    endif
    if (!valid(target) || !valid(delegate) || !is_player(target) || !is_player(delegate))
      raise(E_INVARG, "Both target and delegate must be valid players.");
    endif
    delegates = this.sudo_delegates;
    typeof(delegates) == TYPE_MAP || (delegates = []);
    delegates[target] = delegate;
    this.sudo_delegates = delegates;
    allowed_map = this.sudo_allowed;
    typeof(allowed_map) == TYPE_MAP || (allowed_map = []);
    if (!maphaskey(allowed_map, target))
      allowed_map[target] = {"@llm-budget", "@llm-set-budget", "@llm-reset-usage"};
    endif
    this.sudo_allowed = allowed_map;
    admin_features = target.admin_features;
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    if (!is_member(this, admin_features))
      admin_features = {@admin_features, this};
      target.admin_features = admin_features;
    endif
    this:_append_log(["kind" -> "grant", "actor" -> player, "subject" -> target, "delegate" -> delegate, "allow" -> allowed_map[target]]);
    player:inform_current($event:mk_info(player, "Granted sudo delegation for " + target:name() + " as " + delegate:name() + "."));
    if (target != player)
      target:tell($event:mk_info(target, player:name() + " granted you sudo access as " + delegate:name() + ". Use ", $format.code:inline("@sudo <command>"), " with an allowlisted command."));
    endif
  endverb

  verb "@sudo" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <command> -- Run an allowlisted command with delegated wizard dispatch rights.";
    this:_challenge_command_perms();
    command = argstr:trim();
    if (!command)
      player:inform_current($event:mk_error(player, "Usage: @sudo <command>"));
      return false;
    endif
    delegate = this:_resolve_delegate(player);
    if (!valid(delegate))
      this:_append_log(["kind" -> "sudo", "status" -> "denied", "reason" -> "no_delegate", "subject" -> player, "command" -> command]);
      player:inform_current($event:mk_error(player, "No sudo delegation configured for you."));
      return false;
    endif
    set_task_perms(delegate);
    match_env = {player};
    for item in (player.contents)
      if (valid(item))
        match_env = {@match_env, item};
      endif
    endfor
    for item in (player.wearing)
      if (valid(item))
        match_env = {@match_env, item};
      endif
    endfor
    mailbox = player:find_mailbox();
    if (valid(mailbox))
      match_env = {@match_env, mailbox};
    endif
    location = player.location;
    if (valid(location))
      if (respond_to(location, 'match_scope_for))
        ambient = location:match_scope_for(player);
        if (typeof(ambient) == TYPE_LIST)
          match_env = {@match_env, @ambient};
        endif
      endif
      match_env = {@match_env, location};
    endif
    try
      pc = parse_command(command, match_env, true, 0.3);
    except e (ANY)
      set_task_perms(player);
      this:_append_log(["kind" -> "sudo", "status" -> "denied", "reason" -> "parse_failed", "subject" -> player, "delegate" -> delegate, "command" -> command, "error" -> toliteral(e[2])]);
      player:inform_current($event:mk_error(player, "Could not parse sudo command: ", $format.code:inline(command), "."));
      return false;
    endtry
    command_env = {player};
    features = player.features;
    typeof(features) == TYPE_LIST || raise(E_TYPE, "player.features must be a list");
    command_env = {@command_env, @features};
    authoring_features = player.authoring_features;
    if (valid(authoring_features))
      command_env = {@command_env, authoring_features};
    endif
    admin_features = player.admin_features;
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    for feat in (admin_features)
      if (valid(feat))
        command_env = {@command_env, feat};
      endif
    endfor
    if (valid(location))
      command_env = {@command_env, location};
    endif
    if (valid($wiz_features))
      command_env = {@command_env, $wiz_features};
    endif
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
    any_match = false;
    blocked = [];
    for dobj in (dobj_candidates)
      for iobj in (iobj_candidates)
        test_pc = pc;
        test_pc['dobj] = dobj;
        test_pc['iobj] = iobj;
        vm_matches = find_command_verb(test_pc, command_env);
        if (!vm_matches)
          continue;
        endif
        for m in (vm_matches)
          any_match = true;
          {target, verbspec} = m;
          {definer, flags, verbnames, matched_name} = verbspec;
          if (!this:_is_allowed_verb(player, target, matched_name))
            blocked[matched_name] = 1;
            continue;
          endif
          if (this.sudo_require_confirm)
            question = "Run `" + command + "` as " + delegate:name() + "?";
            metadata = {{"input_type", "yes_no"}, {"prompt", question}};
            try
              response = player:read_with_prompt(metadata);
            except e (ANY)
              set_task_perms(player);
              this:_append_log(["kind" -> "sudo", "status" -> "error", "subject" -> player, "delegate" -> delegate, "command" -> command, "target" -> target, "verb" -> matched_name, "error" -> toliteral(e)]);
              player:inform_current($event:mk_error(player, "Sudo confirmation failed: " + toliteral(e)));
              return false;
            endtry
            typeof(response) == TYPE_STR || (response = tostr(response));
            response = response:trim():lowercase();
            if (!(response == "yes" || response == "y"))
              set_task_perms(player);
              this:_append_log(["kind" -> "sudo", "status" -> "cancelled", "subject" -> player, "delegate" -> delegate, "command" -> command, "target" -> target, "verb" -> matched_name]);
              player:inform_current($event:mk_error(player, "Sudo cancelled."));
              return false;
            endif
          endif
          this:_mark_elevated(player, delegate, matched_name, command);
          try
            suspend(0);
            result = dispatch_command_verb(target, matched_name, test_pc);
          except e (ANY)
            this:_clear_elevated();
            set_task_perms(player);
            this:_append_log(["kind" -> "sudo", "status" -> "error", "subject" -> player, "delegate" -> delegate, "command" -> command, "target" -> target, "verb" -> matched_name, "error" -> toliteral(e[2])]);
            player:inform_current($event:mk_error(player, "Sudo command failed: " + toliteral(e[2])));
            return false;
          endtry
          this:_clear_elevated();
          set_task_perms(player);
          this:_append_log(["kind" -> "sudo", "status" -> "ok", "subject" -> player, "delegate" -> delegate, "command" -> command, "target" -> target, "verb" -> matched_name]);
          return result;
        endfor
      endfor
    endfor
    set_task_perms(player);
    if (!any_match)
      this:_append_log(["kind" -> "sudo", "status" -> "denied", "reason" -> "no_match", "subject" -> player, "delegate" -> delegate, "command" -> command]);
      player:inform_current($event:mk_error(player, "No command verb matched ", $format.code:inline(command), " here."));
      return false;
    endif
    allowed_map = this.sudo_allowed;
    if (typeof(allowed_map) != TYPE_MAP)
      allowed_map = [];
    endif
    allowed = maphaskey(allowed_map, player) ? allowed_map[player] | {};
    if (typeof(allowed) != TYPE_LIST)
      allowed = {};
    endif
    blocked_list = mapkeys(blocked);
    if (blocked_list)
      allowed_str = allowed ? allowed:join(", ") | "(none)";
      this:_append_log(["kind" -> "sudo", "status" -> "denied", "reason" -> "allowlist", "subject" -> player, "delegate" -> delegate, "command" -> command, "blocked" -> blocked_list]);
      player:inform_current($event:mk_error(player, "Command matched " + blocked_list:join(", ") + " but is not in your sudo allowlist. Allowed: " + allowed_str + "."));
      return false;
    endif
    this:_append_log(["kind" -> "sudo", "status" -> "denied", "reason" -> "allowlist", "subject" -> player, "delegate" -> delegate, "command" -> command]);
    player:inform_current($event:mk_error(player, "Command is not permitted by your sudo allowlist."));
    return false;
  endverb

  method _clear_elevated owner: ARCH_WIZARD
    "Clear any elevation mark for the current task.";
    caller_perms().wizard || raise(E_PERM);
    active = this.sudo_active;
    typeof(active) == TYPE_MAP || return 0;
    tid = task_id();
    if (maphaskey(active, tid))
      this.sudo_active = mapdelete(active, tid);
      return 1;
    endif
    return 0;
  endmethod

  method is_elevated owner: ARCH_WIZARD
    "True when current task is running a delegated admin command for subject.";
    {subject} = args;
    typeof(subject) == TYPE_OBJ || return false;
    active = this.sudo_active;
    typeof(active) == TYPE_MAP || return false;
    tid = task_id();
    if (!maphaskey(active, tid))
      return false;
    endif
    entry = active[tid];
    typeof(entry) == TYPE_MAP || return false;
    expires = entry["expires"];
    if (typeof(expires) == TYPE_INT && expires > 0 && time() > expires)
      this.sudo_active = mapdelete(active, tid);
      return false;
    endif
    entry_subject = entry["subject"];
    if (entry_subject != subject)
      return false;
    endif
    return true;
  endmethod

  method _mark_elevated owner: ARCH_WIZARD
    "Mark current task as elevated for delegated admin command execution.";
    caller_perms().wizard || raise(E_PERM);
    {subject, delegate, verb_name, command} = args;
    typeof(subject) == TYPE_OBJ || raise(E_TYPE);
    typeof(delegate) == TYPE_OBJ || raise(E_TYPE);
    active = this.sudo_active;
    typeof(active) == TYPE_MAP || (active = []);
    active[task_id()] = ["subject" -> subject, "delegate" -> delegate, "verb" -> verb_name, "command" -> command, "expires" -> time() + 120];
    this.sudo_active = active;
    return task_id();
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for administration commands via configured help source.";
    {for_player, ?topic = ""} = args;
    source = this.help_source;
    if (valid(source))
      result = source:help_topics(for_player, topic);
      if (typeof(result) != TYPE_INT)
        return result;
      endif
    endif
    verb_help = $help_utils:verb_help_from_hint(this, topic, 'administration);
    typeof(verb_help) != TYPE_INT && return verb_help;
    return 0;
  endmethod

  verb "@dump-database" (none none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: -- Export a database checkpoint and wait for its result.";
    this:_challenge_command_perms();
    "If running via @sudo, we keep the elevated perms. Otherwise, we drop to player perms.";
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    player:inform_current($event:mk_info(player, "Triggering database dump..."));
    try
      if (dump_database(1))
        player:inform_current($event:mk_info(player, "Database dump completed successfully."));
      else
        player:inform_current($event:mk_error(player, "Database dump failed. Check the server log for details."));
      endif
    except e (ANY)
      player:inform_current($event:mk_error(player, "Database dump failed: " + toliteral(e[2])));
    endtry
  endverb

  method _append_log owner: ARCH_WIZARD
    "Append one sudo audit record and enforce capped retention.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {entry} = args;
    typeof(entry) == TYPE_MAP || raise(E_TYPE);
    if (!maphaskey(entry, "time"))
      entry["time"] = time();
    endif
    log = this.sudo_log;
    if (typeof(log) != TYPE_LIST)
      log = {};
    endif
    limit = this.sudo_log_limit;
    if (typeof(limit) != TYPE_INT || limit < 1)
      limit = 200;
      this.sudo_log_limit = limit;
    endif
    log = {@log, entry};
    while (length(log) > limit)
      log = listdelete(log, 1);
    endwhile
    this.sudo_log = log;
    return length(log);
  endmethod

  verb "@sudo-show" (any none none) owner: ARCH_WIZARD flags: "d"
    "HINT: <player> -- Show sudo delegation, allowlist, and active tasks for a player.";
    "Usage: @sudo-show <player>";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    if (dobjstr)
      target = `dobj ! E_INVIND => $nothing';
      if (!valid(target))
        target = $match:match_player(dobjstr, player);
      endif
    else
      target = player;
    endif
    if (!valid(target) || !is_player(target))
      raise(E_INVARG, "Target must be a valid player.");
    endif
    delegates = this.sudo_delegates;
    typeof(delegates) == TYPE_MAP || (delegates = []);
    delegate = maphaskey(delegates, target) ? delegates[target] | $nothing;
    allowed_map = this.sudo_allowed;
    typeof(allowed_map) == TYPE_MAP || (allowed_map = []);
    allowed = maphaskey(allowed_map, target) ? allowed_map[target] | {};
    typeof(allowed) == TYPE_LIST || (allowed = {});
    player:inform_current($event:mk_info(player, "Sudo for " + target:name() + ": delegate=" + (valid(delegate) ? delegate:name() | "(none)") + ", allowlist=" + (allowed ? allowed:join(", ") | "(none)") + "."));
    active = this.sudo_active;
    if (typeof(active) != TYPE_MAP)
      active = [];
    endif
    count = 0;
    for tid in (mapkeys(active))
      entry = active[tid];
      if (typeof(entry) != TYPE_MAP)
        continue;
      endif
      if (entry["subject"] != target)
        continue;
      endif
      expires = entry["expires"];
      if (typeof(expires) == TYPE_INT && expires > 0 && time() > expires)
        continue;
      endif
      count = count + 1;
      verb = tostr(entry["verb"]);
      cmd = tostr(entry["command"]);
      player:inform_current($event:mk_info(player, "  active tid=" + tostr(tid) + " verb=" + verb + " cmd=" + cmd));
    endfor
    if (!count)
      player:inform_current($event:mk_info(player, "  active tasks: none"));
    endif
    return true;
  endverb

  verb "@sudo-who" (none none none) owner: ARCH_WIZARD flags: "d"
    "HINT: -- List active sudo tasks and recent sudo audit entries.";
    "Usage: @sudo-who";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    active = this.sudo_active;
    if (typeof(active) != TYPE_MAP)
      active = [];
    endif
    now = time();
    active_rows = {};
    for tid in (mapkeys(active))
      entry = active[tid];
      if (typeof(entry) != TYPE_MAP)
        continue;
      endif
      expires = entry["expires"];
      if (typeof(expires) == TYPE_INT && expires > 0 && now > expires)
        active = mapdelete(active, tid);
        continue;
      endif
      subject = entry["subject"];
      delegate = entry["delegate"];
      verb = tostr(entry["verb"]);
      cmd = tostr(entry["command"]);
      subject_name = valid(subject) ? subject:name() | "?";
      delegate_name = valid(delegate) ? delegate:name() | "?";
      active_rows = {@active_rows, {tostr(tid), subject_name, delegate_name, verb, cmd}};
    endfor
    this.sudo_active = active;
    log = this.sudo_log;
    if (typeof(log) != TYPE_LIST)
      log = {};
    endif
    audit_rows = {};
    start = length(log) - 9;
    if (start < 1)
      start = 1;
    endif
    for i in [start..length(log)]
      entry = log[i];
      audit_rows = {@audit_rows, this:_format_audit_entry(entry)};
    endfor
    title = $format.title:mk("Sudo Status");
    active_title = $format.title:mk("Active Elevated Tasks");
    if (active_rows)
      active_table = $format.table:mk({"Task", "Subject", "Delegate", "Verb", "Command"}, active_rows);
    else
      active_table = $format.code:mk("No active sudo tasks.");
    endif
    audit_title = $format.title:mk("Recent Audit (last 10)");
    if (audit_rows)
      audit_table = $format.table:mk({"Time", "Status", "Kind", "Actor", "Subject", "Delegate", "Verb", "Command"}, audit_rows);
    else
      audit_table = $format.code:mk("No sudo audit entries.");
    endif
    content = $format.block:mk(title, active_title, active_table, audit_title, audit_table);
    player:inform_current($event:mk_info(player, content):with_audience('utility));
    return true;
  endverb

  verb "@sudo-log" (any none none) owner: ARCH_WIZARD flags: "d"
    "HINT: [N] -- Show recent sudo audit log entries (default 20).";
    "Usage: @sudo-log [N]";
    this:_challenge_command_perms();
    player.wizard || player:has_admin_elevation() || raise(E_PERM);
    if (!player:has_admin_elevation())
      set_task_perms(player);
    endif
    n = 20;
    if (argstr)
      parsed = toint(argstr:trim());
      if (typeof(parsed) != TYPE_INT || parsed < 1)
        raise(E_INVARG, "Usage: @sudo-log [N], where N is a positive integer.");
      endif
      n = parsed;
    endif
    log = this.sudo_log;
    if (typeof(log) != TYPE_LIST || !length(log))
      player:inform_current($event:mk_info(player, "No sudo audit entries."):with_audience('utility));
      return true;
    endif
    start = length(log) - n + 1;
    if (start < 1)
      start = 1;
    endif
    rows = {};
    for i in [start..length(log)]
      entry = log[i];
      rows = {@rows, this:_format_audit_entry(entry)};
    endfor
    title = $format.title:mk("Sudo Audit Log");
    summary = $format.code:mk("Showing entries " + tostr(start) + "-" + tostr(length(log)) + " of " + tostr(length(log)) + ".");
    table = $format.table:mk({"Time", "Status", "Kind", "Actor", "Subject", "Delegate", "Verb", "Command"}, rows);
    content = $format.block:mk(title, summary, table);
    player:inform_current($event:mk_info(player, content):with_audience('utility));
    return true;
  endverb

  method _format_audit_entry owner: ARCH_WIZARD
    "Format one sudo audit entry row, including malformed entries.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {entry} = args;
    if (typeof(entry) != TYPE_MAP)
      return {"", "invalid", "malformed", "", "", "", "", toliteral(entry)};
    endif
    ts = `entry["time"] ! ANY => 0';
    if (typeof(ts) == TYPE_INT && ts > 0)
      ts_str = ctime(ts);
    else
      ts_str = tostr(ts);
    endif
    status = `entry["status"] ! ANY => ""';
    if (!status)
      status = tostr(`entry["kind"] ! ANY => ""');
    else
      status = tostr(status);
    endif
    kind = tostr(`entry["kind"] ! ANY => ""');
    actor = `entry["actor"] ! ANY => $nothing';
    subject = `entry["subject"] ! ANY => $nothing';
    delegate = `entry["delegate"] ! ANY => $nothing';
    verb = tostr(`entry["verb"] ! ANY => ""');
    cmd = tostr(`entry["command"] ! ANY => ""');
    actor_name = valid(actor) ? `actor:name() ! ANY => tostr(actor)' | "";
    subject_name = valid(subject) ? `subject:name() ! ANY => tostr(subject)' | "";
    delegate_name = valid(delegate) ? `delegate:name() ! ANY => tostr(delegate)' | "";
    return {ts_str, status, kind, actor_name, subject_name, delegate_name, verb, cmd};
  endmethod

  method test_format_audit_entry_reports_malformed_entries owner: ARCH_WIZARD
    "Audit display should show malformed entries instead of silently skipping them.";
    row = this:_format_audit_entry("not a map");
    $test_utils:assert_eq(row[2], "invalid", "malformed audit entry should have invalid status");
    $test_utils:assert_eq(row[3], "malformed", "malformed audit entry should identify malformed kind");
    $test_utils:assert_eq(row[8], "\"not a map\"", "malformed audit entry should preserve the literal value");
    return true;
  endmethod
endobject
