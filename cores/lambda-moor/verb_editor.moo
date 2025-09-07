object VERB_EDITOR
  name: "Verb Editor"
  parent: GENERIC_EDITOR
  owner: #96
  readable: true

  property objects (owner: #96, flags: "") = {};
  property verbnames (owner: #96, flags: "") = {};

  override aliases = {"Verb Editor", "vedit", "verbedit", "verb edit"};
  override blessed_task = 665095404;
  override change_msg = "You have changed the verb since last successful compile.";
  override commands = {{"e*dit", "<obj>:<verb>"}, {"com*pile", "[as <obj>:<verb>]"}};
  override commands2 = {
    {
      "say",
      "emote",
      "lis*t",
      "ins*ert",
      "n*ext,p*rev",
      "enter",
      "del*ete",
      "f*ind",
      "s*ubst",
      "m*ove,c*opy",
      "join*l",
      "fill"
    },
    {"y*ank", "w*hat", "e*dit", "com*pile", "abort", "q*uit,done,pause"}
  };
  override depart_msg = "You hear the bips of keyclick, the sliding of mice and the hum of computers in the distance as %n fades slowly out of view, heading towards them.";
  override entrances = {#5749};
  override help = {};
  override no_change_msg = "The verb has no pending changes.";
  override no_littering_msg = {
    "Keeping your verb for later work.  ",
    "To return, give the `@edit' command with no arguments.",
    "Please come back and COMPILE or ABORT if you don't intend to be working on this verb in the immediate future.  Keep Our MOO Clean!  No Littering!"
  };
  override no_text_msg = "Verb body is empty.";
  override nothing_loaded_msg = "First, you have to select a verb to edit with the EDIT command.";
  override object_size = {13962, 1084848672};
  override previous_session_msg = "You need to either COMPILE or ABORT this verb before you can start on another.";
  override return_msg = "There are the light bips of keyclick and the sliding of mice as %n fades into view, shoving %r away from the console, which promptly fades away.";
  override stateprops = {
    {"objects", 0},
    {"verbnames", 0},
    {"texts", 0},
    {"changes", 0},
    {"inserting", 1},
    {"readable", 0}
  };
  override who_location_msg = "%L [editing verbs]";

  verb "e*dit" (any none none) owner: #96 flags: "rd"
    if (!args)
      player:tell("edit what?");
    else
      this:invoke(argstr, verb);
    endif
  endverb

  verb "com*pile save" (none any any) owner: #96 flags: "rd"
    pas = {{}, {}};
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    elseif (!args)
      object = this.objects[who];
      vname = this.verbnames[who];
      if (typeof(vname) == LIST)
        vargs = listdelete(vname, 1);
        vname = vname[1];
      else
        vargs = {};
      endif
      changeverb = 0;
    elseif (args[1] != "as" || (length(args) < 2 || (!(spec = $code_utils:parse_verbref(args[2])) || (typeof(pas = $code_utils:parse_argspec(@args[3..$])) != LIST || pas[2]))))
      if (typeof(pas) != LIST)
        player:tell(pas);
      elseif (pas[2])
        player:tell("I don't understand \"", $string_utils:from_list(pas[2], " "), "\"");
      endif
      player:tell("Usage: ", verb, " [as <object>:<verb>]");
      return;
    elseif ($command_utils:object_match_failed(object = player:my_match_object(spec[1], this:get_room(player)), spec[1]))
      return;
    else
      vname = spec[2];
      vargs = pas[1] && {@pas[1], "none", "none"}[1..3];
      if (vargs)
        vargs[2] = $code_utils:full_prep(vargs[2]) || vargs[2];
      endif
      changeverb = 1;
    endif
    if (vargs)
      vnum = $code_utils:find_verb_named(object, vname);
      while (vnum && this:fetch_verb_args(object, vnum) != vargs)
        vnum = $code_utils:find_verb_named(object, vname, vnum + 1);
      endwhile
      if (!vnum)
        player:tell("There is no ", object, ":", vname, " verb with args (", $string_utils:from_list(vargs, " "), ").");
        if (!changeverb)
          player:tell("Use 'compile as ...' to write your code to another verb.");
        endif
        return;
      endif
      objverbname = tostr(object, ":", vname, " (", $string_utils:from_list(vargs, " "), ")");
    else
      vnum = 0;
      objverbname = tostr(object, ":", $code_utils:toint(vname) == E_TYPE ? vname | this:verb_name(object, vname));
    endif
    "...";
    "...perform eval_subs on verb code if necessary...";
    "...";
    if (player.eval_subs && player:edit_option("eval_subs"))
      verbcode = {};
      for x in (this:text(who))
        verbcode = {@verbcode, $code_utils:substitute(x, player.eval_subs)};
      endfor
    else
      verbcode = this:text(who);
    endif
    "...";
    "...write it out...";
    "...";
    if (result = this:set_verb_code(object, vnum ? vnum | vname, verbcode))
      player:tell(objverbname, " not compiled because:");
      for x in (result)
        player:tell("  ", x);
      endfor
    elseif (typeof(result) == ERR)
      player:tell({result, "You do not have write permission on " + objverbname + ".", "The verb " + objverbname + " does not exist (!?!)", "The object " + tostr(object) + " does not exist (!?!)"}[1 + (result in {E_PERM, E_VERBNF, E_INVARG})]);
      if (!changeverb)
        player:tell("Do 'compile as <object>:<verb>' to write your code to another verb.");
      endif
      changeverb = 0;
    else
      player:tell(objverbname, verbcode ? " successfully compiled." | " verbcode removed.");
      this:set_changed(who, 0);
    endif
    if (changeverb)
      this.objects[who] = object;
      this.verbnames[who] = vargs ? {vname, @vargs} | vname;
    endif
  endverb

  verb working_on (this none this) owner: #96 flags: "rxd"
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    else
      object = this.objects[who];
      verbname = this.verbnames[who];
      if (typeof(verbname) == LIST)
        return tostr(object, ":", verbname[1], " (", $string_utils:from_list(listdelete(verbname, 1), " "), ")");
      else
        return tostr(object, ":", this:verb_name(object, verbname), " (", this:verb_args(object, verbname), ")");
      endif
    endif
    "return this:ok(who = args[1]) && tostr(this.objects[who]) + \":\" + this.verbnames[who];";
  endverb

  verb init_session (this none this) owner: #96 flags: "rxd"
    {who, object, vname, vcode} = args;
    if (this:ok(who))
      this:load(who, vcode);
      this.verbnames[who] = vname;
      this.objects[who] = object;
      (this.active[who]):tell("Now editing ", this:working_on(who), ".");
      "this.active[who]:tell(\"Now editing \", object, \":\", vname, \".\");";
    endif
  endverb

  verb parse_invoke (this none this) owner: #96 flags: "rxd"
    ":parse_invoke(string,v,?code)";
    "  string is the commandline string to parse to obtain the obj:verb to edit";
    "  v is the actual command verb used to invoke the editor";
    " => {object, verbname, verb_code} or error";
    if (caller != this)
      raise(E_PERM);
    endif
    vref = $string_utils:words(args[1]);
    if (!vref || !(spec = $code_utils:parse_verbref(vref[1])))
      player:tell("Usage: ", args[2], " object:verb");
      return;
    endif
    if (argspec = listdelete(vref, 1))
      if (typeof(pas = $code_utils:parse_argspec(@argspec)) == LIST)
        if (pas[2])
          player:tell("I don't understand \"", $string_utils:from_list(pas[2], " "), "\"");
          return;
        endif
        argspec = {@pas[1], "none", "none"}[1..3];
        argspec[2] = $code_utils:full_prep(argspec[2]) || argspec[2];
      else
        player:tell(pas);
        return;
      endif
    endif
    if (!$command_utils:object_match_failed(object = player:my_match_object(spec[1], this:get_room(player)), spec[1]))
      vnum = $code_utils:find_verb_named(object, vname = spec[2]);
      if (argspec)
        while (vnum && this:fetch_verb_args(object, vnum) != argspec)
          vnum = $code_utils:find_verb_named(object, vname, vnum + 1);
        endwhile
      endif
      if (length(args) > 2)
        code = args[3];
      elseif (vnum)
        code = this:fetch_verb_code(object, vnum);
      else
        code = E_VERBNF;
      endif
      if (typeof(code) == ERR)
        player:tell(code != E_VERBNF ? code | "That object does not define that verb", argspec ? " with those args." | ".");
        return code;
      else
        return {object, argspec ? {vname, @argspec} | vname, code};
      endif
    endif
    return 0;
  endverb

  verb fetch_verb_code (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (caller != $verb_editor || caller_perms() != $verb_editor.owner)
      return E_PERM;
    else
      set_task_perms(player);
      return `verb_code(args[1], args[2], !player:edit_option("no_parens")) ! ANY';
    endif
  endverb

  verb set_verb_code (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (caller != $verb_editor || caller_perms() != $verb_editor.owner)
      return E_PERM;
    else
      set_task_perms(player);
      return `set_verb_code(args[1], args[2], args[3]) ! ANY';
    endif
  endverb

  verb local_editing_info (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == $verb_editor)
      set_task_perms(player);
    endif
    {object, vname, code} = args;
    if (typeof(vname) == LIST)
      if (vname[3] != "none")
        vname[3] = $code_utils:short_prep(vname[3]);
      endif
      vargs = tostr(" ", vname[2], " ", vname[3], " ", vname[4]);
      vname = vname[1];
    else
      vargs = "";
    endif
    name = tostr(object.name, ":", vname);
    "... so the next 2 lines are actually wrong, since verb_info won't";
    "... necessarily retrieve the correct verb if we have more than one";
    "... matching the given same name; anyway, if parse_invoke understood vname,";
    "... so will @program.  I suspect these were put here because in the";
    "... old scheme of things, vname was always a number.";
    "vname = strsub($string_utils:explode(verb_info(object, vname)[3])[1], \"*\", \"\")";
    "vargs = verb_args(object, vname)";
    "";
    return {name, code, tostr("@program ", object, ":", vname, vargs)};
  endverb

  verb verb_name (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "verb_name(object, vname)";
    "Find vname on object and return its full name (quoted).";
    "This is useful for when we're working with verb numbers.";
    if (caller != $verb_editor || caller_perms() != $verb_editor.owner)
      return E_PERM;
    else
      set_task_perms(player);
      given = args[2];
      if (typeof(info = `verb_info(args[1], given) ! ANY') == ERR)
        return tostr(given, "[", info, "]");
      elseif (info[3] == given)
        return given;
      else
        return tostr(given, "/\"", info[3], "\"");
      endif
    endif
  endverb

  verb verb_args (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "verb_name(object, vname)";
    "Find vname on object and return its full name (quoted).";
    "This is useful for when we're working with verb numbers.";
    if (caller != $verb_editor || caller_perms() != $verb_editor.owner)
      return E_PERM;
    else
      set_task_perms(player);
      return $string_utils:from_list(`verb_args(args[1], args[2]) ! ANY', " ");
    endif
  endverb

  verb comment (any any any) owner: #96 flags: "rd"
    "Syntax: comment [<range>]";
    "";
    "Turns the specified range of lines, into comments.";
    if (caller != player && caller_perms() != player)
      return E_PERM;
    elseif (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"."}, @args)) != LIST)
      player:tell(tostr(range));
    elseif (range[3])
      player:tell_lines($code_utils:verb_documentation());
    else
      text = this.texts[who];
      {from, to, crap} = range;
      cut = $maxint;
      for line in [from..to]
        cut = min(cut, `match(text[line], "[^ ]")[1] ! E_RANGE => 1');
      endfor
      for line in [from..to]
        text[line] = toliteral((text[line])[cut..$]) + ";";
      endfor
      this.texts[who] = text;
      player:tell(to == from ? "Line" | "Lines", " changed.");
      this.changes[who] = 1;
      this.times[who] = time();
    endif
  endverb

  verb uncomment (any any any) owner: #96 flags: "rd"
    "Syntax: uncomment [<range>]";
    "";
    "Turns the specified range of lines from comments to, uh, not comments.";
    if (caller != player && caller_perms() != player)
      return E_PERM;
    elseif (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"."}, @args)) != LIST)
      player:tell(tostr(range));
    elseif (range[3])
      player:tell_lines($code_utils:verb_documentation());
    else
      text = this.texts[who];
      {from, to, crap} = range;
      bogus = {};
      for line in [from..to]
        if (match(text[line], "^ *\"%([^\\\"]%|\\.%)*\";$"))
          "check from $code_utils:verb_documentation";
          if (!bogus)
            text[line] = $no_one:eval(text[line])[2];
          endif
        else
          bogus = setadd(bogus, line);
        endif
      endfor
      if (bogus)
        player:tell(length(bogus) == 1 ? "Line" | "Lines", " ", $string_utils:english_list(bogus), " ", length(bogus) == 1 ? "is" | "are", " not comments.");
        player:tell("No changes.");
        return;
      endif
      this.texts[who] = text;
      player:tell(to == from ? "Line" | "Lines", " changed.");
      this.changes[who] = 1;
      this.times[who] = time();
    endif
  endverb

  verb fetch_verb_args (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (caller != $verb_editor || caller_perms() != $verb_editor.owner)
      raise(E_PERM);
    else
      set_task_perms(player);
      return `verb_args(args[1], args[2]) ! ANY';
    endif
  endverb
endobject