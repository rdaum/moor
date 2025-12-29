object ROOT_CLASS
  name: "Root Class"
  owner: #2
  fertile: true
  readable: true

  property aliases (owner: #2, flags: "rc") = {};
  property description (owner: #2, flags: "rc") = "";
  property import_export_id (owner: #2, flags: "r") = "root_class";
  property key (owner: #2, flags: "c") = 0;
  property object_size (owner: HACKER, flags: "r") = {22038, 1084848672};

  verb initialize (this none this) owner: #2 flags: "rxd"
    if (typeof(this.owner.owned_objects) == TYPE_LIST)
      this.owner.owned_objects = setadd(this.owner.owned_objects, this);
    endif
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      if (is_clear_property(this, "object_size"))
        "If this isn't clear, then we're being hacked.";
        this.object_size = {0, 0};
      endif
      this.key = 0;
    else
      return E_PERM;
    endif
  endverb

  verb recycle (this none this) owner: #2 flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      try
        if (typeof(this.owner.owned_objects) == TYPE_LIST && !is_clear_property(this.owner, "owned_objects"))
          this.owner.owned_objects = setremove(this.owner.owned_objects, this);
          $recycler.lost_souls = setadd($recycler.lost_souls, this);
        endif
      except (ANY)
        "Oy, doesn't have a .owned_objects??, or maybe .owner is $nothing";
        "Should probably do something...like send mail somewhere.";
      endtry
    else
      return E_PERM;
    endif
  endverb

  verb set_name (this none this) owner: #2 flags: "rxd"
    "set_name(newname) attempts to change this.name to newname";
    "  => E_PERM   if you don't own this or aren't its parent, or are a player trying to do an end-run around $player_db...";
    if (!caller_perms().wizard && (is_player(this) || (caller_perms() != this.owner && this != caller)))
      return E_PERM;
    else
      return typeof(e = `this.name = args[1] ! ANY') != TYPE_ERR || e;
    endif
  endverb

  verb title (this none this) owner: #2 flags: "rxd"
    return this.name;
  endverb

  verb titlec (this none this) owner: #2 flags: "rxd"
    return `this.namec ! E_PROPNF => $string_utils:capitalize(this:title())';
  endverb

  verb set_aliases (this none this) owner: #2 flags: "rxd"
    "set_aliases(alias_list) attempts to change this.aliases to alias_list";
    "  => E_PERM   if you don't own this or aren't its parent";
    "  => E_TYPE   if alias_list is not a list";
    "  => E_INVARG if any element of alias_list is not a string";
    "  => 1        if aliases are set exactly as expected (default)";
    "  => 0        if aliases were set differently than expected";
    "              (children with custom :set_aliases should be aware of this)";
    if (!($perm_utils:controls(caller_perms(), this) || this == caller))
      return E_PERM;
    elseif (typeof(aliases = args[1]) != TYPE_LIST)
      return E_TYPE;
    else
      for s in (aliases)
        if (typeof(s) != TYPE_STR)
          return E_INVARG;
        endif
      endfor
      this.aliases = aliases;
      return 1;
    endif
  endverb

  verb match (this none this) owner: #2 flags: "rxd"
    c = this:contents();
    return $string_utils:match(args[1], c, "name", c, "aliases");
  endverb

  verb match_object (this none this) owner: #2 flags: "rxd"
    ":match_object(string [,who])";
    args[2..1] = {this};
    return $string_utils:match_object(@args);
  endverb

  verb set_description (this none this) owner: #2 flags: "rxd"
    "set_description(newdesc) attempts to change this.description to newdesc";
    "  => E_PERM   if you don't own this or aren't its parent";
    $perm_utils:controls(caller_perms(), this) || this == caller || return E_PERM;
    typeof(desc = args[1]) in {TYPE_LIST, TYPE_STR} || return E_TYPE;
    this.description = desc;
    return 1;
  endverb

  verb description (this none this) owner: #2 flags: "rxd"
    return this.description;
  endverb

  verb look_self (this none this) owner: #2 flags: "rxd"
    desc = this:description();
    if (desc)
      player:tell_lines(desc);
    else
      player:tell("You see nothing special.");
    endif
  endverb

  verb notify (this none this) owner: #2 flags: "rxd"
    if (is_player(this))
      notify(this, @args);
    endif
  endverb

  verb tell (this none this) owner: #2 flags: "rxd"
    this:notify(tostr(@args));
  endverb

  verb tell_lines (this none this) owner: #2 flags: "rxd"
    lines = args[1];
    if (typeof(lines) == TYPE_LIST)
      for line in (lines)
        this:tell(line);
      endfor
    else
      this:tell(lines);
    endif
  endverb

  verb accept (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    return this:acceptable(@args);
  endverb

  verb moveto (this none this) owner: #2 flags: "rxd"
    set_task_perms(this.owner);
    return `move(this, args[1]) ! ANY';
  endverb

  verb "eject eject_nice eject_basic" (this none this) owner: #2 flags: "rxd"
    "eject(victim) --- usable by the owner of this to remove victim from this.contents.  victim goes to its home if different from here, or $nothing or $player_start according as victim is a player.";
    "eject_basic(victim) --- victim goes to $nothing or $player_start according as victim is a player; victim:moveto is not called.";
    what = args[1];
    nice = verb != "eject_basic";
    perms = caller_perms();
    if (!perms.wizard && perms != this.owner)
      raise(E_PERM);
    elseif (!(what in this.contents) || what.wizard)
      return 0;
    endif
    if (nice && $object_utils:has_property(what, "home") && typeof(where = what.home) == TYPE_OBJ && where != this && (is_player(what) ? `where:accept_for_abode(what) ! ANY' | `where:acceptable(what) ! ANY'))
    else
      where = is_player(what) ? $player_start | $nothing;
    endif
    fork (0)
      if (what.location == this)
        "It didn't move when we asked it to, or :moveto is broken. Force it.";
        move(what, where);
      endif
    endfork
    return nice ? `what:moveto(where) ! ANY' | `move(what, where) ! ANY';
  endverb

  verb is_unlocked_for (this none this) owner: #2 flags: "rxd"
    return this.key == 0 || $lock_utils:eval_key(this.key, args[1]);
  endverb

  verb huh (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms() != #-1 ? caller_perms() | player);
    $command_utils:do_huh(verb, args);
  endverb

  verb set_message (this none this) owner: #2 flags: "rxd"
    ":set_message(msg_name,new_value)";
    "Does the actual dirty work of @<msg_name> object is <new_value>";
    "changing the raw value of the message msg_name to be new_value.";
    "Both msg_name and new_value should be strings, though their interpretation is up to the object itself.";
    " => error value (use E_PROPNF if msg_name isn't recognized)";
    " => string error message if something else goes wrong.";
    " => 1 (true non-string) if the message is successfully set";
    " => 0 (false non-error) if the message is successfully `cleared'";
    if (!(caller == this || $perm_utils:controls(caller_perms(), this)))
      return E_PERM;
    else
      return `this.((args[1] + "_msg")) = args[2] ! ANY' && 1;
    endif
  endverb

  verb do_examine (this none this) owner: #2 flags: "rxd"
    "do_examine(examiner)";
    "the guts of examine";
    "call a series of verbs and report their return values to the player";
    who = args[1];
    "if (caller == this || caller == who)";
    if (caller == who)
      "set_task_perms();";
      who:notify_lines(this:examine_names(who) || {});
      "this:examine_names(who);";
      who:notify_lines(this:examine_owner(who) || {});
      "this:examine_owner(who);";
      who:notify_lines(this:examine_desc(who) || {});
      "this:examine_desc(who);";
      who:notify_lines(this:examine_key(who) || {});
      "this:examine_key(who);";
      who:notify_lines(this:examine_contents(who) || {});
      who:notify_lines(this:examine_verbs(who) || {});
    else
      return E_PERM;
    endif
  endverb

  verb examine_key (this none this) owner: #2 flags: "rxd"
    "examine_key(examiner)";
    "return a list of strings to be told to the player, indicating what the key on this type of object means, and what this object's key is set to.";
    "the default will only tell the key to a wizard or this object's owner.";
    who = args[1];
    if (caller == this && $perm_utils:controls(who, this) && this.key != 0)
      return {tostr("Key:  ", $lock_utils:unparse_key(this.key))};
    endif
  endverb

  verb examine_names (this none this) owner: #2 flags: "rxd"
    "examine_names(examiner)";
    "Return a list of strings to be told to the player, indicating the name and aliases (and, by default, the object number) of this.";
    return {tostr(this.name, " (aka ", $string_utils:english_list({tostr(this), @this.aliases}), ")")};
  endverb

  verb examine_desc (this none this) owner: #2 flags: "rxd"
    "examine_desc(who) - return the description, probably";
    "who is the player examining";
    "this should probably go away";
    desc = this:description();
    if (desc)
      if (typeof(desc) != TYPE_LIST)
        desc = {desc};
      endif
      return desc;
    else
      return {"(No description set.)"};
    endif
  endverb

  verb examine_contents (this none this) owner: #2 flags: "rxd"
    "examine_contents(examiner)";
    "by default, calls :tell_contents.";
    "Should probably go away.";
    who = args[1];
    if (caller == this)
      try
        this:tell_contents(this.contents, this.ctype);
      except (ANY)
        "Just ignore it. We shouldn't care about the contents unless the object wants to tell us about them via :tell_contents ($container, $room)";
      endtry
    endif
  endverb

  verb examine_verbs (this none this) owner: #2 flags: "rxd"
    "Return a list of strings to be told to the player.  Standard format says \"Obvious verbs:\" followed by a series of lines explaining syntax for each usable verb.";
    if (caller != this)
      return E_PERM;
    endif
    who = args[1];
    name = dobjstr;
    vrbs = {};
    commands_ok = `this:examine_commands_ok(who) ! ANY => 0';
    dull_classes = {$root_class, $room, $player, $prog, $builder};
    what = this;
    hidden_verbs = this:hidden_verbs(who);
    while (what != $nothing)
      $command_utils:suspend_if_needed(0);
      if (!(what in dull_classes))
        for i in [1..length(verbs(what))]
          $command_utils:suspend_if_needed(0);
          info = verb_info(what, i);
          syntax = verb_args(what, i);
          if (this:examine_verb_ok(what, i, info, syntax, commands_ok, hidden_verbs))
            {dobj, prep, iobj} = syntax;
            if (syntax == {"any", "any", "any"})
              prep = "none";
            endif
            if (prep != "none")
              for x in ($string_utils:explode(prep, "/"))
                if (length(x) <= length(prep))
                  prep = x;
                endif
              endfor
            endif
            "This is the correct way to handle verbs ending in *";
            vname = info[3];
            while (j = index(vname, "* "))
              vname = tostr(vname[1..j - 1], "<anything>", vname[j + 1..$]);
            endwhile
            if (vname[$] == "*")
              vname = vname[1..$ - 1] + "<anything>";
            endif
            vname = strsub(vname, " ", "/");
            rest = "";
            if (prep != "none")
              rest = " " + (prep == "any" ? "<anything>" | prep);
              if (iobj != "none")
                rest = tostr(rest, " ", iobj == "this" ? name | "<anything>");
              endif
            endif
            if (dobj != "none")
              rest = tostr(" ", dobj == "this" ? name | "<anything>", rest);
            endif
            vrbs = setadd(vrbs, "  " + vname + rest);
          endif
        endfor
      endif
      what = parent(what);
    endwhile
    if ($code_utils:verb_or_property(this, "help_msg"))
      vrbs = {@vrbs, tostr("  help ", dobjstr)};
    endif
    return vrbs && {"Obvious verbs:", @vrbs};
  endverb

  verb get_message (this none this) owner: #2 flags: "rxd"
    ":get_message(msg_name)";
    "Use this to obtain a given user-customizable message's raw value, i.e., the value prior to any pronoun-substitution or incorporation of any variant elements --- the value one needs to supply to :set_message().";
    "=> error (use E_PROPNF if msg_name isn't recognized)";
    "=> string or list-of-strings raw value";
    "=> {2, @(list of {msg_name_n,rawvalue_n} pairs to give to :set_message)}";
    "=> {1, other kind of raw value}";
    "=> {E_NONE, error message} ";
    if (!(caller == this || $perm_utils:controls(caller_perms(), this)))
      return E_PERM;
    elseif ((t = typeof(msg = `this.((args[1] + "_msg")) ! ANY')) in {TYPE_ERR, TYPE_STR} || (t == TYPE_LIST && msg && typeof(msg[1]) == TYPE_STR))
      return msg;
    else
      return {1, msg};
    endif
  endverb

  verb "room_announce*_all_but" (this none this) owner: #2 flags: "rxd"
    try
      this.location:(verb)(@args);
    except (ANY)
    endtry
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      deletes = {};
      for vnum in [1..length(verbs(this))]
        $command_utils:suspend_if_needed(0);
        for name in ($string_utils:explode(verb_info(this, vnum)[3]))
          if (rindex(name, "(old)") == max(1, length(name) - 4))
            deletes[1..0] = {vnum};
            break;
          elseif (rindex(name, "(core)") == max(1, length(name) - 5))
            deletes[1..0] = {vnum};
            set_verb_code(this, name[1..$ - 6], verb_code(this, vnum));
            break;
          endif
        endfor
      endfor
      for vnum in (deletes)
        delete_verb(this, vnum);
      endfor
    endif
  endverb

  verb contents (this none this) owner: HACKER flags: "rxd"
    "Returns a list of the objects that are apparently inside this one.  Don't confuse this with .contents, which is a property kept consistent with .location by the server.  This verb should be used in `VR' situations, for instance when looking in a room, and does not necessarily have anything to do with the value of .contents (although the default implementation does).  `Non-VR' commands (like @contents) should look directly at .contents.";
    return this.contents;
  endverb

  verb examine_verb_ok (this none this) owner: #2 flags: "rxd"
    "examine_verb_ok(loc, index, info, syntax, commands_ok, hidden_verbs)";
    "loc is the object that defines the verb; index is which verb on the object; info is verb_info; syntax is verb_args; commands_ok is determined by this:commands_ok, probably, but passed in so we don't have to calculate it for each verb.";
    "hidden_verbs is passed in for the same reasons.  It should be a list, each of whose entries is either a string with the full verb name to be hidden (e.g., \"d*rop th*row\") or a list of the form {verb location, full verb name, args}.";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      {loc, index, info, syntax, commands_ok, hidden_verbs} = args;
      vname = info[3];
      return syntax[2..3] != {"none", "this"} && !index(vname, "(") && (commands_ok || "this" in syntax) && `verb_code(loc, index) ! ANY' && !(vname in hidden_verbs) && !({loc, vname, syntax} in hidden_verbs);
    else
      return E_PERM;
    endif
  endverb

  verb is_listening (this none this) owner: #2 flags: "rxd"
    "return 1 if the object can hear a :tell, or cares. Useful for active objects that want to stop when nothing is listening.";
    return 0;
  endverb

  verb hidden_verbs (this none this) owner: #2 flags: "rxd"
    "hidden_verbs(who)";
    "returns a list of verbs on this that should be hidden from examine";
    "the player who's examining is passed in, so objects can hide verbs from specific players";
    "verbs are returned as {location, full_verb_name, args} or just full_verb_name.  full_verb name is what shows up in verb_info(object, verb)[2], for example \"d*op th*row\".";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      hidden = {};
      what = this;
      while (what != $nothing)
        for i in [1..length(verbs(what))]
          info = verb_info(what, i);
          if (!index(info[2], "r"))
            hidden = setadd(hidden, {what, info[3], verb_args(what, i)});
          endif
        endfor
        what = parent(what);
      endwhile
      return hidden;
    else
      return E_PERM;
    endif
  endverb

  verb examine_owner (this none this) owner: #2 flags: "rxd"
    "examine_owner(examiner)";
    "Return a list of strings to be told to the player, indicating who owns this.";
    return {tostr("Owned by ", this.owner.name, ".")};
  endverb

  verb "announce*_all_but" (this none this) owner: #2 flags: "rxd"
    return;
  endverb

  verb tell_lines_suspended (this none this) owner: #2 flags: "rxd"
    lines = args[1];
    if (typeof(lines) == TYPE_LIST)
      for line in (lines)
        this:tell(line);
        $command_utils:suspend_if_needed(0);
      endfor
    else
      this:tell(lines);
    endif
  endverb

  verb acceptable (this none this) owner: #2 flags: "rxd"
    return 0;
    "intended as a 'quiet' way to determine if :accept will succeed. Currently, some objects have a noisy :accept verb since it is the only thing that a builtin move() call is guaranteed to call.";
    "if you want to tell, before trying, whether :accept will fail, use :acceptable instead. Normally, they'll do the same thing.";
  endverb
endobject