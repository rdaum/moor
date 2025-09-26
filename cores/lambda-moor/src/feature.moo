object FEATURE
  name: "Generic Feature Object"
  parent: THING
  owner: HACKER
  fertile: true
  readable: true

  property feature_ok (owner: HACKER, flags: "r") = 1;
  property feature_verbs (owner: HACKER, flags: "r") = {"Using"};
  property help_msg (owner: HACKER, flags: "rc") = "The Generic Feature Object--not to be used as a feature object.";
  property warehouse (owner: HACKER, flags: "r") = FEATURE_WAREHOUSE;

  override aliases = {
    "Generic Feature Object",
    "Generic .Features_Huh Object",
    "Feature Object",
    ".Features_Huh Object"
  };
  override description = "This is the Generic Feature Object.  It is not meant to be used as a feature object itself, but is handy for making new feature objects.";
  override object_size = {6698, 1084848672};

  verb help_msg (this none this) owner: HACKER flags: "rxd"
    all_help = this.help_msg;
    if (typeof(all_help) == STR)
      all_help = {all_help};
    endif
    helpless = {};
    for vrb in (this.feature_verbs)
      if (loc = $object_utils:has_verb(this, vrb))
        loc = loc[1];
        help = $code_utils:verb_documentation(loc, vrb);
        if (help)
          all_help = {@all_help, "", tostr(loc, ":", verb_info(loc, vrb)[3]), @help};
        else
          helpless = {@helpless, vrb};
        endif
      endif
    endfor
    if (helpless)
      all_help = {@all_help, "", "No help found on " + $string_utils:english_list(helpless, "nothing", " or ") + "."};
    endif
    return {@all_help, "----"};
  endverb

  verb look_self (this none this) owner: HACKER flags: "rxd"
    "Definition from #1";
    desc = this:description();
    if (desc)
      player:tell_lines(desc);
    else
      player:tell("You see nothing special.");
    endif
    player:tell("Please type \"help ", this, "\" for more information.");
  endverb

  verb "using this" (this none this) owner: HACKER flags: "rxd"
    "Proper usage for the Generic Feature Object:";
    "";
    "First of all, the Generic Feature Object is constructed with the idea";
    "that its children will be @moved to #24300, which is kind of a warehouse";
    "for feature objects.  If there's enough interest, I'll try to make the";
    "stuff that works with that in mind optional.";
    "";
    "Make a short description.  This is so I can continue to have looking at";
    "#24300 give the descriptions of each of the objects in its .contents.";
    "The :look_msg automatically includes a pointer to `help <this object>',";
    "so you don't have to.";
    "";
    "Put a list of the commands you want people to use in";
    "<this object>.feature_verbs.  (You need to use the :set_feature_verbs";
    "verb to do this.)";
    "";
    "When someone types `help <this object>', they will be told the comment";
    "strings from each of the verbs named in .feature_verbs.";
  endverb

  verb examine_commands_ok (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this in args[1].features;
  endverb

  verb set_feature_ok (this none this) owner: HACKER flags: "rxd"
    if ($perm_utils:controls(caller_perms(), this) || caller == this)
      return this.feature_ok = args[1];
    else
      return E_PERM;
    endif
  endverb

  verb hidden_verbs (this none this) owner: HACKER flags: "rxd"
    "Can't see `get' unless it's in the room; can't see `drop' unless it's in the player.  Should possibly go on $thing.";
    "Should use :contents, but I'm in a hurry.";
    hidden = pass(@args);
    if (this.location != args[1])
      hidden = setadd(hidden, {$thing, verb_info($thing, "drop")[3], {"this", "none", "none"}});
      hidden = setadd(hidden, {$thing, verb_info($thing, "give")[3], {"this", "at/to", "any"}});
    endif
    if (this.location != args[1].location)
      hidden = setadd(hidden, {$thing, verb_info($thing, "get")[3], {"this", "none", "none"}});
    endif
    return hidden;
  endverb

  verb set_feature_verbs (this none this) owner: HACKER flags: "rxd"
    if ($perm_utils:controls(caller_perms(), this) || caller == this)
      return this.feature_verbs = args[1];
    else
      return E_PERM;
    endif
  endverb

  verb initialize (this none this) owner: HACKER flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      pass(@args);
      this.feature_verbs = {};
    else
      return E_PERM;
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ($code_utils:verb_location() == this && caller_perms().wizard)
      this.warehouse = $feature_warehouse;
      `delete_property(this, "guest_ok") ! ANY';
      `delete_verb(this, "set_ok_for_guest_use") ! ANY';
      pass(@args);
    endif
  endverb

  verb feature_remove (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This is just a blank verb definition to encourage others to use this verb name if they care when a user is no longer using that feature.";
  endverb

  verb player_connected (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return;
  endverb

  verb has_feature_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":has_feature_verb(verb, dlist, plist, ilist)";
    "If this feature has a feature verb that matches <verb> and whose {dobj, prep, iobj} arguments match the possibilities listed in <dlist>, <plist> and <ilist>, then return the name of that verb, otherwise return false.";
    "Note: Individual FOs may over-ride this the method to redirect particular feature verbs to different verbs on the object. For example, 'sit with <any>' and 'sit on <any>' could be directed to separate :sit_with() and :sit_on() verbs -- which is something that the code below cannot do.";
    {vrb, dlist, plist, ilist} = args;
    if (`valid(loc = $object_utils:has_callable_verb(this, vrb)[1]) ! ANY => 0')
      vargs = verb_args(loc, vrb);
      if (vargs[2] in plist && (vargs[1] in dlist && vargs[3] in ilist))
        return vrb;
      endif
    endif
    return 0;
  endverb
endobject