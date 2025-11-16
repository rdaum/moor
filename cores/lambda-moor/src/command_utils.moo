object COMMAND_UTILS
  name: "command utilities"
  parent: GENERIC_UTILS
  owner: #2
  readable: true

  property feature_task (owner: HACKER, flags: "") = {388912170, "@abort", {}, "", #-1, "", "", #-1, ""};
  property lag_samples (owner: #2, flags: "r") = {};

  override description = {
    "This is the command utilities utility package.  See `help $command_utils' for more details."
  };
  override help_msg = {
    "$command_utils is the repository for verbs that are of general usefulness to authors of all sorts of commands.  For more details about any of these verbs, use `help $command_utils:<verb-name>'.",
    "",
    "Detecting and Handling Failures in Matching",
    "-------------------------------------------",
    ":object_match_failed(match_result, name)",
    "    Test whether or not a :match_object() call failed and print messages if so.",
    ":player_match_failed(match_result, name)",
    "    Test whether or not a :match_player() call failed and print messages if so.",
    ":player_match_result(match_results, names)",
    "    ...similar to :player_match_failed, but does a whole list at once.",
    "",
    "Reading Input from the Player",
    "-----------------------------",
    ":read()         -- Read one line of input from the player and return it.",
    ":yes_or_no([prompt])",
    "                -- Prompt for and read a `yes' or `no' answer.",
    ":read_lines()   -- Read zero or more lines of input from the player.",
    ":dump_lines(lines) ",
    "                -- Return list of lines quoted so that feeding them to ",
    "                   :read_lines() will reproduce the original lines.",
    ":read_lines_escape(escapes[,help])",
    "                -- Like read_lines, except you can provide more escapes",
    "                   to terminate the read.",
    "",
    "Feature Objects",
    "---------------",
    ":validate_feature -- compare command line against feature verb argument spec",
    "",
    "Utilities for Suspending",
    "------------------------",
    ":running_out_of_time()",
    "                -- Return true if we're low on ticks or seconds.",
    ":suspend_if_needed(time)",
    "                -- Suspend (and return true) if we're running out of time.",
    "",
    "Client Support for Lengthy Commands",
    "-----------------------------------",
    ":suspend(args)  -- Handle PREFIX and SUFFIX for clients in long commands."
  };
  override import_export_id = "command_utils";
  override object_size = {18931, 1084848672};

  verb object_match_failed (this none this) owner: #2 flags: "rxd"
    "Usage: object_match_failed(object, string)";
    "Prints a message if string does not match object.  Generally used after object is derived from a :match_object(string).";
    {match_result, string} = args;
    tell = $perm_utils:controls(caller_perms(), player) ? "notify" | "tell";
    if (index(string, "#") == 1 && $code_utils:toobj(string) != E_TYPE)
      "...avoid the `I don't know which `#-2' you mean' message...";
      if (!valid(match_result))
        player:(tell)(tostr(string, " does not exist."));
      endif
      return !valid(match_result);
    elseif (match_result == $nothing)
      player:(tell)("You must give the name of some object.");
    elseif (match_result == $failed_match)
      player:(tell)(tostr("I see no \"", string, "\" here."));
    elseif (match_result == $ambiguous_match)
      player:(tell)(tostr("I don't know which \"", string, "\" you mean."));
    elseif (!valid(match_result))
      player:(tell)(tostr(match_result, " does not exist."));
    else
      return 0;
    endif
    return 1;
  endverb

  verb "player_match_result player_match_failed" (this none this) owner: #2 flags: "rxd"
    ":player_match_failed(result,string)";
    "  is exactly like :object_match_failed(result,string)";
    "  except that its messages are more suitable for player searches.";
    ":player_match_result(results,strings)";
    "  handles a list of results, also presumably from $string_utils:match_player(strings), printing messages to player for *each* of the nonmatching strings.  It returns a list, an overall result (true if some string didn't match --- just like player_match_failed), followed by the list players that matched.";
    "";
    "An optional 3rd arg gives an identifying string to prefix to each of the nasty messages.";
    if (valid(player))
      tell = $perm_utils:controls(caller_perms(), player) ? "notify" | "tell";
      plyr = player;
    else
      tell = "notify";
      plyr = $login;
    endif
    "...";
    {match_results, strings, ?cmdid = ""} = args;
    pmf = verb == "player_match_failed";
    if (typeof(match_results) == OBJ)
      match_results = {match_results};
      strings = {strings};
    endif
    pset = {};
    bombed = 0;
    for i in [1..length(match_results)]
      if (valid(result = match_results[i]))
        pset = setadd(pset, match_results[i]);
      elseif (result == $nothing)
        "... player_match_result quietly skips over blank strings";
        if (pmf)
          plyr:(tell)("You must give the name of some player.");
          bombed = 1;
        endif
      elseif (result == $failed_match)
        plyr:(tell)(tostr(cmdid, "\"", strings[i], "\" is not the name of any player."));
        bombed = 1;
      elseif (result == $ambiguous_match)
        lst = $player_db:find_all(strings[i]);
        plyr:(tell)(tostr(cmdid, "\"", strings[i], "\" could refer to ", length(lst) > 20 ? tostr("any of ", length(lst), " players") | $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", lst), "no one", " or "), "."));
        bombed = 1;
      else
        plyr:(tell)(tostr(result, " does not exist."));
        bombed = 1;
      endif
    endfor
    if (!bombed && !pset)
      "If there were NO valid results, but not any actual 'error', fail anyway.";
      plyr:(tell)("You must give the name of some player.");
      bombed = 1;
    endif
    return pmf ? bombed | {bombed, @pset};
  endverb

  verb read (this none this) owner: #2 flags: "rxd"
    "$command_utils:read() -- read a line of input from the player and return it";
    "Optional argument is a prompt portion to replace `a line of input' in the prompt.";
    "";
    "Returns E_PERM if the current task is not the most recent task spawned by a command from player.";
    {?prompt = "a line of input"} = args;
    c = callers();
    p = c[$][5];
    p:notify(tostr("[Type ", prompt, " or `@abort' to abort the command.]"));
    try
      ans = read();
      if ($string_utils:trim(ans) == "@abort")
        p:notify(">> Command Aborted <<");
        kill_task(task_id());
      endif
      return ans;
    except error (ANY)
      return error[1];
    endtry
  endverb

  verb read_lines (this none this) owner: #2 flags: "rxd"
    "$command_utils:read_lines([max]) -- read zero or more lines of input";
    "";
    "Returns a list of strings, the (up to MAX, if given) lines typed by the player.  Returns E_PERM if the current task is not a command task that has never called suspend().";
    "In order that one may enter arbitrary lines, including \"@abort\" or \".\", if the first character in an input line is `.' and there is some nonwhitespace afterwords, the `.' is dropped and the rest of the line is taken verbatim, so that, e.g., \".@abort\" enters as \"@abort\" and \"..\" enters as \".\".";
    {?max = 0} = args;
    c = callers();
    p = c[$][5];
    p:notify(tostr("[Type", max ? tostr(" up to ", max) | "", " lines of input; use `.' to end or `@abort' to abort the command.]"));
    ans = {};
    while (1)
      try
        line = read();
        if (line[1..min(6, $)] == "@abort" && (tail = line[7..$]) == $string_utils:space(tail))
          p:notify(">> Command Aborted <<");
          kill_task(task_id());
        elseif (!line || line[1] != ".")
          ans = {@ans, line};
        elseif ((tail = line[2..$]) == $string_utils:space(tail))
          return ans;
        else
          ans = {@ans, tail};
        endif
        if (max && length(ans) >= max)
          return ans;
        endif
      except error (ANY)
        return error[1];
      endtry
    endwhile
  endverb

  verb yes_or_no (this none this) owner: #2 flags: "rxd"
    ":yes-or-no([prompt]) -- prompts the player for a yes or no answer and returns a true value iff the player enters a line of input that is some prefix of \"yes\"";
    "";
    "Returns E_NONE if the player enters a blank line, E_INVARG, if the player enters something that isn't a prefix of \"yes\" or \"no\", and E_PERM if the current task is not a command task that has never called suspend().";
    c = callers();
    p = c[$][5];
    p:notify(tostr(args ? args[1] + " " | "", "[Enter `yes' or `no']"));
    try
      ans = read(@caller == p || $perm_utils:controls(caller_perms(), p) ? {p} | {});
      if (ans = $string_utils:trim(ans))
        if (ans == "@abort")
          p:notify(">> Command Aborted <<");
          kill_task(task_id());
        endif
        return index("yes", ans) == 1 || (index("no", ans) != 1 && E_INVARG);
      else
        return E_NONE;
      endif
    except error (ANY)
      return error[1];
    endtry
  endverb

  verb read_lines_escape (this none this) owner: #2 flags: "rxd"
    "$command_utils:read_lines_escape(escapes[,help]) -- read zero or more lines of input";
    "";
    "Similar to :read_lines() except that help is available and one may specify other escape sequences to terminate the read.";
    "  escapes should be either a string or list of strings; this specifies which inputs other from `.' or `@abort' should terminate the read (... don't use anything beginning with a `.').";
    "  help should be a string or list of strings to be printed in response to the player typing `?'; the first line of the help text should be a general comment about what the input text should be used for.  Successive lines should describe the effects of the alternative escapes.";
    "Returns {end,list-of-strings-input} where end is the particular line that terminated this input or 0 if input terminated normally with `.'.  Returns E_PERM if the current task is not a command task that has never called suspend().  ";
    "@abort and lines beginning with `.' are treated exactly as with :read_lines()";
    {escapes, ?help = "You are currently in a read loop."} = args;
    c = callers();
    p = c[$][5];
    escapes = {".", "@abort", @typeof(escapes) == LIST ? escapes | {escapes}};
    p:notify(tostr("[Type lines of input; `?' for help; end with `", $string_utils:english_list(escapes, "", "' or `", "', `", ""), "'.]"));
    ans = {};
    escapes[1..0] = {"?"};
    "... set up the help text...";
    if (typeof(help) != LIST)
      help = {help};
    endif
    help[2..1] = {"Type `.' on a line by itself to finish.", "Anything else with a leading period is entered with the period removed.", "Type `@abort' to abort the command completely."};
    while (1)
      try
        line = read();
        if ((trimline = $string_utils:trimr(line)) in escapes)
          if (trimline == ".")
            return {0, ans};
          elseif (trimline == "@abort")
            p:notify(">> Command Aborted <<");
            kill_task(task_id());
          elseif (trimline == "?")
            p:notify_lines(help);
          else
            return {trimline, ans};
          endif
        else
          if (line && line[1] == ".")
            line[1..1] = "";
          endif
          ans = {@ans, line};
        endif
      except error (ANY)
        return error[1];
      endtry
    endwhile
  endverb

  verb suspend (this none this) owner: #2 flags: "rxd"
    "Suspend, using output_delimiters() in case a client needs to keep track";
    "of the output of the current command.";
    "Args are TIME, amount of time to suspend, and optional (misnamed) OUTPUT.";
    "If given no OUTPUT, just do a suspend.";
    "If OUTPUT is neither list nor string, suspend and return output_delimiters";
    "If OUTPUT is a list, it should be in the output_delimiters() format:";
    "  {PREFIX, SUFFIX}.  Use these to handle that client stuff.";
    "If OUTPUT is a string, it should be SUFFIX (output_delimiters[2])";
    "";
    "Proper usage:";
    "The first time you want to suspend, use";
    "  output_delimiters = $command_utils:suspend(time, x);";
    "where x is some non-zero number.";
    "Following, use";
    "  $command_utils:suspend(time, output_delimiters);";
    "To wrap things up, use";
    "  $command_utils:suspend(time, output_delimiters[2]);";
    "You'll probably want time == 0 most of the time.";
    "Note: Using this from verbs called by other verbs could get pretty weird.";
    {time, ?output = 0} = args;
    set_task_perms(caller_perms());
    value = 0;
    if (!output)
      suspend(time);
    else
      if (typeof(output) == LIST)
        PREFIX = output[1];
        SUFFIX = output[2];
        if (PREFIX)
          player:tell(output[2]);
        endif
        suspend(time);
        if (SUFFIX)
          player:tell(output[1]);
        endif
      elseif (typeof(output) == STR)
        if (output)
          player:tell(output);
        endif
      else
        output = output_delimiters(player);
        suspend(time);
        if (output != {"", ""})
          player:tell(output[1]);
        endif
        value = output;
      endif
    endif
    return output;
  endverb

  verb running_out_of_time (this none this) owner: HACKER flags: "rxd"
    "Return true if we're running out of ticks or seconds.";
    return ticks_left() < 4000 || seconds_left() < 2;
    "If this verb is changed make sure to change :suspend_if_needed as well.";
  endverb

  verb suspend_if_needed (this none this) owner: #2 flags: "rxd"
    "Usage:  $command_utils:suspend_if_needed(<time>[, @<announcement>])";
    "See if we're running out of ticks or seconds, and if so suspend(<time>) and return true.  If more than one arg is given, print the remainder with player:tell.";
    {?time = 10, @ann} = args;

    "Use the builtin for efficient commit when time is 0";
    if (time == 0)
      "Use 4000 tick threshold to match legacy behavior";
      if (suspend_if_needed(4000))
        if (ann && valid(player))
          player:tell(tostr(@ann));
        endif
        return 1;
      endif
    else
      "Legacy behavior: check manually and suspend with time delay";
      if (ticks_left() < 4000 || seconds_left() < 2)
        "Note: above computation should be the same as :running_out_of_time.";
        if (ann && valid(player))
          player:tell(tostr(@ann));
        endif
        amount = max(time, min($login:current_lag(), 10));
        set_task_perms(caller_perms());
        "this is trying to back off according to lag...";
        suspend(amount);
        return 1;
      endif
    endif
  endverb

  verb dump_lines (this none this) owner: HACKER flags: "rxd"
    ":dump_lines(text) => text `.'-quoted for :read_lines()";
    "  text is assumed to be a list of strings";
    "Returns a corresponding list of strings which, when read via :read_lines, ";
    "produces the original list of strings (essentially, any strings beginning ";
    "with a period \".\" have the period doubled).";
    "The list returned includes a final \".\"";
    text = args[1];
    newtext = {};
    i = lasti = 0;
    for line in (text)
      if (match(line, "^%(%.%| *@abort *$%)"))
        newtext = {@newtext, @i > lasti ? text[lasti + 1..i] | {}, "." + line};
        lasti = i = i + 1;
      else
        i = i + 1;
      endif
    endfor
    return {@newtext, @i > lasti ? text[lasti + 1..i] | {}, "."};
  endverb

  verb explain_syntax (this none this) owner: #2 flags: "rxd"
    ":explain_syntax(here,verb,args)";
    verb = args[2];
    for x in ({player, args[1], @valid(dobj) ? {dobj} | {}, @valid(iobj) ? {iobj} | {}})
      what = x;
      while (hv = $object_utils:has_verb(what, verb))
        what = hv[1];
        i = 1;
        while (i = $code_utils:find_verb_named(what, verb, i))
          if (evs = $code_utils:explain_verb_syntax(x, verb, @verb_args(what, i)))
            player:tell("Try this instead:  ", evs);
            return 1;
          endif
          i = i + 1;
        endwhile
        what = parent(what);
      endwhile
    endfor
    return 0;
  endverb

  verb do_huh (this none this) owner: #2 flags: "rx"
    ":do_huh(verb,args)  what :huh should do by default.";
    {verb, args} = args;
    if ($perm_utils:controls(caller_perms(), player) || caller_perms() == player)
      this.feature_task = {task_id(), verb, args, argstr, dobj, dobjstr, prepstr, iobj, iobjstr};
    endif
    set_task_perms(cp = caller_perms());
    notify = $perm_utils:controls(cp, player) ? "notify" | "tell";
    if (verb == "")
      "should only happen if a player types backslash";
      player:(notify)("I don't understand that.");
      return;
    endif
    if (player:my_huh(verb, args))
      "... the player found something funky to do ...";
    elseif (caller:here_huh(verb, args))
      "... the room found something funky to do ...";
    elseif (player:last_huh(verb, args))
      "... player's second round found something to do ...";
    elseif (dobj == $ambiguous_match)
      if (iobj == $ambiguous_match)
        player:(notify)(tostr("I don't understand that (\"", dobjstr, "\" and \"", iobjstr, "\" are both ambiguous names)."));
      else
        player:(notify)(tostr("I don't understand that (\"", dobjstr, "\" is an ambiguous name)."));
      endif
    elseif (iobj == $ambiguous_match)
      player:(notify)(tostr("I don't understand that (\"", iobjstr, "\" is an ambiguous name)."));
    else
      player:(notify)("I don't understand that.");
      player:my_explain_syntax(caller, verb, args) || (caller:here_explain_syntax(caller, verb, args) || this:explain_syntax(caller, verb, args));
    endif
  endverb

  verb task_info (this none this) owner: #2 flags: "rxd"
    "task_info(task id)";
    "Return info (the same info supplied by queued_tasks()) about a given task id, or E_INVARG if there's no such task queued.";
    "WIZARDLY";
    set_task_perms(caller_perms());
    tasks = queued_tasks();
    task_id = args[1];
    for task in (tasks)
      if (task[1] == task_id)
        return task;
      endif
    endfor
    return E_INVARG;
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.lag_samples = {};
      this.feature_task = "hey, neat, no feature verbs have been run yet!";
    endif
  endverb

  verb kill_if_laggy (this none this) owner: HACKER flags: "rxd"
    "Kills this task if the current lag is greater than args[1].  Args[2..n] will be passed to player:tell.";
    cutoff = args[1];
    if ($login:current_lag() > cutoff)
      player:tell(@listdelete(args, 1));
      kill_task(task_id());
    endif
  endverb

  verb validate_feature (this none this) owner: HACKER flags: "rxd"
    ":validate_feature(verb, args)";
    "  (where `verb' and `args' are the arguments passed to :my_huh)";
    "  returns true or false based on whether this is the same command typed by the user (comparing it against $command_utils.feature_task, set by $command_utils:do_huh).";
    "  assumes that the :my_huh parsing has not suspended";
    return {task_id(), @args, argstr, dobj, dobjstr, prepstr, iobj, iobjstr} == this.feature_task;
  endverb
endobject