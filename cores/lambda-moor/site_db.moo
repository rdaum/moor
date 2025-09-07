object SITE_DB
  name: "Site DB"
  parent: GENERIC_DB
  owner: HACKER

  property " l" (owner: HACKER, flags: "") = {
    "ocal",
    "",
    {"localdomain", "localhost.localdomain"},
    {{"localhost"}, {BYTE_QUOTA_UTILS_WORKING}}
  };
  property alphabet (owner: HACKER, flags: "rc") = "abcdefghijklmnopqrstuvwxy0123456789_z";
  property domain (owner: HACKER, flags: "r") = "localdomain";
  property prune_progress (owner: HACKER, flags: "c") = "aaa";
  property prune_stop (owner: HACKER, flags: "rc") = "zzz";
  property prune_task (owner: HACKER, flags: "rc") = 298000796;
  property total_pruned_people (owner: HACKER, flags: "rc") = 0;
  property total_pruned_sites (owner: HACKER, flags: "rc") = 0;

  override " " = {"", "l", {}, {}};
  override aliases = {"sitedb", "site", "db"};
  override description = {
    "This object holds a db of places from which players have connected (see `help $site_db').",
    "The site blacklist and the graylist live as well (see `help blacklist')."
  };
  override node_perms = "";
  override object_size = {13167, 1084848672};

  verb "find* _only* _every*" (this none this) owner: HACKER flags: "rxd"
    return caller == this || caller_perms().wizard ? pass(@args) | E_PERM;
  endverb

  verb add (this none this) owner: HACKER flags: "rxd"
    ":add(player,site)";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {who, domain} = args;
    if (this:domain_literal(domain))
      "... just enter it...";
      l = this:find_exact(domain);
      if (l == $failed_match)
        this:insert(domain, {who});
      elseif (!(who in l))
        this:insert(domain, setadd(l, who));
      endif
    else
      "...an actual domain name; add player to list for that domain...";
      "...then add domain itself to list for the next larger domain; repeat...";
      "...  Example:  domain == foo.bar.edu:  ";
      "...            enter #who  on foo.bar.edu list";
      "...            enter `foo' on bar.edu list";
      "...            enter `bar' on edu list";
      if (!(dot = index(domain, ".")))
        dot = length(domain) + 1;
        domain = tostr(domain, ".", this.domain);
      endif
      prev = who;
      while ($failed_match == (l = this:find_exact(domain)))
        this:insert(domain, {prev});
        if (dot)
          prev = domain[1..dot - 1];
          domain = domain[dot + 1..$];
        else
          return;
        endif
        dot = index(domain, ".");
      endwhile
      if (!(prev in l))
        this:insert(domain, {@l, prev});
      endif
      return;
    endif
  endverb

  verb load (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":load([start]) -- reloads site_db with the connection places of all players.";
    "This routine calls suspend() if it runs out of time.";
    "WIZARDLY";
    "...needs to be able to read .all_connect_places";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    plist = players();
    if (!args)
      this:clearall();
    elseif (i = args[1] in plist)
      plist[1..i - 1] = {};
    else
      return E_INVARG;
    endif
    for p in (plist)
      if (valid(p) && (is_player(p) && !$object_utils:isa(p, $guest)))
        "... player may be recycled or toaded during the suspend(),...";
        "... guests login from everywhere...";
        for c in (p.all_connect_places)
          this:add(p, c);
          if ($command_utils:running_out_of_time())
            player:tell("...", p);
            suspend(0);
          endif
        endfor
      endif
    endfor
  endverb

  verb domain_literal (this none this) owner: HACKER flags: "rxd"
    ":domain_literal(string)";
    " => true iff string is a domain literal (i.e., numeric IP address).";
    if (10 <= (len = length(hnum = strsub(args[1], ".", ""))))
      return toint(hnum[1..9]) && toint(hnum[6..len]);
    else
      return toint(hnum);
    endif
    "SLEAZY CODE ALERT";
    "... what I wanted to do was return toint(strsub(args[1],\".\",\"\"))";
    "... but on a 32-bit machine, this has a 1 in 4294967296 chance of failing";
    "... (e.g., on \"42.94.967.296\", though I'll grant this particular example";
    "...  entails some very strange subnetting on net 42, to say the least).";
    "... So we do something that is guaranteed to work so long as internet";
    "... addresses stay under 32 bits --- a while yet...";
    "";
    "... As soon as we're sure match() is working, this will become a one-liner:";
    return match(args[1], "[0-9]+%.[0-9]+%.[0-9]+%.[0-9]+");
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this:clearall();
      this.domain = "localdomain";
      this:prune_reset();
    endif
  endverb

  verb prune_alpha (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Carefully loop through the db and delete items associated with !valid and !is_player objects.  If that results in no objects remaining for a site, delete that site.";
    "Attempt to keep memory usage down by only asking for a small number of items at a time.  Should probably have some arguments to control this.";
    "Another thing it should do is be clever about string typed items.  (What did I mean by this?)";
    "New feature: If the site name contains `dialup', then, if none of the users who have connected from there still have it in their .all_connect_places, then consider it trashable.  Maybe this will get some space savings.";
    "To run: call $site_db:prune_reset() then $site_db:prune_alpha().";
    "or $site_db:prune_alpha(1) for verbose output";
    verbose = args && args[1];
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    this.prune_task = task_id();
    probe = this.prune_progress;
    while (probe <= this.prune_stop && length(probe) == length(this.prune_stop))
      for sitename in (z = this:find_all_keys(probe))
        items = this:find_exact(sitename);
        orig = items;
        dialup = index(sitename, "dialup");
        "Don't keep around dialups.";
        for y in (items)
          if (typeof(y) == OBJ && (!valid(y) || !is_player(y) || dialup && !(sitename in y.all_connect_places)))
            verbose && player:tell("removing ", $string_utils:nn(y), " from ", sitename);
            items = setremove(items, y);
          endif
          $command_utils:suspend_if_needed(0);
        endfor
        useless = 1;
        "If no player has this site in eir .all_connect_places, nuke it anyway.";
        for y in (items)
          if (typeof(y) != OBJ || sitename in y.all_connect_places)
            useless = 0;
            break;
            "unfortunately this can get kinna O(n^2).";
          endif
          $command_utils:suspend_if_needed(0);
        endfor
        if (useless)
          verbose && player:tell(sitename, " declared useless and nuked");
          items = {};
        endif
        if (!items)
          this:delete(sitename);
          this.total_pruned_sites = this.total_pruned_sites + 1;
        elseif (items == orig)
        else
          this:insert(sitename, items);
          this.total_pruned_people = this.total_pruned_people + length(orig) - length(items);
        endif
        $command_utils:suspend_if_needed(0);
        if (probe >= this.prune_stop)
          return player:tell("Prune stopped at ", toliteral(this.prune_progress));
        endif
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
    if (typeof(this.prune_progress) == STR)
      alphalen = length(this.alphabet);
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
    player:tell("There were ", this.total_pruned_people, " individual list entries removed, and ", this.total_pruned_sites, " whole sites removed.");
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

  verb prune_fixup (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    if (!args)
      for x in ({"com", "edu", "us", "au", "net", "za", "uk", "at", "ca", "org", "il", "mil", "no", "gov", "se", "fi", "it", "be", "jp", "de", "pt", "sg", "ie", "br", "nl", "gr", "ch", "pl", "nz", "<none>", "<bad>", "ee", "dk", "fr", "si", "cz", "th", "tw", "hk", "su", "es", "kr", "hr", "is", "mx", "my", "ro", "kw", "cl", "ph", "cr", "tr", "in", "eg", "ec", "lv", "ve", "sk", "ar", "co", "pe", "hu", "jm", "ni", "ru", "id", "bm", "mt", "cn", "bg", "pk", "uy", "yu", "ae", "zw", "gi", "sm", "nu"})
        this:prune_fixup(x);
      endfor
      return;
    endif
    root = args[1];
    items = this:find_exact(root);
    orig = items;
    if (items == #-3)
      return 1;
    endif
    $site_db.prune_progress = root;
    $site_db.prune_task = task_id();
    for item in (items)
      if (typeof(item) == STR)
        if (this:prune_fixup(item + "." + root))
          items = setremove(items, item);
        endif
      endif
      if ($command_utils:running_out_of_time())
        set_task_perms($wiz_utils:random_wizard());
        suspend(0);
      endif
    endfor
    if (!items)
      this:delete(root);
      this.total_pruned_sites = this.total_pruned_sites + 1;
      return 1;
    elseif (orig == items)
    else
      this:insert(root, items);
    endif
  endverb

  verb prune_numeric (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Carefully loop through the db and delete items associated with !valid and !is_player objects.  If that results in no objects remaining for a site, delete that site.";
    "Attempt to keep memory usage down by only asking for a small number of items at a time.  Should probably have some arguments to control this.";
    "Another thing it should do is be clever about string typed items.";
    "Rewriting this to do numerics now.";
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    this.prune_task = task_id();
    probe = this.prune_progress;
    while (probe[1] <= this.prune_stop)
      probestring = tostr(probe[1], ".", probe[2], ".");
      for sitename in (z = this:find_all_keys(probestring))
        items = this:find_exact(sitename);
        orig = items;
        for y in (items)
          if (typeof(y) == OBJ && (!valid(y) || !is_player(y)))
            items = setremove(items, y);
          endif
          $command_utils:suspend_if_needed(0);
        endfor
        if (!items)
          this:delete(sitename);
          this.total_pruned_sites = this.total_pruned_sites + 1;
        elseif (items == orig)
        else
          this:insert(sitename, items);
          this.total_pruned_people = this.total_pruned_people + length(orig) - length(items);
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      if (probe[2] == 255)
        probe[1] = probe[1] + 1;
        probe[2] = 0;
      else
        probe[2] = probe[2] + 1;
      endif
      this.prune_progress = probe;
      if ($command_utils:running_out_of_time())
        set_task_perms($wiz_utils:random_wizard());
        suspend(0);
      endif
    endwhile
    player:tell("Prune stopped at ", toliteral(this.prune_progress));
  endverb

  verb schedule_prune (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    day = 24 * 3600;
    hour_of_day_GMT = 9;
    target = hour_of_day_GMT * 60 * 60 + day - time() % day;
    if (target > 86400)
      target = target - 86400;
    endif
    fork (target)
      "Stop at 2am before checkpoint.";
      if ($code_utils:task_valid(this.prune_task))
        $site_db.prune_stop = "aaa";
        "Restart after 3am.  Er, 4am.";
        suspend(7500);
        this:schedule_prune();
        $site_db.prune_stop = "zzz";
        "Just in case it didn't actually stop...";
        if (!$code_utils:task_valid(this.prune_task))
          $site_db:prune_alpha();
        endif
      endif
    endfork
  endverb

  verb prune_reset (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    this:report_prune_progress();
    player:tell("Resetting...");
    this.total_pruned_sites = 0;
    this.total_pruned_people = 0;
    this.prune_progress = "aaa";
    this.prune_stop = "zzz";
    `kill_task(this.prune_task) ! ANY';
  endverb
endobject