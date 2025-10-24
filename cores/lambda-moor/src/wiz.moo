object WIZ
  name: "generic wizard"
  parent: PROG
  owner: #2
  readable: true

  property advertised (owner: #2, flags: "rc") = 1;
  property mail_identity (owner: #2, flags: "c") = LOCAL;
  property newt_msg (owner: #2, flags: "rc") = "%n @newts %d (%[#d])";
  property newt_victim_msg (owner: #2, flags: "rc") = "";
  property programmer_msg (owner: #2, flags: "rc") = "%d is now a programmer.";
  property programmer_victim_msg (owner: #2, flags: "rc") = "You are now a programmer.";
  property public_identity (owner: #2, flags: "rc") = LOCAL;
  property toad_msg (owner: #2, flags: "rc") = "%n @toads %d (%[#d])";
  property toad_victim_msg (owner: #2, flags: "rc") = "Have a nice life...";

  override aliases = {"player"};
  override description = "You see a wizard who chooses not to reveal its true appearance.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override help = WIZ_HELP;
  override mail_notify (owner: #2, flags: "rc");
  override object_size = {56607, 1084848672};
  override password = "really impossible password to type";

  verb "@chown*#" (any any any) owner: #2 flags: "rd"
    if (!player.wizard || player != this)
      player:notify("Sorry.");
      return;
    endif
    set_task_perms(player);
    args = setremove(args, "to");
    if (length(args) != 2 || !args[2])
      player:notify(tostr("Usage:  ", verb, " <object-or-property-or-verb> <owner>"));
      return;
    endif
    what = args[1];
    owner = $string_utils:match_player(args[2]);
    bynumber = verb == "@chown#";
    if ($command_utils:player_match_result(owner, args[2])[1])
    elseif (spec = $code_utils:parse_verbref(what))
      object = this:my_match_object(spec[1]);
      if (!$command_utils:object_match_failed(object, spec[1]))
        vname = spec[2];
        if (bynumber)
          vname = $code_utils:toint(vname);
          if (vname == E_TYPE)
            return player:notify("Verb number expected.");
          elseif (vname < 1 || vname > length(verbs(object)))
            return player:notify("Verb number out of range.");
          endif
        endif
        info = `verb_info(object, vname) ! ANY';
        if (info == E_VERBNF)
          player:notify("That object does not define that verb.");
        elseif (typeof(info) == ERR)
          player:notify(tostr(info));
        else
          try
            result = set_verb_info(object, vname, listset(info, owner, 1));
            player:notify("Verb owner set.");
          except e (ANY)
            player:notify(e[2]);
          endtry
        endif
      endif
    elseif (bynumber)
      player:notify("@chown# can only be used with verbs.");
    elseif (index(what, ".") && (spec = $code_utils:parse_propref(what)))
      object = this:my_match_object(spec[1]);
      if (!$command_utils:object_match_failed(object, spec[1]))
        pname = spec[2];
        e = $wiz_utils:set_property_owner(object, pname, owner);
        if (e == E_NONE)
          player:notify("+c Property owner set.  Did you really want to do that?");
        else
          player:notify(tostr(e && "Property owner set."));
        endif
      endif
    else
      object = this:my_match_object(what);
      if (!$command_utils:object_match_failed(object, what))
        player:notify(tostr($wiz_utils:set_owner(object, owner) && "Object ownership changed."));
      endif
    endif
  endverb

  verb "@shout" (any any any) owner: #2 flags: "rd"
    if (caller != this)
      raise(E_PERM);
    endif
    set_task_perms(player);
    if (length(args) == 1 && argstr[1] == "\"")
      argstr = args[1];
    endif
    shout = $gender_utils:get_conj("shouts", player);
    for person in (connected_players())
      if (person != player)
        person:notify(tostr(player.name, " ", shout, ", \"", argstr, "\""));
      endif
    endfor
    player:notify(tostr("You shout, \"", argstr, "\""));
  endverb

  verb "@grant @grants* @transfer" (any at any) owner: #2 flags: "rd"
    "@grant <object> to <player>";
    "@grants <object> to <player>   --- same as @grant but may suspend.";
    "@transfer <expression> to <player> -- like 'grant', but evalutes a possible list of objects to transfer, and modifies quota.";
    "Ownership of the object changes as in @chown and :set_owner (i.e., .owner and all c properties change).  In addition all verbs and !c properties owned by the original owner change ownership as well.  Finally, for !c properties, instances on descendant objects change ownership (as in :set_property_owner).";
    if (!player.wizard || player != this)
      player:notify("Sorry.");
      return;
    endif
    set_task_perms(player);
    if (!iobjstr || !dobjstr)
      return player:notify(tostr("Usage:  ", verb, " <object> to <player>"));
    endif
    if ($command_utils:player_match_failed(newowner = $string_utils:match_player(iobjstr), iobjstr))
      "...newowner is bogus...";
      return;
    endif
    if (verb == "@transfer")
      objlist = player:eval_cmd_string(dobjstr, 0);
      if (!objlist[1])
        player:notify(tostr("Had trouble reading `", dobjstr, "': "));
        player:notify_lines(@objlist[2]);
        return;
      elseif (typeof(objlist[2]) == OBJ)
        objlist = objlist[2..2];
      elseif (typeof(objlist[2]) != LIST)
        player:notify(tostr("Value of `", dobjstr, "' is not an object or list:  ", toliteral(objlist[2])));
        return;
      else
        objlist = objlist[2];
      endif
    elseif ($command_utils:object_match_failed(object = this:my_match_object(dobjstr), dobjstr))
      "...object is bogus...";
      return;
    else
      objlist = {object};
    endif
    "Used to check for quota of newowner, but doesn't anymore, cuz the quota check doesn't work";
    suspendok = verb != "@grant";
    player:tell("Transferring ", toliteral(objlist), " to ", $string_utils:nn(newowner));
    for object in (objlist)
      $command_utils:suspend_if_needed(0);
      same = object.owner == newowner;
      for vnum in [1..length(verbs(object))]
        info = verb_info(object, vnum);
        if (!(info[1] != object.owner && (valid(info[1]) && is_player(info[1]))))
          same = same && info[1] == newowner;
          set_verb_info(object, vnum, listset(info, newowner, 1));
        endif
      endfor
      for prop in (properties(object))
        if (suspendok && (ticks_left() < 5000 || seconds_left() < 2))
          suspend(0);
        endif
        info = property_info(object, prop);
        if (!(index(info[2], "c") || (info[1] != object.owner && valid(info[1]) && is_player(info[1]))))
          same = same && info[1] == newowner;
          $wiz_utils:set_property_owner(object, prop, newowner, suspendok);
        endif
      endfor
      if (suspendok)
        suspend(0);
      endif
      $wiz_utils:set_owner(object, newowner, suspendok);
      if (same)
        player:notify(tostr(newowner.name, " already owns everything ", newowner.ps, " is entitled to on ", object.name, "."));
      else
        player:notify(tostr("Ownership changed on ", $string_utils:nn(object), ", verb, properties and descendants' properties."));
      endif
    endfor
    player:notify(tostr(verb, " complete."));
  endverb

  verb "@programmer" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    dobj = $string_utils:match_player(dobjstr);
    if (dobj == $nothing)
      player:notify(tostr("Usage:  ", verb, " <playername>"));
    elseif ($command_utils:player_match_result(dobj, dobjstr)[1])
    elseif ($wiz_utils:check_prog_restricted(dobj))
      return player:notify(tostr("Sorry, ", dobj.name, " is not allowed to be a programmer."));
    elseif (dobj.description == $player.description && !$command_utils:yes_or_no($string_utils:pronoun_sub("@Programmer %d despite %[dpp] lack of description?")))
      player:notify(tostr("Okay, leaving ", dobj.name, " !programmer."));
      return;
    elseif (result = $wiz_utils:set_programmer(dobj))
      player:notify(tostr(dobj.name, " (", dobj, ") is now a programmer.  ", dobj.ppc, " quota is currently ", $quota_utils:get_quota(dobj), "."));
      player:notify(tostr(dobj.name, " and the other wizards have been notified."));
      if (msg = this:programmer_victim_msg())
        dobj:notify(msg);
      endif
      if ($object_utils:isa(dobj.location, $room) && (msg = this:programmer_msg()))
        dobj.location:announce_all_but({dobj}, msg);
      endif
    elseif (result == E_NONE)
      player:notify(tostr(dobj.name, " (", dobj, ") is already a programmer..."));
    else
      player:notify(tostr(result));
    endif
  endverb

  verb "make-core-database" (any none none) owner: #2 flags: "rd"
    {?core_variant_name = ""} = args;
    if (!player.wizard)
      player:notify("Nice try, but permission denied.");
      return;
    elseif (length(connected_players()) > 1)
      player:notify("You need to @boot everybody else before I'll believe this isn't the real MOO.");
      abort = 1;
    elseif (`boot_player(open_network_connection("localhost", 666)) ! ANY' != E_PERM)
      player:notify("Why are outbound connections enabled?  I bet this is the real MOO.");
      abort = 1;
    else
      abort = !$command_utils:yes_or_no("Continuing with this command will destroy all but the central core of the database.  Are you sure you want to do this?  ") || !$command_utils:yes_or_no("Really sure? ");
    endif
    if (abort)
      player:notify("Core database extraction aborted.");
      return;
    endif
    "----------------------------------------";
    player:notify("Messing with server options...");
    spi = {};
    for p in ({"protect_recycle", "protect_set_property_info", "protect_add_property", "protect_chparent", "bg_ticks"})
      spi = {@spi, {p, $server_options.(p)}};
      $server_options.(p) = 0;
    endfor
    $server_options.bg_ticks = 1000000;
    add_property($server_options, "bg_seconds", 7, {player, "r"});
    `load_server_options() ! ANY';
    add_property($server_options, "__mcd__savesopt", spi, {player, "r"});
    "----------------------------------------";
    player:notify("Killing all queued tasks ...");
    for t in (queued_tasks())
      kill_task(t[1]);
    endfor
    suspend(0);
    "----------------------------------------";
    player:notify(tostr("Identifying objects to be saved", @core_variant_name ? {" for core variant '", core_variant_name, "'"} | {}, " ..."));
    "... TODO --- core variant name lookup?";
    core_variant = {{"name", core_variant_name}};
    {saved, saved_props, skipped_parents, proxy_original, proxy_incore} = $core_object_info(core_variant, 1);
    if (!(player in saved))
      player:notify("Sorry, but this won't work unless you yourself are on the list of objects to be saved.");
      player:notify("Core database extraction aborted.");
      return;
    endif
    for ops in (saved_props)
      {o, o_props} = ops;
      for p in (o_props)
        if (i = o.(p) in proxy_original)
          o.(p) = proxy_incore[i];
        endif
      endfor
    endfor
    "... TODO --- why isn't this on #0:init_for_core ? --Rog";
    $player_class = $mail_recipient_class;
    "----------------------------------------";
    player:notify("Stripping you of any personal verbs and/or properties ...");
    for i in [1..length(verbs(player))]
      delete_verb(player, 1);
    endfor
    for p in (properties(player))
      delete_property(player, p);
    endfor
    chparent(player, $wiz);
    for p in ($object_utils:all_properties(player))
      clear_property(player, p);
    endfor
    player:set_name("Wizard");
    player:set_aliases({"Wizard"});
    player.description = "";
    player.key = 0;
    player.ownership_quota = 100;
    player.password = 0;
    player.last_password_time = 0;
    $gender_utils:set(player, "neuter");
    "----------------------------------------";
    suspend(0);
    owners_original = owners_incore = {};
    for i in [1..length(proxy_original)]
      o = proxy_original[i];
      if (is_player(o) && o != $no_one)
        owners_original = {@owners_original, o};
        owners_incore = {@owners_incore, proxy_incore[i]};
      endif
    endfor
    for o in (saved)
      if (is_player(o) && o != $no_one)
        owners_original = {@owners_original, o};
        owners_incore = {@owners_incore, o};
      endif
    endfor
    player:notify(tostr("Chowning every saved object, verb and property to one of ", $string_utils:nn(owners_incore), "..."));
    for o in (saved)
      $command_utils:suspend_if_needed(0, "... ", length(saved) - (o in saved), " to go");
      if (i = o.owner in owners_original)
        o.owner = owners_incore[i];
      elseif (valid(o.owner) && o.owner.wizard)
        o.owner = player;
      else
        o.owner = $hacker;
      endif
      old_verbs = {};
      for j in [1..length(verbs(o))]
        $command_utils:suspend_if_needed(0, "... ", length(saved) - (o in saved), " to go");
        info = verb_info(o, j);
        if (i = info[1] in owners_original)
          info[1] = owners_incore[i];
        elseif (valid(info[1]) && info[1].wizard)
          info[1] = player;
        else
          info[1] = $hacker;
        endif
        set_verb_info(o, j, info);
        if (index(info[3], "(old)"))
          old_verbs = {j, @old_verbs};
        endif
      endfor
      for vname in (old_verbs)
        delete_verb(o, vname);
      endfor
      for p in ($object_utils:all_properties(o))
        $command_utils:suspend_if_needed(0, "... ", length(saved) - (o in saved), " to go");
        info = property_info(o, p);
        if (i = info[1] in owners_original)
          info[1] = owners_incore[i];
        elseif (valid(info[1]) && info[1].wizard)
          info[1] = player;
        else
          info[1] = $hacker;
        endif
        set_property_info(o, p, info);
      endfor
    endfor
    "----------------------------------------";
    player:notify("Removing all unsaved :recycle and :exitfunc verbs ...");
    for o in [#0..max_object()]
      $command_utils:suspend_if_needed(0, "... ", o);
      if (valid(o) && !(o in saved))
        for v in ({"recycle", "exitfunc"})
          while ($object_utils:defines_verb(o, v))
            delete_verb(o, v);
          endwhile
        endfor
      endif
    endfor
    "----------------------------------------";
    player:notify("Recycling unsaved objects ...");
    add_property(this, "__mcd__pos", toint(max_object()), {player, "r"});
    add_property(this, "__mcd__save", {core_variant, saved, saved_props, skipped_parents}, {player, "r"});
    suspend(0);
    try
      this:mcd_2(core_variant, saved, saved_props, skipped_parents);
    finally
      if (!queued_tasks() && `this.__mcd__save ! E_PROPNF => 0')
        "...use raw notify since we have no idea what will be b0rken";
        notify(player, "Core database extraction failed.");
      endif
    endtry
  endverb

  verb "@shutdown" (any any any) owner: #2 flags: "rd"
    if (!player.wizard)
      player:notify("Sorry.");
      return;
    elseif ($code_utils:task_valid($shutdown_task))
      player:notify(tostr("Shutdown already in progress.  The MOO will be shut down in ", $time_utils:english_time($shutdown_time - time()), ", by ", $shutdown_message));
      return;
    endif
    if (s = match(argstr, "^in +%([0-9]+%)%( +%|$%)"))
      bounds = s[3][1];
      delay = toint(argstr[bounds[1]..bounds[2]]);
      argstr = argstr[s[2] + 1..$];
    else
      delay = 2;
    endif
    if (!$command_utils:yes_or_no(tostr("Do you really want to shut down the server in ", delay, " minutes?")))
      player:notify("Aborted.");
      return;
    endif
    announce_times = {};
    if (delay > 0)
      while (delay > 0)
        announce_times = {@announce_times, delay * 60};
        delay = delay / 2;
      endwhile
      announce_times = {@announce_times, 30, 10};
      $shutdown_time = time() + announce_times[1];
    endif
    $shutdown_message = tostr(player.name, " (", player, "): ", argstr);
    $shutdown_task = task_id();
    for i in [1..length(announce_times)]
      base_msg = tostr("*** The server will be shut down by ", player.name, " (", player, ") in ", $time_utils:english_time(announce_times[i]), ":");
      msg = {base_msg, @$generic_editor:fill_string("*** " + argstr, length(base_msg) - 4, "*** ")};
      "...use raw notify() since :notify() verb could be broken...";
      for p in (connected_players())
        for line in (msg)
          notify(p, line);
        endfor
        $command_utils:suspend_if_needed(0);
      endfor
      suspend(announce_times[i] - {@announce_times, 0}[i + 1]);
    endfor
    for p in (connected_players())
      notify(p, tostr("*** Server shutdown by ", $shutdown_message, " ***"));
      boot_player(p);
    endfor
    suspend(0);
    $shutdown_task = E_NONE;
    set_task_perms(player);
    shutdown(argstr);
  endverb

  verb "@dump-d*atabase" (none none none) owner: #2 flags: "rd"
    set_task_perms(player);
    dump_database();
    player:notify("Dumping...");
  endverb

  verb "@who-calls" (any any any) owner: #2 flags: "rd"
    set_task_perms(player);
    if (argstr[1] != ":")
      argstr = ":" + argstr;
    endif
    player:notify(tostr("Searching for verbs that appear to call ", argstr, " ..."));
    player:notify("");
    $code_utils:find_verbs_containing(argstr + "(");
  endverb

  verb mcd_2 (none none none) owner: #2 flags: "rxd"
    if (!caller_perms().wizard)
      return;
    elseif (length(connected_players()) > 1)
      return;
    elseif (`boot_player(open_network_connection("localhost", 666)) ! ANY' != E_PERM)
      return;
    elseif (!("__mcd__pos" in properties(this)))
      return;
    endif
    end = this.__mcd__pos;
    {core_variant, saved, saved_props, skipped_parents} = args;
    player:notify(tostr("*** Recycling from #", end, " ..."));
    suspend(0);
    fork (0)
      try
        this:mcd_2(core_variant, saved, saved_props, skipped_parents);
      finally
        if (!queued_tasks() && `this.__mcd__save ! E_PROPNF => 0')
          "...use raw notify since we have no idea what will be b0rken";
          notify(player, "Core database extraction failed.");
        endif
      endtry
    endfork
    for i in [0..end]
      this.__mcd__pos = end - i;
      o = toobj(end - i);
      if ($command_utils:running_out_of_time())
        return;
      endif
      if (valid(o) && !(o in saved))
        for x in (o.contents)
          move(x, #-1);
        endfor
        if (is_player(o))
          "o.features = {}";
          set_player_flag(o, 0);
        endif
        if (!(o in skipped_parents))
          chparent(o, #-1);
        endif
        recycle(o);
      endif
    endfor
    delete_property(this, "__mcd__pos");
    spi = $server_options.__mcd__savesopt;
    delete_property($server_options, "__mcd__savesopt");
    delete_property($server_options, "bg_seconds");
    for pv in (spi)
      $server_options.((pv[1])) = pv[2];
    endfor
    load_server_options();
    "----------------------------------------";
    suspend(0);
    player:notify("Killing queued tasks ...");
    for t in (queued_tasks())
      kill_task(t[1]);
    endfor
    "----------------------------------------";
    player:notify("Compacting object numbers ...");
    old_oids = new_oids = {player};
    for o_ps in (saved_props)
      $command_utils:suspend_if_needed(0);
      {o, o_props} = o_ps;
      for p in (o_props)
        if (p == "owner" || p == "location")
          "...renumber() takes care of these";
        elseif (i = (old = o.(p)) in old_oids)
          o.(p) = new_oids[i];
        elseif (valid(old))
          new_oids[1..0] = {o.(p) = renumber(old)};
          old_oids[1..0] = {old};
        endif
      endfor
    endfor
    for o in (saved)
      if (valid(o) && o != player)
        renumber(o);
      endif
    endfor
    reset_max_object();
    "...rebuild saved list so that parents come before children...";
    saved = {};
    for o in [#0..max_object()]
      os = {};
      while (valid(o) && !(o in saved))
        os = {o, @os};
        o = parent(o);
      endwhile
      saved = {@saved, @os};
    endfor
    "----------------------------------------";
    player:notify("Performing miscellaneous cleanups ...");
    succeeded = 1;
    for o in [#0..max_object()]
      $command_utils:suspend_if_needed(0);
      try
        move(o, #-1);
      except e (ANY)
        player:notify(tostr("Couldn't move ", o, " => ", e[2]));
        player:notify(toliteral(e[4]));
        succeeded = 0;
      endtry
    endfor
    for o in (saved)
      $command_utils:suspend_if_needed(0);
      if ($object_utils:has_callable_verb(o, "init_for_core"))
        try
          o:init_for_core(core_variant);
        except e (ANY)
          player:notify(tostr("Error from ", o, ":init_for_core => ", e[2]));
          player:notify(toliteral(e[4]));
          succeeded = 0;
        endtry
      endif
    endfor
    player:notify("Re-measuring everything ...");
    for o in [#0..max_object()]
      $command_utils:suspend_if_needed(0);
      if (valid(o))
        $byte_quota_utils:object_bytes(o);
      endif
    endfor
    $wiz_utils:initialize_owned();
    $byte_quota_utils:summarize_one_user(player);
    delete_property(this, "__mcd__save");
    player:notify("Core database extraction " + (succeeded ? "is complete." | "failed."));
    if (succeeded)
      boot_player(player);
      shutdown();
    endif
  endverb

  verb "@toad @toad! @toad!!" (any any any) owner: #2 flags: "rd"
    "@toad[!][!] <player> [blacklist|redlist|graylist] [commentary]";
    whostr = args[1];
    comment = $string_utils:first_word(argstr)[2];
    if (verb == "@toad!!")
      listname = "redlist";
    elseif (verb == "@toad!")
      listname = "blacklist";
    elseif ((ln = {@args, ""}[2]) && index(listname = $login:listname(ln), ln) == 1)
      "...first word of coment is one of the magic words...";
      comment = $string_utils:first_word(comment)[2];
    else
      listname = "";
    endif
    if (!player.wizard || player != this)
      player:notify("Yeah, right... you wish.");
      return;
    elseif ($command_utils:player_match_failed(who = $string_utils:match_player(whostr), whostr))
      return;
    elseif (whostr != who.name && !(whostr in who.aliases) && whostr != tostr(who))
      player:notify(tostr("Must be a full name or an object number:  ", who.name, "(", who, ")"));
      return;
    elseif (who == player)
      player:notify("If you want to toad yourself, you have to do it by hand.");
      return;
    endif
    dobj = who;
    if (msg = player:toad_victim_msg())
      notify(who, msg);
    endif
    if ($wiz_utils:rename_all_instances(who, "disfunc", "toad_disfunc"))
      player:notify(tostr(who, ":disfunc renamed."));
    endif
    if ($wiz_utils:rename_all_instances(who, "recycle", "toad_recycle"))
      player:notify(tostr(who, ":recycle renamed."));
    endif
    "MOO-specific cleanup while still a player object.";
    this:toad_cleanup(who);
    e = $wiz_utils:unset_player(who, $hacker);
    player:notify(e ? tostr(who.name, "(", who, ") is now a toad.") | tostr(e));
    if (e && ($object_utils:isa(who.location, $room) && (msg = player:toad_msg())))
      who.location:announce_all_but({who}, msg);
    endif
    if (listname && !$login:((listname + "ed"))(cname = $string_utils:connection_hostname(who.last_connect_place)))
      $login:((listname + "_add"))(cname);
      player:notify(tostr("Site ", cname, " ", listname, "ed."));
    else
      cname = "";
    endif
    if (!comment)
      player:notify("So why is this person being toaded?");
      comment = $command_utils:read();
    endif
    $mail_agent:send_message(player, $toad_log, tostr("@toad ", who.name, " (", who, ")"), {$string_utils:from_list(who.all_connect_places, " "), @cname ? {$string_utils:capitalize(listname + "ed:  ") + cname} | {}, @comment ? {comment} | {}});
    player:notify(tostr("Mail sent to ", $mail_agent:name($toad_log), "."));
    `$local.waitlist:note_reapee(who, tostr("@toaded by ", player.name)) ! ANY';
  endverb

  verb "@untoad @detoad" (any any any) owner: #2 flags: "rd"
    "@untoad <object> [as namespec]";
    "Turns object into a player.  Anything that isn't a guest is chowned to itself.";
    if (!player.wizard)
      player:notify("Yeah, right... you wish.");
    elseif (prepstr && prepstr != "as")
      player:notify(tostr("Usage:  ", verb, " <object> [as name,alias,alias...]"));
    elseif ($command_utils:object_match_failed(dobj, dobjstr))
    elseif (prepstr && !(e = $building_utils:set_names(dobj, iobjstr)))
      player:notify(tostr("Initial rename failed:  ", e));
    elseif (e = $wiz_utils:set_player(dobj, g = $object_utils:isa(dobj, $guest)))
      player:notify(tostr(dobj.name, "(", dobj, ") is now a ", g ? "usable guest." | "player."));
    elseif (e == E_INVARG)
      player:notify(tostr(dobj.name, "(", dobj, ") is not of an appropriate player class."));
      player:notify("@chparent it to $player or some descendant.");
    elseif (e == E_NONE)
      player:notify(tostr(dobj.name, "(", dobj, ") is already a player."));
    elseif (e == E_NACC)
      player:notify("Wait until $player_db is finished updating...");
    elseif (e == E_RECMOVE)
      player:notify(tostr("The name `", dobj.name, "' is currently unavailable."));
      player:notify(tostr("Try again with   ", verb, " ", dobj, " as <newname>"));
    else
      player:notify(tostr(e));
    endif
  endverb

  verb "@quota" (any is any) owner: #2 flags: "rd"
    "@quota <player> is [public] <number> [<reason>]";
    "  changes a player's quota.  sends mail to the wizards.";
    if (player != this)
      return player:notify("Permission denied.");
    endif
    set_task_perms(player);
    dobj = $string_utils:match_player(dobjstr);
    if ($command_utils:player_match_result(dobj, dobjstr)[1])
      return;
    elseif (!valid(dobj))
      player:notify("Set whose quota?");
      return;
    endif
    if (iobjstr[1..min(7, $)] == "public ")
      iobjstr[1..7] = "";
      if ($object_utils:has_property($local, "public_quota_log"))
        recipients = {$quota_log, $local.public_quota_log};
      else
        player:tell("No public quota log.");
        return E_INVARG;
      endif
    else
      recipients = {$quota_log};
    endif
    old = $quota_utils:get_quota(dobj);
    qstr = iobjstr[1..(n = index(iobjstr + " ", " ")) - 1];
    new = $code_utils:toint(qstr[1] == "+" ? qstr[2..$] | qstr);
    reason = iobjstr[n + 1..$] || "(none)";
    if (typeof(new) != INT)
      player:notify(tostr("Set ", dobj.name, "'s quota to what?"));
      return;
    elseif (qstr[1] == "+")
      new = old + new;
    endif
    result = $quota_utils:set_quota(dobj, new);
    if (typeof(result) == ERR)
      player:notify(tostr(result));
    else
      player:notify(tostr(dobj.name, "'s quota set to ", new, "."));
    endif
    $mail_agent:send_message(player, recipients, tostr("@quota ", dobj.name, " (", dobj, ") ", new, " (from ", old, ")"), tostr("Reason for quota ", new - old < 0 ? "decrease: " | "increase: ", reason, index("?.!", reason[$]) ? "" | "."));
  endverb

  verb "@players" (any any any) owner: #2 flags: "rd"
    set_task_perms(player);
    "The time below is Oct. 1, 1990, roughly the birthdate of the LambdaMOO server.";
    start = 654768000;
    now = time();
    day = 24 * 60 * 60;
    week = 7 * day;
    month = 30 * day;
    days_objects = days_players = {0, 0, 0, 0, 0, 0, 0};
    weeks_objects = weeks_players = {0, 0, 0, 0};
    months_objects = months_players = {};
    nonplayer_objects = invalid_objects = 0;
    always_objects = always_players = 0;
    never_objects = never_players = 0;
    numo = 0;
    if (argstr)
      if (!dobjstr && prepstr == "with" && index("objects", iobjstr) == 1)
        with_objects = 1;
      else
        player:notify(tostr("Usage:  ", verb, " [with objects]"));
        return;
      endif
    else
      with_objects = 0;
      players = players();
    endif
    for i in [1..with_objects ? toint(max_object()) + 1 | length(players)]
      if (with_objects)
        o = toobj(i - 1);
      else
        o = players[i];
      endif
      if ($command_utils:running_out_of_time())
        player:notify(tostr("... ", o));
        suspend(0);
      endif
      if (valid(o))
        numo = numo + 1;
        p = is_player(o) ? o | o.owner;
        if (!valid(p))
          invalid_objects = invalid_objects + 1;
        elseif (!$object_utils:isa(p, $player))
          nonplayer_objects = nonplayer_objects + 1;
        else
          seconds = now - p.last_connect_time;
          days = seconds / day;
          weeks = seconds / week;
          months = seconds / month;
          if (seconds < 0)
            if (is_player(o))
              always_players = always_players + 1;
            else
              always_objects = always_objects + 1;
            endif
          elseif (seconds > now - start)
            if (is_player(o))
              never_players = never_players + 1;
            else
              never_objects = never_objects + 1;
            endif
          elseif (months > 0)
            while (months > length(months_players))
              months_players = {@months_players, 0};
              months_objects = {@months_objects, 0};
            endwhile
            if (is_player(o))
              months_players[months] = months_players[months] + 1;
            endif
            months_objects[months] = months_objects[months] + 1;
          elseif (weeks > 0)
            if (is_player(o))
              weeks_players[weeks] = weeks_players[weeks] + 1;
            endif
            weeks_objects[weeks] = weeks_objects[weeks] + 1;
          else
            if (is_player(o))
              days_players[days + 1] = days_players[days + 1] + 1;
            endif
            days_objects[days + 1] = days_objects[days + 1] + 1;
          endif
        endif
      endif
    endfor
    player:notify("");
    player:notify(tostr("Last connected"));
    player:notify(tostr("at least this     Num.     Cumul.   Cumul. %", with_objects ? "     Num.     Cumul.   Cumul. %" | ""));
    player:notify(tostr("long ago        players   players   players ", with_objects ? "   objects   objects   objects" | ""));
    player:notify(tostr("---------------------------------------------", with_objects ? "--------------------------------" | ""));
    su = $string_utils;
    col1 = 14;
    col2 = 7;
    col3 = 10;
    col4 = 9;
    col5 = 11;
    col6 = 11;
    col7 = 10;
    nump = length(players());
    totalp = totalo = 0;
    for x in ({{days_players, days_objects, "day", 1}, {weeks_players, weeks_objects, "week", 0}, {months_players, months_objects, "month", 0}})
      pcounts = x[1];
      ocounts = x[2];
      unit = x[3];
      offset = x[4];
      for i in [1..length(pcounts)]
        $command_utils:suspend_if_needed(0);
        j = i - offset;
        player:notify(tostr(su:left(tostr(j, " ", unit, j == 1 ? ":" | "s:"), col1), su:right(pcounts[i], col2), su:right(totalp = totalp + pcounts[i], col3), su:right(totalp * 100 / nump, col4), "%", with_objects ? tostr(su:right(ocounts[i], col5), su:right(totalo = totalo + ocounts[i], col6), su:right(totalo * 100 / numo, col7), "%") | ""));
      endfor
      player:notify("");
    endfor
    player:notify(tostr(su:left("Never:", col1), su:right(never_players, col2), su:right(totalp = totalp + never_players, col3), su:right(totalp * 100 / nump, col4), "%", with_objects ? tostr(su:right(never_objects, col5), su:right(totalo = totalo + never_objects, col6), su:right(totalo * 100 / numo, col7), "%") | ""));
    player:notify(tostr(su:left("Always:", col1), su:right(always_players, col2), su:right(totalp = totalp + always_players, col3), su:right(totalp * 100 / nump, col4), "%", with_objects ? tostr(su:right(always_objects, col5), su:right(totalo = totalo + always_objects, col6), su:right(totalo * 100 / numo, col7), "%") | ""));
    with_objects && player:notify(tostr(su:left("Non-player owner:", col1 + col2 + col3 + col4 + 1), su:right(nonplayer_objects, col5), su:right(totalo = totalo + nonplayer_objects, col6), su:right(totalo * 100 / numo, col7), "%"));
    with_objects && player:notify(tostr(su:left("Invalid owner:", col1 + col2 + col3 + col4 + 1), su:right(invalid_objects, col5), su:right(totalo = totalo + invalid_objects, col6), su:right(totalo * 100 / numo, col7), "%"));
    player:notify("");
  endverb

  verb kill_aux_wizard_parse (this none this) owner: #2 flags: "rxd"
    "Auxiliary verb for parsing @kill soon [#-of-seconds] [player | everyone]";
    "Args[1] is either # of seconds or player/everyone.";
    "Args[2], if it exists, is player/everyone, and forces args[1] to have been # of seconds.";
    "Return value: {# of seconds [default 60] , 1 for all, object for player.}";
    set_task_perms(caller_perms());
    nargs = length(args);
    soon = toint(args[1]);
    if (nargs > 1)
      everyone = args[2];
    elseif (soon <= 0)
      everyone = args[1];
    else
      everyone = 0;
    endif
    if (everyone == "everyone")
      everyone = 1;
    elseif (typeof(everyone) == STR)
      result = $string_utils:match_player(everyone);
      if ($command_utils:player_match_failed(result, everyone))
        player:notify(tostr("Usage:  ", callers()[1][2], " soon [number of seconds] [\"everyone\" | player name]"));
        return {-1, -1};
      else
        return {soon ? soon | 60, result};
      endif
    endif
    return {soon ? soon | 60, everyone ? everyone | player};
  endverb

  verb "@grepcore @egrepcore" (any any any) owner: #2 flags: "rd"
    set_task_perms(player);
    if (!args)
      player:notify(tostr("Usage:  ", verb, " <pattern>"));
      return;
    endif
    pattern = argstr;
    regexp = verb == "@egrepcore";
    player:notify(tostr("Searching for core verbs ", regexp ? "matching the regular expression " | "containing the string ", toliteral(pattern), " ..."));
    player:notify("");
    $code_utils:((regexp ? "find_verbs_matching" | "find_verbs_containing"))(pattern, $core_objects());
  endverb

  verb "@net-who @@who" (any any any) owner: #2 flags: "rd"
    "@net-who prints all connected users and hosts.";
    "@net-who player player player prints specified users and current or most recent connected host.";
    "@net-who from hoststring prints all players who have connected from that host or host substring.  Substring can include *'s, e.g. @net-who from *.foo.edu.";
    set_task_perms(player);
    su = $string_utils;
    if (prepstr == "from" && dobjstr)
      player:notify(tostr("Usage:  ", verb, " from <host string>"));
    elseif (prepstr != "from" || dobjstr || !iobjstr)
      "Not parsing 'from' here...  Instead printing connected/recent users.";
      if (!(pstrs = args))
        unsorted = connected_players();
      else
        unsorted = listdelete($command_utils:player_match_result(su:match_player(pstrs), pstrs), 1);
      endif
      if (!unsorted)
        return;
      endif
      $wiz_utils:show_netwho_listing(player, unsorted);
    else
      $wiz_utils:show_netwho_from_listing(player, iobjstr);
    endif
  endverb

  verb "@make-player" (any any any) owner: #2 flags: "rd"
    "Creates a player.";
    "Syntax:  @make-player name email-address comments....";
    "Generates a random password for the player.";
    if (!player.wizard || callers())
      return E_PERM;
    elseif (length(args) < 2)
      player:tell("Syntax:  @make-player name email-address comments....");
      return;
    elseif (args[2] == "for")
      "common mistake: @make-player <name> for <email-address> ...";
      args = listdelete(args, 2);
    endif
    return $wiz_utils:do_make_player(@args);
  endverb

  verb "@abort-sh*utdown" (any any any) owner: #2 flags: "rd"
    if (!player.wizard)
      player:notify("Sorry.");
    elseif (!$code_utils:task_valid($shutdown_task))
      player:notify("No server shutdown in progress.");
      $shutdown_task = E_NONE;
    else
      "... Reset time so that $login:check_for_shutdown shuts up...";
      kill_task($shutdown_task);
      $shutdown_task = E_NONE;
      $shutdown_time = time() - 1;
      for p in (connected_players())
        notify(p, tostr("*** Server shutdown ABORTED by ", player.name, " (", player, ")", argstr && ":  " + argstr, " ***"));
      endfor
    endif
  endverb

  verb "toad_msg toad_victim_msg programmer_msg programmer_victim_msg newt_msg newt_victim_msg" (this none this) owner: #2 flags: "rxd"
    "This is the canonical doing-something-to-somebody message.";
    "The corresponding property can either be";
    "   string             msg for all occasions";
    "   list of 2 strings  {we-are-there-msg,we-are-elsewhere-msg}";
    m = this.(verb);
    if (typeof(m) != LIST)
      return $string_utils:pronoun_sub(m);
    elseif (this.location == dobj.location || length(m) < 2)
      return $string_utils:pronoun_sub(m[1]);
    else
      return $string_utils:pronoun_sub(m[2]);
    endif
  endverb

  verb moveto (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller in {this, $generic_editor, $verb_editor, $mail_editor, $note_editor} ? this.owner | caller_perms());
    return `move(this, args[1]) ! ANY';
  endverb

  verb "@newt" (any any any) owner: #2 flags: "rd"
    "@newt <player> [commentary]";
    "turns a player into a newt.  It can get better...";
    "adds player to $login.newted, they will not be allowed to log in.";
    "Sends mail to $newt_log giving .all_connect_places and commentary.";
    whostr = args[1];
    comment = $string_utils:first_word(argstr)[2];
    if (!player.wizard)
      player:notify("Yeah, right.");
    elseif ($command_utils:player_match_failed(who = $string_utils:match_player(whostr), whostr))
      return;
    elseif (whostr != who.name && !(whostr in who.aliases) && whostr != tostr(who))
      player:notify(tostr("Must be a full name or an object number:  ", who.name, "(", who, ")"));
      return;
    elseif (who == player)
      player:notify("If you want to newt yourself, you have to do it by hand.");
      return;
    elseif (who in $login.newted)
      player:notify(tostr(who.name, " appears to already be a newt."));
      return;
    else
      $wiz_utils:newt_player(who, comment);
    endif
  endverb

  verb "@unnewt @denewt @get-better" (any any any) owner: #2 flags: "rd"
    "@denewt <player> [commentary]";
    "Remove the player from $Login.newted";
    "Sends mail to $newt_log with commentary.";
    whostr = args[1];
    comment = $string_utils:first_word(argstr)[2];
    if (!player.wizard)
      player:notify("Yeah, right.");
    elseif ($command_utils:player_match_failed(who = $string_utils:match_player(whostr), whostr))
      return;
    else
      "Should parse email address and register user in some clever way.  Ick.";
      if (!(who in $login.newted))
        player:notify(tostr(who.name, " does not appear to be a newt."));
      else
        $login.newted = setremove($login.newted, who);
        if (entry = $list_utils:assoc(who, $login.temporary_newts))
          $login.temporary_newts = setremove($login.temporary_newts, entry);
        endif
        player:notify(tostr(who.name, " (", who, ") got better."));
        $mail_agent:send_message(player, $newt_log, tostr("@denewt ", who.name, " (", who, ")"), comment ? {comment} | {});
      endif
    endif
  endverb

  verb "@register" (any any any) owner: #2 flags: "rd"
    "Registers a player.";
    "Syntax:  @register name email-address [additional commentary]";
    "Email-address is stored in $registration_db and on the player object.";
    if (!player.wizard)
      return player:tell(E_PERM);
    endif
    $wiz_utils:do_register(@args);
  endverb

  verb "@new-password @newpassword" (any is any) owner: #2 flags: "rd"
    "@newpassword player is [string]";
    "Set's a player's password; omit string to have one randomly generated.";
    "Offer to email the password.";
    if (!player.wizard)
      return E_PERM;
    elseif ($command_utils:player_match_failed(dobj = $string_utils:match_player(dobjstr), dobjstr))
      return;
    elseif (!(dobjstr in {@dobj.aliases, tostr(dobj)}))
      player:notify(tostr("Must be a full name or an object number: ", dobj.name, " (", dobj, ")"));
    else
      $wiz_utils:do_new_password(dobj, iobjstr);
    endif
  endverb

  verb "@log" (any any any) owner: #2 flags: "rd"
    "@log [<string>]    enters a comment in the server log.";
    "If no string is given, you are prompted to enter one or more lines for an extended comment.";
    set_task_perms(player);
    whostr = tostr("from ", player.name, " (", player, ")");
    if (!player.wizard || player != caller)
      player:notify("Yeah, right.");
    elseif (argstr)
      server_log(tostr("COMMENT: [", whostr, "]  ", argstr));
      player:notify("One-line comment logged.");
    elseif (lines = $command_utils:read_lines())
      server_log(tostr("COMMENT: [", whostr, "]"));
      for l in (lines)
        server_log(l);
      endfor
      server_log(tostr("END_COMMENT."));
      player:notify(tostr(length(lines), " lines logged as extended comment."));
    endif
  endverb

  verb "@guests" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    n = dobjstr == "all" ? 0 | $code_utils:toint(dobjstr || "20");
    if (caller != this)
      player:notify("You lose.");
    elseif (n == E_TYPE && index("now", dobjstr) != 1)
      player:notify(tostr("Usage:  ", verb, " <number>  (where <number> indicates how many entries to look at in the guest log)"));
      player:notify(tostr("Usage:  ", verb, " now (to see information about currently connected guests only)"));
    elseif (!dobjstr || index("now", dobjstr) != 1)
      $guest_log:last(n);
    else
      "*way* too much copied code in here from @who...  Sorry.  --yduJ";
      su = $string_utils;
      conn = connected_players();
      unsorted = {};
      for g in ($object_utils:leaves($guest))
        if (g in conn)
          unsorted = {@unsorted, g};
        endif
      endfor
      if (!unsorted)
        player:tell("No guests found.");
        return;
      endif
      footnotes = {};
      alist = {};
      nwidth = length("Player name");
      for u in (unsorted)
        pref = u.programmer ? "% " | "  ";
        u.programmer && (footnotes = setadd(footnotes, "prog"));
        u3 = {tostr(pref, u.name, " (", u, ")"), su:from_seconds(connected_seconds(u)), su:from_seconds(idle_seconds(u)), where = $string_utils:connection_hostname(connection_name(u))};
        nwidth = max(length(u3[1]), nwidth);
        if ($login:blacklisted(where))
          where = "(*) " + where;
          footnotes = setadd(footnotes, "black");
        elseif ($login:graylisted(where))
          where = "(+) " + where;
          footnotes = setadd(footnotes, "gray");
        endif
        alist = {@alist, u3};
        $command_utils:suspend_if_needed(0);
      endfor
      alist = $list_utils:sort_alist_suspended(0, alist, 3);
      $command_utils:suspend_if_needed(0);
      headers = {"Player name", "Connected", "Idle Time", "From Where"};
      time_width = length("59 minutes") + 2;
      before = {0, w1 = nwidth + 3, w2 = w1 + time_width, w3 = w2 + time_width};
      tell1 = "  " + headers[1];
      tell2 = "  " + su:space(headers[1], "-");
      for j in [2..4]
        tell1 = su:left(tell1, before[j]) + headers[j];
        tell2 = su:left(tell2, before[j]) + su:space(headers[j], "-");
      endfor
      player:notify(tell1);
      player:notify(tell2);
      active = 0;
      for a in (alist)
        $command_utils:suspend_if_needed(0);
        tell1 = a[1];
        for j in [2..4]
          tell1 = su:left(tell1, before[j]) + tostr(a[j]);
        endfor
        player:notify(tell1[1..min($, 79)]);
      endfor
      if (footnotes)
        player:notify("");
        if ("prog" in footnotes)
          player:notify(" %  == programmer.");
        endif
        if ("black" in footnotes)
          player:notify("(*) == blacklisted site.");
        endif
        if ("gray" in footnotes)
          player:notify("(+) == graylisted site.");
        endif
      endif
      player:tell("@guests display complete.");
    endif
  endverb

  verb "@rn mail_catch_up check_mail_lists current_message set_current_message get_current_message make_current_message kill_current_message @nn" (none none none) owner: #2 flags: "rxd"
    if (caller != this)
      set_task_perms(valid(caller_perms()) ? caller_perms() | player);
    endif
    use = this.mail_identity;
    if (valid(use) && use != this)
      return use:(verb)(@args);
    else
      return pass(@args);
    endif
  endverb

  verb "@blacklist @graylist @redlist @unblacklist @ungraylist @unredlist @spooflist @unspooflist" (any any any) owner: #2 flags: "rd"
    "@[un]blacklist [<site or subnet>  [for <duration>] [commentary]]";
    "@[un]graylist  [<site or subnet>  [for <duration>] [commentary]]";
    "@[un]redlist   [<site or subnet>  [for <duration>] [commentary]]";
    "@[un]spooflist [<site of subnet>  [for <duration>] [commentary]]";
    "The `for <duration>' is for temporary colorlisting a site only. The duration should be in english time units:  for 1 hour, for 1 day 2 hours 15 minutes, etc. The commentary should be after all durations. Note, if you are -not- using a duration, do not start your commentary with the word `for'.";
    set_task_perms(player);
    if (player != this || !player.wizard)
      player:notify("Ummm.  no.");
      return;
    endif
    undo = verb[2..3] == "un";
    which = $login:listname(verb[undo ? 4 | 2]);
    downgrade = {"", "graylist", "blacklist"}[1 + index("br", which[1])];
    if (!(fw = $string_utils:first_word(argstr)))
      "... Just print the list...";
      this:display_list(which);
      return;
    endif
    target = fw[1];
    if (fw[2] && (parse = this:parse_templist_duration(fw[2]))[1])
      if (typeof(parse[3]) == ERR || !parse[3])
        player:notify(tostr("Could not parse the duration for @", which, "ing site \"", target, "\""));
        return;
      endif
      start = parse[2];
      duration = parse[3];
      comment = parse[4] ? {parse[4]} | {};
      comment = {tostr("for ", $time_utils:english_time(duration)), @comment};
    elseif (fw[2])
      comment = {fw[2]};
    else
      "Get the right vars set up as though parse had been called";
      parse = {0, ""};
      comment = {};
    endif
    player:tell("comment is currently ", toliteral(comment));
    if (is_literal = $site_db:domain_literal(target))
      if (target[$] == ".")
        target = target[1..$ - 1];
      endif
      fullname = "subnet " + target;
    else
      if (target[1] == ".")
        target[1..1] = "";
      endif
      fullname = "domain `" + target + "'";
    endif
    entrylist = $login.(which)[1 + !is_literal];
    if (!undo && target in entrylist)
      player:notify(tostr(fullname, " is already ", which, "ed."));
      return;
    endif
    entrylist = setremove(entrylist, target);
    if (!(result = this:check_site_entries(undo, which, target, is_literal, entrylist))[1])
      return;
    endif
    rm = result[2];
    namelist = $string_utils:english_list(rm);
    downgraded = {};
    if (rm)
      ntries = length(rm) == 1 ? "ntry" | "ntries";
      if ($command_utils:yes_or_no(tostr("Remove e", ntries, " for ", namelist, "?")))
        dg = undo && (downgrade && $command_utils:yes_or_no(downgrade + " them?"));
        for s in (rm)
          $login:((which + "_remove"))(s);
          dg && ($login:((downgrade + "_add"))(s) && (downgraded = {@downgraded, s}));
        endfor
        player:notify(tostr("E", ntries, " removed", @dg ? {" and ", downgrade, "ed."} | {"."}));
      else
        player:notify(tostr(namelist, " will continue to be ", which, "ed."));
        rm = {};
      endif
    endif
    if (downgraded)
      comment[1..0] = {tostr(downgrade, "ed ", $string_utils:english_list(downgraded), ".")};
    endif
    tempentrylist = $login.(("temporary_" + which))[1 + !is_literal];
    if (!undo && target in $list_utils:slice(tempentrylist))
      player:notify(tostr(fullname, " is already temporarily ", which, "ed."));
      return;
    endif
    if (en = $list_utils:assoc(target, tempentrylist))
      tempentrylist = setremove(tempentrylist, en);
    endif
    if (!(result = this:check_site_entries(undo, which, target, is_literal, $list_utils:slice(tempentrylist)))[1])
      return;
    endif
    rmtemp = result[2];
    tempnamelist = $string_utils:english_list(rmtemp);
    tempdowngraded = {};
    if (rmtemp)
      ntries = length(rmtemp) == 1 ? "ntry" | "ntries";
      if ($command_utils:yes_or_no(tostr("Remove e", ntries, " for ", tempnamelist, "?")))
        dg = undo && (downgrade && $command_utils:yes_or_no(downgrade + " them?"));
        for s in (rmtemp)
          old = $list_utils:assoc(s, tempentrylist);
          $login:((which + "_remove_temp"))(s);
          dg && ($login:((downgrade + "_add_temp"))(s, old[2], old[3]) && (tempdowngraded = {@tempdowngraded, s}));
        endfor
        player:notify(tostr("E", ntries, " removed", @dg ? {" and ", downgrade, "ed with durations transferred."} | {"."}));
      else
        player:notify(tostr(tempnamelist, " will continue to be temporarily ", which, "ed."));
        rmtemp = {};
      endif
    endif
    if (tempdowngraded)
      comment[1..0] = {tostr(downgrade, "ed ", $string_utils:english_list(tempdowngraded), ".")};
    endif
    if (!undo)
      if (parse[1])
        $login:((which + "_add_temp"))(target, start, duration);
        player:notify(tostr(fullname, " ", which, "ed for ", $time_utils:english_time(duration)));
      else
        $login:((which + "_add"))(target);
        player:notify(tostr(fullname, " ", which, "ed."));
      endif
      if (rm)
        comment[1..0] = {tostr("Subsumes ", which, "ing for ", namelist, ".")};
      endif
      if (rmtemp)
        comment[1..0] = {tostr("Subsumes temporary ", which, "ing for ", tempnamelist, ".")};
      endif
    elseif ($login:((which + "_remove"))(target))
      player:notify(tostr(fullname, " un", which, "ed."));
      if (!downgrade)
      elseif ($command_utils:yes_or_no(downgrade + " it?"))
        $login:((downgrade + "_add"))(target) && (downgraded = {target, @downgraded});
        player:notify(tostr(fullname, " ", downgrade, "ed."));
      else
        player:notify(tostr(fullname, " not ", downgrade, "ed."));
      endif
      if (downgraded)
        player:tell("Comment currently: ", toliteral(comment), " ; downgrade = ", toliteral(downgrade), " ; downgraded = ", toliteral(downgraded));
        comment[1..0] = {tostr(downgrade, "ed ", $string_utils:english_list(downgraded), ".")};
      endif
      if (rm)
        comment[1..0] = {tostr("Also removed ", namelist, ".")};
      endif
    elseif ((old = $list_utils:assoc(target, $login.(("temporary_" + which))[1 + !is_literal])) && $login:((which + "_remove_temp"))(target))
      player:notify(tostr(fullname, " un", which, "ed."));
      if (!downgrade)
      elseif ($command_utils:yes_or_no(downgrade + " it?"))
        $login:((downgrade + "_add_temp"))(target, old[2], old[3]) && (tempdowngraded = {target, @tempdowngraded});
        player:notify(tostr(fullname, " ", downgrade, "ed, currently for ", $time_utils:english_time(old[3]), " from ", $time_utils:time_sub("$1/$3", old[2])));
      else
        player:notify(tostr(fullname, " not ", downgrade, "ed."));
      endif
      if (tempdowngraded)
        comment[1..0] = {tostr(downgrade, "ed ", $string_utils:english_list(tempdowngraded), "with durations transferred.")};
      endif
      if (rmtemp)
        comment[1..0] = {tostr("Also removed ", tempnamelist, ".")};
      endif
    elseif (rm || rmtemp)
      player:notify(tostr(fullname, " itself was never actually ", which, "ed."));
      comment[1..0] = {tostr("Removed ", namelist, " from regular and ", tempnamelist, " from temporary.")};
    else
      player:notify(tostr(fullname, " was not ", which, "ed before."));
      return;
    endif
    subject = tostr(undo ? "@un" | "@", which, " ", fullname);
    $mail_agent:send_message(player, $site_log, subject, comment);
    "...";
    "... make sure we haven't screwed ourselves...";
    uhoh = {};
    for site in (player.all_connect_places)
      if (index(site, target) && $login:((which + "ed"))(site))
        uhoh = {@uhoh, site};
      endif
    endfor
    if (uhoh)
      player:notify(tostr("WARNING:  ", $string_utils:english_list(uhoh), " are now ", which, "ed!"));
    endif
  endverb

  verb "@corify" (any as any) owner: #2 flags: "rd"
    "Usage:  @corify <object> as <propname>";
    "Adds <object> to the core, as $<propname>";
    "Reminds the wizard to write an :init_for_core verb, if there isn't one already.";
    if (!player.wizard)
      player:tell("Sorry, the core is wizardly territory.");
      return;
    endif
    if (dobj == $failed_match)
      dobj = player:my_match_object(dobjstr);
    endif
    if ($command_utils:object_match_failed(dobj, dobjstr))
      return;
    endif
    if (!iobjstr)
      player:tell("Usage:  @corify <object> as <propname>");
      return;
    elseif (iobjstr[1] == "$")
      iobjstr = iobjstr[2..$];
    endif
    try
      add_property(#0, iobjstr, dobj, {player, "r"});
    except e (ANY)
      return player:tell(e[1], ":", e[2]);
    endtry
    if (!("init_for_core" in verbs(dobj)))
      player:tell(dobj:titlec(), " has no :init_for_core verb.  Strongly consider adding one before doing anything else.");
    else
      player:tell("Corified ", $string_utils:nn(dobj), " as $", iobjstr, ".");
    endif
  endverb

  verb "@make-guest" (any none none) owner: #2 flags: "rd"
    "Usage:  @make-guest <guestname>";
    "Creates a player called <guestname>_Guest owned by $hacker and a child of $guest. Or, if $local.guest exists, make a child of that, assuming that all other guests are children of it too.";
    if (!player.wizard)
      player:tell("If you think this MOO needs more guests, you should contact a wizard.");
      return E_PERM;
    endif
    if (length(args) != 1)
      player:tell("Usage: ", verb, " <guest name>");
      return;
    endif
    guest_parent = $object_utils:has_property($local, "guest") && valid($local.guest) && $object_utils:isa($local.guest, $guest) ? $local.guest | $guest;
    i = length(children(guest_parent));
    while (!$player_db:available(guestnum = tostr("Guest", i = i + 1)))
    endwhile
    guestname = args[1] + "_Guest";
    guestaliases = {guestname, adj = args[1], guestnum};
    if (!player.wizard)
      return;
    elseif ($player_db.frozen)
      player:tell("Sorry, the player db is frozen, so no players can be made right now.  Please try again in a few minutes.");
      return;
    elseif (!$player_db:available(guestname))
      player:tell("\"", guestname, "\" is not an available name.");
      return;
    elseif (!$player_db:available(adj))
      player:Tell("\"", adj, "\" is not an available name.");
      return;
    else
      new = $quota_utils:bi_create(guest_parent, $hacker);
      new:set_name(guestname);
      new:set_aliases(guestaliases);
      if (!(e = $wiz_utils:set_player(new, 1)))
        player:Tell("Unable to make ", new.name, " (", new, ") a player.");
        player:Tell(tostr(e));
      else
        player:Tell("Guest: ", new.name, " (", new, ") made.");
        new.default_description = {"By definition, guests appear nondescript."};
        new.description = new.default_description;
        new.last_connect_time = $maxint;
        new.last_disconnect_time = time();
        new.password = 0;
        new.size_quota = new.size_quota;
        new:set_gender(new.default_gender);
        move(new, $player_start);
        player:tell("Now don't forget to @describe ", new, " as something.");
      endif
    endif
  endverb

  verb "@temp-newt" (any for any) owner: #2 flags: "rd"
    if (!player.wizard)
      return player:tell("Permission denied.");
    elseif (!valid(who = $string_utils:match_player(dobjstr)))
      return $command_utils:player_match_failed(who, dobjstr);
    elseif (dobjstr != who.name && !(dobjstr in who.aliases) && dobjstr != tostr(who))
      return player:tell(tostr("Must be a full name or an object number:  ", who.name, "(", who, ")"));
    elseif (who == player)
      player:notify("If you want to newt yourself, you have to do it by hand.");
      return;
    elseif (!(howlong = $time_utils:parse_english_time_interval(iobjstr)))
      return player:tell("Can't parse time: ", howlong);
    else
      if (who in $login.newted)
        player:notify(tostr(who.name, " appears to already be a newt."));
      else
        $wiz_utils:newt_player(who, "", "For " + iobjstr + ".  ");
      endif
      if (index = $list_utils:iassoc(who, $login.temporary_newts))
        $login.temporary_newts[index][2] = time();
        $login.temporary_newts[index][3] = howlong;
      else
        $login.temporary_newts = {@$login.temporary_newts, {who, time(), howlong}};
      endif
      player:tell(who.name, " (", who, ") will be a newt until ", ctime(time() + howlong));
    endif
  endverb

  verb "@deprog*rammer" (any any any) owner: #2 flags: "rd"
    "@deprogrammer victim [for <duration>] [reason]";
    "";
    "Removes the prog-bit from victim.  If a duration is specified (see help $time_utils:parse_english_time_interval), then the victim is put into the temporary list. He will be automatically removed the first time he asks for a progbit after the duration expires.  Either with or without the duration you can specify a reason, or you will be prompted for one. However, if you don't have a duration, don't start the reason with the word `For'.";
    set_task_perms(player);
    if (player != this || !player.wizard)
      player:notify("No go.");
      return;
    endif
    if (!args)
      player:notify(tostr("Usage:  ", verb, " <playername> [for <duration>] [reason]"));
    endif
    fw = $string_utils:first_word(argstr);
    if (fw[2] && (parse = this:parse_templist_duration(fw[2]))[1])
      if (typeof(parse[3]) == ERR || !parse[3])
        player:notify(tostr("Could not parse the duration for restricting programming for ", fw[1], "."));
        return;
      endif
      start = parse[2];
      duration = parse[3];
      reason = parse[4] ? {parse[4]} | {};
    else
      start = duration = 0;
      reason = fw[2] ? {fw[2]} | {};
    endif
    if (!reason)
      reason = {$command_utils:read("reason for resetting programmer flag")};
    endif
    if (duration)
      reason = {tostr("for ", $time_utils:english_time(duration)), @reason};
    endif
    if ($command_utils:player_match_failed(victim = $string_utils:match_player(fw[1]), fw[1]))
      "...done...";
    elseif (result = $wiz_utils:unset_programmer(victim, reason, @start ? {start, duration} | {}))
      player:notify(tostr(victim.name, " (", victim, ") is no longer a programmer.", duration ? tostr("  This restriction will be lifted in ", $string_utils:from_seconds(duration), ".") | ""));
    elseif (result == E_NONE)
      player:notify(tostr(victim.name, " (", victim, ") was already a nonprogrammer..."));
    else
      player:notify(tostr(result));
    endif
  endverb

  verb display_list (this none this) owner: #2 flags: "rxd"
    if (caller != this && !caller_perms().wizard)
      return E_PERM;
    endif
    which = args[1];
    slist = {};
    if (s = $login.(which)[1])
      slist = {@slist, "--- Subnets ---", @s};
    endif
    if (s = $login.(which)[2])
      slist = {@slist, "--- Domains ---", @s};
    endif
    if (s = $login.(("temporary_" + which))[1])
      slist = {@slist, "--- Temporary Subnets ---"};
      for d in (s)
        slist = {@slist, tostr(d[1], " until ", $time_utils:time_sub("$1/$3 $H:$M", d[2] + d[3]))};
        $command_utils:suspend_if_needed(2);
      endfor
    endif
    if (s = $login.(("temporary_" + which))[2])
      slist = {@slist, "--- Temporary Domains ---"};
      for d in (s)
        slist = {@slist, tostr(d[1], " until ", $time_utils:time_sub("$1/$3 $H:$M", d[2] + d[3]))};
        $command_utils:suspend_if_needed(2);
      endfor
    endif
    if (slist)
      player:notify_lines($string_utils:columnize(slist, 2));
    else
      player:notify(tostr("The ", which, " is empty."));
    endif
  endverb

  verb parse_templist_duration (this none this) owner: HACKER flags: "rxd"
    "parses out the time interval at the beginning of the args[1], assumes rest is commentary.";
    if ((fw = $string_utils:first_word(args[1]))[1] == "for")
      words = $string_utils:words(fw[2]);
      try_ = {};
      ind = cont = 1;
      while (cont)
        word = words[ind];
        cont = ind;
        if (toint(word))
          try_ = {@try_, word};
          ind = ind + 1;
        else
          for set in ($time_utils.time_units)
            if (word in set)
              try_ = {@try_, word};
              ind = ind + 1;
            endif
          endfor
        endif
        if (cont == ind || ind > length(words))
          cont = 0;
        endif
      endwhile
      dur = $time_utils:parse_english_time_interval(@try_);
      rest = $string_utils:from_list(words[ind..$], " ");
      return {1, time(), dur, rest};
    else
      return {0, argstr};
    endif
  endverb

  verb check_site_entries (this none this) owner: #2 flags: "rxd"
    "Called by @[un]<color>list to check existence of the target site.";
    "=> {done okay, LIST of sites to remove}";
    if (caller != this)
      return E_PERM;
    endif
    {undo, which, target, is_literal, entrylist} = args;
    rm = {};
    confirm = 0;
    if (is_literal)
      for s in (entrylist)
        if ((i = index(s, target + ".")) == 1)
          "... target is a prefix of s, s should probably go...";
          rm = {@rm, s};
        elseif (index(target + ".", s + ".") != 1)
          "... s is not a prefix of target...";
        elseif (undo)
          player:notify(tostr("You will need to un", which, " subnet ", s, " as well."));
        elseif (confirm)
          player:notify(tostr("...Subnet ", s, " already ", which, "ed..."));
        else
          player:notify(tostr("Subnet ", s, " already ", which, "ed."));
          if (!(confirm = $command_utils:yes_or_no(tostr(which, " ", target, " anyway?"))))
            return {0, {}};
          endif
        endif
      endfor
    else
      for s in (entrylist)
        if ((i = rindex(s, "." + target)) && i == length(s) - length(target))
          "... target is a suffix of s, s should probably go...";
          rm = {@rm, s};
        elseif (!(i = rindex("." + target, "." + s)) || i < length(target) - length(s) + 1)
          "... s is not a suffix of target...";
        elseif (undo)
          player:notify(tostr("You will need to un", which, " domain `", s, "' as well."));
        elseif (confirm)
          player:notify(tostr("...Domain `", s, "' already ", which, "ed..."));
        else
          player:notify(tostr("Domain `", s, "' already ", which, "ed."));
          if (!(confirm = $command_utils:yes_or_no(tostr(which, " ", target, " anyway?"))))
            return {0, {}};
          endif
        endif
      endfor
    endif
    return {1, rm};
  endverb

  verb "@lock-login @unlock-login @lock-login!" (any any any) owner: #2 flags: "rd"
    "Syntax:  @lock-login <message>";
    "         @lock-login! <message>";
    "         @unlock-login";
    "";
    "The @lock-login calls prevent all non-wizard users from logging in, displaying <message> to them when they try.  (The second syntax, with @lock-login!, additionally boots any nonwizards who are already connected.)  @unlock-login turns this off.";
    if (caller != this)
      raise(E_PERM);
    elseif (verb[2] == "u")
      $no_connect_message = 0;
      player:notify("Login restrictions removed.");
    elseif (!argstr)
      player:notify("You must provide some message to display to users who attempt to login:  @lock-login <message>");
    else
      $no_connect_message = argstr;
      player:notify(tostr("Logins are now blocked for non-wizard players.  Message displayed when attempted:  ", $no_connect_message));
      if (verb == "@lock-login!")
        wizards = $wiz_utils:all_wizards_unadvertised();
        for x in (connected_players())
          if (!(x in wizards))
            boot_player(x);
          endif
        endfor
        player:notify("All nonwizards have been booted.");
      endif
    endif
  endverb

  verb __fix (this none this) owner: #2 flags: "rxd"
    "...was on $player, now archived here for posterity...";
    "Runs the old->new format conversion on every message in this.messages.";
    " => 1 if successful";
    " => 0 if anything toward happened during a suspension";
    "      (e.g., new message received, someone deleted stuff) ";
    "      in which case this.messages is left as if this routine were never run.";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    msgs = {};
    i = 1;
    for m in (oldmsgs = this.messages)
      msgs = {@msgs, {m[1], $mail_agent:__convert_new(@m[2])}};
      if ($command_utils:running_out_of_time())
        player:notify(tostr("...", i, " ", this));
        suspend(0);
        if (oldmsgs != this.messages)
          return 0;
        endif
      endif
      i = i + 1;
    endfor
    this.messages = msgs;
    return 1;
  endverb

  verb toad_cleanup (this none this) owner: #2 flags: "rxd"
    if (!player.wizard || caller != this)
      raise(E_PERM);
    endif
    "Noop. Placeholder verb for MOO-specific cleanups.";
  endverb
endobject