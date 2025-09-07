object EXIT
  name: "generic exit"
  parent: ROOT_CLASS
  owner: BYTE_QUOTA_UTILS_WORKING
  fertile: true
  readable: true

  property arrive_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property dest (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = LOCAL;
  property leave_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property nogo_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property oarrive_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property obvious (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 1;
  property oleave_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property onogo_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property source (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = LOCAL;

  override aliases = {"generic exit"};
  override object_size = {7191, 1084848672};

  verb invoke (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    this:move(player);
  endverb

  verb move (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    what = args[1];
    "if ((what.location != this.source) || (!(this in this.source.exits)))";
    "  player:tell(\"You can't go that way.\");";
    "  return;";
    "endif";
    unlocked = this:is_unlocked_for(what);
    if (unlocked)
      this.dest:bless_for_entry(what);
    endif
    if (unlocked && this.dest:acceptable(what))
      start = what.location;
      if (msg = this:leave_msg(what))
        what:tell_lines(msg);
      endif
      what:moveto(this.dest);
      if (what.location != start)
        "Don't print oleave messages if WHAT didn't actually go anywhere...";
        this:announce_msg(start, what, this:oleave_msg(what) || this:defaulting_oleave_msg(what) || "has left.");
      endif
      if (what.location == this.dest)
        "Don't print arrive messages if WHAT didn't really end up there...";
        if (msg = this:arrive_msg(what))
          what:tell_lines(msg);
        endif
        this:announce_msg(what.location, what, this:oarrive_msg(what) || "has arrived.");
      endif
    else
      if (msg = this:nogo_msg(what))
        what:tell_lines(msg);
      else
        what:tell("You can't go that way.");
      endif
      if (msg = this:onogo_msg(what))
        this:announce_msg(what.location, what, msg);
      endif
    endif
  endverb

  verb recycle (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      try
        this.source:remove_exit(this);
        this.dest:remove_entrance(this);
      except id (ANY)
      endtry
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb "leave_msg oleave_msg arrive_msg oarrive_msg nogo_msg onogo_msg" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    msg = this.(verb);
    return msg ? $string_utils:pronoun_sub(msg, @args) | "";
  endverb

  verb set_name (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ($perm_utils:controls(cp = caller_perms(), this) || valid(this.source) && this.source.owner == cp)
      return typeof(e = `this.name = args[1] ! ANY') != ERR || e;
    else
      return E_PERM;
    endif
  endverb

  verb set_aliases (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ($perm_utils:controls(cp = caller_perms(), this) || valid(this.source) && this.source.owner == cp)
      if (typeof(e = `this.aliases = args[1] ! ANY') == ERR)
        return e;
      else
        return 1;
      endif
    else
      return E_PERM;
    endif
  endverb

  verb announce_all_but (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This is intended to be called only by exits, for announcing various oxxx messages.  First argument is room to announce in.  Second argument is as in $room:announce_all_but's first arg, who not to announce to.  Rest args are what to say.  If the final arg is a list, prepends all the other rest args to the first line and emits the lines separately.";
    where = args[1];
    whobut = args[2];
    last = args[$];
    if (typeof(last) == LIST)
      where:announce_all_but(whobut, @args[3..$ - 1], last[1]);
      for line in (last[2..$])
        where:announce_all_but(whobut, line);
      endfor
    else
      where:announce_all_but(@args[3..$]);
    endif
  endverb

  verb defaulting_oleave_msg (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    for k in ({this.name, @this.aliases})
      if (k in {"east", "west", "south", "north", "northeast", "southeast", "southwest", "northwest", "out", "up", "down", "nw", "sw", "ne", "se", "in"})
        return "goes " + k + ".";
      elseif (k in {"leave", "out", "exit"})
        return "leaves";
      endif
    endfor
    if (index(this.name, "an ") == 1 || index(this.name, "a ") == 1)
      return "leaves for " + this.name + ".";
    else
      return "leaves for the " + this.name + ".";
    endif
  endverb

  verb moveto (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller in {this, this.owner} || $perm_utils:controls(caller_perms(), this))
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb examine_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "examine_key(examiner)";
    "return a list of strings to be told to the player, indicating what the key on this type of object means, and what this object's key is set to.";
    "the default will only tell the key to a wizard or this object's owner.";
    who = args[1];
    if (caller == this && $perm_utils:controls(who, this) && this.key != 0)
      return {tostr(this:title(), " will only transport objects matching this key:"), tostr("  ", $lock_utils:unparse_key(this.key))};
    endif
  endverb

  verb announce_msg (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":announce_msg(place, what, msg)";
    "  announce msg in place (except to what). Prepend with what:title if it isn't part of the string";
    msg = args[3];
    what = args[2];
    title = what:titlec();
    if (!$string_utils:index_delimited(msg, title))
      msg = tostr(title, " ", msg);
    endif
    (args[1]):announce_all_but({what}, msg);
  endverb
endobject