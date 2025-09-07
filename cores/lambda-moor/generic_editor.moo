object GENERIC_EDITOR
  name: "Generic Editor"
  parent: ROOM
  owner: #96
  fertile: true
  readable: true

  property active (owner: #96, flags: "r") = {};
  property change_msg (owner: #96, flags: "rc") = "Text has been altered since the last save.";
  property changes (owner: #96, flags: "") = {};
  property commands (owner: #96, flags: "rc") = {
    {"say", "<text>"},
    {"emote", "<text>"},
    {"lis*t", "[<range>] [nonum]"},
    {"ins*ert", "[<ins>] [\"<text>]"},
    {"n*ext,p*rev", "[n] [\"<text>]"},
    {"del*ete", "[<range>]"},
    {"f*ind", "/<str>[/[c][<range>]]"},
    {"s*ubst", "/<str1>/<str2>[/[g][c][r][<range>]]"},
    {"m*ove,c*opy", "[<range>] to <ins>"},
    {"join*l", "[<range>]"},
    {"fill", "[<range>] [@<col>]"},
    {"w*hat", ""},
    {"abort", ""},
    {"q*uit,done,pause", ""},
    {"enter", ""},
    {"y*ank", "from <text-source>"}
  };
  property commands2 (owner: #96, flags: "rc") = {
    {
      "say",
      "emote",
      "lis*t",
      "ins*ert",
      "n*ext,p*rev",
      "del*ete",
      "f*ind",
      "s*ubst",
      "m*ove,c*opy",
      "join*l",
      "fill"
    },
    {"y*ank", "w*hat", "abort", "q*uit,done,pause"}
  };
  property depart_msg (owner: #96, flags: "rc") = "%N heads off to the Generic Editing Room.";
  property exit_on_abort (owner: #96, flags: "rc") = 0;
  property help (owner: #96, flags: "rc") = EDITOR_HELP;
  property inserting (owner: #96, flags: "") = {};
  property invoke_task (owner: #96, flags: "r") = 0;
  property no_change_msg (owner: #96, flags: "rc") = "There have been no changes since the last save.";
  property no_littering_msg (owner: #96, flags: "rc") = "Keeping your [whatever] for later work.  Since this the Generic Editor, you have to do your own :set_changed(0) so that we'll know to get rid of whatever it you're working on when you leave.  Please don't litter... especially in the Generic Editor.";
  property no_text_msg (owner: #96, flags: "rc") = "There are no lines of text.";
  property nothing_loaded_msg (owner: #96, flags: "rc") = "You're not currently editing anything.";
  property original (owner: #96, flags: "r") = {};
  property previous_session_msg (owner: #96, flags: "rc") = "";
  property readable (owner: #96, flags: "r") = {};
  property return_msg (owner: #96, flags: "rc") = "%N comes back from the Generic Editing Room.";
  property stateprops (owner: #96, flags: "r") = {{"texts", 0}, {"changes", 0}, {"inserting", 1}, {"readable", 0}};
  property texts (owner: #96, flags: "") = {};
  property times (owner: #96, flags: "r") = {};

  override aliases = {"Generic Editor", "gedit", "edit"};
  override blessed_task = 1399008566;
  override description = {};
  override entrances = {#5751};
  override object_size = {51968, 1084848672};
  override who_location_msg = "%L [editing]";

  verb say (any any any) owner: #96 flags: "rxd"
    if (caller != player && caller_perms() != player)
      return E_PERM;
    endif
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    else
      this:insert_line(who, argstr);
    endif
  endverb

  verb emote (any any any) owner: #96 flags: "rxd"
    if (caller != player && caller_perms() != player)
      return E_PERM;
    endif
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    else
      this:append_line(who, argstr);
    endif
  endverb

  verb enter (any none none) owner: #96 flags: "rd"
    if (!this:loaded(player))
      player:tell(this:nothing_loaded_msg());
    else
      lines = $command_utils:read_lines();
      if (typeof(lines) == ERR)
        player:notify(tostr(lines));
        return;
      endif
      this:insert_line(this:loaded(player), lines, 0);
    endif
  endverb

  verb "lis*t view" (any any any) owner: #96 flags: "rd"
    nonum = 0;
    if (verb == "view")
      if (!args)
        l = {};
        for i in [1..length(this.active)]
          if (this.readable[i])
            l = {@l, this.active[i]};
          endif
        endfor
        if (l)
          player:tell("Players having readable texts in this editor:  ", $string_utils:names_of(l));
        else
          player:tell("No one has published anything in this editor.");
        endif
        return;
      elseif ($command_utils:player_match_result(plyr = $string_utils:match_player(args[1]), args[1])[1])
        "...no such player";
        return;
      elseif (!(who = this:loaded(plyr)) || !this:readable(who))
        player:tell(plyr.name, "(", plyr, ") has not published anything in this editor.");
        return;
      endif
      args = listdelete(args, 1);
    elseif (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    len = length(this.texts[who]);
    ins = this.inserting[who];
    window = 8;
    if (len < 2 * window)
      default = {"1-$"};
    elseif (ins <= window)
      default = {tostr("1-", 2 * window)};
    else
      default = {tostr(window, "_-", window, "^"), tostr(2 * window, "$-$")};
    endif
    if (typeof(range = this:parse_range(who, default, @args)) != LIST)
      player:tell(tostr(range));
    elseif (range[3] && !(nonum = "nonum" == $string_utils:trim(range[3])))
      player:tell("Don't understand this:  ", range[3]);
    elseif (nonum)
      player:tell_lines((this.texts[who])[range[1]..range[2]]);
    else
      for line in [range[1]..range[2]]
        this:list_line(who, line);
        if ($command_utils:running_out_of_time())
          suspend(0);
          if (!(who = this:loaded(player)))
            player:tell("ack!  something bad happened during a suspend...");
            return;
          endif
        endif
      endfor
      if (ins > len && len == range[2])
        player:tell("^^^^");
      endif
    endif
  endverb

  verb "ins*ert n*ext p*revious ." (any none none) owner: #96 flags: "rd"
    if (i = index(argstr, "\""))
      text = argstr[i + 1..$];
      argstr = argstr[1..i - 1];
    else
      text = 0;
    endif
    spec = $string_utils:trim(argstr);
    if (index("next", verb) == 1)
      verb = "next";
      spec = "+" + (spec || "1");
    elseif (index("prev", verb) == 1)
      verb = "prev";
      spec = "-" + (spec || "1");
    else
      spec = spec || ".";
    endif
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (ERR == typeof(number = this:parse_insert(who, spec)))
      if (verb in {"next", "prev"})
        player:tell("Argument must be a number.");
      else
        player:tell("You must specify an integer or `$' for the last line.");
      endif
    elseif (number > (max = length(this.texts[who]) + 1) || number < 1)
      player:tell("That would take you out of range (to line ", number, "?).");
    else
      this.inserting[who] = number;
      if (typeof(text) == STR)
        this:insert_line(who, text);
      else
        if (verb != "next")
          number > 1 ? this:list_line(who, number - 1) | player:tell("____");
        endif
        if (verb != "prev")
          number < max ? this:list_line(who, number) | player:tell("^^^^");
        endif
      endif
    endif
  endverb

  verb "del*ete" (any any any) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"_", "1"}, @args)) != LIST)
      player:tell(range);
    elseif (range[3])
      player:tell("Junk at end of cmd:  ", range[3]);
    else
      player:tell_lines((text = this.texts[who])[from = range[1]..to = range[2]]);
      player:tell("---Line", to > from ? "s" | "", " deleted.  Insertion point is before line ", from, ".");
      this.texts[who] = {@text[1..from - 1], @text[to + 1..$]};
      if (!(this.changes[who]))
        this.changes[who] = 1;
        this.times[who] = time();
      endif
      this.inserting[who] = from;
    endif
  endverb

  verb "f*ind" (any any any) owner: #96 flags: "rxd"
    if (callers() && caller != this)
      return E_PERM;
    endif
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(subst = this:parse_subst(argstr && argstr[1] + argstr, "c", "Empty search string?")) != LIST)
      player:tell(tostr(subst));
    elseif (typeof(start = subst[4] ? this:parse_insert(who, subst[4]) | this.inserting[who]) == ERR)
      player:tell("Starting from where?", subst[4] ? "  (can't parse " + subst[4] + ")" | "");
    else
      search = subst[2];
      case = !index(subst[3], "c", 1);
      text = this.texts[who];
      tlen = length(text);
      while (start <= tlen && !index(text[start], search, case))
        start = start + 1;
      endwhile
      if (start > tlen)
        player:tell("`", search, "' not found.");
      else
        this.inserting[who] = start + 1;
        this:list_line(who, start);
      endif
    endif
  endverb

  verb "m*ove c*opy" (any any any) owner: #96 flags: "rd"
    verb = (is_move = verb[1] == "m") ? "move" | "copy";
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    wargs = args;
    t = to_pos = 0;
    while (t = "to" in (wargs = wargs[t + 1..$]))
      to_pos = to_pos + t;
    endwhile
    range_args = args[1..to_pos - 1];
    if (!to_pos || ERR == typeof(dest = this:parse_insert(who, $string_utils:from_list(wargs, " "))))
      player:tell(verb, " to where? ");
    elseif (dest < 1 || dest > (last = length(this.texts[who])) + 1)
      player:tell("Destination (", dest, ") out of range.");
    elseif ("from" in range_args || "to" in range_args)
      player:tell("Don't use that kind of range specification with this command.");
    elseif (typeof(range = this:parse_range(who, {"_", "^"}, @args[1..to_pos - 1])) != LIST)
      player:tell(range);
    elseif (range[3])
      player:tell("Junk before `to':  ", range[3]);
    elseif (is_move && dest >= range[1] && dest <= range[2] + 1)
      player:tell("Destination lies inside range of lines to be moved.");
    else
      from = range[1];
      to = range[2];
      ins = this.inserting[who];
      text = this.texts[who];
      if (!is_move)
        this.texts[who] = {@text[1..dest - 1], @text[from..to], @text[dest..last]};
        if (ins >= dest)
          this.inserting[who] = ins + to - from + 1;
        endif
      else
        "oh shit... it's a move";
        if (dest < from)
          newtext = {@text[1..dest - 1], @text[from..to], @text[dest..from - 1], @text[to + 1..last]};
          if (ins >= dest && ins <= to)
            ins = ins > from ? ins - from + dest | ins + to - from + 1;
          endif
        else
          newtext = {@text[1..from - 1], @text[to + 1..dest - 1], @text[from..to], @text[dest..last]};
          if (ins > from && ins < dest)
            ins = ins <= to ? ins + dest - to - 1 | ins - to + from - 1;
          endif
        endif
        this.texts[who] = newtext;
        this.inserting[who] = ins;
      endif
      if (!(this.changes[who]))
        this.changes[who] = 1;
        this.times[who] = time();
      endif
      player:tell("Lines ", is_move ? "moved." | "copied.");
    endif
  endverb

  verb "join*literal" (any any any) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"_-^", "_", "^"}, @args)) != LIST)
      player:tell(range);
    elseif (range[3])
      player:tell("Junk at end of cmd:  ", range[3]);
    elseif (!(result = this:join_lines(who, @range[1..2], length(verb) <= 4)))
      player:tell(result == 0 ? "Need at least two lines to join." | result);
    else
      this:list_line(who, range[1]);
    endif
  endverb

  verb fill (any any any) owner: #96 flags: "rd"
    fill_column = 70;
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"_", "1"}, @args)) != LIST)
      player:tell(range);
    elseif (range[3] && (range[3][1] != "@" || (fill_column = toint((range[3])[2..$])) < 10))
      player:tell("Usage:  fill [<range>] [@ column]   (where column >= 10).");
    else
      join = this:join_lines(who, @range[1..2], 1);
      newlines = this:fill_string((text = this.texts[who])[from = range[1]], fill_column);
      if (fill = (nlen = length(newlines)) > 1 || newlines[1] != text[from])
        this.texts[who] = {@text[1..from - 1], @newlines, @text[from + 1..$]};
        if ((insert = this.inserting[who]) > from && nlen > 1)
          this.inserting[who] = insert + nlen - 1;
        endif
      endif
      if (fill || join)
        for line in [from..from + nlen - 1]
          this:list_line(who, line);
        endfor
      else
        player:tell("No changes.");
      endif
    endif
  endverb

  verb "pub*lish perish unpub*lish depub*lish" (none none none) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    if (typeof(e = this:set_readable(who, index("publish", verb) == 1)) == ERR)
      player:tell(e);
    elseif (e)
      player:tell("Your text is now globally readable.");
    else
      player:tell("Your text is read protected.");
    endif
  endverb

  verb "w*hat" (none none none) owner: #96 flags: "rxd"
    if (!(this:ok(who = player in this.active) && typeof(this.texts[who]) == LIST))
      player:tell(this:nothing_loaded_msg());
    else
      player:tell("You are editing ", this:working_on(who), ".");
      player:tell("Your insertion point is ", this.inserting[who] > length(this.texts[who]) ? "after the last line: next line will be #" | "before line ", this.inserting[who], ".");
      player:tell(this.changes[who] ? this:change_msg() | this:no_change_msg());
      if (this.readable[who])
        player:tell("Your text is globally readable.");
      endif
    endif
  endverb

  verb abort (none none none) owner: #96 flags: "rd"
    if (!(this.changes[who = player in this.active]))
      player:tell("No changes to throw away.  Editor cleared.");
    else
      player:tell("Throwing away session for ", this:working_on(who), ".");
    endif
    this:reset_session(who);
    if (this.exit_on_abort)
      this:done();
    endif
  endverb

  verb "done q*uit pause" (none none none) owner: #96 flags: "rxd"
    if (!(caller in {this, player}))
      return E_PERM;
    elseif (!(who = player in this.active))
      player:tell("You are not actually in ", this.name, ".");
      return;
    elseif (!valid(origin = this.original[who]))
      player:tell("I don't know where you came here from.");
    else
      player:moveto(origin);
      if (player.location == this)
        player:tell("Hmmm... the place you came from doesn't want you back.");
      else
        if (msg = this:return_msg())
          player.location:announce($string_utils:pronoun_sub(msg));
        endif
        return;
      endif
    endif
    player:tell("You'll have to use 'home' or a teleporter.");
  endverb

  verb huh2 (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This catches subst and find commands that don't fit into the usual model, e.g., s/.../.../ without the space after the s, and find commands without the verb `find'.  Still behaves in annoying ways (e.g., loses if the search string contains multiple whitespace), but better than before.";
    set_task_perms(caller_perms());
    if ((c = callers()) && (c[1][1] != this || length(c) > 1))
      return pass(@args);
    endif
    verb = args[1];
    v = 1;
    vmax = min(length(verb), 5);
    while (v <= vmax && verb[v] == "subst"[v])
      v = v + 1;
    endwhile
    argstr = $code_utils:argstr(verb, args[2]);
    if (v > 1 && v <= length(verb) && ((vl = verb[v]) < "A" || vl > "Z"))
      argstr = verb[v..$] + (argstr && " ") + argstr;
      return this:subst();
    elseif ("/" == verb[1])
      argstr = verb + (argstr && " ") + argstr;
      return this:find();
    else
      pass(@args);
    endif
  endverb

  verb insertion (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && this.inserting[who];
  endverb

  verb set_insertion (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && (((ins = toint(args[2])) < 1 ? E_INVARG | ins <= (max = length(this.texts[who]) + 1) || (ins = max)) && (this.inserting[who] = ins));
  endverb

  verb "changed retain_session_on_exit" (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && this.changes[who];
  endverb

  verb set_changed (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && (((unchanged = !(args[2])) || (this.times[who] = time())) && (this.changes[who] = !unchanged));
  endverb

  verb origin (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && this.original[who];
  endverb

  verb set_origin (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && ((valid(origin = args[2]) && origin != this || (origin == $nothing || E_INVARG)) && (this.original[who] = origin));
  endverb

  verb readable (this none this) owner: #96 flags: "rxd"
    return (who = args[1]) < 1 || who > length(this.active) ? E_RANGE | this.readable[who];
  endverb

  verb set_readable (this none this) owner: #96 flags: "rxd"
    return this:ok(who = args[1]) && (this.readable[who] = !!(args[2]));
  endverb

  verb text (this none this) owner: #96 flags: "rxd"
    {?who = player in this.active} = args;
    return (this:readable(who) || this:ok(who)) && this.texts[who];
  endverb

  verb load (this none this) owner: #96 flags: "rxd"
    texts = args[2];
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    elseif (typeof(texts) == STR)
      texts = {texts};
    elseif (typeof(texts) != LIST || length(texts) && typeof(texts[1]) != STR)
      return E_TYPE;
    endif
    this.texts[who] = texts;
    this.inserting[who] = length(texts) + 1;
    this.changes[who] = 0;
    this.readable[who] = 0;
    this.times[who] = time();
  endverb

  verb working_on (this none this) owner: #96 flags: "rxd"
    "Dummy routine.  The child editor should provide something informative";
    return this:ok(who = args[1]) && "something [in " + this.name + "]";
  endverb

  verb ok (this none this) owner: #96 flags: "rxd"
    who = args[1];
    if (who < 1 || who > length(this.active))
      return E_RANGE;
    elseif (length(c = callers()) < 2 ? player == this.active[who] | c[2][1] == this || ($perm_utils:controls(c[2][3], this.active[who]) || c[2][3] == $generic_editor.owner))
      return 1;
    else
      return E_PERM;
    endif
  endverb

  verb loaded (this none this) owner: #96 flags: "rxd"
    return (who = args[1] in this.active) && typeof(this.texts[who]) == LIST && who;
  endverb

  verb list_line (this none this) owner: #96 flags: "rxd"
    if (this:ok(who = args[1]))
      f = 1 + ((line = args[2]) in {(ins = this.inserting[who]) - 1, ins});
      player:tell($string_utils:right(line, 3, " _^"[f]), ":_^"[f], " ", this.texts[who][line]);
    endif
  endverb

  verb insert_line (this none this) owner: #96 flags: "rxd"
    ":insert_line([who,] line or list of lines [,quiet])";
    "  inserts the given text at the insertion point.";
    "  returns E_NONE if the session has no text loaded yet.";
    if (typeof(args[1]) != INT)
      args = {player in this.active, @args};
    endif
    {who, lines, ?quiet = (this.active[who]):edit_option("quiet_insert")} = args;
    if (!(fuckup = this:ok(who)))
      return fuckup;
    elseif (typeof(text = this.texts[who]) != LIST)
      return E_NONE;
    else
      if (typeof(lines) != LIST)
        lines = {lines};
      endif
      p = this.active[who];
      insert = this.inserting[who];
      this.texts[who] = {@text[1..insert - 1], @lines, @text[insert..$]};
      this.inserting[who] = insert + length(lines);
      if (lines)
        if (!(this.changes[who]))
          this.changes[who] = 1;
          this.times[who] = time();
        endif
        if (!quiet)
          if (length(lines) != 1)
            p:tell("Lines ", insert, "-", insert + length(lines) - 1, " added.");
          else
            p:tell("Line ", insert, " added.");
          endif
        endif
      else
        p:tell("No lines added.");
      endif
    endif
  endverb

  verb append_line (this none this) owner: #96 flags: "rxd"
    ":append_line([who,] string)";
    "  appends the given string to the line before the insertion point.";
    "  returns E_NONE if the session has no text loaded yet.";
    if (typeof(args[1]) != INT)
      args = {player in this.active, @args};
    endif
    {who, string} = args;
    if (!(fuckup = this:ok(who)))
      return fuckup;
    elseif ((append = this.inserting[who] - 1) < 1)
      return this:insert_line(who, {string});
    elseif (typeof(text = this.texts[who]) != LIST)
      return E_NONE;
    else
      this.texts[who][append] = text[append] + string;
      if (!(this.changes[who]))
        this.changes[who] = 1;
        this.times[who] = time();
      endif
      p = this.active[who];
      if (!p:edit_option("quiet_insert"))
        p:tell("Appended to line ", append, ".");
      endif
    endif
  endverb

  verb join_lines (this none this) owner: #96 flags: "rxd"
    {who, from, to, english} = args;
    if (!(fuckup = this:ok(who)))
      return fuckup;
    elseif (from >= to)
      return 0;
    else
      nline = "";
      for line in ((text = this.texts[who])[from..to])
        if (!english)
          nline = nline + line;
        else
          len = length(line) + 1;
          while ((len = len - 1) && line[len] == " ")
          endwhile
          if (len > 0)
            nline = nline + line + (index(".:", line[len]) ? "  " | " ");
          endif
        endif
      endfor
      this.texts[who] = {@text[1..from - 1], nline, @text[to + 1..$]};
      if ((insert = this.inserting[who]) > from)
        this.inserting[who] = insert <= to ? from + 1 | insert - to + from;
      endif
      if (!(this.changes[who]))
        this.changes[who] = 1;
        this.times[who] = time();
      endif
      return to - from;
    endif
  endverb

  verb parse_number (this none this) owner: #96 flags: "rxd"
    "parse_number(who,string,before)   interprets string as a line number.  In the event that string is `.', `before' tells us which line to use.  Return 0 if string is bogus.";
    {who, string, before} = args;
    if (!(fuckup = this:ok(who)))
      return fuckup;
    endif
    last = length(this.texts[who]);
    ins = this.inserting[who] - 1;
    after = !before;
    if (!string)
      return 0;
    elseif ("." == string)
      return ins + after;
    elseif (!(i = index("_^$", string[slen = length(string)])))
      return toint(string);
    else
      start = {ins + 1, ins, last + 1}[i];
      n = 1;
      if (slen > 1 && !(n = toint(string[1..slen - 1])))
        return 0;
      elseif (i % 2)
        return start - n;
      else
        return start + n;
      endif
    endif
  endverb

  verb parse_range (this none this) owner: #96 flags: "rxd"
    "parse_range(who,default,@args) => {from to rest}";
    numargs = length(args);
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    elseif (!(last = length(this.texts[who])))
      return this:no_text_msg();
    endif
    default = args[2];
    r = 0;
    while (default && LIST != typeof(r = this:parse_range(who, {}, default[1])))
      default = listdelete(default, 1);
    endwhile
    if (typeof(r) == LIST)
      from = r[1];
      to = r[2];
    else
      from = to = 0;
    endif
    saw_from_to = 0;
    not_done = 1;
    a = 2;
    while ((a = a + 1) <= numargs && not_done)
      if (args[a] == "from")
        if (a == numargs || !(from = this:parse_number(who, args[a = a + 1], 0)))
          return "from ?";
        endif
        saw_from_to = 1;
      elseif (args[a] == "to")
        if (a == numargs || !(to = this:parse_number(who, args[a = a + 1], 1)))
          return "to ?";
        endif
        saw_from_to = 1;
      elseif (saw_from_to)
        a = a - 1;
        not_done = 0;
      elseif (i = index(args[a], "-"))
        from = this:parse_number(who, (args[a])[1..i - 1], 0);
        to = this:parse_number(who, (args[a])[i + 1..$], 1);
        not_done = 0;
      elseif (f = this:parse_number(who, args[a], 0))
        from = f;
        if (a == numargs || !(to = this:parse_number(who, args[a + 1], 1)))
          to = from;
        else
          a = a + 1;
        endif
        not_done = 0;
      else
        a = a - 1;
        not_done = 0;
      endif
    endwhile
    if (from < 1)
      return tostr("from ", from, "?  (out of range)");
    elseif (to > last)
      return tostr("to ", to, "?  (out of range)");
    elseif (from > to)
      return tostr("from ", from, " to ", to, "?  (backwards range)");
    else
      return {from, to, $string_utils:from_list(args[a..numargs], " ")};
    endif
  endverb

  verb parse_insert (this none this) owner: #96 flags: "rxd"
    "parse_ins(who,string)  interprets string as an insertion point, i.e., a position between lines and returns the number of the following line or 0 if bogus.";
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    endif
    {who, string} = args;
    if (length(string) == 0)
      return E_INVARG;
    endif
    last = length(this.texts[who]) + 1;
    ins = this.inserting[who];
    if (i = index("-+", string[1]))
      rest = string[2..$];
      return (n = toint(rest)) || rest == "0" ? {ins - n, ins + n}[i] | E_INVARG;
    else
      if (!(j = index(string, "^") || index(string, "_")))
        offset = 0;
      else
        offset = j == 1 || toint(string[1..j - 1]);
        if (!offset)
          return E_INVARG;
        elseif (string[j] == "^")
          offset = -offset;
        endif
      endif
      rest = string[j + 1..$];
      if (i = rest in {".", "$"})
        return offset + {ins, last}[i];
      elseif (!(n = toint(rest)))
        return E_INVARG;
      else
        return offset + (j && string[j] == "^") + n;
      endif
    endif
  endverb

  verb parse_subst (this none this) owner: #96 flags: "rxd"
    {cmd, ?recognized_flags = "gcr", ?null_subst_msg = "Null substitution?"} = args;
    if (!cmd)
      return "s*ubst/<str1>/<str2>[/[g][c][r][<range>]] expected...";
    endif
    bchar = cmd[1];
    cmd = cmd[2..$];
    fromstr = cmd[1..(b2 = index(cmd + bchar, bchar, 1)) - 1];
    cmd = cmd[b2 + 1..$];
    tostr = cmd[1..(b2 = index(cmd + bchar, bchar, 1)) - 1];
    cmd = cmd[b2 + 1..$];
    cmdlen = length(cmd);
    b2 = 0;
    while ((b2 = b2 + 1) <= cmdlen && index(recognized_flags, cmd[b2]))
    endwhile
    return fromstr == "" && tostr == "" ? null_subst_msg | {fromstr, tostr, cmd[1..b2 - 1], cmd[b2..$]};
  endverb

  verb invoke (this none this) owner: #96 flags: "rxd"
    ":invoke(...)";
    "to find out what arguments this verb expects,";
    "see this editor's parse_invoke verb.";
    new = args[1];
    if (!(caller in {this, player}) && !$perm_utils:controls(caller_perms(), player))
      "...non-editor/non-player verb trying to send someone to the editor...";
      return E_PERM;
    endif
    if ((who = this:loaded(player)) && this:changed(who))
      if (!new)
        if (this:suck_in(player))
          player:tell("You are working on ", this:working_on(who));
        endif
        return;
      elseif (player.location == this)
        player:tell("You are still working on ", this:working_on(who));
        if (msg = this:previous_session_msg())
          player:tell(msg);
        endif
        return;
      endif
      "... we're not in the editor and we're about to start something new,";
      "... but there's still this pending session...";
      player:tell("You were working on ", this:working_on(who));
      if (!$command_utils:yes_or_no("Do you wish to delete that session?"))
        if (this:suck_in(player))
          player:tell("Continuing with ", this:working_on(player in this.active));
          if (msg = this:previous_session_msg())
            player:tell(msg);
          endif
        endif
        return;
      endif
      "... note session number may have changed => don't trust `who'";
      this:kill_session(player in this.active);
    endif
    spec = this:parse_invoke(@args);
    if (typeof(spec) == LIST)
      if (player:edit_option("local") && $object_utils:has_verb(this, "local_editing_info") && (info = this:local_editing_info(@spec)))
        this:invoke_local_editor(@info);
      elseif (this:suck_in(player))
        this:init_session(player in this.active, @spec);
      endif
    endif
  endverb

  verb suck_in (this none this) owner: #96 flags: "rxd"
    "The correct way to move someone into the editor.";
    if ((loc = (who_obj = args[1]).location) != this && caller == this)
      this.invoke_task = task_id();
      who_obj:moveto(this);
      if (who_obj.location == this)
        try
          "...forked, just in case loc:announce is broken...";
          "changed to a try-endtry. Lets reduce tasks..Ho_Yan 12/20/96";
          if (valid(loc) && (msg = this:depart_msg()))
            loc:announce($string_utils:pronoun_sub(msg));
          endif
        except (ANY)
          "Just drop it and move on";
        endtry
      else
        who_obj:tell("For some reason, I can't move you.   (?)");
        this:exitfunc(who_obj);
      endif
      this.invoke_task = 0;
    endif
    return who_obj.location == this;
  endverb

  verb new_session (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    {who_obj, from} = args;
    if ($object_utils:isa(from, $generic_editor))
      "... never put an editor in .original, ...";
      if (w = who_obj in from.active)
        from = from.original[w];
      else
        from = #-1;
      endif
    endif
    if (caller != this)
      return E_PERM;
    elseif (who = who_obj in this.active)
      "... edit in progress here...";
      if (valid(from))
        this.original[who] = from;
      endif
      return -1;
    else
      for p in ({{"active", who_obj}, {"original", valid(from) ? from | $nothing}, {"times", time()}, @this.stateprops})
        this.((p[1])) = {@this.((p[1])), p[2]};
      endfor
      return length(this.active);
    endif
  endverb

  verb kill_session (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    else
      for p in ({@this.stateprops, {"original"}, {"active"}, {"times"}})
        this.((p[1])) = listdelete(this.((p[1])), who);
      endfor
      return who;
    endif
  endverb

  verb reset_session (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    else
      for p in (this.stateprops)
        this.((p[1]))[who] = p[2];
      endfor
      this.times[who] = time();
      return who;
    endif
  endverb

  verb kill_all_sessions (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (caller != this && !caller_perms().wizard)
      return E_PERM;
    else
      for victim in (this.contents)
        victim:tell("Sorry, ", this.name, " is going down.  Your editing session is hosed.");
        victim:moveto((who = victim in this.active) && valid(origin = this.original[who]) ? origin | (valid(victim.home) ? victim.home | $player_start));
      endfor
      for p in ({@this.stateprops, {"original"}, {"active"}, {"times"}})
        this.((p[1])) = {};
      endfor
      return 1;
    endif
  endverb

  verb acceptable (this none this) owner: #96 flags: "rxd"
    return is_player(who_obj = args[1]) && (who_obj.wizard || pass(@args));
  endverb

  verb enterfunc (this none this) owner: #96 flags: "rxd"
    who_obj = args[1];
    if (who_obj.wizard && !(who_obj in this.active))
      this:accept(who_obj);
    endif
    pass(@args);
    if (this.invoke_task == task_id())
      "Means we're about to load something, so be quiet.";
      this.invoke_task = 0;
    elseif (who = this:loaded(who_obj))
      who_obj:tell("You are working on ", this:working_on(who), ".");
    elseif (msg = this:nothing_loaded_msg())
      who_obj:tell(msg);
    endif
  endverb

  verb exitfunc (this none this) owner: #96 flags: "rxd"
    if (!(who = (who_obj = args[1]) in this.active))
    elseif (this:retain_session_on_exit(who))
      if (msg = this:no_littering_msg())
        who_obj:tell_lines(msg);
      endif
    else
      this:kill_session(who);
    endif
    pass(@args);
  endverb

  verb "@flush" (this any any) owner: #96 flags: "rxd"
    "@flush <editor>";
    "@flush <editor> at <month> <day>";
    "@flush <editor> at <weekday>";
    "The first form removes all sessions from the editor; the other two forms remove everything older than the given date.";
    if (caller_perms() != #-1 && caller_perms() != player)
      raise(E_PERM);
    elseif (!$perm_utils:controls(player, this))
      player:tell("Only the owner of the editor can do a ", verb, ".");
      return;
    endif
    if (!prepstr)
      player:tell("Trashing all sessions.");
      this:kill_all_sessions();
    elseif (prepstr != "at")
      player:tell("Usage:  ", verb, " ", dobjstr, " [at [mon day|weekday]]");
    else
      p = prepstr in args;
      if (t = $time_utils:from_day(iobjstr, -1))
      elseif (t = $time_utils:from_month(args[p + 1], -1))
        if (length(args) > p + 1)
          if (!(n = toint(args[p + 2])))
            player:tell(args[p + 1], " WHAT?");
            return;
          endif
          t = t + (n - 1) * 86400;
        endif
      else
        player:tell("couldn't parse date");
        return;
      endif
      this:do_flush(t, "noisy");
    endif
    player:tell("Done.");
  endverb

  verb "@stateprop" (any for this) owner: #96 flags: "rd"
    if (!$perm_utils:controls(player, this))
      player:tell(E_PERM);
      return;
    endif
    if (i = index(dobjstr, "="))
      default = dobjstr[i + 1..$];
      prop = dobjstr[1..i - 1];
      if (argstr[1 + index(argstr, "=")] == "\"")
      elseif (default[1] == "#")
        default = toobj(default);
      elseif (index("0123456789", default[1]))
        default = toint(default);
      elseif (default == "{}")
        default = {};
      endif
    else
      default = 0;
      prop = dobjstr;
    endif
    if (typeof(result = this:set_stateprops(prop, default)) == ERR)
      player:tell(result == E_RANGE ? tostr(".", prop, " needs to hold a list of the same length as .active (", length(this.active), ").") | (result != E_NACC ? result | prop + " is already a property on an ancestral editor."));
    else
      player:tell("Property added.");
    endif
  endverb

  verb "@rmstateprop" (any from this) owner: #96 flags: "rd"
    if (!$perm_utils:controls(player, this))
      player:tell(E_PERM);
    elseif (typeof(result = this:set_stateprops(dobjstr)) == ERR)
      player:tell(result != E_NACC ? result | dobjstr + " is already a property on an ancestral editor.");
    else
      player:tell("Property removed.");
    endif
  endverb

  verb initialize (this none this) owner: #96 flags: "rxd"
    if ($perm_utils:controls(caller_perms(), this))
      pass(@args);
      this:kill_all_sessions();
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this:kill_all_sessions();
      if (this == $generic_editor)
        this.help = $editor_help;
      endif
      if ($object_utils:defines_verb(this, "is_not_banned"))
        delete_verb(this, "is_not_banned");
      endif
    endif
  endverb

  verb set_stateprops (this none this) owner: #96 flags: "rxd"
    remove = length(args) < 2;
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (!(length(args) in {1, 2}))
      return E_ARGS;
    elseif (typeof(prop = args[1]) != STR)
      return E_TYPE;
    elseif (i = $list_utils:iassoc(prop, this.stateprops))
      if (!remove)
        this.stateprops[i] = {prop, args[2]};
      elseif ($object_utils:has_property(parent(this), prop))
        return E_NACC;
      else
        this.stateprops = listdelete(this.stateprops, i);
      endif
    elseif (remove)
    elseif (prop in `properties(this) ! ANY => {}')
      if (this:_stateprop_length(prop) != length(this.active))
        return E_RANGE;
      endif
      this.stateprops = {{prop, args[2]}, @this.stateprops};
    else
      return $object_utils:has_property(this, prop) ? E_NACC | E_PROPNF;
    endif
    return 0;
  endverb

  verb description (this none this) owner: #96 flags: "rxd"
    is_look_self = 1;
    for c in (callers())
      if (is_look_self && c[2] in {"enterfunc", "confunc"})
        return {"", "Do a 'look' to get the list of commands, or 'help' for assistance.", "", @this.description};
      elseif (c[2] != "look_self" && c[2] != "pass")
        is_look_self = 0;
      endif
    endfor
    d = {"Commands:", ""};
    col = {{}, {}};
    for c in [1..2]
      for cmd in (this.commands2[c])
        cmd = this:commands_info(cmd);
        col[c] = {cmdargs = $string_utils:left(cmd[1] + " ", 12) + cmd[2], @col[c]};
      endfor
    endfor
    i1 = length(col[1]);
    i2 = length(col[2]);
    right = 0;
    while (i1 || i2)
      if (!(i1 && length(col[1][i1]) > 35 || i2 && length(col[2][i2]) > 35))
        d = {@d, $string_utils:left(i1 ? col[1][i1] | "", 40) + (i2 ? col[2][i2] | "")};
        i1 && (i1 = i1 - 1);
        i2 && (i2 = i2 - 1);
        right = 0;
      elseif (right && i2)
        d = {@d, length(col[2][i2]) > 35 ? $string_utils:right(col[2][i2], 75) | $string_utils:space(40) + col[2][i2]};
        i2 = i2 - 1;
        right = 0;
      elseif (i1)
        d = {@d, col[1][i1]};
        i1 = i1 - 1;
        right = 1;
      else
        right = 1;
      endif
    endwhile
    return {@d, "", "----  Do `help <cmdname>' for help with a given command.  ----", "", "  <ins> ::= $ (the end) | [^]n (above line n) | _n (below line n) | . (current)", "<range> ::= <lin> | <lin>-<lin> | from <lin> | to <lin> | from <lin> to <lin>", "  <lin> ::= n | [n]$ (n from the end) | [n]_ (n before .) | [n]^ (n after .)", "`help insert' and `help ranges' describe these in detail.", @this.description};
  endverb

  verb commands_info (this none this) owner: #96 flags: "rxd"
    cmd = args[1];
    if (pc = $list_utils:assoc(cmd, this.commands))
      return pc;
    elseif (this == $generic_editor)
      return {cmd, "<<<<<======= Need to add this to .commands"};
    else
      return parent(this):commands_info(cmd);
    endif
  endverb

  verb match_object (this none this) owner: #96 flags: "rxd"
    {objstr, ?who = player} = args;
    origin = this;
    while ((where = player in origin.active) && ($recycler:valid(origin = origin.original[where]) && origin != this))
      if (!$object_utils:isa(origin, $generic_editor))
        return origin:match_object(objstr, who);
      endif
    endwhile
    return who:my_match_object(objstr, #-1);
  endverb

  verb who_location_msg (this none this) owner: #96 flags: "rxd"
    who = args[1];
    where = {#-1, @this.original}[1 + (who in this.active)];
    wherestr = `where:who_location_msg(who) ! ANY => "An Editor"';
    if (typeof(wherestr) != STR)
      wherestr = "broken who_location_msg";
    endif
    return strsub(this.who_location_msg, "%L", wherestr);
    return $string_utils:pronoun_sub(this.who_location_msg, who, this, where);
  endverb

  verb "nothing_loaded_msg no_text_msg change_msg no_change_msg no_littering_msg depart_msg return_msg previous_session_msg" (this none this) owner: #96 flags: "rxd"
    return $code_utils:verb_or_property(player, verb, this) || this.(verb);
  endverb

  verb "announce announce_all announce_all_but tell_contents" (this none this) owner: #96 flags: "rxd"
    return;
  endverb

  verb fill_string (this none this) owner: #96 flags: "rxd"
    "fill(string [, width [, prefix]])";
    "tries to cut <string> into substrings of length < <width> along word boundaries.  Prefix, if supplied, will be prefixed to the 2nd..last substrings.";
    {string, ?width = 1 + player:linelen(), ?prefix = ""} = args;
    width = width + 1;
    if (width < 3 + length(prefix))
      return E_INVARG;
    endif
    string = "$" + string + " $";
    len = length(string);
    if (len <= width)
      last = len - 1;
      next = len;
    else
      last = rindex(string[1..width], " ");
      if (last < (width + 1) / 2)
        last = width + index(string[width + 1..len], " ");
      endif
      next = last;
      while (string[next = next + 1] == " ")
      endwhile
    endif
    while (string[last = last - 1] == " ")
    endwhile
    ret = {string[2..last]};
    width = width - length(prefix);
    minlast = (width + 1) / 2;
    while (next < len)
      string = "$" + string[next..len];
      len = len - next + 2;
      if (len <= width)
        last = len - 1;
        next = len;
      else
        last = rindex(string[1..width], " ");
        if (last < minlast)
          last = width + index(string[width + 1..len], " ");
        endif
        next = last;
        while (string[next = next + 1] == " ")
        endwhile
      endif
      while (string[last = last - 1] == " ")
      endwhile
      if (last > 1)
        ret = {@ret, prefix + string[2..last]};
      endif
    endwhile
    return ret;
  endverb

  verb here_huh (this none this) owner: #96 flags: "rxd"
    "This catches subst and find commands that don't fit into the usual model, e.g., s/.../.../ without the space after the s, and find commands without the verb `find'.  Still behaves in annoying ways (e.g., loses if the search string contains multiple whitespace), but better than before.";
    if (caller != this && caller_perms() != player)
      return E_PERM;
    endif
    {verb, args} = args;
    v = 1;
    vmax = min(length(verb), 5);
    while (v <= vmax && verb[v] == "subst"[v])
      v = v + 1;
    endwhile
    argstr = $code_utils:argstr(verb, args);
    if (v > 1 && (v <= length(verb) && ((vl = verb[v]) < "A" || vl > "Z")))
      argstr = verb[v..$] + (argstr && " ") + argstr;
      this:subst();
      return 1;
    elseif ("/" == verb[1])
      argstr = verb + (argstr && " ") + argstr;
      this:find();
      return 1;
    else
      return 0;
    endif
  endverb

  verb match (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return $failed_match;
  endverb

  verb get_room (this none this) owner: #96 flags: "rxd"
    ":get_room([player])  => correct room to match in on invocation.";
    {?who = player} = args;
    if (who.location != this)
      return who.location;
    else
      origin = this;
      while ((where = player in origin.active) && (valid(origin = origin.original[where]) && origin != this))
        if (!$object_utils:isa(origin, $generic_editor))
          return origin;
        endif
      endwhile
      return this;
    endif
  endverb

  verb invoke_local_editor (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":invoke_local_editor(name, text, upload)";
    "Spits out the magic text that invokes the local editor in the player's client.";
    "NAME is a good human-readable name for the local editor to use for this particular piece of text.";
    "TEXT is a string or list of strings, the initial body of the text being edited.";
    "UPLOAD, a string, is a MOO command that the local editor can use to save the text when the user is done editing.  The local editor is going to send that command on a line by itself, followed by the new text lines, followed by a line containing only `.'.  The UPLOAD command should therefore call $command_utils:read_lines() to get the new text as a list of strings.";
    if (caller != this)
      return;
    endif
    {name, text, upload} = args;
    if (typeof(text) == STR)
      text = {text};
    endif
    notify(player, tostr("#$# edit name: ", name, " upload: ", upload));
    ":dump_lines() takes care of the final `.' ...";
    for line in ($command_utils:dump_lines(text))
      notify(player, line);
    endfor
  endverb

  verb _stateprop_length (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "+c properties on children cannot necessarily be read, so we need this silliness...";
    if (caller != this)
      return E_PERM;
    else
      return length(this.((args[1])));
    endif
  endverb

  verb print (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    txt = this:text(player in this.active);
    if (typeof(txt) == LIST)
      player:tell_lines(txt);
    else
      player:tell("Text unreadable:  ", txt);
    endif
    player:tell("--------------------------");
  endverb

  verb accept (this none this) owner: #96 flags: "rxd"
    return this:acceptable(who_obj = args[1]) && this:new_session(who_obj, who_obj.location);
  endverb

  verb "y*ank" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Usage: yank from <note>";
    "       yank <message-sequence> from <mail-recipient>";
    "       yank from <object>:<verb>";
    "       yank from <object>.<property>";
    "Grabs the specified text and inserts it at the cursor.";
    set_task_perms(player);
    if (dobjstr)
      "yank <message-sequence> from <mail-recipient>";
      if (!(p = player:parse_mailread_cmd(verb, args, "", "from")))
        return;
      elseif ($seq_utils:size(sequence = p[2]) != 1)
        player:notify(tostr("You can only ", verb, " one message at a time"));
        return;
      else
        m = (folder = p[1]):messages_in_seq(sequence);
        msg = m[1];
        header = tostr("Message ", msg[1]);
        if (folder != player)
          header = tostr(header, " on ", $mail_agent:name(folder));
        endif
        header = tostr(header, ":");
        lines = {header, @player:msg_full_text(@msg[2])};
        this:insert_line(this:loaded(player), lines, 0);
      endif
    elseif (pr = $code_utils:parse_propref(iobjstr))
      o = player:my_match_object(pr[1]);
      if ($command_utils:object_match_failed(o, pr[1]))
        return;
      elseif ((lines = `o.((pr[2])) ! ANY') == E_PROPNF)
        player:notify(tostr("There is no `", pr[2], "' property on ", $string_utils:nn(o), "."));
        return;
      elseif (lines == E_PERM)
        player:notify(tostr("Error: Permission denied reading ", iobjstr));
        return;
      elseif (typeof(lines) == ERR)
        player:notify(tostr("Error: ", lines, " reading ", iobjstr));
        return;
      elseif (typeof(lines) == STR)
        this:insert_line(this:loaded(player), lines, 0);
        return;
      elseif (typeof(lines) == LIST)
        for x in (lines)
          if (typeof(x) != STR)
            player:notify(tostr("Error: ", iobjstr, " does not contain a ", verb, "-able value."));
            return;
          endif
        endfor
        this:insert_line(this:loaded(player), lines, 0);
        return;
      else
        player:notify(tostr("Error: ", iobjstr, " does not contain a ", verb, "-able value."));
        return;
      endif
    elseif (pr = $code_utils:parse_verbref(iobjstr))
      o = player:my_match_object(pr[1]);
      if ($command_utils:object_match_failed(o, pr[1]))
        return;
      elseif (lines = `verb_code(o, pr[2], !player:edit_option("no_parens")) ! ANY')
        this:insert_line(this:loaded(player), lines, 0);
        return;
      elseif (lines == E_PERM)
        player:notify(tostr("Error: Permission denied reading ", iobjstr));
        return;
      elseif (lines == E_VERBNF)
        player:notify(tostr("There is no `", pr[2], "' verb on ", $string_utils:nn(o), "."));
      else
        player:notify(tostr("Error: ", lines, " reading ", iobjstr));
        return;
      endif
    elseif ($command_utils:object_match_failed(iobj = player:my_match_object(iobjstr), iobjstr))
      return;
    elseif ((lines = `iobj:text() ! ANY') == E_PERM)
      player:notify(tostr("Error: Permission denied reading ", iobjstr));
      return;
    elseif (lines == E_VERBNF)
      player:notify(tostr($string_utils:nn(iobj), " doesn't seem to be a note."));
    elseif (typeof(lines) == ERR)
      player:notify(tostr("Error: ", lines, " reading ", iobjstr));
      return;
    else
      this:insert_line(this:loaded(player), lines, 0);
    endif
  endverb

  verb do_flush (this none this) owner: #96 flags: "rxd"
    "Flushes editor sessions older than args[1].  If args[2] is true, prints status as it runs.  If args[2] is false, runs silently.";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    else
      {t, noisy} = args;
      for i in [-length(this.active)..-1]
        if (this.times[-i] < t)
          if (noisy)
            player:tell($string_utils:nn(this.active[-i]), ctime(this.times[-i]));
          endif
          this:kill_session(-i);
        endif
      endfor
    endif
  endverb

  verb "s*ubst" (any any any) owner: #96 flags: "rxd"
    if (callers() && caller != this)
      return E_PERM;
    elseif (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(subst = this:parse_subst(argstr)) != LIST)
      player:tell(tostr(subst));
    elseif (typeof(range = this:parse_range(who, {"_", "1"}, @$string_utils:explode(subst[4]))) != LIST)
      player:tell(range);
    elseif (range[3])
      player:tell("Junk at end of cmd:  ", range[3]);
    else
      {fromstr, tostr, specs, dummy} = subst;
      global = index(specs, "g", 1);
      regexp = index(specs, "r", 1);
      case = !index(specs, "c", 1);
      munged = {};
      text = this.texts[who];
      changed = {};
      {from, to} = range[1..2];
      for line in [from..to]
        t = t0 = text[line];
        if (!fromstr)
          t = tostr + t;
        elseif (global)
          if (regexp)
            while (new = this:subst_regexp(t, fromstr, tostr, case))
              t = new;
            endwhile
          else
            t = strsub(t, fromstr, tostr, case);
          endif
        else
          if (regexp)
            (new = this:subst_regexp(t, fromstr, tostr, case)) && (t = new);
          else
            (i = index(t, fromstr, case)) && (t = t[1..i - 1] + tostr + t[i + length(fromstr)..length(t)]);
          endif
        endif
        if (strcmp(t0, t))
          changed = {@changed, line};
        endif
        munged = {@munged, t};
      endfor
      if (!changed)
        player:tell("No changes in line", from == to ? tostr(" ", from) | tostr("s ", from, "-", to), ".");
      else
        this.texts[who] = {@text[1..from - 1], @munged, @text[to + 1..$]};
        if (!(this.changes[who]))
          this.changes[who] = 1;
          this.times[who] = time();
        endif
        for line in (changed)
          this:list_line(who, line);
        endfor
      endif
    endif
  endverb

  verb subst_regexp (this none this) owner: #96 flags: "rxd"
    "Copied from Domain (#8111):subst_regexp by Mooshie (#106469) Mon Jan  5 19:27:26 1998 PST";
    "Usage: subst_regexp(STR text, STR from string, STR to string, INT case)";
    {text, from, to, case} = args;
    if (m = match(text, from, case))
      {start, end} = m[1..2];
      text[start..end] = substitute(to, m);
      return text;
    else
      return m;
    endif
  endverb

  verb include_for_core (this none this) owner: #96 flags: "rxd"
    return this == $generic_editor ? {"owner"} | {};
  endverb
endobject