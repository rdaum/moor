object PLAYER_DB
  name: "Player Database"
  parent: GENERIC_DB
  owner: HACKER
  readable: true

  property " Ev" (owner: HACKER, flags: "r") = {"ery", "", {"everyone", "Everyman"}, {NO_ONE, NO_ONE}};
  property " H" (owner: HACKER, flags: "r") = {"", "", {"Hacker", "housekeeper"}, {HACKER, HOUSEKEEPER}};
  property " e" (owner: HACKER, flags: "r") = {"", "v", {"Editor_Owner"}, {#96}};
  property " n" (owner: HACKER, flags: "r") = {"o", "", {"noone", "no_one"}, {NO_ONE, NO_ONE}};
  property frozen (owner: HACKER, flags: "rc") = 0;
  property reserved (owner: HACKER, flags: "r") = {};
  property stupid_names (owner: HACKER, flags: "rc") = {
    "with",
    "using",
    "at",
    "to",
    "in",
    "into",
    "on",
    "onto",
    "upon",
    "out",
    "from",
    "inside",
    "over",
    "through",
    "under",
    "underneath",
    "beneath",
    "behind",
    "beside",
    "for",
    "about",
    "is",
    "as",
    "off",
    "of",
    "me",
    "you",
    "here"
  };

  override " " = {"", "Hen", {"Wizard"}, {#2}};
  override aliases = {"player_db", "plyrdb", "pdb"};
  override description = {
    "A database containing all player names and aliases.  ",
    "See `help $player_db' for more information."
  };
  override object_size = {8069, 1084848672};

  verb load (this none this) owner: HACKER flags: "rxd"
    ":load() -- reloads the player_db with the names of all existing players.";
    "This routine calls suspend() if it runs out of time.";
    ".frozen is set to 1 while the load is in progress so that other routines are warned and don't try to do any updates.  Sometimes, an update is unavoidable (e.g., player gets recycled) in which case the offending routine should set .frozen to 2, causing the load to start over at the beginning.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    "...N.B. clearall suspends, therefore we put the .frozen mark on FIRST...";
    this.frozen = 1;
    this:clearall();
    for p in (players())
      this:suspend_restart(p);
      "... note that if a player is recycled or toaded during the suspension,...";
      "... it won't be removed from the for loop iteration; thus this test:     ";
      if (valid(p) && is_player(p))
        if (typeof(po = this:find_exact(p.name)) == ERR)
          player:tell(p.name, ":  ", po);
          return;
        elseif (po != p)
          if (valid(po) && is_player(po))
            player:tell("name `", p.name, "' for ", p, " subsumes alias for ", po.name, "(", po, ").");
          endif
          this:insert(p.name, p);
        endif
        for a in (p.aliases)
          this:suspend_restart(p);
          if (index(a, " ") || index(a, "\t"))
            "don't bother, space or tab";
          elseif (typeof(ao = this:find_exact(a)) == ERR)
            player:tell(a, ":  ", ao);
            return;
          elseif (!(valid(ao) && is_player(ao)))
            this:insert(a, p);
          elseif (ao != p)
            player:tell("alias `", a, "' for ", p.name, "(", p, ") used by ", ao.name, "(", ao, ").");
          endif
        endfor
      endif
    endfor
    this.frozen = 0;
  endverb

  verb check (this none none) owner: HACKER flags: "rxd"
    ":check() -- checks for recycled and toaded players that managed not to get expunged from the db.";
    for p in (properties($player_db))
      if (ticks_left() < 500 || seconds_left() < 2)
        player:tell("...", p);
        suspend(0);
      endif
      if (p[1] == " ")
        nlist = this.(p)[3];
        olist = this.(p)[4];
        for i in [1..length(nlist)]
          if (valid(olist[i]) && (is_player(olist[i]) && nlist[i] in olist[i].aliases))
          else
            player:tell(".", p[2..$], " <- ", nlist[i], " ", olist[i]);
          endif
        endfor
      endif
    endfor
    player:tell("done.");
  endverb

  verb init_for_core (this none this) owner: HACKER flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.reserved = {};
      this:load();
    endif
  endverb

  verb available (this none this) owner: HACKER flags: "rxd"
    ":available(name,who) => 1 if a name is available for use, or the object id of whoever is currently using it, or 0 if the name is otherwise forbidden.";
    "If $player_db is not .frozen and :available returns 1, then $player:set_name will succeed.";
    {name, ?target = valid(caller) ? caller | player} = args;
    if (name in this.stupid_names || name in this.reserved)
      return 0;
    elseif (target in $wiz_utils.rename_restricted)
      return 0;
    elseif (!name || index(name, " ") || index(name, "\\") || index(name, "\"") || index(name, "\t"))
      return 0;
    elseif (index("*#()", name[1]))
      return 0;
    elseif ($code_utils:match_objid(name))
      return 0;
    elseif (valid(who = this:find_exact(name)) && is_player(who))
      return who;
    elseif ($object_utils:has_callable_verb($local, "legal_name") && !$local:legal_name(name, target))
      return 0;
    else
      return 1;
    endif
  endverb

  verb suspend_restart (this none this) owner: #2 flags: "rxd"
    "used during :load to do the usual out-of-time check.";
    "if someone makes a modification during the suspension (indicated by this.frozen being set to 2), we have to restart the entire load.";
    if (caller != this)
      return E_PERM;
    elseif ($command_utils:running_out_of_time())
      player:tell("...", args[1]);
      set_task_perms($byte_quota_utils:task_perms());
      suspend(0);
      if (this.frozen != 1)
        player:tell("...argh... restarting $player_db:load...");
        fork (0)
          this:load();
        endfork
        kill_task(task_id());
      endif
    endif
  endverb

  verb why_bad_name (this none this) owner: #2 flags: "rxd"
    ":why_bad_name(player, namespec) => Returns a message explaining why a player name change is invalid.  Stolen from APHiD's #15411:name_okay.";
    who = args[1];
    name = $building_utils:parse_names(args[2])[1];
    si = index(name, " ");
    qi = index(name, "\"");
    bi = index(name, "\\");
    ti = index(name, "\t");
    if (si || qi || bi || ti)
      return tostr("You may not use a name containing ", $string_utils:english_list({@si ? {"spaces"} | {}, @qi ? {"quotation marks"} | {}, @bi ? {"backslashes"} | {}, @ti ? {"tabs"} | {}}, "ERROR", " or "), ".  Try \"", strsub(strsub(strsub(strsub(name, " ", "_"), "\"", "'"), "\\", "/"), "\t", "___"), "\" instead.");
    elseif (name == "")
      return tostr("You may not use a blank name.");
    elseif (i = index("*#()", name[1]))
      return tostr("You may not begin a name with the \"", "*#()"[i], "\" character.");
    elseif ($code_utils:match_objid(name))
      return tostr("A name can't contain a parenthesized object number.");
    elseif (name in $player_db.stupid_names)
      return tostr("The name \"", name, "\" would probably cause problems in command parsing or similar usage.");
    elseif (name in $player_db.reserved)
      return tostr("The name \"", name, "\" is reserved.");
    elseif (length(name) > $login.max_player_name)
      return tostr("The name \"", name, "\" is too long.  Maximum name length is ", $login.max_player_name, " characters.");
    elseif (valid(match = $player_db:find_exact(name)) && is_player(match) && who != match)
      return tostr("The name \"", name, "\" is already being used by ", match.name, "(", match, ").");
    elseif ($player_db.frozen)
      return tostr("$player_db is not accepting new changes at the moment.");
    elseif ($object_utils:has_callable_verb($local, "legal_name") && !$local:legal_name(name, who))
      return "That name is reserved.";
    elseif (who in $wiz_utils.rename_restricted)
      return "This player is not allowed to change names.";
    endif
  endverb
endobject