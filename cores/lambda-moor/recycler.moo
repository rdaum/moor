object RECYCLER
  name: "Recycling Center"
  parent: THING
  owner: HACKER
  readable: true

  property announce_removal_msg (owner: HACKER, flags: "rc") = "";
  property history (owner: HACKER, flags: "") = {};
  property lost_souls (owner: HACKER, flags: "rc") = {};
  property nhist (owner: HACKER, flags: "") = 50;
  property orphans (owner: HACKER, flags: "r") = {};

  override aliases = {"Recycling Center", "Center"};
  override description = "Object reuse. Call $recycler:_create() to create an object (semantics the same as create()), $recycler:_recycle() to recycle an object. Will create a new object if nothing available in its contents. Note underscores, to avoid builtin :recycle() verb called when objects are recycled. Uses $building_utils:recreate() to prepare objects.";
  override object_size = {11836, 1084848672};

  verb _recreate (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Return a toad (child of #1, owned by $hacker) from this.contents.  Move it to #-1.  Recreate as a child of args[1], or of #1 if no args are given.  Chown to caller_perms() or args[2] if present.";
    {?what = #1, ?who = caller_perms()} = args;
    if (!(caller_perms().wizard || who == caller_perms()))
      return E_PERM;
    elseif (!(valid(what) && is_player(who)))
      return E_INVARG;
    elseif (who != what.owner && !what.f && !who.wizard && !caller_perms().wizard)
      return E_PERM;
    endif
    for potential in (this.contents)
      if (potential.owner == $hacker && parent(potential) == $garbage && !children(potential))
        return this:setup_toad(potential, who, what);
      endif
    endfor
    return E_NONE;
  endverb

  verb _recycle (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Take the object in args[1], and turn it into a child of #1 owned by $hacker.";
    "If the object is a player, decline.";
    item = args[1];
    if (!$perm_utils:controls(caller_perms(), item))
      raise(E_PERM);
    elseif (is_player(item))
      raise(E_INVARG);
    endif
    this:addhist(caller_perms(), item);
    "...recreate can fail (:recycle can crash)...";
    this:add_orphan(item);
    this:kill_all_tasks(item);
    $quota_utils:preliminary_reimburse_quota(item.owner, item);
    $building_utils:recreate(item, $garbage);
    this:remove_orphan(item);
    "...";
    $wiz_utils:set_owner(item, $hacker);
    item.name = tostr("Recyclable ", item);
    `move(item, this) ! ANY => 0';
  endverb

  verb _create (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    e = `set_task_perms(caller_perms()) ! ANY';
    if (typeof(e) == ERR)
      return e;
    else
      val = this:_recreate(@args);
      return val == E_NONE ? $quota_utils:bi_create(@args) | val;
    endif
  endverb

  verb addhist (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this)
      h = this.history;
      if ((len = length(h)) > this.nhist)
        h = h[len - this.nhist..len];
      endif
      this.history = {@h, args};
    endif
  endverb

  verb "show*-history" (this none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ($perm_utils:controls(valid(caller_perms()) ? caller_perms() | player, this))
      for x in (this.history)
        pname = valid(x[1]) ? (x[1]).name | "A recycled player";
        oname = valid(x[2]) ? (x[2]).name | "recycled";
        player:notify(tostr(pname, " (", x[1], ") recycled ", x[2], " (now ", oname, ")"));
      endfor
    else
      player:tell("Sorry.");
    endif
  endverb

  verb request (any from this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "added check that obj is already $garbage - Bits 12/16/5";
    if (!(caller_perms() in {player, #-1}))
      raise(E_PERM);
    endif
    dobj = valid(dobj) ? dobj | $string_utils:match_object(dobjstr, player.location);
    if (!valid(dobj))
      parsed_obj = $code_utils:toobj(dobjstr);
      dobj = parsed_obj == E_TYPE ? #-1 | parsed_obj;
    endif
    if (!valid(dobj))
      player:tell("Couldn't parse ", dobjstr, " as a valid object number.");
    elseif (!(dobj in this.contents))
      player:tell("Couldn't find ", dobj, " in ", this.name, ".");
    elseif (!$object_utils:isa(dobj, $garbage))
      player:tell("Sorry, that isn't recyclable.");
    elseif ($object_utils:has_callable_verb(this, "request_refused") && (msg = this:request_refused(player, dobj)))
      player:tell("Sorry, can't do that:  ", msg);
    else
      if (typeof(emsg = this:setup_toad(dobj, player, $root_class)) != ERR)
        dobj:moveto(player);
        dobj.aliases = {dobj.name = "Object " + tostr(dobj)};
        player:tell("You now have ", dobj, " ready for @recreation.");
        if (this.announce_removal_msg)
          player.location:announce($string_utils:pronoun_sub(this.announce_removal_msg));
        endif
      else
        player:tell(emsg);
      endif
    endif
  endverb

  verb setup_toad (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "this:setup_toad(objnum,new_owner,parent)";
    "Called by :_create and :request.";
    if (caller != this)
      return E_PERM;
    endif
    {potential, who, what} = args;
    if (!$quota_utils:creation_permitted(who))
      return E_QUOTA;
    else
      $wiz_utils:set_owner(potential, who);
      move(potential, #-1);
      set_task_perms({@callers(), {#-1, "", player}}[2][3]);
      "... if :initialize crashes...";
      this:add_orphan(potential);
      $building_utils:recreate(potential, what);
      this:remove_orphan(potential);
      "... if we don't get this far, the object stays on the orphan list...";
      "... orphan list should be checked periodically...";
      return potential;
    endif
  endverb

  verb add_orphan (this none this) owner: HACKER flags: "rxd"
    if (caller == this)
      this.orphans = setadd(this.orphans, args[1]);
    endif
  endverb

  verb remove_orphan (this none this) owner: HACKER flags: "rxd"
    if (caller == this)
      this.orphans = setremove(this.orphans, args[1]);
    endif
  endverb

  verb valid (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Usage:  valid(object)";
    "True if object is valid and not $garbage.";
    return valid(args[1]) && parent(args[1]) != $garbage;
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      this.orphans = {};
      this.history = {};
      this.lost_souls = {};
      pass(@args);
    endif
  endverb

  verb resurrect (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    who = caller_perms();
    if (!valid(parent = {@args, $garbage}[1]))
      return E_INVARG;
    elseif (!who.wizard)
      return E_PERM;
    elseif (typeof(o = renumber($quota_utils:bi_create(parent, $hacker))) == ERR)
      "..death...";
    elseif (parent == $garbage)
      $recycler:_recycle(o);
    else
      o.aliases = {o.name = tostr("Resurrectee ", o)};
      $wiz_utils:set_owner(o, who);
      move(o, who);
    endif
    reset_max_object();
    return o;
  endverb

  verb reclaim_lost_souls (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    fork (1800)
      this:(verb)();
    endfork
    for x in (this.lost_souls)
      this.lost_souls = setremove(this.lost_souls, x);
      if (valid(x) && typeof(x.owner.owned_objects) == LIST && !(x in x.owner.owned_objects))
        x.owner.owned_objects = setadd(x.owner.owned_objects, x);
        $quota_utils:summarize_one_user(x.owner);
      endif
      $command_utils:suspend_if_needed(0);
    endfor
  endverb

  verb look_self (this none this) owner: HACKER flags: "rxd"
    if (prepstr in {"in", "inside", "into"})
      recycler = this;
      linelen = (linelen = abs(player.linelen)) < 20 ? 78 | linelen;
      intercolumn_gap = 2;
      c_width = length(tostr(max_object())) + intercolumn_gap;
      n_columns = (linelen + (c_width - 1)) / c_width;
      things = $list_utils:sort_suspended(0, this.contents);
      header = tostr(this.name, " (", this, ") contains:");
      player:tell_lines({header, @$string_utils:columnize_suspended(0, things, n_columns)});
    else
      return pass(@args);
    endif
    "This code contributed by Mickey.";
  endverb

  verb check_quota_scam (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    who = args[1];
    if ($quota_utils.byte_based && (is_clear_property(who, "size_quota") || is_clear_property(who, "owned_objects")))
      raise(E_QUOTA);
    endif
    cheater = 0;
    other_cheaters = {};
    for x in (this.lost_souls)
      if (valid(x) && (owner = x.owner) != $hacker && typeof(owner.owned_objects) == LIST && !(x in owner.owned_objects))
        if (owner == who)
          who.owned_objects = setadd(who.owned_objects, x);
          cheater = 1;
        else
          "it's someone else's quota scam we're detecting...";
          other_cheaters = setadd(other_cheaters, owner);
          owner.owned_objects = setadd(owner.owned_objects, x);
          this.lost_souls = setremove(this.lost_souls, x);
        endif
      endif
      this.lost_souls = setremove(this.lost_souls, x);
    endfor
    if ($quota_utils.byte_based)
      if (cheater)
        $quota_utils:summarize_one_user(who);
      endif
      if (other_cheaters)
        fork (0)
          for x in (other_cheaters)
            $quota_utils:summarize_one_user(x);
          endfor
        endfork
      endif
    endif
  endverb

  verb gc (this none this) owner: HACKER flags: "rxd"
    for x in (this.orphans)
      if (!valid(x) || (x.owner != $hacker && x in x.owner.owned_objects))
        this.orphans = setremove(this.orphans, x);
      endif
    endfor
  endverb

  verb moveto (this none this) owner: HACKER flags: "rxd"
    pass(#-1);
  endverb

  verb kill_all_tasks (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "kill_all_tasks ( object being recycled )";
    " -- kill all tasks involving this now-recycled object";
    caller == this || caller == #0 || raise(E_PERM);
    {object} = args;
    typeof(object) == OBJ || raise(E_INVARG);
    if (!valid(object) || parent(object) != $garbage)
      fork (0)
        for t in (queued_tasks())
          for c in (`task_stack(t[1]) ! E_INVARG => {}')
            if (object in c)
              kill_task(t[1]);
              continue t;
            endif
          endfor
        endfor
      endfork
    endif
  endverb
endobject
