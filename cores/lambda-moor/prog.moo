object PROG
  name: "generic programmer"
  parent: BUILDER
  owner: BYTE_QUOTA_UTILS_WORKING
  fertile: true
  readable: true

  property eval_env (owner: HACKER, flags: "r") = "here=player.location;me=player";
  property eval_subs (owner: HACKER, flags: "r") = {};
  property eval_ticks (owner: HACKER, flags: "r") = 3;
  property prog_options (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};

  override aliases = {"generic", "programmer"};
  override description = "You see a player who is too experienced to have any excuse for not having a description.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override help = {PROG_HELP, BUILTIN_FUNCTION_HELP, VERB_HELP, CORE_HELP};
  override mail_notify (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc");
  override object_size = {59612, 1084848672};

  verb "@prop*erty" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!player.programmer)
      player:notify("You need to be a programmer to do this.");
      player:notify("If you want to become a programmer, talk to a wizard.");
      return;
    elseif (!$quota_utils:property_addition_permitted(player))
      player:tell("Property addition not permitted because quota exceeded.");
      return;
    endif
    nargs = length(args);
    usage = tostr("Usage:  ", verb, " <object>.<prop-name> [<init_value> [<perms> [<owner>]]]");
    if (nargs < 1 || !(spec = $code_utils:parse_propref(args[1])))
      player:notify(usage);
      return;
    endif
    object = player:my_match_object(spec[1]);
    name = spec[2];
    if ($command_utils:object_match_failed(object, spec[1]))
      return;
    endif
    if (nargs < 2)
      value = 0;
    else
      q = $string_utils:prefix_to_value(argstr[$string_utils:word_start(argstr)[2][1]..$]);
      if (q[1] == 0)
        player:notify(tostr("Syntax error in initial value:  ", q[2]));
        return;
      endif
      value = q[2];
      args = {args[1], value, @$string_utils:words(q[1])};
      nargs = length(args);
    endif
    default = player:prog_option("@prop_flags");
    if (!default)
      default = "rc";
    endif
    perms = nargs < 3 ? default | $perm_utils:apply(default, args[3]);
    if (nargs < 4)
      owner = player;
    else
      owner = $string_utils:match_player(args[4]);
      if ($command_utils:player_match_result(owner, args[4])[1])
        return;
      endif
    endif
    if (nargs > 4)
      player:notify(usage);
      return;
    endif
    try
      add_property(object, name, value, {owner, perms});
      player:notify(tostr("Property added with value ", toliteral(object.(name)), "."));
    except (E_INVARG)
      if ($object_utils:has_property(object, name))
        player:notify(tostr("Property ", object, ".", name, " already exists."));
      else
        for i in [1..length(perms)]
          if (!index("rcw", perms[i]))
            player:notify(tostr("Unknown permission bit:  ", perms[i]));
            return;
          endif
        endfor
        "...the only other possibility...";
        player:notify("Property is already defined on one or more descendents.");
        player:notify(tostr("Try @check-prop ", args[1]));
      endif
    except e (ANY)
      player:notify(e[2]);
    endtry
  endverb

  verb "@chmod*#" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    bynumber = verb == "@chmod#";
    if (length(args) != 2)
      player:notify(tostr("Usage:  ", verb, " <object-or-property-or-verb> <permissions>"));
      return;
    endif
    {what, perms} = args;
    if (spec = $code_utils:parse_verbref(what))
      if (!player.programmer)
        player:notify("You need to be a programmer to do this.");
        player:notify("If you want to become a programmer, talk to a wizard.");
        return;
      endif
      if (valid(object = player:my_match_object(spec[1])))
        vname = spec[2];
        if (bynumber)
          vname = $code_utils:toint(vname);
          if (vname == E_TYPE)
            return player:notify("Verb number expected.");
          elseif (vname < 1 || `vname > length(verbs(object)) ! E_PERM => 0')
            return player:notify("Verb number out of range.");
          endif
        endif
        try
          info = verb_info(object, vname);
          if (!valid(owner = info[1]))
            player:notify(tostr("That verb is owned by an invalid object (", owner, "); it needs to be @chowned."));
          elseif (!is_player(owner))
            player:notify(tostr("That verb is owned by a non-player object (", owner.name, ", ", owner, "); it needs to be @chowned."));
          else
            info[2] = perms = $perm_utils:apply(info[2], perms);
            try
              result = set_verb_info(object, vname, info);
              player:notify(tostr("Verb permissions set to \"", perms, "\"."));
            except (E_INVARG)
              player:notify(tostr("\"", perms, "\" is not a valid permissions string for a verb."));
            except e (ANY)
              player:notify(e[2]);
            endtry
          endif
        except (E_VERBNF)
          player:notify("That object does not define that verb.");
        except error (ANY)
          player:notify(error[2]);
        endtry
        return;
      endif
    elseif (bynumber)
      return player:notify("@chmod# can only be used for verbs.");
    elseif (index(what, ".") && (spec = $code_utils:parse_propref(what)))
      if (valid(object = player:my_match_object(spec[1])))
        pname = spec[2];
        try
          info = property_info(object, pname);
          info[2] = perms = $perm_utils:apply(info[2], perms);
          try
            result = set_property_info(object, pname, info);
            player:notify(tostr("Property permissions set to \"", perms, "\"."));
          except (E_INVARG)
            player:notify(tostr("\"", perms, "\" is not a valid permissions string for a property."));
          except error (ANY)
            player:notify(error[2]);
          endtry
        except (E_PROPNF)
          player:notify("That object does not have that property.");
        except error (ANY)
          player:notify(error[2]);
        endtry
        return;
      endif
    elseif (valid(object = player:my_match_object(what)))
      perms = $perm_utils:apply((object.r ? "r" | "") + (object.w ? "w" | "") + (object.f ? "f" | ""), perms);
      r = w = f = 0;
      for i in [1..length(perms)]
        if (perms[i] == "r")
          r = 1;
        elseif (perms[i] == "w")
          w = 1;
        elseif (perms[i] == "f")
          f = 1;
        else
          player:notify(tostr("\"", perms, "\" is not a valid permissions string for an object."));
          return;
        endif
      endfor
      try
        object.r = r;
        object.w = w;
        object.f = f;
        player:notify(tostr("Object permissions set to \"", perms, "\"."));
      except (E_PERM)
        player:notify("Permission denied.");
      endtry
      return;
    endif
    $command_utils:object_match_failed(object, what);
  endverb

  verb "@args*#" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (player != caller)
      return;
    endif
    set_task_perms(player);
    if (!player.programmer)
      player:notify("You need to be a programmer to do this.");
      player:notify("If you want to become a programmer, talk to a wizard.");
      return;
    endif
    if (!(args && (spec = $code_utils:parse_verbref(args[1]))))
      player:notify(tostr(args ? "\"" + args[1] + "\"?  " | "", "<object>:<verb>  expected."));
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      "...can't find object...";
    else
      if (verb == "@args#")
        name = $code_utils:toint(spec[2]);
        if (name == E_TYPE)
          return player:notify("Verb number expected.");
        elseif (name < 1 || `name > length(verbs(object)) ! E_PERM => 0')
          return player:notify("Verb number out of range.");
        endif
      else
        name = spec[2];
      endif
      try
        info = verb_args(object, name);
        if (typeof(pas = $code_utils:parse_argspec(@listdelete(args, 1))) != LIST)
          "...arg spec is bogus...";
          player:notify(tostr(pas));
        elseif (!(newargs = pas[1]))
          player:notify($string_utils:from_list(info, " "));
        elseif (pas[2])
          player:notify(tostr("\"", pas[2][1], "\" unexpected."));
        else
          info[2] = (info[2])[1..index(info[2] + "/", "/") - 1];
          info = {@newargs, @info[length(newargs) + 1..$]};
          try
            result = set_verb_args(object, name, info);
            player:notify("Verb arguments changed.");
          except (E_INVARG)
            player:notify(tostr("\"", info[2], "\" is not a valid preposition (?)"));
          except error (ANY)
            player:notify(error[2]);
          endtry
        endif
      except (E_VERBNF)
        player:notify("That object does not have a verb with that name.");
      except error (ANY)
        player:notify(error[2]);
      endtry
    endif
  endverb

  verb "eval*-d" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "A MOO-code evaluator.  Type `;CODE' or `eval CODE'.";
    "Calls player:eval_cmd_string to first transform CODE in any way appropriate (e.g., prefixing .eval_env) and then do the actual evaluation.  See documentation for this:eval_cmd_string";
    "If you set your .eval_time property to 1, you find out how many ticks and seconds you used.";
    "If eval-d is used, the evaluation is performed as if the debug flag were unset.";
    if (player != this)
      player:tell("I don't understand that.");
      return;
    elseif (!player.programmer)
      player:tell("You need to be a programmer to eval code.");
      return;
    endif
    set_task_perms(player);
    result = player:eval_cmd_string(argstr, verb != "eval-d");
    if (result[1])
      player:notify(this:eval_value_to_string(result[2]));
      if (player:prog_option("eval_time") && !`output_delimiters(player)[2] ! ANY')
        player:notify(tostr("[used ", result[3], " tick", result[3] != 1 ? "s, " | ", ", result[4], " second", result[4] != 1 ? "s" | "", ".]"));
      endif
    else
      player:notify_lines(result[2]);
      nerrors = length(result[2]);
      player:notify(tostr(nerrors, " error", nerrors == 1 ? "." | "s."));
    endif
  endverb

  verb "@rmprop*erty" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (length(args) != 1 || !(spec = $code_utils:parse_propref(args[1])))
      player:notify(tostr("Usage:  ", verb, " <object>.<property>"));
      return;
    endif
    object = player:my_match_object(spec[1]);
    pname = spec[2];
    if ($command_utils:object_match_failed(object, spec[1]))
      return;
    endif
    try
      result = delete_property(object, pname);
      player:notify("Property removed.");
    except (E_PROPNF)
      player:notify("That object does not define that property.");
    except res (ANY)
      player:notify(res[2]);
    endtry
  endverb

  verb "@verb" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!player.programmer)
      player:notify("You need to be a programmer to do this.");
      player:notify("If you want to become a programmer, talk to a wizard.");
      return;
    elseif (!$quota_utils:verb_addition_permitted(player))
      player:tell("Verb addition not permitted because quota exceeded.");
      return;
    endif
    if (!(args && (spec = $code_utils:parse_verbref(args[1]))))
      player:notify(tostr("Usage:  ", verb, " <object>:<verb-name(s)> [<dobj> [<prep> [<iobj> [<permissions> [<owner>]]]]]"));
      return;
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      return;
    endif
    name = spec[2];
    "...Adding another verb of the same name is often a mistake...";
    namelist = $string_utils:explode(name);
    for n in (namelist)
      if (i = index(n, "*"))
        n = n[1..i - 1] + n[i + 1..$];
      endif
      if ((hv = $object_utils:has_verb(object, n)) && hv[1] == object)
        player:notify(tostr("Warning:  Verb `", n, "' already defined on that object."));
      endif
    endfor
    if (typeof(pas = $code_utils:parse_argspec(@listdelete(args, 1))) != LIST)
      player:notify(tostr(pas));
      return;
    endif
    verbargs = pas[1] || (player:prog_option("verb_args") || {});
    verbargs = {@verbargs, "none", "none", "none"}[1..3];
    rest = pas[2];
    if (verbargs == {"this", "none", "this"})
      perms = "rxd";
    else
      perms = "rd";
    endif
    if (rest)
      perms = $perm_utils:apply(perms, rest[1]);
    endif
    if (length(rest) < 2)
      owner = player;
    elseif (length(rest) > 2)
      player:notify(tostr("\"", rest[3], "\" unexpected."));
      return;
    elseif ($command_utils:player_match_result(owner = $string_utils:match_player(rest[2]), rest[2])[1])
      return;
    elseif (owner == $nothing)
      player:notify("Verb can't be owned by no one!");
      return;
    endif
    try
      x = add_verb(object, {owner, perms, name}, verbargs);
      player:notify(tostr("Verb added (", x > 0 ? x | length($object_utils:accessible_verbs(object)), ")."));
    except (E_INVARG)
      player:notify(tostr(rest ? tostr("\"", perms, "\" is not a valid set of permissions.") | tostr("\"", verbargs[2], "\" is not a valid preposition (?)")));
    except e (ANY)
      player:notify(e[2]);
    endtry
  endverb

  verb "@rmverb*#" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!(args && (spec = $code_utils:parse_verbref(args[1]))))
      player:notify(tostr("Usage:  ", verb, " <object>:<verb>"));
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      "...bogus object...";
    elseif (typeof(argspec = $code_utils:parse_argspec(@listdelete(args, 1))) != LIST)
      player:notify(tostr(argspec));
    elseif (argspec[2])
      player:notify($string_utils:from_list(argspec[2], " ") + "??");
    elseif (length(argspec = argspec[1]) in {1, 2})
      player:notify({"Missing preposition", "Missing iobj specification"}[length(argspec)]);
    else
      verbname = spec[2];
      if (verb == "@rmverb#")
        loc = $code_utils:toint(verbname);
        if (loc == E_TYPE)
          return player:notify("Verb number expected.");
        elseif (loc < 1 || loc > `length(verbs(object)) ! E_PERM => 0')
          return player:notify("Verb number out of range.");
        endif
      else
        if (index(verbname, "*") > 1)
          verbname = strsub(verbname, "*", "");
        endif
        loc = $code_utils:find_last_verb_named(object, verbname);
        if (argspec)
          argspec[2] = $code_utils:full_prep(argspec[2]) || argspec[2];
          while (loc != -1 && `verb_args(object, loc) ! ANY' != argspec)
            loc = $code_utils:find_last_verb_named(object, verbname, loc - 1);
          endwhile
        endif
        if (loc < 0)
          player:notify(tostr("That object does not define that verb", argspec ? " with those args." | "."));
          return;
        endif
      endif
      info = `verb_info(object, loc) ! ANY';
      vargs = `verb_args(object, loc) ! ANY';
      vcode = `verb_code(object, loc, 1, 1) ! ANY';
      try
        delete_verb(object, loc);
        if (info)
          player:notify(tostr("Verb ", object, ":", info[3], " (", loc, ") {", $string_utils:from_list(vargs, " "), "} removed."));
          if (player:prog_option("rmverb_mail_backup"))
            $mail_agent:send_message(player, player, tostr(object, ":", info[3], " (", loc, ") {", $string_utils:from_list(vargs, " "), "}"), vcode);
          endif
        else
          player:notify(tostr("Unreadable verb ", object, ":", loc, " removed."));
        endif
      except e (ANY)
        player:notify(e[2]);
      endtry
    endif
  endverb

  verb "@forked*-verbose" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Syntax:  @forked [player]";
    "         @forked all wizards";
    "";
    "For a normal player, shows all the tasks you have waiting in your queue, especially those forked or suspended. A wizard will see all the tasks of all the players unless the optional argument is provided. @forked-v*erbose will show the full callers() stack for each task that has suspended (not a fresh fork).";
    "The second form is only usable by wizards and provides an output of all tasks owned by characters who are .wizard=1. Useful to find a task that may get put in a random queue due to $wiz_utils:random_wizard. Or even finding verbs that run with wizard permissions that shouldn't be.";
    set_task_perms(player);
    verbose = $code_utils:verbname_match("@forked-v*erbose", verb);
    if (!dobjstr)
      tasks = queued_tasks();
    elseif (dobjstr == "all wizards" && player.wizard)
      tasks = {};
      for t in (queued_tasks())
        if (valid(t[5]) && (t[5]).wizard)
          tasks = {@tasks, t};
        endif
        $command_utils:suspend_if_needed(1);
      endfor
    elseif ($command_utils:player_match_result(dobj = $string_utils:match_player(dobjstr), dobjstr)[1])
      return;
    elseif (typeof(tasks = $wiz_utils:queued_tasks(dobj)) != LIST)
      player:notify(tostr(verb, " ", dobj.name, "(", dobj, "):  ", tasks));
      return;
    endif
    if (tasks)
      su = $string_utils;
      player:notify("Queue ID    Start Time            Owner         {Size} Verb (Line) [This]");
      player:notify("--------    ----------            -----         -----------------");
      now = time();
      for task in (tasks)
        $command_utils:suspend_if_needed(0);
        {q_id, start, nu, nu2, owner, vloc, vname, lineno, this, ?size = 0} = task;
        time = start >= now ? ctime(start)[5..24] | su:left(start == -1 ? "Reading input ..." | tostr(now - start, " seconds ago..."), 20);
        owner_name = valid(owner) ? owner.name | tostr("Dead ", owner);
        player:notify(tostr(su:left(tostr(q_id), 10), "  ", time, "  ", su:left(owner_name, 12), "  {", $building_utils:size_string(size), "} ", vloc, ":", vname, " (", lineno, ")", this != vloc ? tostr(" [", this, "]") | ""));
        if (verbose || index(vname, "suspend") && vloc == $command_utils)
          "Display the first (or, if verbose, every) line of the callers() list, which is gotten by taking the second through last elements of task_stack().";
          stack = `task_stack(q_id, 1) ! E_INVARG => {}';
          for frame in (stack[2..verbose ? $ | 2])
            {sthis, svname, sprogger, svloc, splayer, slineno} = frame;
            player:notify(tostr("                    Called By...  ", su:left(valid(sprogger) ? sprogger.name | tostr("Dead ", sprogger), 19), "  ", svloc, ":", svname, sthis != svloc ? tostr(" [", sthis, "]") | "", " (", slineno, ")"));
          endfor
        endif
      endfor
      player:notify("-----------------------------------------------------------------");
    else
      player:notify("No tasks.");
    endif
  endverb

  verb "@kill @killq*uiet" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Kills one or more tasks.";
    "Arguments:";
    "   object:verb -- kills all tasks which were started from that object and verb.";
    "   all -- kills all tasks owned by invoker";
    "   all player-name -- wizard variant:  kills all tasks owned by player.";
    "   all everyone -- wizard variant:  really kills all tasks.";
    "   Integer taskid -- kills the specifically named task.";
    "   soon [integer] -- kills all tasks scheduled to run in the next [integer] seconds, which defaults to 60.";
    "   %integer -- kills all tasks which end in the digits contained in integer.";
    "   The @killquiet alias kills tasks without the pretty printout if more than one task is being killed.";
    set_task_perms(player);
    quiet = index(verb, "q");
    if (length(args) == 0)
      player:notify_lines({tostr("Usage:  ", verb, " [object]:[verb]"), tostr("        ", verb, " task_id"), tostr("        ", verb, " soon [number-of-seconds]", player.wizard ? " [everyone|<player name>]" | ""), tostr("        ", verb, " all", player.wizard ? " [everyone|<player name>]" | "")});
      return;
    elseif (taskid = toint(args[1]))
    elseif (all = args[1] == "all")
      everyone = 0;
      realplayer = player;
      if (player.wizard && length(args) > 1)
        realplayer = $string_utils:match_player(args[2]);
        everyone = args[2] == "everyone";
        if (!valid(realplayer) && !everyone)
          $command_utils:player_match_result(realplayer, args[2]);
          return;
        elseif (!everyone)
          set_task_perms(realplayer);
        endif
      endif
    elseif (soon = args[1] == "soon")
      realplayer = player;
      if (length(args) > 1)
        soon = toint(args[2]);
        if (soon <= 0 && !player.wizard)
          player:notify(tostr("Usage:  ", verb, " soon [positive-number-of-seconds]"));
          return;
        elseif (player.wizard)
          result = this:kill_aux_wizard_parse(@args[2..$]);
          soon = result[1];
          if (result[1] < 0)
            "already gave them an error message";
            return;
          elseif (result[2] == 1)
            everyone = 1;
          else
            everyone = 0;
            set_task_perms(result[2]);
            realplayer = result[2];
          endif
        endif
      else
        soon = 60;
        everyone = 0;
      endif
    elseif (percent = args[1][1] == "%")
      l = length(args[1]);
      digits = toint((args[1])[2..l]);
      percent = toint("1" + "0000000000"[1..l - 1]);
    elseif (colon = index(argstr, ":"))
      whatstr = argstr[1..colon - 1];
      vrb = argstr[colon + 1..$];
      if (whatstr)
        what = player:my_match_object(whatstr);
      endif
    else
      player:notify_lines({tostr("Usage:  ", verb, " [object]:[verb]"), tostr("        ", verb, " task_id"), tostr("        ", verb, " soon [number-of-seconds]", player.wizard ? " [everyone|<player name>]" | ""), tostr("        ", verb, " all", player.wizard ? " [\"everyone\"|<player name>]" | "")});
      return;
    endif
    "OK, parsed the line, and punted them if it was bogus.  This verb could have been a bit shorter at the expense of readability.  I think it's getting towards unreadable as is.  At this point we've set_task_perms'd, and set up an enormous number of local variables.  Evaluate them in the order we set them, and we should never get var not found.";
    queued_tasks = queued_tasks();
    killed = 0;
    if (taskid)
      try
        kill_task(taskid);
        player:notify(tostr("Killed task ", taskid, "."));
        killed = 1;
      except error (ANY)
        player:notify(tostr("Can't kill task ", taskid, ": ", error[2]));
      endtry
    elseif (all)
      for task in (queued_tasks)
        if (everyone || realplayer == task[5])
          `kill_task(task[1]) ! ANY';
          killed = killed + 1;
          if (!quiet)
            this:_kill_task_message(task);
          endif
        endif
        $command_utils:suspend_if_needed(3, "... killing tasks");
      endfor
    elseif (soon)
      now = time();
      for task in (queued_tasks)
        if (task[2] - now < soon && (!player.wizard || (everyone || realplayer == task[5])))
          `kill_task(task[1]) ! ANY';
          killed = killed + 1;
          if (!quiet)
            this:_kill_task_message(task);
          endif
        endif
        $command_utils:suspend_if_needed(3, "... killing tasks");
      endfor
    elseif (percent)
      for task in (queued_tasks)
        if (digits == task[1] % percent)
          `kill_task(task[1]) ! ANY';
          killed = killed + 1;
          if (!quiet)
            this:_kill_task_message(task);
          endif
        endif
        $command_utils:suspend_if_needed(3, "... killing tasks");
      endfor
    elseif (colon || vrb || whatstr)
      for task in (queued_tasks)
        if ((whatstr == "" || valid(task[6]) && index((task[6]).name, whatstr) == 1 || valid(task[9]) && index((task[9]).name, whatstr) == 1 || task[9] == what || task[6] == what) && (vrb == "" || index(" " + strsub(task[7], "*", ""), " " + vrb) == 1))
          `kill_task(task[1]) ! ANY';
          killed = killed + 1;
          if (!quiet)
            this:_kill_task_message(task);
          endif
        endif
        $command_utils:suspend_if_needed(3, "... killing tasks");
      endfor
    else
      player:notify("Something is funny; I didn't understand your @kill command.  You shouldn't have gotten here.  Please send yduJ mail saying you got this message from @kill, and what you had typed to @kill.");
    endif
    if (!killed)
      player:notify("No tasks killed.");
    elseif (quiet)
      player:notify(tostr("Killed ", killed, " tasks."));
    endif
  endverb

  verb "@copy @copy-x @copy-move" (any at any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Usage:  @copy source:verbname to target[:verbname]";
    "  the target verbname, if not given, defaults to that of the source.  If the target verb doesn't already exist, a new verb is installed with the same args, names, code, and permission flags as the source.  Otherwise, the existing target's verb code is overwritten and no other changes are made.";
    "This the poor man's version of multiple inheritance... the main problem is that someone may update the verb you're copying and you'd never know.";
    "  if @copy-x is used, makes an unusable copy (!x, this none this).  If @copy-move is used, deletes the source verb as well.";
    set_task_perms(player);
    if (!player.programmer)
      player:notify("You need to be a programmer to do this.");
      player:notify("If you want to become a programmer, talk to a wizard.");
      return;
    elseif (verb != "@copy-move" && !$quota_utils:verb_addition_permitted(player))
      player:notify("Verb addition not permitted because quota exceeded.");
      return;
    elseif (!(from = $code_utils:parse_verbref(dobjstr)) || !iobjstr)
      player:notify(tostr("Usage:  ", verb, " obj:verb to obj:verb"));
      player:notify(tostr("        ", verb, " obj:verb to obj"));
      player:notify(tostr("        ", verb, " obj:verb to :verb"));
      return;
    elseif ($command_utils:object_match_failed(fobj = player:my_match_object(from[1]), from[1]))
      return;
    elseif (iobjstr[1] == ":")
      to = {fobj, iobjstr[2..$]};
    elseif (!(to = $code_utils:parse_verbref(iobjstr)))
      iobj = player:my_match_object(iobjstr);
      if ($command_utils:object_match_failed(iobj, iobjstr))
        return;
      endif
      to = {iobj, from[2]};
    elseif ($command_utils:object_match_failed(tobj = player:my_match_object(to[1]), to[1]))
      return;
    else
      to[1] = tobj;
    endif
    from[1] = fobj;
    if (verb == "@copy-move")
      if (!$perm_utils:controls(player, fobj) && !$quota_utils:verb_addition_permitted(player))
        player:notify("Won't be able to delete old verb.  Quota exceeded, so unable to continue.  Aborted.");
        return;
      elseif ($perm_utils:controls(player, fobj))
        "only try to move if the player controls the verb. Otherwise, skip and treat as regular @copy";
        if (typeof(result = $code_utils:move_verb(@from, @to)) == ERR)
          player:notify(tostr("Unable to move verb from ", from[1], ":", from[2], " to ", to[1], ":", to[2], " --> ", result));
        else
          player:notify(tostr("Moved verb from ", from[1], ":", from[2], " to ", result[1], ":", result[2]));
        endif
        return;
      else
        player:notify("Won't be able to delete old verb.  Treating this as regular @copy.");
      endif
    endif
    to_firstname = strsub((to[2])[1..index(to[2] + " ", " ") - 1], "*", "") || "*";
    if (!(hv = $object_utils:has_verb(to[1], to_firstname)) || hv[1] != to[1])
      if (!(info = `verb_info(@from) ! ANY') || !(vargs = `verb_args(@from) ! ANY'))
        player:notify(tostr("Retrieving ", from[1], ":", from[2], " --> ", info && vargs));
        return;
      endif
      if (!player.wizard)
        info[1] = player;
      endif
      if (verb == "@copy-x")
        "... make sure this is an unusable copy...";
        info[2] = strsub(info[2], "x", "");
        vargs = {"this", "none", "this"};
      endif
      if (from[2] != to[2])
        info[3] = to[2];
      endif
      if (ERR == typeof(e = `add_verb(to[1], info, vargs) ! ANY'))
        player:notify(tostr("Adding ", to[1], ":", to[2], " --> ", e));
        return;
      endif
    endif
    code = `verb_code(@from) ! ANY';
    owner = `verb_info(@from)[1] ! ANY';
    if (typeof(code) == ERR)
      player:notify(tostr("Couldn't retrieve code from ", (from[1]).name, " (", from[1], "):", from[2], " => ", code));
      return;
    endif
    if (owner != player)
      comment = tostr("Copied from ", $string_utils:nn(from[1]), ":", from[2], from[1] == owner ? "" | tostr(" [verb author ", $string_utils:nn(owner), "]"), " at ", ctime());
      code = {$string_utils:print(comment) + ";", @code};
      if (!player:prog_option("copy_expert"))
        player:notify("Use of @copy is discouraged.  Please do not use @copy if you can use inheritance or features instead.  Use @copy carefully, and only when absolutely necessary, as it is wasteful of database space.");
      endif
    endif
    e = `set_verb_code(to[1], to_firstname, code) ! ANY';
    if (ERR == typeof(e))
      player:notify(tostr("Copying ", from[1], ":", from[2], " to ", to[1], ":", to[2], " --> ", e));
    elseif (typeof(e) == LIST && e)
      player:notify(tostr("Copying ", from[1], ":", from[2], " to ", to[1], ":", to[2], " -->"));
      player:notify_lines(e);
    else
      player:notify(tostr(to[1], ":", to[2], " code set."));
    endif
  endverb

  verb _kill_task_message (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    task = args[1];
    player:notify(tostr("Killed: ", $string_utils:right(tostr("task ", task[1]), 17), ", verb ", task[6], ":", task[7], ", line ", task[8], task[9] != task[6] ? ", this==" + tostr(task[9]) | ""));
  endverb

  verb "@prog*ram @program#" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "This version of @program deals with multiple verbs having the same name.";
    "... @program <object>:<verbname> <dobj> <prep> <iobj>  picks the right one.";
    if (player != caller)
      return;
    endif
    set_task_perms(player);
    "...";
    "...catch usage errors first...";
    "...";
    punt = "...set punt to 0 only if everything works out...";
    if (!(args && (spec = $code_utils:parse_verbref(args[1]))))
      player:notify(tostr("Usage: ", verb, " <object>:<verb> [<dobj> <prep> <iobj>]"));
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      "...bogus object...";
    elseif (typeof(argspec = $code_utils:parse_argspec(@listdelete(args, 1))) != LIST)
      player:notify(tostr(argspec));
    elseif (verb == "@program#")
      verbname = $code_utils:toint(spec[2]);
      if (verbname == E_TYPE)
        player:notify("Verb number expected.");
      elseif (length(args) > 1)
        player:notify("Don't give args for @program#.");
      elseif (verbname < 1 || `verbname > length(verbs(object)) ! E_PERM')
        player:notify("Verb number out of range.");
      else
        argspec = 0;
        punt = 0;
      endif
    elseif (argspec[2])
      player:notify($string_utils:from_list(argspec[2], " ") + "??");
    elseif (length(argspec = argspec[1]) in {1, 2})
      player:notify({"Missing preposition", "Missing iobj specification"}[length(argspec)]);
    else
      punt = 0;
      verbname = spec[2];
      if (index(verbname, "*") > 1)
        verbname = strsub(verbname, "*", "");
      endif
    endif
    "...";
    "...if we have an argspec, we'll need to reset verbname...";
    "...";
    if (punt)
    elseif (argspec)
      if (!(argspec[2] in {"none", "any"}))
        argspec[2] = $code_utils:full_prep(argspec[2]);
      endif
      loc = $code_utils:find_verb_named(object, verbname);
      while (loc > 0 && `verb_args(object, loc) ! ANY' != argspec)
        loc = $code_utils:find_verb_named(object, verbname, loc + 1);
      endwhile
      if (!loc)
        punt = "...can't find it....";
        player:notify("That object has no verb matching that name + args.");
      else
        verbname = loc;
      endif
    else
      loc = 0;
    endif
    "...";
    "...get verb info...";
    "...";
    if (punt || !(punt = "...reset punt to TRUE..."))
    else
      try
        info = verb_info(object, verbname);
        punt = 0;
        aliases = info[3];
        if (!loc)
          loc = aliases in (verbs(object) || {});
        endif
      except (E_VERBNF)
        player:notify("That object does not have that verb definition.");
      except error (ANY)
        player:notify(error[2]);
      endtry
    endif
    "...";
    "...read the code...";
    "...";
    if (punt)
      player:notify(tostr("Now ignoring code for ", args ? args[1] | "nothing in particular", "."));
      $command_utils:read_lines();
      player:notify("Verb code ignored.");
    else
      player:notify(tostr("Now programming ", object.name, ":", aliases, "(", !loc ? "??" | loc, ")."));
      lines = $command_utils:read_lines_escape((active = player in $verb_editor.active) ? {} | {"@edit"}, {tostr("You are editing ", $string_utils:nn(object), ":", verbname, "."), @active ? {} | {"Type `@edit' to take this into the verb editor."}});
      if (lines[1] == "@edit")
        $verb_editor:invoke(args[1], "@program", lines[2]);
        return;
      endif
      try
        if (result = set_verb_code(object, verbname, lines[2]))
          player:notify_lines(result);
          player:notify(tostr(length(result), " error(s)."));
          player:notify("Verb not programmed.");
        else
          player:notify("0 errors.");
          player:notify("Verb programmed.");
        endif
      except error (ANY)
        player:notify(error[2]);
        player:notify("Verb not programmed.");
      endtry
    endif
  endverb

  verb "@setenv" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Usage: @setenv <environment string>";
    "Set your .eval_env property.";
    set_task_perms(player);
    if (!argstr)
      player:notify(tostr("Usage:  ", verb, " <environment string>"));
      return;
    endif
    player:notify(tostr("Current eval environment is: ", player.eval_env));
    result = player:set_eval_env(argstr);
    if (typeof(result) == ERR)
      player:notify(tostr(result));
      return;
    endif
    player:notify(tostr(".eval_env set to \"", player.eval_env, "\" (", player.eval_ticks, " ticks)."));
  endverb

  verb "@pros*pectus pros*pectus" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Usage: @prospectus <player> [from <start>] [to <end>]";
    set_task_perms(caller_perms() == $nothing ? player | caller_perms());
    dobj = dobjstr ? $string_utils:match_player(dobjstr) | player;
    if ($command_utils:player_match_result(dobj, dobjstr)[1])
      return;
    endif
    dobjwords = $string_utils:words(dobjstr);
    if (args[1..length(dobjwords)] == dobjwords)
      args = args[length(dobjwords) + 1..$];
    endif
    if (!(parse_result = $code_utils:_parse_audit_args(@args)))
      player:notify(tostr("Usage:  ", verb, " player [from <start>] [to <end>]"));
      return;
    endif
    return $building_utils:do_prospectus(dobj, @parse_result);
  endverb

  verb "@d*isplay" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@display <object>[.[property]]*[,[inherited_property]]*[:[verb]]*[;[inherited_verb]]*";
    if (player != this)
      player:notify(tostr("Sorry, you can't use ", this:title(), "'s `", verb, "' command."));
      return E_PERM;
    endif
    "null names for properties and verbs are interpreted as meaning all of them.";
    opivu = {{}, {}, {}, {}, {}};
    string = "";
    punc = 1;
    literal = 0;
    set_task_perms(player);
    for jj in [1..length(argstr)]
      j = argstr[jj];
      if (literal)
        string = string + j;
        literal = 0;
      elseif (j == "\\")
        literal = 1;
      elseif (y = index(".,:;", j))
        opivu[punc] = {@opivu[punc], string};
        punc = 1 + y;
        string = "";
      else
        string = string + j;
      endif
    endfor
    opivu[punc] = {@opivu[punc], string};
    objname = opivu[1][1];
    it = this:my_match_object(objname);
    if ($command_utils:object_match_failed(it, objname))
      return;
    endif
    readable = it.owner == this || (it.r || this.wizard);
    cant = {};
    if ("" in opivu[2])
      if (readable)
        prop = properties(it);
      else
        prop = {};
        cant = setadd(cant, it);
      endif
      if (!this:display_option("thisonly"))
        what = it;
        while (!prop && valid(what = parent(what)))
          if (what.owner == this || (what.r || this.wizard))
            prop = properties(what);
          else
            cant = setadd(cant, what);
          endif
        endwhile
      endif
    else
      prop = opivu[2];
    endif
    if ("" in opivu[3])
      inh = {};
      for what in ({it, @$object_utils:ancestors(it)})
        if (what.owner == this || what.r || this.wizard)
          inh = {@inh, @properties(what)};
        else
          cant = setadd(cant, what);
        endif
      endfor
    else
      inh = opivu[3];
    endif
    for q in (inh)
      if (q in `properties(it) ! ANY => {}')
        prop = setadd(prop, q);
        inh = setremove(inh, q);
      endif
    endfor
    vrb = {};
    if ("" in opivu[4])
      if (readable)
        vrbs = verbs(it);
      else
        vrbs = $object_utils:accessible_verbs(it);
        cant = setadd(cant, it);
      endif
      what = it;
      if (!this:display_option("thisonly"))
        while (!vrbs && valid(what = parent(what)))
          if (what.owner == this || (what.r || this.wizard))
            vrbs = verbs(what);
          else
            cant = setadd(cant, what);
          endif
        endwhile
      endif
      for n in [1..length(vrbs)]
        vrb = setadd(vrb, {what, n});
      endfor
    else
      for w in (opivu[4])
        if (y = $object_utils:has_verb(it, w))
          vrb = setadd(vrb, {y[1], w});
        else
          this:notify(tostr("No such verb, \"", w, "\""));
        endif
      endfor
    endif
    if ("" in opivu[5])
      for z in ({it, @$object_utils:ancestors(it)})
        if (this == z.owner || z.r || this.wizard)
          for n in [1..length(verbs(z))]
            vrb = setadd(vrb, {z, n});
          endfor
        else
          cant = setadd(cant, z);
        endif
      endfor
    else
      for w in (opivu[5])
        if (typeof(y = $object_utils:has_verb(it, w)) == LIST)
          vrb = setadd(vrb, {y[1], w});
        else
          this:notify(tostr("No such verb, \"", w, "\""));
        endif
      endfor
    endif
    if ({""} in opivu || opivu[2..5] == {{}, {}, {}, {}})
      this:notify(tostr(it.name, " (", it, ") [ ", it.r ? "readable " | "", it.w ? "writeable " | "", it.f ? "fertile " | "", is_player(it) ? "(player) " | "", it.programmer ? "programmer " | "", it.wizard ? "wizard " | "", "]"));
      if (it.owner != (is_player(it) ? it | this))
        this:notify(tostr("  Owned by ", valid(p = it.owner) ? p.name | "** extinct **", " (", p, ")."));
      endif
      this:notify(tostr("  Child of ", valid(p = parent(it)) ? p.name | "** none **", " (", p, ")."));
      if (it.location != $nothing)
        this:notify(tostr("  Location ", valid(p = it.location) ? p.name | "** unplace (tell a wizard, fast!) **", " (", p, ")."));
      endif
      if ($quota_utils.byte_based && $object_utils:has_property(it, "object_size"))
        this:notify(tostr("  Size: ", $string_utils:group_number(it.object_size[1]), " bytes at ", this:ctime(it.object_size[2])));
      endif
    endif
    blankargs = this:display_option("blank_tnt") ? {"this", "none", "this"} | #-1;
    for b in (vrb)
      $command_utils:suspend_if_needed(0);
      where = b[1];
      q = b[2];
      short = typeof(q) == INT ? q | strsub(y = index(q, " ") ? q[1..y - 1] | q, "*", "");
      inf = `verb_info(where, short) ! ANY';
      if (typeof(inf) == LIST || inf == E_PERM)
        name = typeof(inf) == LIST ? index(inf[3], " ") ? "\"" + inf[3] + "\"" | inf[3] | q;
        line = $string_utils:left(tostr($string_utils:right(tostr(where), 6), ":", name, " "), 32);
        if (inf == E_PERM)
          line = line + "   ** unreadable **";
        else
          line = $string_utils:left(tostr(line, (inf[1]).name, " (", inf[1], ") "), 53) + ((i = inf[2] in {"x", "xd", "d", "rd"}) ? {" x", " xd", "  d", "r d"}[i] | inf[2]);
          vargs = `verb_args(where, short) ! ANY';
          if (vargs != blankargs)
            if (this:display_option("shortprep") && !(vargs[2] in {"any", "none"}))
              vargs[2] = $code_utils:short_prep(vargs[2]);
            endif
            line = $string_utils:left(line + " ", 60) + $string_utils:from_list(vargs, " ");
          endif
        endif
        this:notify(line);
      elseif (inf == E_VERBNF)
        this:notify(tostr(inf));
        this:notify(tostr("  ** no such verb, \"", short, "\" **"));
      else
        this:notify("This shouldn't ever happen. @display is buggy.");
      endif
    endfor
    all = {@prop, @inh};
    max = length(all) < 4 ? 999 | this:linelen() - 56;
    depth = length(all) < 4 ? -1 | 1;
    truncate_owner_names = length(all) > 1;
    for q in (all)
      $command_utils:suspend_if_needed(0);
      inf = `property_info(it, q) ! ANY';
      if (inf == E_PROPNF)
        if (q in $code_utils.builtin_props)
          this:notify(tostr($string_utils:left("," + q, 25), "Built in property            ", $string_utils:abbreviated_value(it.(q), max, depth)));
        else
          this:notify(tostr("  ** property not found, \"", q, "\" **"));
        endif
      else
        pname = $string_utils:left(tostr(q in `properties(it) ! ANY => {}' ? "." | (`is_clear_property(it, q) ! ANY' ? " " | ","), q, " "), 25);
        if (inf == E_PERM)
          this:notify(pname + "   ** unreadable **");
        else
          oname = (inf[1]).name;
          truncate_owner_names && (length(oname) > 12 && (oname = oname[1..12]));
          `inf[2][1] != "r" ! E_RANGE => 1' && ((inf[2])[1..0] = " ");
          `inf[2][2] != "w" ! E_RANGE => 1' && ((inf[2])[2..1] = " ");
          this:notify($string_utils:left(tostr($string_utils:left(tostr(pname, oname, " (", inf[1], ") "), 47), inf[2], " "), 54) + $string_utils:abbreviated_value(it.(q), max, depth));
        endif
      endif
    endfor
    if (cant)
      failed = {};
      for k in (cant)
        failed = listappend(failed, tostr(k.name, " (", k, ")"));
      endfor
      this:notify($string_utils:centre(tostr(" no permission to read ", $string_utils:english_list(failed, ", ", " or ", " or "), ". "), 75, "-"));
    else
      this:notify($string_utils:centre(" finished ", 75, "-"));
    endif
  endverb

  verb "@db*size" (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    "Let 'em @kill it.";
    count = 0;
    for i in [#0..max_object()]
      if ($command_utils:running_out_of_time())
        player:notify(tostr("Counting... [", count, "/", i, "]"));
        suspend(0);
      endif
      if (valid(i))
        count = count + 1;
      endif
    endfor
    player:notify(tostr("There are ", count, " valid objects out of ", toint(max_object()) + 1, " allocated object numbers."));
  endverb

  verb "@gethelp" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "@gethelp [<topic>] [from <db or dblist>]";
    "  Prints the raw text of topic from the appropriate help db.";
    "  With no argument, gets the blank (\"\") topic from wherever it lives";
    "  Text is printed as a script for changing this help topic ";
    "  (somewhat like @dump...)";
    if (!prepstr)
      topic = argstr;
      dblist = $code_utils:help_db_list();
    elseif (prepstr != "from")
      player:notify("Usage:  ", verb, " [<topic>] [from <db>]");
      return;
    elseif (!(e = $no_one:eval_d(iobjstr = argstr[$string_utils:word_start(argstr)[(prepstr in args) + 1][1]..$])))
      player:notify(tostr(e));
      return;
    elseif (!(e[1]))
      player:notify_lines(e[2]);
      return;
    elseif (!(typeof(dblist = e[2]) in {OBJ, LIST}))
      player:notify(tostr(iobjstr, " => ", dblist, " -- not an object or a list"));
      return;
    else
      topic = dobjstr;
      if (typeof(dblist) == OBJ)
        dblist = {dblist};
      endif
    endif
    search = $code_utils:help_db_search(topic, dblist);
    if (!search)
      player:notify("Topic not found.");
    elseif (search[1] == $ambiguous_match)
      player:notify(tostr("Topic `", topic, "' ambiguous:  ", $string_utils:english_list(search[2], "none", " or ")));
    elseif (typeof(text = (db = search[1]):dump_topic(fulltopic = search[2])) == ERR)
      "...ok...shoot me.  This is a -d verb...";
      player:notify(tostr("Cannot retrieve `", fulltopic, "' on ", $code_utils:corify_object(db), ":  ", text));
    else
      player:notify_lines(text);
    endif
  endverb

  verb "@grep*all @egrep*all" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (prepstr == "in")
      pattern = dobjstr;
      objlist = player:eval_cmd_string(iobjstr, 0);
      if (!(objlist[1]))
        player:notify(tostr("Had trouble reading `", iobjstr, "':  "));
        player:notify_lines(@objlist[2]);
        return;
      elseif (typeof(objlist[2]) == OBJ)
        objlist = {objlist[2..2]};
      elseif (typeof(objlist[2]) != LIST)
        player:notify(tostr("Value of `", iobjstr, "' is not an object or list:  ", toliteral(objlist[2])));
        return;
      else
        objlist = objlist[2..2];
      endif
    elseif (prepstr == "from" && (player.wizard && (n = toint(toobj(iobjstr)))))
      pattern = dobjstr;
      objlist = {n};
    elseif (args && player.wizard)
      pattern = argstr;
      objlist = {};
    else
      player:notify(tostr("Usage:  ", verb, " <pattern> ", player.wizard ? "[in {<objectlist>} | from <number>]" | "in {<objectlist>}"));
      return;
    endif
    player:notify(tostr("Searching for verbs ", @prepstr ? {prepstr, " ", iobjstr, " "} | {}, verb == "@egrep" ? "matching the pattern " | "containing the string ", toliteral(pattern), " ..."));
    player:notify("");
    egrep = verb[2] == "e";
    all = index(verb, "a");
    $code_utils:((all ? egrep ? "find_verb_lines_matching" | "find_verb_lines_containing" | (egrep ? "find_verbs_matching" | "find_verbs_containing")))(pattern, @objlist);
  endverb

  verb "@s*how" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (dobjstr == "")
      player:notify(tostr("Usage:  ", verb, " <object-or-property-or-verb>"));
      return;
    endif
    if (index(dobjstr, ".") && (spec = $code_utils:parse_propref(dobjstr)))
      if (valid(object = player:my_match_object(spec[1])))
        return $code_utils:show_property(object, spec[2]);
      endif
    elseif (spec = $code_utils:parse_verbref(dobjstr))
      if (valid(object = player:my_match_object(spec[1])))
        return $code_utils:show_verbdef(object, spec[2]);
      endif
    elseif (dobjstr[1] == "$" && (pname = dobjstr[2..$]) in properties(#0) && typeof(#0.(pname)) == OBJ)
      if (valid(object = #0.(pname)))
        return $code_utils:show_object(object);
      endif
    elseif (dobjstr[1] == "$" && (spec = $code_utils:parse_propref(dobjstr)))
      return $code_utils:show_property(#0, spec[2]);
    else
      if (valid(object = player:my_match_object(dobjstr)))
        return $code_utils:show_object(object);
      endif
    endif
    $command_utils:object_match_failed(object, dobjstr);
  endverb

  verb "@check-p*roperty" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@check-prop object.property";
    "  checks for descendents defining the given property.";
    set_task_perms(player);
    if (!(spec = $code_utils:parse_propref(dobjstr)))
      player:notify(tostr("Usage:  ", verb, " <object>.<prop-name>"));
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      "...bogus object...";
    elseif (!($perm_utils:controls(player, object) || object.w))
      player:notify("You can't create a property on that object anyway.");
    elseif ($object_utils:has_property(object, prop = spec[2]))
      player:notify("That object already has that property.");
    elseif (olist = $object_utils:descendants_with_property_suspended(object, prop))
      player:notify("The following descendents have this property defined:");
      player:notify("  " + $string_utils:from_list(olist, " "));
    else
      player:notify("No property name conflicts found.");
    endif
  endverb

  verb set_eval_env (this none this) owner: HACKER flags: "rxd"
    "set_eval_env(string);";
    "Run <string> through eval.  If it doesn't compile, return E_INVARG.  If it crashes, well, it crashes.  If it works okay, set .eval_env to it and set .eval_ticks to the amount of time it took.";
    if (is_player(this) && $perm_utils:controls(caller_perms(), this))
      program = args[1];
      value = $no_one:eval_d(";ticks = ticks_left();" + program + ";return ticks - ticks_left() - 2;");
      if (!(value[1]))
        return E_INVARG;
      elseif (typeof(value[2]) == ERR)
        return value[2];
      endif
      try
        ok = this.eval_env = program;
        this.eval_ticks = value[2];
        return 1;
      except error (ANY)
        return error[1];
      endtry
    endif
  endverb

  verb "@clearp*roperty @clprop*erty" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@clearproperty <obj>.<prop>";
    "Set the value of <obj>.<prop> to `clear', making it appear to be the same as the property on its parent.";
    set_task_perms(player);
    if (!(l = $code_utils:parse_propref(dobjstr)))
      player:notify(tostr("Usage:  ", verb, " <object>.<property>"));
    elseif ($command_utils:object_match_failed(dobj = player:my_match_object(l[1]), l[1]))
      "... bogus object...";
    endif
    try
      if (is_clear_property(dobj, prop = l[2]))
        player:notify(tostr("Property ", dobj, ".", prop, " is already clear!"));
        return;
      endif
      clear_property(dobj, prop);
      player:notify(tostr("Property ", dobj, ".", prop, " cleared; value is now ", toliteral(dobj.(prop)), "."));
    except (E_INVARG)
      player:notify(tostr("You can't clear ", dobj, ".", prop, "; none of the ancestors define that property."));
    except error (ANY)
      player:notify(error[2]);
    endtry
  endverb

  verb "@disown @disinherit" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Syntax: @disown <object> [from <object>]";
    "This command is used to remove unwanted children of objects you control. If you control an object, and there is a child of that object you do not want, this command will chparent() the object to its grandparent.";
    set_task_perms(player);
    if (prepstr)
      if (prepstr != "from")
        player:notify("Usage:  ", verb, " <object> [from <object>]");
        return;
      elseif ($command_utils:object_match_failed(iobj = player:my_match_object(iobjstr), iobjstr))
        "... from WHAT?..";
        return;
      elseif (valid(dobj = $string_utils:literal_object(dobjstr)))
        "... literal object number...";
        if (parent(dobj) != iobj)
          player:notify(tostr(dobj, " is not a child of ", iobj.name, " (", iobj, ")"));
          return;
        endif
      elseif ($command_utils:object_match_failed(dobj = $string_utils:match(dobjstr, children(iobj), "name", children(iobj), "aliases"), dobjstr))
        "... can't match dobjstr against any children of iobj";
        return;
      endif
    elseif ($command_utils:object_match_failed(dobj = player:my_match_object(dobjstr), dobjstr))
      "... can't match dobjstr...";
      return;
    endif
    try
      if ($object_utils:disown(dobj))
        player:notify(tostr(dobj.name, " (", dobj, ")'s parent is now ", (grandparent = parent(dobj)).name, " (", grandparent, ")."));
      else
        "this should never happen";
      endif
    except e (E_PERM, E_INVARG)
      {code, message, value, traceback} = e;
      player:notify(message);
    endtry
  endverb

  verb eval_cmd_string (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":eval_cmd_string(string[,debug])";
    "Evaluates the string the way this player would normally expect to see it evaluated if it were typed on the command line.  debug (defaults to 1) indicates how the debug flag should be set during the evaluation.";
    " => {@eval_result, ticks, seconds}";
    "where eval_result is the result of the actual eval() call.";
    "";
    "For the case where string is an expression, we need to prefix `return ' and append `;' to string before passing it to eval().  However this is not appropriate for statements, where it is assumed an explicit return will be provided somewhere or that the return value is irrelevant.  The code below assumes that string is an expression unless it either begins with a semicolon `;' or one of the MOO language statement keywords.";
    "Next, the substitutions described by this.eval_subs, which should be a list of pairs {string, sub}, are performed on string";
    "Finally, this.eval_env is prefixed to the beginning while this.eval_ticks is subtracted from the eventual tick count.  This allows string to refer to predefined variables like `here' and `me'.";
    set_task_perms(caller_perms());
    {program, ?debug = 1} = args;
    program = program + ";";
    debug = debug ? 38 | 0;
    if (!match(program, "^ *%(;%|%(if%|fork?%|return%|while%|try%)[^a-z0-9A-Z_]%)"))
      program = "return " + program;
    endif
    program = tostr(this.eval_env, ";", $code_utils:substitute(program, this.eval_subs));
    ticks = ticks_left() - 53 - this.eval_ticks + debug;
    seconds = seconds_left();
    value = debug ? eval(program) | $code_utils:eval_d(program);
    seconds = seconds - seconds_left();
    ticks = ticks - ticks_left();
    return {@value, ticks, seconds};
  endverb

  verb "@dump" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@dump something [with [id=...] [noprops] [noverbs] [create]]";
    "This spills out all properties and verbs on an object, calling suspend at appropriate intervals.";
    "   id=#nnn -- specifies an idnumber to use in place of the object's actual id (for porting to another MOO)";
    "   noprops -- don't show properties.";
    "   noverbs -- don't show verbs.";
    "   create  -- indicates that a @create command should be generated and all of the verbs be introduced with @verb rather than @args; the default assumption is that the object already exists and you're just doing this to have a look at it.";
    set_task_perms(player);
    dobj = player:my_match_object(dobjstr);
    if ($command_utils:object_match_failed(dobj, dobjstr))
      return;
    endif
    if (prepstr && prepstr != "with")
      player:notify(tostr("Usage:  ", verb, " something [with [id=...] [noprops] [noverbs] [create]]"));
      return;
    endif
    targname = tostr(dobj);
    options = {"props", "verbs"};
    create = 0;
    if (iobjstr)
      for o in ($string_utils:explode(iobjstr))
        if (index(o, "id=") == 1)
          targname = o[4..$];
        elseif (o in {"noprops", "noverbs"})
          options = setremove(options, o[3..$]);
        elseif (o in {"create"})
          create = 1;
        else
          player:notify(tostr("`", o, "' not understood as valid option."));
          player:notify(tostr("Usage:  ", verb, " something [with [id=...] [noprops] [noverbs] [create]]"));
          return;
        endif
      endfor
    endif
    if (create)
      player:notify($code_utils:dump_preamble(dobj));
    endif
    if ("props" in options)
      player:notify_lines_suspended($code_utils:dump_properties(dobj, create, targname));
    endif
    if (!("verbs" in options))
      player:notify("\"***finished***");
      return;
    endif
    player:notify("");
    player:notify_lines_suspended($code_utils:dump_verbs(dobj, create, targname));
    player:notify("\"***finished***");
  endverb

  verb "#*" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Copied from Player Class hacked with eval that does substitutions and assorted stuff (#8855):# by Geust (#24442) Sun May  9 20:19:05 1993 PDT";
    "#<string>[.<property>|.parent] [exit|player|inventory] [for <code>] returns information about the object (we'll call it <thing>) named by string.  String is matched in the current room unless one of exit|player|inventory is given.";
    "If neither .<property>|.parent nor <code> is specified, just return <thing>.";
    "If .<property> is named, return <thing>.<property>.  .parent returns parent(<thing>).";
    "If <code> is given, it is evaluated, with the value returned by the first part being substituted for %# in <code>.";
    "For example, the command";
    "  #JoeFeedback.parent player for toint(%#)";
    "will return 26026 (unless Joe has chparented since writing this).";
    set_task_perms(player);
    if (!(whatstr = verb[2..dot = min(index(verb + ".", "."), index(verb + ":", ":")) - 1]))
      player:notify("Usage:  #string [exit|player|inventory]");
      return;
    elseif (!args)
      what = player:my_match_object(whatstr);
    elseif (index("exits", args[1]) == 1)
      what = player.location:match_exit(whatstr);
    elseif (index("inventory", args[1]) == 1)
      what = player:match(whatstr);
    elseif (index("players", args[1]) == 1)
      what = $string_utils:match_player(whatstr);
      if ($command_utils:player_match_failed(what, whatstr))
        return;
      endif
    else
      what = player:my_match_object(whatstr);
    endif
    if (!valid(what) && match(whatstr, "^[0-9]+$"))
      what = toobj(whatstr);
    endif
    if ($command_utils:object_match_failed(what, whatstr))
      return;
    endif
    while (index(verb, ".parent") == dot + 1)
      what = parent(what);
      dot = dot + 7;
    endwhile
    if (dot >= length(verb))
      val = what;
    elseif ((value = $code_utils:eval_d(tostr("return ", what, verb[dot + 1..$], ";")))[1])
      val = value[2];
    else
      player:notify_lines(value[2]);
      return;
    endif
    if (prepstr)
      program = strsub(iobjstr + ";", "%#", toliteral(val));
      end = 1;
      "while (\"A\" <= (l = argstr[end]) && l <= \"Z\")";
      while ("A" <= (l = program[end]) && l <= "Z")
        end = end + 1;
      endwhile
      if (program[1] == ";" || program[1..end - 1] in {"if", "for", "fork", "return", "while", "try"})
        program = $code_utils:substitute(program, this.eval_subs);
      else
        program = $code_utils:substitute("return " + program, this.eval_subs);
      endif
      if ((value = eval(program))[1])
        player:notify(this:eval_value_to_string(value[2]));
      else
        player:notify_lines(value[2]);
        nerrors = length(value[2]);
        player:notify(tostr(nerrors, " error", nerrors == 1 ? "." | "s."));
      endif
    else
      player:notify(this:eval_value_to_string(val));
    endif
  endverb

  verb eval_value_to_string (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    if (typeof(val = args[1]) == OBJ)
      return tostr("=> ", val, "  ", valid(val) ? "(" + val.name + ")" | ((a = $list_utils:assoc(val, {{#-1, "<$nothing>"}, {#-2, "<$ambiguous_match>"}, {#-3, "<$failed_match>"}})) ? a[2] | "<invalid>"));
    elseif (typeof(val) == ERR)
      return tostr("=> ", toliteral(val), "  (", val, ")");
    else
      return tostr("=> ", toliteral(val));
    endif
  endverb

  verb "@progo*ptions @prog-o*ptions @programmero*ptions @programmer-o*ptions" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@<what>-option <option> [is] <value>   sets <option> to <value>";
    "@<what>-option <option>=<value>        sets <option> to <value>";
    "@<what>-option +<option>     sets <option>   (usually equiv. to <option>=1";
    "@<what>-option -<option>     resets <option> (equiv. to <option>=0)";
    "@<what>-option !<option>     resets <option> (equiv. to <option>=0)";
    "@<what>-option <option>      displays value of <option>";
    set_task_perms(player);
    what = "prog";
    options = what + "_options";
    option_pkg = #0.(options);
    set_option = "set_" + what + "_option";
    if (!args)
      player:notify_lines({"Current " + what + " options:", "", @option_pkg:show(this.(options), option_pkg.names)});
      return;
    elseif (typeof(presult = option_pkg:parse(args)) == STR)
      player:notify(presult);
      return;
    else
      if (length(presult) > 1)
        if (typeof(sresult = this:(set_option)(@presult)) == STR)
          player:notify(sresult);
          return;
        elseif (!sresult)
          player:notify("No change.");
          return;
        endif
      endif
      player:notify_lines(option_pkg:show(this.(options), presult[1]));
    endif
  endverb

  verb prog_option (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":prog_option(name)";
    "Returns the value of the specified prog option";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      return $prog_options:get(this.prog_options, args[1]);
    else
      return E_PERM;
    endif
  endverb

  verb set_prog_option (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_prog_option(oname,value)";
    "Changes the value of the named option.";
    "Returns a string error if something goes wrong.";
    if (!(caller == this || $perm_utils:controls(caller_perms(), this)))
      return tostr(E_PERM);
    endif
    "...this is kludgy, but it saves me from writing the same verb 3 times.";
    "...there's got to be a better way to do this...";
    verb[1..4] = "";
    foo_options = verb + "s";
    "...";
    if (typeof(s = #0.(foo_options):set(this.(foo_options), @args)) == STR)
      return s;
    elseif (s == this.(foo_options))
      return 0;
    else
      this.(foo_options) = s;
      return 1;
    endif
  endverb

  verb "@list*#" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@list <obj>:<verb> [<dobj> <prep> <iobj>] [with[out] paren|num] [all] [ranges]";
    set_task_perms(player);
    bynumber = verb == "@list#";
    pflag = player:prog_option("list_all_parens");
    nflag = !player:prog_option("list_no_numbers");
    permflag = player:prog_option("list_show_permissions");
    aflag = 0;
    argspec = {};
    range = {};
    spec = args ? $code_utils:parse_verbref(args[1]) | E_INVARG;
    args = spec ? listdelete(args, 1) | E_INVARG;
    while (args)
      if (args[1] && (index("without", args[1]) == 1 || args[1] == "wo"))
        "...w,wi,wit,with => 1; wo,witho,withou,without => 0...";
        fval = !index(args[1], "o");
        if (`index("parentheses", args[2]) ! ANY' == 1)
          pflag = fval;
          args[1..2] = {};
        elseif (`index("numbers", args[2]) ! ANY' == 1)
          nflag = fval;
          args[1..2] = {};
        else
          player:notify(tostr(args[1], " WHAT?"));
          args = E_INVARG;
        endif
      elseif (index("all", args[1]) == 1)
        if (bynumber)
          player:notify("Don't use `all' with @list#.");
          args = E_INVARG;
        else
          aflag = 1;
          args[1..1] = {};
        endif
      elseif (index("0123456789", args[1][1]) || index(args[1], "..") == 1)
        if (E_INVARG == (s = $seq_utils:from_string(args[1])))
          player:notify(tostr("Garbled range:  ", args[1]));
          args = E_INVARG;
        else
          range = $seq_utils:union(range, s);
          args = listdelete(args, 1);
        endif
      elseif (bynumber)
        player:notify("Don't give args with @list#.");
        args = E_INVARG;
      elseif (argspec)
        "... second argspec?  Not likely ...";
        player:notify(tostr(args[1], " unexpected."));
        args = E_INVARG;
      elseif (typeof(pas = $code_utils:parse_argspec(@args)) == LIST)
        argspec = pas[1];
        if (length(argspec) < 2)
          player:notify(tostr("Argument `", @argspec, "' malformed."));
          args = E_INVARG;
        else
          argspec[2] = $code_utils:full_prep(argspec[2]) || argspec[2];
          args = pas[2];
        endif
      else
        "... argspec is bogus ...";
        player:notify(tostr(pas));
        args = E_INVARG;
      endif
    endwhile
    if (args == E_INVARG)
      if (bynumber)
        player:notify(tostr("Usage:  ", verb, " <object>:<verbnumber> [with|without parentheses|numbers] [ranges]"));
      else
        player:notify(tostr("Usage:  ", verb, " <object>:<verb> [<dobj> <prep> <iobj>] [with|without parentheses|numbers] [all] [ranges]"));
      endif
      return;
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1]), spec[1]))
      return;
    endif
    shown_one = 0;
    for what in ({object, @$object_utils:ancestors(object)})
      if (bynumber)
        vname = $code_utils:toint(spec[2]);
        if (vname == E_TYPE)
          return player:notify("Verb number expected.");
        elseif (vname < 1 || `vname > length(verbs(what)) ! E_PERM => 0')
          return player:notify("Verb number out of range.");
        endif
        code = `verb_code(what, vname, pflag) ! ANY';
      elseif (argspec)
        vnum = $code_utils:find_verb_named(what, spec[2]);
        while (vnum && `verb_args(what, vnum) ! ANY' != argspec)
          vnum = $code_utils:find_verb_named(what, spec[2], vnum + 1);
        endwhile
        vname = vnum;
        code = !vnum ? E_VERBNF | `verb_code(what, vnum, pflag) ! ANY';
      else
        vname = spec[2];
        code = `verb_code(what, vname, pflag) ! ANY';
      endif
      if (code != E_VERBNF)
        if (shown_one)
          player:notify("");
        elseif (what != object)
          player:notify(tostr("Object ", object, " does not define that verb", argspec ? " with those args" | "", ", but its ancestor ", what, " does."));
        endif
        if (typeof(code) == ERR)
          player:notify(tostr(what, ":", vname, " -- ", code));
        else
          info = verb_info(what, vname);
          vargs = verb_args(what, vname);
          fullname = info[3];
          if (index(fullname, " "))
            fullname = toliteral(fullname);
          endif
          if (index(vargs[2], "/"))
            vargs[2] = tostr("(", vargs[2], ")");
          endif
          player:notify(tostr(what, ":", fullname, "   ", $string_utils:from_list(vargs, " "), permflag ? " " + info[2] | ""));
          if (code == {})
            player:notify("(That verb has not been programmed.)");
          else
            lineseq = {1, length(code) + 1};
            range && (lineseq = $seq_utils:intersection(range, lineseq));
            if (!lineseq)
              player:notify("(No lines in that range.)");
            endif
            for k in [1..length(lineseq) / 2]
              for i in [lineseq[2 * k - 1]..lineseq[2 * k] - 1]
                if (nflag)
                  end = 0;
                  if (i < 10)
                    end = 1;
                  endif
                  player:notify(tostr(" "[1..end], i, ":  ", code[i]));
                else
                  player:notify(code[i]);
                endif
                $command_utils:suspend_if_needed(0);
              endfor
            endfor
          endif
        endif
        shown_one = 1;
      endif
      if (shown_one && !aflag)
        return;
      endif
    endfor
    if (!shown_one)
      player:notify(tostr("That object does not define that verb", argspec ? " with those args." | "."));
    endif
  endverb

  verb set_eval_subs (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Copied from Player Class hacked with eval that does substitutions and assorted stuff (#8855):set_eval_subs by Geust (#24442) Fri Aug  5 13:18:59 1994 PDT";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (typeof(subs = args[1]) != LIST)
      return E_TYPE;
    else
      for pair in (subs)
        if (length(pair) != 2 || typeof(pair[1] != STR) || typeof(pair[2] != STR))
          return E_INVARG;
        endif
      endfor
    endif
    return `this.eval_subs = subs ! ANY';
  endverb

  verb "@verbs*" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      try
        if (verb[7] != "(" && verb[$] != ")")
          player:tell("Usage:  @verbs <object>");
          return;
        else
          dobjstr = verb[8..$ - 1];
        endif
      except (E_RANGE)
        return player:tell("Usage:  @verbs <object>");
      endtry
    endif
    thing = player:my_match_object(dobjstr);
    if (!$command_utils:object_match_failed(thing, dobjstr))
      verbs = $object_utils:accessible_verbs(thing);
      player:tell(";verbs(", thing, ") => ", toliteral(verbs));
    endif
  endverb

  verb "@old-forked-v*erbose" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Syntax:  @forked-v*erbose [player]";
    "         @forked-v*erbose all wizards";
    "";
    "For a normal player, shows all the tasks you have waiting in your queue, especially those forked or suspended. A wizard will see all the tasks of all the players unless the optional argument is provided. For a task which has suspended, and not a fresh fork, shows the full callers() stack.";
    "The second form is only usable by wizards and provides an output of all tasks owned by characters who are .wizard=1. Useful to find a task that may get put in a random queue due to $wiz_utils:random_wizard. Or even finding verbs that run with wizard permissions that shouldn't be.";
    set_task_perms(player);
    if (!dobjstr)
      tasks = queued_tasks();
    elseif (dobjstr == "all wizards" && player.wizard)
      tasks = {};
      for t in (queued_tasks())
        if (valid(t[5]) && (t[5]).wizard)
          tasks = {@tasks, t};
        endif
        $command_utils:suspend_if_needed(1);
      endfor
    elseif ($command_utils:player_match_result(dobj = $string_utils:match_player(dobjstr), dobjstr)[1])
      return;
    elseif (typeof(tasks = $wiz_utils:queued_tasks(dobj)) != LIST)
      player:notify(tostr(verb, " ", dobj.name, "(", dobj, "):  ", tasks));
      return;
    endif
    if (tasks)
      su = $string_utils;
      player:notify("Queue ID    Start Time            Owner         Verb (Line) [This]");
      player:notify("--------    ----------            -----         -----------------");
      now = time();
      for task in (tasks)
        $command_utils:suspend_if_needed(0);
        {q_id, start, nu, nu2, owner, vloc, vname, lineno, this, ?size = 0} = task;
        time = start >= now ? ctime(start)[5..24] | su:left(start == -1 ? "Reading input ..." | tostr(now - start, " seconds ago..."), 20);
        owner_name = valid(owner) ? owner.name | tostr("Dead ", owner);
        player:notify(tostr(su:left(tostr(q_id), 10), "  ", time, "  ", su:left(owner_name, 12), "  ", vloc, ":", vname, " (", lineno, ")", this != vloc ? tostr(" [", this, "]") | ""));
        if (stack = `task_stack(q_id, 1) ! E_INVARG => 0')
          for frame in (listdelete(stack, 1))
            {sthis, svname, sprogger, svloc, splayer, slineno} = frame;
            player:notify(tostr("                    Called By...  ", su:left(valid(sprogger) ? sprogger.name | tostr("Dead ", sprogger), 12), "  ", svloc, ":", svname, sthis != svloc ? tostr(" [", sthis, "]") | "", " (", slineno, ")"));
          endfor
        endif
      endfor
      player:notify("-----------------------------------------------------------------");
    else
      player:notify("No tasks.");
    endif
  endverb

  verb "@props @properties" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "r"
    "Usage: @properties <object>";
    "Alias: @props";
    "Displays all properties defined on <object>. Properties unreadable by you display as `E_PERM'.";
    if (player != this)
      return player:tell(E_PERM);
    endif
    set_task_perms(player);
    ob = this:my_match_object(argstr);
    if (!$command_utils:object_match_failed(ob, argstr))
      this:notify(tostr(";properties(", $code_utils:corify_object(ob), ") => ", toliteral($object_utils:accessible_props(ob))));
    endif
    "Last modified Mon Nov 28 06:21:21 2005 PST, by Roebare (#109000).";
  endverb
endobject