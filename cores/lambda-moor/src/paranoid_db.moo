object PARANOID_DB
  name: "@paranoid database"
  parent: ROOT_CLASS
  owner: HACKER
  readable: true

  property max_lines (owner: #2, flags: "r") = 30;

  override aliases = {"@paranoid database", "paranoid"};
  override description = {
    "",
    "This object stores the @paranoid data from :tell.  Normally it is not necessary to access these things directly.  All verbs are controlled by a caller_perms() check.  All data is stored in the old .responsible format.",
    "",
    ":add_data(who,data) adds one line's worth of data to the collection, trimming from the front as necessary.",
    "",
    ":get_data(who) retrieves the entire batch of data.",
    "",
    ":erase_data(who) sets the data to {}",
    "",
    ":set_kept_lines(who,number) Changes the number of kept lines.  Maximum is 20.",
    "",
    "Core verbs that call the above are this are $player:tell, @check, @paranoid, and :erase_paranoid_data.",
    "",
    "Internal:  ",
    "   Properties used are",
    "   tostr(player)+\"lines\"",
    "   tostr(player)+\"pdata\"",
    "   :ensure_props_exist(who,linesname,dataname):  creates the above",
    "   :GC() --- loops over all data and verifies they're for players."
  };
  override import_export_id = "paranoid_db";
  override object_size = {5921, 1084848672};

  verb ensure_props_exist (this none this) owner: HACKER flags: "rxd"
    "*Must* be called with PDATA first, and LINES second.";
    if (caller != this && !caller_perms().wizard)
      return E_PERM;
    else
      try
        this.((args[2]));
      except (E_PROPNF)
        add_property(this, args[2], {}, {$hacker, ""});
      endtry
      try
        this.((args[3]));
      except (E_PROPNF)
        add_property(this, args[3], 5, {$hacker, ""});
      endtry
    endif
  endverb

  verb init_for_core (this none this) owner: HACKER flags: "rxd"
    if (!caller_perms().wizard)
      return;
    else
      for x in (properties(this))
        if (x[1] == "#")
          delete_property(this, x);
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      pass(@args);
    endif
  endverb

  verb add_data (this none this) owner: HACKER flags: "rxd"
    {who, newdata} = args;
    if (is_player(who) && caller_perms().wizard)
      "if ($perm_utils:controls(caller_perms(), who) && is_player(who))";
      d = tostr(who, "pdata");
      l = tostr(who, "lines");
      this:ensure_props_exist(who, d, l);
      data = this.(d);
      lines = this.(l);
      "Icky G7 code copied straight out of $player:tell.";
      if ((len = length(this.(d) = {@data, newdata})) * 2 > lines * 3)
        this.(d) = (this.(d))[len - lines + 1..len];
      endif
    else
      return E_PERM;
    endif
  endverb

  verb get_data (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if ($perm_utils:controls(caller_perms(), who))
      d = tostr(who, "pdata");
      if (typeof(`this.(d) ! ANY') == TYPE_LIST)
        return this.(d);
      else
        return {};
      endif
    else
      return E_PERM;
    endif
  endverb

  verb erase_data (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if ($perm_utils:controls(caller_perms(), who))
      d = tostr(who, "pdata");
      "OK if this would toss its cookies if no prop, no damage.";
      `this.(d) = {} ! ANY';
    else
      return E_PERM;
    endif
  endverb

  verb set_kept_lines (this none this) owner: HACKER flags: "rxd"
    maximum = this.max_lines;
    who = args[1];
    if ($perm_utils:controls(caller_perms(), who) && is_player(who))
      l = tostr(who, "lines");
      this:ensure_props_exist(who, l, l);
      kept = min(args[2], maximum);
      this.(l) = kept;
      return kept;
    else
      return E_PERM;
    endif
  endverb

  verb gc (this none this) owner: HACKER flags: "rxd"
    if (caller != this && caller_perms() != #-1 && caller_perms() != player || !player.wizard)
      $error:raise(E_PERM);
    endif
    threshold = 60 * 60 * 24 * 3;
    for x in (properties(this))
      if (x[1] == "#")
        l = length(x);
        who = toobj(x[1..l - 5]);
        if (!valid(who) || !is_player(who) || !this:is_paranoid(who))
          delete_property(this, x);
        else
          if (index(x, "lines"))
            if (typeof(this.(x)) != TYPE_INT)
              this.(x) = 10;
            endif
          elseif (index(x, "pdata"))
            if (!$object_utils:connected(who) && who.last_disconnect_time < time() - threshold && who.last_connect_time < time() - threshold)
              this.(x) = {};
            endif
            if (typeof(this.(x)) != TYPE_LIST)
              this.(x) = {};
            endif
          endif
        endif
      endif
      $command_utils:suspend_if_needed(0);
    endfor
  endverb

  verb help_msg (this none this) owner: #2 flags: "rxd"
    return this:description();
  endverb

  verb semiweeklyish (this none this) owner: #2 flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      threedays = 3 * 24 * 3600;
      fork (7 * 60 * 60 + threedays - time() % threedays)
        this:(verb)();
      endfork
      this:gc();
    endif
  endverb

  verb is_paranoid (this none this) owner: #2 flags: "rxd"
    "Some people make their .paranoid !r.  Wizardly verb to retrieve value.";
    return `args[1].paranoid ! ANY';
  endverb
endobject