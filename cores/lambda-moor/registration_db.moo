object REGISTRATION_DB
  name: "Registration Database"
  parent: GENERIC_DB
  owner: HACKER

  property alphabet (owner: HACKER, flags: "rc") = "abcdefghijklmnopqrstuvwxy0123456789_.@+z";
  property prune_progress (owner: HACKER, flags: "rc") = "aaa";
  property prune_stop (owner: HACKER, flags: "rc") = "zzz";
  property prune_task (owner: HACKER, flags: "rc") = 0;
  property registrar (owner: HACKER, flags: "rc") = BYTE_QUOTA_UTILS_WORKING;
  property total_pruned_characters (owner: HACKER, flags: "rc") = 0;
  property total_pruned_people (owner: HACKER, flags: "rc") = 0;

  override aliases = {"Registration Database"};
  override node_perms = "";
  override object_size = {8549, 1084848672};

  verb "find* _only* _every*" (this none this) owner: HACKER flags: "rxd"
    return caller == this || caller_perms().wizard ? pass(@args) | E_PERM;
  endverb

  verb add (this none this) owner: HACKER flags: "rxd"
    ":add(player,email[,comment])";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {who, email, @comment} = args;
    l = this:find_exact(email);
    if (l == $failed_match)
      this:insert(email, {{who, @comment}});
    elseif (i = $list_utils:iassoc(who, l))
      this:insert(email, listset(l, {who, @comment}, i));
    else
      this:insert(email, {@l, {who, @comment}});
    endif
  endverb

  verb init_for_core (this none this) owner: HACKER flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this:clearall();
      this.registrar = #2;
      this:prune_reset();
    endif
  endverb

  verb suspicious_address (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "suspicious(address [,who])";
    "Determine whether an address appears to be another player in disguise.";
    "returns a list of similar addresses.";
    "If second argument given, then if all similar addresses are held by that";
    "person, let it pass---they're just switching departments at the same school";
    "or something.";
    "";
    "at the moment,";
    "  foo@bar.baz.bing.boo";
    "is considered 'similar' to anything matching";
    "  foo@*.bing.boo";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {address, ?allowed = #-1} = args;
    {userid, site} = $network:parse_address(address);
    exact = !site && this:find_exact(address);
    if (!site)
      site = $network.site;
    endif
    site = $network:local_domain(site);
    sitelen = length(site);
    others = this:find_all_keys(userid + "@");
    for other in (others)
      if (other[max(1, $ - sitelen + 1)..$] != site)
        others = setremove(others, other);
      endif
    endfor
    if (exact)
      others = listinsert(others, address);
    endif
    for x in (others)
      allzapped = 1;
      for y in (this:find_exact(x))
        if (length(y) == 2 && (y[2] == "zapped due to inactivity" || y[2] == "toaded due to inactivity") || y[1] == allowed || $object_utils:has_property($local, "second_char_registry") && typeof(them = $local.second_char_registry:other_chars(y[1])) == LIST && allowed in them)
          "let them change to the address if it is them, or if it is a registered char of theirs.";
          "Hrm. Need typeof==LIST check because returns E_INVARG for shared characters. bleah Ho_Yan 5/8/95";
        else
          allzapped = 0;
        endif
      endfor
      if (allzapped)
        others = setremove(others, x);
      endif
    endfor
    return others;
  endverb

  verb suspicious_userid (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "suspicious_userid(userid)";
    "Return yes if userid is root or postmaster or something like that.";
    if ($object_utils:has_property(#0, "local") && $object_utils:has_property($local, "suspicious_userids"))
      extra = $local.suspicious_userids;
    else
      extra = {};
    endif
    return args[1] in {@$network.suspicious_userids, @extra} || match(args[1], "^guest") || match(args[1], "^help") || index(args[1], "-owner") || index(args[1], "owner-");
    "Thinking about ruling out hyphenated names, on the grounds that they're probably mailing lists.";
  endverb

  verb describe_registration (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Returns a list of strings describing the registration data for an email address.  Args[1] should be the result of this:find.";
    set_task_perms(caller_perms());
    result = {};
    for x in (args[1])
      name = valid(x[1]) && is_player(x[1]) ? (x[1]).name | "<recycled>";
      email = valid(x[1]) && is_player(x[1]) ? $wiz_utils:get_email_address(x[1]) | "<???>";
      result = {@result, tostr("  ", name, " (", x[1], ") current email: ", email, length(x) > 1 ? " [" + x[2] + "]" | "")};
    endfor
    return result;
  endverb

  verb prune (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Carefully loop through the db and delete items associated with reaped objects.  If that results in no objects remaining for a username, delete that username.";
    "Attempt to keep memory usage down by only asking for a small number of items at a time.  Should probably have some arguments to control this.";
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    this.prune_task = task_id();
    probe = this.prune_progress;
    while (probe < this.prune_stop)
      for username in (this:find_all_keys(probe))
        items = this:find_exact(username);
        orig = items;
        for y in (items)
          {who, @whys} = y;
          if (!valid(who) || !is_player(who))
            nuke = 1;
            for why in (whys)
              if (why && why != "zapped due to inactivity" && why != "toaded due to inactivity" && why != "Additional email address")
                nuke = 0;
              endif
            endfor
            if (nuke)
              items = setremove(items, y);
            endif
          endif
          $command_utils:suspend_if_needed(0);
        endfor
        if (!items)
          this:delete(username);
          this.total_pruned_people = this.total_pruned_people + 1;
        elseif (items != orig)
          this:insert(username, items);
          this.total_pruned_characters = this.total_pruned_characters + length(orig) - length(items);
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      probe = $string_utils:incr_alpha(probe, this.alphabet);
      this.prune_progress = probe;
      if ($command_utils:running_out_of_time())
        set_task_perms($wiz_utils:random_wizard());
        suspend(0);
      endif
    endwhile
    player:tell("Prune stopped at ", toliteral(this.prune_progress));
  endverb

  verb report_prune_progress (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    player:tell("Prune is up to ", toliteral(this.prune_progress), ".");
    mine = 0;
    alphalen = length(this.alphabet);
    if (typeof(this.prune_progress) == STR)
      total = alphalen * alphalen * alphalen;
      for x in [1..3]
        mine = mine * alphalen + index(this.alphabet, this.prune_progress[x]) - 1;
      endfor
    else
      total = 256 * 256;
      mine = this.prune_progress[1] * 256 + this.prune_progress[2];
    endif
    percent = 100.0 * tofloat(mine) / tofloat(total);
    player:tell("We have processed ", mine, " entries out of ", total, ", or ", toint(percent), ".", toint(10.0 * percent) % 10, "%.");
    player:tell("There were ", this.total_pruned_characters, " individual list entries removed, and ", this.total_pruned_people, " whole email addresses removed.");
    if ($code_utils:task_valid(this.prune_task))
      player:tell("Prune task is ", this.prune_task, ".  Stacktrace:");
      for x in (task_stack(this.prune_task, 1))
        if (valid(x[4]))
          player:tell(x[4], ":", x[2], " [", x[1], "]  ", (x[3]).name, "  (", x[6], ")");
        endif
      endfor
    else
      player:tell("The recorded task_id is no longer valid.");
    endif
  endverb

  verb prune_reset (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    this:report_prune_progress();
    player:tell("Resetting...");
    this.prune_progress = "aaa";
    this.prune_stop = "zzz";
    this.total_pruned_people = 0;
    this.total_pruned_characters = 0;
    this.prune_task = 0;
  endverb

  verb search (this for any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    who = caller_perms();
    if (who != #-1 && !(who == player || caller == this) || !(who.wizard || who in $local.registrar_pet_core.members))
      raise(E_PERM);
    endif
    total = 0;
    player:tell("Searching...");
    for k in ($registration_db:find_all_keys(""))
      $command_utils:suspend_if_needed(0);
      line = k + " " + toliteral($registration_db:find_exact(k));
      if (index(line, iobjstr))
        player:tell(line);
        total = total + 1;
      endif
    endfor
    player:tell("Search over.  ", total, " matches found.");
  endverb
endobject