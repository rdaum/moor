object ROOM
  name: "generic room"
  parent: ROOT_CLASS
  owner: BYTE_QUOTA_UTILS_WORKING
  fertile: true
  readable: true

  property blessed_object (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = LOCAL;
  property blessed_task (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property ctype (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 3;
  property dark (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property ejection_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You expel %d from %i.";
  property entrances (owner: BYTE_QUOTA_UTILS_WORKING, flags: "c") = {};
  property exits (owner: BYTE_QUOTA_UTILS_WORKING, flags: "c") = {};
  property free_entry (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 1;
  property free_home (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property oejection_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "%N unceremoniously %{!expels} %d from %i.";
  property residents (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property victim_ejection_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You have been expelled from %i by %n.";
  property who_location_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "%T";

  override aliases = {"generic room"};
  override object_size = {28944, 1084848672};

  verb confunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ((cp = caller_perms()) == player || $perm_utils:controls(cp, player) || caller == this)
      "Need the first check because guests don't control themselves";
      this:look_self(player.brief);
      this:announce($string_utils:pronoun_sub("%N %<has> connected.", player));
    endif
  endverb

  verb disfunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ((cp = caller_perms()) == player || $perm_utils:controls(cp, player) || caller == this)
      this:announce($string_utils:pronoun_sub("%N %<has> disconnected.", player));
      "need the first check since guests don't control themselves";
      if (!$object_utils:isa(player, $guest))
        "guest disfuncs are handled by $guest:disfunc. Don't add them here";
        $housekeeper:move_players_home(player);
      endif
    endif
  endverb

  verb say (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rx"
    try
      player:tell("You say, \"", argstr, "\"");
      this:announce(player.name, " ", $gender_utils:get_conj("says", player), ", \"", argstr, "\"");
    except (ANY)
      "Don't really need to do anything but ignore the idiot who has a bad :tell";
    endtry
  endverb

  verb emote (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (argstr != "" && argstr[1] == ":")
      this:announce_all(player.name, argstr[2..length(argstr)]);
    else
      this:announce_all(player.name, " ", argstr);
    endif
  endverb

  verb announce (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    for dude in (setremove(this:contents(), player))
      try
        dude:tell(@args);
      except (ANY)
        "Just skip the dude with the bad :tell";
        continue dude;
      endtry
    endfor
  endverb

  verb match_exit (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    what = args[1];
    if (what)
      yes = $failed_match;
      for e in (this.exits)
        if (valid(e) && what in {e.name, @e.aliases})
          if (yes == $failed_match)
            yes = e;
          elseif (yes != e)
            return $ambiguous_match;
          endif
        endif
      endfor
      return yes;
    else
      return $nothing;
    endif
  endverb

  verb add_exit (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    return `this.exits = setadd(this.exits, args[1]) ! E_PERM' != E_PERM;
  endverb

  verb tell_contents (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {contents, ctype} = args;
    if (!this.dark && contents != {})
      if (ctype == 0)
        player:tell("Contents:");
        for thing in (contents)
          player:tell("  ", thing:title());
        endfor
      elseif (ctype == 1)
        for thing in (contents)
          if (is_player(thing))
            player:tell($string_utils:pronoun_sub(tostr("%N ", $gender_utils:get_conj("is", thing), " here."), thing));
          else
            player:tell("You see ", thing:title(), " here.");
          endif
        endfor
      elseif (ctype == 2)
        player:tell("You see ", $string_utils:title_list(contents), " here.");
      elseif (ctype == 3)
        players = things = {};
        for x in (contents)
          if (is_player(x))
            players = {@players, x};
          else
            things = {@things, x};
          endif
        endfor
        if (things)
          player:tell("You see ", $string_utils:title_list(things), " here.");
        endif
        if (players)
          player:tell($string_utils:title_listc(players), length(players) == 1 ? " " + $gender_utils:get_conj("is", players[1]) | " are", " here.");
        endif
      endif
    endif
  endverb

  verb "@exits" (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!$perm_utils:controls(valid(caller_perms()) ? caller_perms() | player, this))
      player:tell("Sorry, only the owner of a room may list its exits.");
    elseif (this.exits == {})
      player:tell("This room has no conventional exits.");
    else
      try
        for exit in (this.exits)
          try
            player:tell(exit.name, " (", exit, ") leads to ", valid(exit.dest) ? exit.dest.name | "???", " (", exit.dest, ") via {", $string_utils:from_list(exit.aliases, ", "), "}.");
          except (ANY)
            player:tell("Bad exit or missing .dest property:  ", $string_utils:nn(exit));
            continue exit;
          endtry
        endfor
      except (E_TYPE)
        player:tell("Bad .exits property. This should be a list of exit objects. Please fix this.");
      endtry
    endif
  endverb

  verb look_self (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {?brief = 0} = args;
    player:tell(this:title());
    if (!brief)
      pass();
    endif
    this:tell_contents(setremove(this:contents(), player), this.ctype);
  endverb

  verb acceptable (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    what = args[1];
    return this:is_unlocked_for(what) && (this:free_entry(@args) || what == this.blessed_object && task_id() == this.blessed_task || what.owner == this.owner || typeof(this.residents) == LIST && (what in this.residents || what.owner in this.residents));
  endverb

  verb add_entrance (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    return `this.entrances = setadd(this.entrances, args[1]) ! E_PERM' != E_PERM;
  endverb

  verb bless_for_entry (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller in {@this.entrances, this})
      this.blessed_object = args[1];
      this.blessed_task = task_id();
    endif
  endverb

  verb "@entrances" (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (!$perm_utils:controls(valid(caller_perms()) ? caller_perms() | player, this))
      player:tell("Sorry, only the owner of a room may list its entrances.");
    elseif (this.entrances == {})
      player:tell("This room has no conventional entrances.");
    else
      try
        for exit in (this.entrances)
          try
            player:tell(exit.name, " (", exit, ") comes from ", valid(exit.source) ? exit.source.name | "???", " (", exit.source, ") via {", $string_utils:from_list(exit.aliases, ", "), "}.");
          except (ANY)
            player:tell("Bad entrance object or missing .source property: ", $string_utils:nn(exit));
            continue exit;
          endtry
        endfor
      except (E_TYPE)
        player:tell("Bad .entrances property. This should be a list of exit objects. Please fix this.");
      endtry
    endif
  endverb

  verb go (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!args || !(dir = args[1]))
      player:tell("You need to specify a direction.");
      return E_INVARG;
    elseif (valid(exit = player.location:match_exit(dir)))
      exit:invoke();
      if (length(args) > 1)
        old_room = player.location;
        "Now give objects in the room we just entered a chance to act.";
        suspend(0);
        if (player.location == old_room)
          "player didn't move or get moved while we were suspended";
          player.location:go(@listdelete(args, 1));
        endif
      endif
    elseif (exit == $failed_match)
      player:tell("You can't go that way (", dir, ").");
    else
      player:tell("I don't know which direction `", dir, "' you mean.");
    endif
  endverb

  verb "l*ook" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (dobjstr == "" && !prepstr)
      this:look_self();
    elseif (prepstr != "in" && prepstr != "on")
      if (!dobjstr && prepstr == "at")
        dobjstr = iobjstr;
        iobjstr = "";
      else
        dobjstr = dobjstr + (prepstr && (dobjstr && " ") + prepstr);
        dobjstr = dobjstr + (iobjstr && (dobjstr && " ") + iobjstr);
      endif
      dobj = this:match_object(dobjstr);
      if (!$command_utils:object_match_failed(dobj, dobjstr))
        dobj:look_self();
      endif
    elseif (!iobjstr)
      player:tell(verb, " ", prepstr, " what?");
    else
      iobj = this:match_object(iobjstr);
      if (!$command_utils:object_match_failed(iobj, iobjstr))
        if (dobjstr == "")
          iobj:look_self();
        elseif ((thing = iobj:match(dobjstr)) == $failed_match)
          player:tell("I don't see any \"", dobjstr, "\" ", prepstr, " ", iobj.name, ".");
        elseif (thing == $ambiguous_match)
          player:tell("There are several things ", prepstr, " ", iobj.name, " one might call \"", dobjstr, "\".");
        else
          thing:look_self();
        endif
      endif
    endif
  endverb

  verb announce_all (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    for dude in (this:contents())
      try
        dude:tell(@args);
      except (ANY)
        "Just ignore the dude with the stupid :tell";
        continue dude;
      endtry
    endfor
  endverb

  verb announce_all_but (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":announce_all_but(LIST objects to ignore, text)";
    {ignore, @text} = args;
    contents = this:contents();
    for l in (ignore)
      contents = setremove(contents, l);
    endfor
    for listener in (contents)
      try
        listener:tell(@text);
      except (ANY)
        "Ignure listener with bad :tell";
        continue listener;
      endtry
    endfor
  endverb

  verb enterfunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    object = args[1];
    if (is_player(object) && object.location == this)
      player = object;
      this:look_self(player.brief);
    endif
    if (object == this.blessed_object)
      this.blessed_object = #-1;
    endif
  endverb

  verb exitfunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return;
  endverb

  verb remove_exit (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    exit = args[1];
    if (caller != exit)
      set_task_perms(caller_perms());
    endif
    return `this.exits = setremove(this.exits, exit) ! E_PERM' != E_PERM;
  endverb

  verb remove_entrance (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    exit = args[1];
    if (caller != exit)
      set_task_perms(caller_perms());
    endif
    return `this.entrances = setremove(this.entrances, exit) ! E_PERM' != E_PERM;
  endverb

  verb "@add-exit" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      player:tell("Usage:  @add-exit <exit-number>");
      return;
    endif
    exit = this:match_object(dobjstr);
    if ($command_utils:object_match_failed(exit, dobjstr))
      return;
    endif
    if (!($exit in $object_utils:ancestors(exit)))
      player:tell("That doesn't look like an exit object to me...");
      return;
    endif
    try
      dest = exit.dest;
    except (E_PERM)
      player:tell("You can't read the exit's destination to check that it's consistent!");
      return;
    endtry
    try
      source = exit.source;
    except (E_PERM)
      player:tell("You can't read that exit's source to check that it's consistent!");
      return;
    endtry
    if (source == $nothing)
      player:tell("That exit's source has not yet been set; set it to be this room, then run @add-exit again.");
      return;
    elseif (source != this)
      player:tell("That exit wasn't made to be attached here; it was made as an exit from ", source.name, " (", source, ").");
      return;
    elseif (typeof(dest) != OBJ || !valid(dest) || !($room in $object_utils:ancestors(dest)))
      player:tell("That exit doesn't lead to a room!");
      return;
    endif
    if (!this:add_exit(exit))
      player:tell("Sorry, but you must not have permission to add exits to this room.");
    else
      player:tell("You have added ", exit, " as an exit that goes to ", exit.dest.name, " (", exit.dest, ") via ", $string_utils:english_list(setadd(exit.aliases, exit.name)), ".");
    endif
  endverb

  verb "@add-entrance" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      player:tell("Usage:  @add-entrance <exit-number>");
      return;
    endif
    exit = this:match_object(dobjstr);
    if ($command_utils:object_match_failed(exit, dobjstr))
      return;
    endif
    if (!($exit in $object_utils:ancestors(exit)))
      player:tell("That doesn't look like an exit object to me...");
      return;
    endif
    try
      dest = exit.dest;
    except (E_PERM)
      player:tell("You can't read the exit's destination to check that it's consistent!");
      return;
    endtry
    if (dest != this)
      player:tell("That exit doesn't lead here!");
      return;
    endif
    if (!this:add_entrance(exit))
      player:tell("Sorry, but you must not have permission to add entrances to this room.");
    else
      player:tell("You have added ", exit, " as an entrance that gets here via ", $string_utils:english_list(setadd(exit.aliases, exit.name)), ".");
    endif
  endverb

  verb recycle (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Make a mild attempt to keep people and objects from ending up in #-1 when people recycle a room";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      "... first try spilling them out onto the floor of enclosing room if any";
      if (valid(this.location))
        for x in (this.contents)
          try
            x:moveto(this.location);
          except (ANY)
            continue x;
          endtry
        endfor
      endif
      "... try sending them home...";
      for x in (this.contents)
        if (is_player(x))
          if (typeof(x.home) == OBJ && valid(x.home))
            try
              x:moveto(x.home);
            except (ANY)
              continue x;
            endtry
          endif
          if (x.location == this)
            move(x, $player_start);
          endif
        elseif (valid(x.owner))
          try
            x:moveto(x.owner);
          except (ANY)
            continue x;
          endtry
        endif
      endfor
      pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb "e east w west s south n north ne northeast nw northwest se southeast sw southwest u up d down" (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms() == #-1 ? player | caller_perms());
    exit = this:match_exit(verb);
    if (valid(exit))
      exit:invoke();
    elseif (exit == $failed_match)
      player:tell("You can't go that way.");
    else
      player:tell("I don't know which direction `", verb, "' you mean.");
    endif
  endverb

  verb "@eject @eject! @eject!!" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if ($command_utils:object_match_failed(dobj, dobjstr))
      return;
    elseif (dobj.location != this)
      is = $gender_utils:get_conj("is", dobj);
      player:tell(dobj.name, "(", dobj, ") ", is, " not here.");
      return;
    elseif (!$perm_utils:controls(player, this))
      player:tell("You are not the owner of this room.");
      return;
    elseif (dobj.wizard)
      player:tell("Sorry, you can't ", verb, " a wizard.");
      dobj:tell(player.name, " tried to ", verb, " you.");
      return;
    endif
    iobj = this;
    player:tell(this:ejection_msg());
    this:((verb == "@eject" ? "eject" | "eject_basic"))(dobj);
    if (verb != "@eject!!")
      dobj:tell(this:victim_ejection_msg());
    endif
    this:announce_all_but({player, dobj}, this:oejection_msg());
  endverb

  verb "ejection_msg oejection_msg victim_ejection_msg" (this none this) owner: HACKER flags: "rxd"
    return $gender_utils:pronoun_sub(this.(verb));
  endverb

  verb accept_for_abode (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    who = args[1];
    return this:basic_accept_for_abode(who) && this:acceptable(who);
  endverb

  verb "@resident*s" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (!$perm_utils:controls(player, this))
      player:tell("You must own this room to manipulate the legal residents list.  Try contacting ", this.owner.name, ".");
    else
      if (typeof(this.residents) != LIST)
        this.residents = {this.residents};
      endif
      if (!dobjstr)
        "First, remove !valid objects from this room...";
        for x in (this.residents)
          if (typeof(x) != OBJ || !$recycler:valid(x))
            player:tell("Warning: removing ", x, ", an invalid object, from the residents list.");
            this.residents = setremove(this.residents, x);
          endif
        endfor
        player:tell("Allowable residents in this room:  ", $string_utils:english_list($list_utils:map_prop(this.residents, "name"), "no one"), ".");
        return;
      elseif (dobjstr[1] == "!")
        notflag = 1;
        dobjstr = dobjstr[2..$];
      else
        notflag = 0;
      endif
      result = $string_utils:match_player_or_object(dobjstr);
      if (!result)
        return;
      else
        "a one element list was returned to us if it won.";
        result = result[1];
        if (notflag)
          if (!(result in this.residents))
            player:tell(result.name, " doesn't appear to be in the residents list of ", this.name, ".");
          else
            this.residents = setremove(this.residents, result);
            player:tell(result.name, " removed from the residents list of ", this.name, ".");
          endif
        else
          if (result in this.residents)
            is = $gender_utils:get_conj("is", result);
            player:tell(result.name, " ", is, " already an allowed resident of ", this.name, ".");
          else
            this.residents = {@this.residents, result};
            player:tell(result.name, " added to the residents list of ", this.name, ".");
          endif
        endif
      endif
    endif
  endverb

  verb match (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    target = {@this:contents(), @this:exits()};
    return $string_utils:match(args[1], target, "name", target, "aliases");
  endverb

  verb "@remove-exit" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      player:tell("Usage:  @remove-exit <exit>");
      return;
    endif
    exit = this:match_object(dobjstr);
    if (!(exit in this.exits))
      if ($command_utils:object_match_failed(exit, dobjstr))
        return;
      endif
      player:tell("Couldn't find \"", dobjstr, "\" in the exits list of ", this.name, ".");
      return;
    elseif (!this:remove_exit(exit))
      player:tell("Sorry, but you do not have permission to remove exits from this room.");
    else
      name = valid(exit) ? exit.name | "<recycled>";
      player:tell("Exit ", exit, " (", name, ") removed from exit list of ", this.name, " (", this, ").");
    endif
  endverb

  verb "@remove-entrance" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      player:tell("Usage:  @remove-entrance <entrance>");
      return;
    endif
    entrance = $string_utils:match(dobjstr, this.entrances, "name", this.entrances, "aliases");
    if (!valid(entrance))
      "Try again to parse it.  Maybe they gave object number.  Don't complain if it's invalid though; maybe it's been recycled in some nefarious way.";
      entrance = this:match_object(dobjstr);
    endif
    if (!(entrance in this.entrances))
      player:tell("Couldn't find \"", dobjstr, "\" in the entrances list of ", this.name, ".");
      return;
    elseif (!this:remove_entrance(entrance))
      player:tell("Sorry, but you do not have permission to remove entrances from this room.");
    else
      name = valid(entrance) ? entrance.name | "<recycled>";
      player:tell("Entrance ", entrance, " (", name, ") removed from entrance list of ", this.name, " (", this, ").");
    endif
  endverb

  verb moveto (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller in {this, this.owner} || $perm_utils:controls(caller_perms(), this))
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb who_location_msg (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return (msg = `this.(verb) ! ANY') ? $string_utils:pronoun_sub(msg, args[1]) | "";
  endverb

  verb "exits entrances" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      return this.(verb);
    else
      return E_PERM;
    endif
  endverb

  verb "obvious_exits obvious_entrances" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    exits = {};
    for exit in (`verb == "obvious_exits" ? this.exits | this.entrances ! ANY => {}')
      if (`$code_utils:verb_or_property(exit, "obvious") ! ANY')
        exits = setadd(exits, exit);
      endif
    endfor
    return exits;
  endverb

  verb here_huh (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":here_huh(verb,args)  -- room-specific :huh processing.  This should return 1 if it finds something interesting to do and 0 otherwise; see $command_utils:do_huh.";
    "For the generic room, we check for the case of the caller specifying an exit for which a corresponding verb was never defined.";
    set_task_perms(caller_perms());
    if (args[2] || $failed_match == (exit = this:match_exit(verb = args[1])))
      "... okay, it's not an exit.  we give up...";
      return 0;
    elseif (valid(exit))
      exit:invoke();
    else
      "... ambiguous exit ...";
      player:tell("I don't know which direction `", verb, "' you mean.");
    endif
    return 1;
  endverb

  verb "room_announce*_all_but" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    this:(verb[6..$])(@args);
  endverb

  verb examine_commands_ok (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this == (args[1]).location;
  endverb

  verb examine_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "examine_key(examiner)";
    "return a list of strings to be told to the player, indicating what the key on this type of object means, and what this object's key is set to.";
    "the default will only tell the key to a wizard or this object's owner.";
    who = args[1];
    if (caller == this && $perm_utils:controls(who, this) && this.key != 0)
      return {tostr(this:title(), " will accept only objects matching the following key:"), tostr("  ", $lock_utils:unparse_key(this.key))};
    endif
  endverb

  verb examine_contents (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "examine_contents(who)";
    if (caller == this)
      this:tell_contents(this.contents, this.ctype);
    endif
  endverb

  verb free_entry (this none this) owner: HACKER flags: "rxd"
    return this.free_entry;
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      if (this == $player_start)
        "... If there are ever multiple rooms, then the question of";
        "....which one is to be $player_start may well be an option of some sort,";
        "... so this goes better here than hardcoded into some specific room:init_for_core verb.";
        move(player, this);
      endif
    endif
  endverb

  verb dark (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this.(verb);
  endverb

  verb announce_lines_x (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Copied from generic room (#3):announce by Haakon (#2) Thu Oct 24 16:15:01 1996 PDT";
    for dude in (setremove(this:contents(), player))
      try
        dude:tell_lines(@args);
      except id (ANY)
      endtry
    endfor
  endverb

  verb basic_accept_for_abode (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    who = args[1];
    return valid(who) && (this.free_home || $perm_utils:controls(who, this) || (typeof(residents = this.residents) == LIST ? who in this.residents | who == this.residents));
  endverb
endobject