object GUEST_LOG
  name: "Guest Log"
  parent: ROOT_CLASS
  owner: #2

  property connections (owner: #2, flags: "") = {};
  property max_entries (owner: #2, flags: "") = 511;

  override aliases = {"Guest Log"};
  override import_export_id = "guest_log";
  override object_size = {3738, 1084848672};

  verb enter (this none this) owner: #2 flags: "rxd"
    ":enter(who,islogin,time,site)";
    "adds an entry to the connection log for a given guest (caller).";
    if ($object_utils:isa(caller, $guest))
      $guest_log.connections = {{caller, @args}, @($guest_log.connections)[1..min($guest_log.max_entries, $)]};
    else
      return E_PERM;
    endif
  endverb

  verb last (this none this) owner: #2 flags: "rxd"
    ":last([n,[guest_list]])";
    "print list of the last n entries in the guest log";
    " (use n=0 if you want all entries)";
    " optional second arg limits listing to the specified guest(s)";
    set_task_perms(caller_perms());
    {?howmany = 0, ?which = 0} = args;
    howmany = min(howmany || $maxint, length($guest_log.connections));
    if (!caller_perms().wizard)
      player:notify("Sorry.");
    else
      current = {};
      listing = {};
      last = 0;
      for c in (($guest_log.connections)[1..howmany])
        if (which && !(c[1] in which))
        elseif (c[2])
          "...login...";
          if (a = $list_utils:assoc(c[1], current))
            listing[a[2]][3] = c[3];
            current = setremove(current, a);
          else
            listing = {@listing, {c[1], c[4], c[3], $object_utils:connected(c[1]) ? -idle_seconds(c[1]) | 1}};
            last = last + 1;
          endif
        else
          "...logout...";
          listing = {@listing, {c[1], c[4], 0, c[3]}};
          last = last + 1;
          if (i = $list_utils:iassoc(c[1], current))
            current[i][2] = last;
          else
            current = {@current, {c[1], last}};
          endif
        endif
        $command_utils:suspend_if_needed(2);
      endfor
      su = $string_utils;
      player:notify(su:left(su:left(su:left("Guest", 20) + "Connected", 36) + "Idle/Disconn.", 52) + "From");
      player:notify(su:left(su:left(su:left("-----", 20) + "---------", 36) + "-------------", 52) + "----");
      for l in (listing)
        on = l[3] ? (ct = ctime(l[3]))[1..3] + ct[9..19] | "earlier";
        off = l[4] > 0 ? (ct = ctime(l[4]))[1..3] + ct[9..19] | "  " + $string_utils:from_seconds(-l[4]);
        player:notify(su:left(su:left(su:right(tostr(strsub(l[1].name, "uest", "."), " (", l[1], ")  "), -20) + on, 36) + off, 52) + l[2]);
        $command_utils:suspend_if_needed(2);
      endfor
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.connections = {};
    endif
  endverb

  verb find (this none this) owner: #2 flags: "rxd"
    ":find(guest_id,time)";
    " => site name of guest logged in at that time";
    " => 0 if not logged in";
    " => E_NACC if this is earlier than the earliest guest recorded";
    set_task_perms(caller_perms());
    {who, when} = args;
    if (!caller_perms().wizard)
      raise(E_PERM);
    else
      found = who in connected_players() ? $string_utils:connection_hostname(who.last_connect_place) | 0;
      for c in ($guest_log.connections)
        if (c[3] < when)
          return found;
        elseif (c[1] != who)
          "... different guest...";
        elseif (c[2])
          "...login...";
          if (c[3] == when)
            return found;
          endif
          found = 0;
        else
          "...logout...";
          found = c[4];
        endif
      endfor
      return E_NACC;
    endif
  endverb
endobject