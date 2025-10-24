object OBJECT_QUOTA_UTILS
  name: "Object Quota Utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property byte_based (owner: HACKER, flags: "rc") = 0;

  override aliases = {"Object Quota Utilities"};
  override description = {
    "This is the Object Quota Utilities utility package.  See `help $object_quota_utils' for more details."
  };
  override help_msg = "This is the default package that interfaces to the $player/$prog quota manipulation verbs.";
  override object_size = {6728, 1084848672};

  verb initialize_quota (this none this) owner: HACKER flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      args[1].ownership_quota = $wiz_utils.default_player_quota;
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      pass(@args);
      "Uncomment this if you want to send the core out with object quota.";
      "  $quota_utils = this";
    endif
  endverb

  verb adjust_quota_for_programmer (this none this) owner: HACKER flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      victim = args[1];
      oldquota = victim.ownership_quota;
      if ($object_utils:has_property($local, "second_char_registry") && $local.second_char_registry:is_second_char(victim))
        "don't increment quota for 2nd chars when programmering";
        victim.ownership_quota = oldquota;
      else
        victim.ownership_quota = oldquota + ($wiz_utils.default_programmer_quota - $wiz_utils.default_player_quota);
      endif
    endif
  endverb

  verb bi_create (this none this) owner: #2 flags: "rxd"
    "Calls built-in create.";
    set_task_perms(caller_perms());
    return `create(@args) ! ANY';
  endverb

  verb creation_permitted (this none this) owner: HACKER flags: "rxd"
    $recycler:check_quota_scam(args[1]);
    return args[1].ownership_quota > 0;
  endverb

  verb "verb_addition_permitted property_addition_permitted" (this none this) owner: HACKER flags: "rxd"
    return 1;
  endverb

  verb display_quota (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if (caller_perms() == who)
      q = who.ownership_quota;
      total = typeof(who.owned_objects) == LIST ? length(setremove(who.owned_objects, who)) | 0;
      if (q == 0)
        player:tell(tostr("You can't create any more objects", total < 1 ? "." | tostr(" until you recycle some of the ", total, " you already own.")));
      else
        player:tell(tostr("You can create ", q, " new object", q == 1 ? "" | "s", total == 0 ? "." | tostr(" without recycling any of the ", total, " that you already own.")));
      endif
    else
      if ($perm_utils:controls(caller_perms(), who))
        player:tell(tostr(who.name, "'s quota is currently ", who.ownership_quota, "."));
      else
        player:tell("Permission denied.");
      endif
    endif
  endverb

  verb "get_quota quota_remaining" (this none this) owner: HACKER flags: "rxd"
    if ($perm_utils:controls(caller_perms(), args[1]) || caller == this)
      return args[1].ownership_quota;
    else
      return E_PERM;
    endif
  endverb

  verb charge_quota (this none this) owner: HACKER flags: "rxd"
    "Charge args[1] for the quota required to own args[2]";
    {who, what} = args;
    if (caller == this || caller_perms().wizard)
      who.ownership_quota = who.ownership_quota - 1;
    else
      return E_PERM;
    endif
  endverb

  verb reimburse_quota (this none this) owner: HACKER flags: "rxd"
    "Reimburse args[1] for the quota required to own args[2]";
    {who, what} = args;
    if (caller == this || caller_perms().wizard)
      who.ownership_quota = who.ownership_quota + 1;
    else
      return E_PERM;
    endif
  endverb

  verb set_quota (this none this) owner: HACKER flags: "rxd"
    "Set args[1]'s quota to args[2]";
    {who, quota} = args;
    if (caller_perms().wizard || caller == this)
      return who.ownership_quota = quota;
    else
      return E_PERM;
    endif
  endverb

  verb preliminary_reimburse_quota (this none this) owner: HACKER flags: "rxd"
    return 0;
  endverb

  verb can_peek (this none this) owner: HACKER flags: "rxd"
    "Is args[1] permitted to examine args[2]'s quota information?";
    return $perm_utils:controls(args[1], args[2]);
  endverb

  verb can_touch (this none this) owner: HACKER flags: "rxd"
    "Is args[1] permitted to examine args[2]'s quota information?";
    return args[1].wizard;
  endverb
endobject