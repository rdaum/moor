object THING
  name: "generic thing"
  parent: ROOT_CLASS
  owner: #2
  fertile: true
  readable: true

  property drop_failed_msg (owner: #2, flags: "rc") = "You can't seem to drop %t here.";
  property drop_succeeded_msg (owner: #2, flags: "rc") = "You drop %t.";
  property odrop_failed_msg (owner: #2, flags: "rc") = "tries to drop %t but fails!";
  property odrop_succeeded_msg (owner: #2, flags: "rc") = "drops %t.";
  property otake_failed_msg (owner: #2, flags: "rc") = "";
  property otake_succeeded_msg (owner: #2, flags: "rc") = "picks up %t.";
  property take_failed_msg (owner: #2, flags: "rc") = "You can't pick that up.";
  property take_succeeded_msg (owner: #2, flags: "rc") = "You take %t.";

  override aliases = {"generic thing"};
  override object_size = {4787, 1084848672};

  verb "g*et t*ake" (this none none) owner: #2 flags: "rxd"
    set_task_perms(callers() ? caller_perms() | player);
    if (this.location == player)
      player:tell("You already have that!");
    elseif (this.location != player.location)
      player:tell("I don't see that here.");
    else
      this:moveto(player);
      if (this.location == player)
        player:tell(this:take_succeeded_msg() || "Taken.");
        if (msg = this:otake_succeeded_msg())
          player.location:announce(player.name, " ", msg);
        endif
      else
        player:tell(this:take_failed_msg() || "You can't pick that up.");
        if (msg = this:otake_failed_msg())
          player.location:announce(player.name, " ", msg);
        endif
      endif
    endif
  endverb

  verb "d*rop th*row" (this none none) owner: #2 flags: "rxd"
    set_task_perms(callers() ? caller_perms() | player);
    if (this.location != player)
      player:tell("You don't have that.");
    elseif (!player.location:acceptable(this))
      player:tell("You can't drop that here.");
    else
      this:moveto(player.location);
      if (this.location == player.location)
        player:tell_lines(this:drop_succeeded_msg() || "Dropped.");
        if (msg = this:odrop_succeeded_msg())
          player.location:announce(player.name, " ", msg);
        endif
      else
        player:tell_lines(this:drop_failed_msg() || "You can't seem to drop that here.");
        if (msg = this:odrop_failed_msg())
          player.location:announce(player.name, " ", msg);
        endif
      endif
    endif
  endverb

  verb moveto (this none this) owner: #2 flags: "rxd"
    where = args[1];
    "if (!valid(where) || this:is_unlocked_for(where))";
    if (this:is_unlocked_for(where))
      pass(where);
    endif
  endverb

  verb "take_failed_msg take_succeeded_msg otake_failed_msg otake_succeeded_msg drop_failed_msg drop_succeeded_msg odrop_failed_msg odrop_succeeded_msg" (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    return $string_utils:pronoun_sub(this.(verb));
  endverb

  verb "gi*ve ha*nd" (this at any) owner: #2 flags: "rxd"
    set_task_perms(callers() ? caller_perms() | player);
    if (this.location != player)
      player:tell("You don't have that!");
    elseif (!valid(player.location))
      player:tell("I see no \"", iobjstr, "\" here.");
    elseif ($command_utils:object_match_failed(who = player.location:match_object(iobjstr), iobjstr))
    elseif (who.location != player.location)
      player:tell("I see no \"", iobjstr, "\" here.");
    elseif (who == player)
      player:tell("Give it to yourself?");
    else
      this:moveto(who);
      if (this.location == who)
        player:tell("You hand ", this:title(), " to ", who:title(), ".");
        who:tell(player:titlec(), " ", $gender_utils:get_conj("hands/hand", player), " you ", this:title(), ".");
      else
        player:tell(who:titlec(), " ", $gender_utils:get_conj("does/do", who), " not want that item.");
      endif
    endif
  endverb

  verb examine_key (this none this) owner: #2 flags: "rxd"
    "examine_key(examiner)";
    "return a list of strings to be told to the player, indicating what the key on this type of object means, and what this object's key is set to.";
    "the default will only tell the key to a wizard or this object's owner.";
    who = args[1];
    if (caller == this && $perm_utils:controls(who, this) && this.key != 0)
      return {tostr(this:title(), " can only be moved to locations matching this key:"), tostr("  ", $lock_utils:unparse_key(this.key))};
    endif
  endverb
endobject