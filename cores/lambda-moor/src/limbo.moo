object LIMBO
  name: "Limbo"
  parent: ROOT_CLASS
  owner: #2
  readable: true

  override aliases = {"The Body Bag"};
  override object_size = {2330, 1084848672};

  verb acceptable (this none this) owner: #2 flags: "rxd"
    what = args[1];
    return is_player(what) && !(what in connected_players());
  endverb

  verb confunc (this none this) owner: #2 flags: "rxd"
    caller == #0 || raise(E_PERM);
    {who} = args;
    "this:eject(who)";
    if (!$recycler:valid(home = who.home))
      clear_property(who, "home");
      home = who.home;
      if (!$recycler:valid(home))
        home = who.home = $player_start;
      endif
    endif
    "Modified 08-22-98 by TheCat to foil people who manually set their home to places they shouldn't.";
    if (!home:acceptable(who) || !home:accept_for_abode(who))
      home = $player_start;
    endif
    try
      move(who, home);
    except (ANY)
      move(who, $player_start);
    endtry
    who.location:announce_all_but({who}, who.name, " has connected.");
  endverb

  verb who_location_msg (this none this) owner: HACKER flags: "rxd"
    return $player_start:who_location_msg(@args);
  endverb

  verb moveto (this none this) owner: HACKER flags: "rxd"
    "Don't go anywhere.";
  endverb

  verb eject (this none this) owner: #2 flags: "rxd"
    if ($perm_utils:controls(caller_perms(), this))
      if ((what = args[1]).wizard && what.location == this)
        move(what, what.home);
      else
        return pass(@args);
      endif
    endif
  endverb
endobject