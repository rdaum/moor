object CONTAINER
  name: "generic container"
  parent: THING
  owner: BYTE_QUOTA_UTILS_WORKING
  fertile: true
  readable: true

  property close_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You close %d.";
  property dark (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 1;
  property empty_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "It is empty.";
  property oclose_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "closes %d.";
  property oopen_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "";
  property oopen_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "opens %d.";
  property opaque (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 1;
  property open_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You can't open that.";
  property open_key (owner: BYTE_QUOTA_UTILS_WORKING, flags: "c") = 0;
  property open_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You open %d.";
  property opened (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property oput_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "";
  property oput_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "puts %d in %i.";
  property oremove_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "";
  property oremove_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "removes %d from %i.";
  property put_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You can't put %d in that.";
  property put_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You put %d in %i.";
  property remove_fail_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You can't remove that.";
  property remove_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You remove %d from %i.";

  override aliases = {"generic container"};
  override object_size = {9415, 1084848672};

  verb "p*ut in*sert d*rop" (any in this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (this.location != player && this.location != player.location)
      player:tell("You can't get at ", this.name, ".");
    elseif (dobj == $nothing)
      player:tell("What do you want to put ", prepstr, " ", this.name, "?");
    elseif ($command_utils:object_match_failed(dobj, dobjstr))
    elseif (dobj.location != player && dobj.location != player.location)
      player:tell("You don't have ", dobj.name, ".");
    elseif (!this.opened)
      player:tell(this.name, " is closed.");
    else
      set_task_perms(callers() ? caller_perms() | player);
      dobj:moveto(this);
      if (dobj.location == this)
        player:tell(this:put_msg());
        if (msg = this:oput_msg())
          player.location:announce(player.name, " ", msg);
        endif
      else
        player:tell(this:put_fail_msg());
        if (msg = this:oput_fail_msg())
          player.location:announce(player.name, " ", msg);
        endif
      endif
    endif
  endverb

  verb "re*move ta*ke g*et" (any from this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!(this.location in {player, player.location}))
      player:tell("Sorry, you're too far away.");
    elseif (!this.opened)
      player:tell(this.name, " is not open.");
    elseif (this.dark)
      player:tell("You can't see into ", this.name, " to remove anything.");
    elseif ((dobj = this:match_object(dobjstr)) == $nothing)
      player:tell("What do you want to take from ", this.name, "?");
    elseif ($command_utils:object_match_failed(dobj, dobjstr))
    elseif (!(dobj in this:contents()))
      player:tell(dobj.name, " isn't in ", this.name, ".");
    else
      set_task_perms(callers() ? caller_perms() | player);
      dobj:moveto(player);
      if (dobj.location == player)
        player:tell(this:remove_msg());
        if (msg = this:oremove_msg())
          player.location:announce(player.name, " ", msg);
        endif
      else
        dobj:moveto(this.location);
        if (dobj.location == this.location)
          player:tell(this:remove_msg());
          if (msg = this:oremove_msg())
            player.location:announce(player.name, " ", msg);
          endif
          player:tell("You can't pick up ", dobj.name, ", so it tumbles onto the floor.");
        else
          player:tell(this:remove_fail_msg());
          if (msg = this:oremove_fail_msg())
            player.location:announce(player.name, " ", msg);
          endif
        endif
      endif
    endif
  endverb

  verb look_self (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    pass();
    if (!this.dark)
      this:tell_contents();
    endif
  endverb

  verb acceptable (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return !is_player(args[1]);
  endverb

  verb open (this none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    perms = callers() && caller != this ? caller_perms() | player;
    if (this.opened)
      player:tell("It's already open.");
      "elseif (this:is_openable_by(player))";
    elseif (this:is_openable_by(perms))
      this:set_opened(1);
      player:tell(this:open_msg());
      if (msg = this:oopen_msg())
        player.location:announce(player.name, " ", msg);
      endif
    else
      player:tell(this:open_fail_msg());
      if (msg = this:oopen_fail_msg())
        player.location:announce(player.name, " ", msg);
      endif
    endif
  endverb

  verb "@lock_for_open @lock-for-open" (this with any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    key = $lock_utils:parse_keyexp(iobjstr, player);
    if (typeof(key) == STR)
      player:tell("That key expression is malformed:");
      player:tell("  ", key);
    else
      try
        this.open_key = key;
        player:tell("Locked opening of ", this.name, " with this key:");
        player:tell("  ", $lock_utils:unparse_key(key));
      except error (ANY)
        player:tell(error[2], ".");
      endtry
    endif
  endverb

  verb is_openable_by (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this.open_key == 0 || $lock_utils:eval_key(this.open_key, args[1]);
  endverb

  verb close (this none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!this.opened)
      player:tell("It's already closed.");
    else
      this:set_opened(0);
      player:tell(this:close_msg());
      if (msg = this:oclose_msg())
        player.location:announce(player.name, " ", msg);
      endif
    endif
  endverb

  verb "@unlock_for_open @unlock-for-open" (this none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    try
      dobj.open_key = 0;
      player:tell("Unlocked ", dobj.name, " for opening.");
    except error (ANY)
      player:tell(error[2], ".");
    endtry
  endverb

  verb tell_contents (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (this.contents)
      player:tell("Contents:");
      for thing in (this:contents())
        player:tell("  ", thing:title());
      endfor
    elseif (msg = this:empty_msg())
      player:tell(msg);
    endif
  endverb

  verb set_opened (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!$perm_utils:controls(caller.owner, this))
      return E_PERM;
    else
      this.opened = opened = !(!args[1]);
      this.dark = this.opaque > opened;
      return opened;
    endif
  endverb

  verb "@opacity" (this is any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (!$perm_utils:controls(player, this))
      player:tell("Can't set opacity of something you don't own.");
    elseif (iobjstr != "0" && !toint(iobjstr))
      player:tell("Opacity must be an integer (0, 1, 2).");
    else
      player:tell("Opacity changed:  Now " + {"transparent.", "opaque.", "a black hole."}[1 + this:set_opaque(toint(iobjstr))]);
    endif
  endverb

  verb set_opaque (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!$perm_utils:controls(caller.owner, this))
      return E_PERM;
    elseif (typeof(number = args[1]) != INT)
      return E_INVARG;
    else
      number = number < 0 ? 0 | (number > 2 ? 2 | number);
      this.dark = number > this.opened;
      return this.opaque = number;
    endif
  endverb

  verb "oclose_msg close_msg oopen_msg open_msg oput_fail_msg put_fail_msg oremove_fail_msg oremove_msg remove_fail_msg remove_msg oput_msg put_msg oopen_fail_msg open_fail_msg empty_msg" (this none this) owner: HACKER flags: "rxd"
    return (msg = `this.(verb) ! ANY') ? $string_utils:pronoun_sub(msg) | "";
  endverb

  verb dark (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this.(verb);
  endverb
endobject