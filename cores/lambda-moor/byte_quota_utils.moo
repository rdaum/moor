object BYTE_QUOTA_UTILS
  name: "Byte Quota Utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property byte_based (owner: HACKER, flags: "rc") = 1;
  property cycle_days (owner: HACKER, flags: "rc") = 5;
  property default_quota (owner: HACKER, flags: "rc") = {20000, 0, 0, 1};
  property exempted (owner: HACKER, flags: "rc") = {};
  property large_negative_number (owner: HACKER, flags: "rc") = -10000;
  property large_objects (owner: HACKER, flags: "rc") = {SPELL};
  property max_unmeasured (owner: HACKER, flags: "rc") = 10;
  property measurement_task_running (owner: HACKER, flags: "rc") = 0;
  property repeat_cycle (owner: HACKER, flags: "rc") = 0;
  property report_recipients (owner: HACKER, flags: "rc") = {BYTE_QUOTA_UTILS_WORKING};
  property task_repeat (owner: HACKER, flags: "rc") = 1;
  property task_time_limit (owner: HACKER, flags: "rc") = 500;
  property too_large (owner: HACKER, flags: "rc") = 1000000;
  property unmeasured_multiplier (owner: HACKER, flags: "rc") = 100;
  property working (owner: HACKER, flags: "rc") = BYTE_QUOTA_UTILS_WORKING;

  override aliases = {"Byte Quota Utilities"};
  override description = {
    "This is the Byte Quota Utilities utility package.  See `help $quota_utils' for more details."
  };
  override help_msg = {
    "Verbs a user might want to call from a program:",
    " :bi_create -- built-in create() call, takes same args.",
    "",
    " :get_quota(who) -- just get the raw size_quota property",
    " :display_quota(who) -- prints to player the quota of who.  If caller_perms() controls who, include any secondary characters.  Called by @quota.",
    " :get_size_quota(who [allchars]) -- return the quota of who, if allchars flag set, add info from all secondary chars, if caller_perms() permits.",
    "",
    " :value_bytes(value) -- computes the size of the value.",
    " :object_bytes(object) -- computes the size of the object and caches it.",
    " :recent_object_bytes(object, days) -- computes and caches the size of object only if cached value more than days old.  Returns cached value.",
    " :do_summary(user) -- prints out the results of summarize-one-user.",
    " :summarize_one_user(user) -- summarizes and caches space usage for user.  See verb help for details.",
    "",
    "Verbs the system calls:",
    " :\"creation_permitted verb_addition_permitted property_addition_permitted\"(who) -- returns true if who is permitted to build.",
    " :initialize_quota(who) -- sets quota for newly created players",
    " :adjust_quota_for_programmer(who) -- empty; might add more quota to newly @progged player.",
    " :enable_create(who) -- sets .ownership_quota to 1",
    " :disable_create(who) -- sets .ownership_quota back to -1000 to prohibit create()",
    " :charge_quota(who, object) -- subtract the size of object from who's quota.  Manipulates the #-unmeasured if what is not currently measured.  Called by $wiz_utils:set_owner.",
    " :reimburse_quota(who, object) -- add the size of object to who's quota.  Ditto.",
    " :preliminary_reimburse_quota(who, object) -- Because the set_owner is done *after* an object has been turned into $garbage, ordinary reimbursement fails.  So we use this verb in the $recycler.",
    " :set_quota(who, howmuch)",
    " :quota_remaining(who) ",
    " :display_quota_summary -- internal, called by display quota",
    "",
    "The measurement task:",
    "",
    " :measurement_task() -- runs once every 24 hours measuring stuff, separated from the scheduling in case you just want to run it once.  Calls the body and then reports via moomail.",
    " :schedule_measurement_task() -- actually schedules it.  Look here to change the start time.",
    " :measurement_task_body(timeout) -- does the real work, working for no longer than timeout seconds.",
    " .task_time_limit -- integer number of seconds indicating for how long it should run each day.",
    " .working -- object indicating the player whom it is either working on now (or if not running) will pick up working on when it commences tonight.",
    " .cycle_days -- integer numbers indicating how long ago an object must have been measured before it will be remeasured.",
    " .repeat_cycle -- boolean.  0 means have a vanilla cycle (goes through all players() exactly once measuring their objects measured more than .cycle_days ago).  1 means to have a much more complex algorithm: The first cycle, it only measures stuff owned by people who have logged in within .cycle_days.  If, in .task_time_limit seconds, it measures all objects not measured in cycle_days owned by such people, it will run again measuring those objects which have not been measured in cycle_days - 1, considering people who have logged in within 4 * cycle_days, repeating until it has used up its seconds.  (\"Doing some of tomorrow's work.\")  Selecting .repeat_cycle = 1 is appropriate only for large MOOs.",
    " .exempted -- list of objects to never measure (useful if there are huge objects).  Suggested huge objects include $player_db and $site_db.",
    " .measurement_task -- indicates the task_id() of the most recent measurement task -- used to prevent duplicate invocation.",
    " .report_recipients -- recipients of the daily reports.  Set to {} to disable reporting entirely.",
    "",
    "See help @measure and help @quota for the command line verbs.",
    "",
    "",
    "Porter's notes:  If you are planning on porting this system to another MOO, here are the things to grab in addition to @dumping all of $quota_utils:",
    "",
    "The following verbs have been changed on $prog:",
    "@prop*erty @verb @copy (@add-alias @copy-move as well)",
    "",
    "The following verbs have been changed on $wiz:",
    "@programmer @quota",
    "",
    "The following verbs have been changed on $wiz_utils:",
    "set_programmer set_owner make_player",
    "",
    "The following verbs have been changed on $builder:",
    "@quota _create",
    "",
    "This verb probably should have gone on $builder.",
    "@measure",
    "",
    "The followig verbs have been changed on $recycler",
    "_recycle _create setup_toad",
    "",
    "The following verb has been changed on $login:",
    "create",
    "",
    "And don't forget $object_quota_utils, which has the object based implementation."
  };
  override object_size = {32429, 1084848672};

  verb initialize_quota (this none this) owner: HACKER flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      (args[1]).size_quota = this.default_quota;
      (args[1]).ownership_quota = this.large_negative_number;
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      pass(@args);
      this.exempted = {};
      this.working = #2;
      this.task_time_limit = 500;
      this.repeat_cycle = 0;
      this.large_objects = {};
      this.report_recipients = {#2};
      this.default_quota = {100000, 0, 0, 1};
      $quota_utils = this;
    endif
  endverb

  verb adjust_quota_for_programmer (this none this) owner: HACKER flags: "rxd"
    return 0;
  endverb

  verb bi_create (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    who = this:parse_create_args(@args);
    "Because who can be E_INVARG, need to catch E_TYPE. Let $recycler:_create deal with returning E_PERM since that's what's going to happen. Ho_Yan 11/19/96.";
    if (!`who.wizard ! E_TYPE => 0' && $recycler.contents)
      return $recycler:_create(@args);
    elseif (this:creation_permitted(who))
      this:enable_create(who);
      value = `create(@args) ! ANY';
      this:disable_create(who);
      if (typeof(value) != ERR)
        this:charge_quota(who, value);
        if (typeof(who.owned_objects) == LIST && !(value in who.owned_objects))
          this:add_owned_object(who, value);
        endif
      endif
      return value;
    else
      return E_QUOTA;
    endif
  endverb

  verb enable_create (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != this && !caller_perms().wizard)
      return E_PERM;
    else
      (args[1]).ownership_quota = 1;
    endif
  endverb

  verb disable_create (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != this && !caller_perms().wizard)
      return E_PERM;
    else
      (args[1]).ownership_quota = this.large_negative_number;
    endif
  endverb

  verb parse_create_args (this none this) owner: HACKER flags: "rxd"
    "This figures out who is gonna own the stuff @create does.  If one arg, return caller_perms().  If two args, then if caller_perms().wizard, args[2].";
    {what, ?who = #-1} = args;
    if (!valid(who))
      return caller_perms();
    elseif ($perm_utils:controls(caller_perms(), who))
      return who;
    else
      return E_INVARG;
    endif
  endverb

  verb "creation_permitted verb_addition_permitted property_addition_permitted" (this none this) owner: HACKER flags: "rxd"
    "Here's the tricky one.  Collect all the user's characters' cached usage data and total quotas.  Compare same.  If usage bigger than quotas, return 0.  Else, add up the total number of objects that haven't been measured recently.  If greater than the allowed, return 0.  Else, reluctantly, return 1.";
    who = args[1];
    if (who.wizard || who == $hacker)
      "... sorry folks --Rog";
      return 1;
    endif
    if (!$object_utils:has_property(who, "size_quota") || is_clear_property(who, "size_quota"))
      return 0;
    endif
    $recycler:check_quota_scam(who);
    allwho = this:all_characters(who);
    quota = 0;
    usage = 0;
    unmeasured = 0;
    for x in (allwho)
      quota = quota + x.size_quota[1];
      usage = usage + x.size_quota[2];
      unmeasured = unmeasured + x.size_quota[4];
    endfor
    if (usage >= quota)
      return 0;
    elseif (unmeasured >= this.max_unmeasured)
      return 0;
    else
      return 1;
    endif
  endverb

  verb all_characters (this none this) owner: HACKER flags: "rxd"
    {who} = args;
    if (caller != this && !this:can_peek(caller_perms(), who))
      return E_PERM;
    elseif ($object_utils:has_property($local, "second_char_registry"))
      seconds = $local.second_char_registry:all_second_chars(who);
      if (seconds == E_INVARG)
        return {who};
      else
        return seconds;
      endif
    else
      return {who};
    endif
  endverb

  verb display_quota (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if (this:can_peek(caller_perms(), who) && length(all = this:all_characters(who)) > 1)
      many = 1;
    else
      many = 0;
      all = {who};
    endif
    if (many)
      tquota = 0;
      tusage = 0;
      ttime = $maxint;
      tunmeasured = 0;
      tunmeasurable = 0;
    endif
    for x in (all)
      {quota, usage, timestamp, unmeasured} = x.size_quota;
      unmeasurable = 0;
      if (unmeasured >= 100)
        unmeasurable = unmeasured / 100;
        unmeasured = unmeasured % 100;
      endif
      if (many)
        player:tell(x.name, " quota: ", $string_utils:group_number(quota), "; usage: ", $string_utils:group_number(usage), "; unmeasured: ", unmeasured, "; no .object_size: ", unmeasurable, ".");
        tquota = tquota + quota;
        tusage = tusage + usage;
        ttime = min(ttime, timestamp);
        tunmeasured = tunmeasured + unmeasured;
        tunmeasurable = tunmeasurable + unmeasurable;
      endif
    endfor
    if (many)
      this:display_quota_summary(who, tquota, tusage, ttime, tunmeasured, tunmeasurable);
    else
      this:display_quota_summary(who, quota, usage, timestamp, unmeasured, unmeasurable);
    endif
  endverb

  verb get_quota (this none this) owner: HACKER flags: "rxd"
    return (args[1]).size_quota[1];
  endverb

  verb charge_quota (this none this) owner: HACKER flags: "rxd"
    "Charge args[1] for the quota required to own args[2]";
    {who, what} = args;
    if (caller == this || caller_perms().wizard)
      usage_index = 2;
      unmeasured_index = 4;
      object_size = $object_utils:has_property(what, "object_size") ? what.object_size[1] | -1;
      if (object_size <= 0)
        who.size_quota[unmeasured_index] = who.size_quota[unmeasured_index] + 1;
      else
        who.size_quota[usage_index] = who.size_quota[usage_index] + object_size;
      endif
    else
      return E_PERM;
    endif
  endverb

  verb reimburse_quota (this none this) owner: HACKER flags: "rxd"
    "reimburse args[1] for the quota required to own args[2]";
    "If it is a $garbage, then if who = $hacker, then we mostly ignore everything.  Who cares what $hacker's quota looks like.";
    {who, what} = args;
    if (caller == this || caller_perms().wizard)
      usage_index = 2;
      unmeasured_index = 4;
      if (parent(what) == $garbage)
        return 0;
      elseif (valid(who) && is_player(who) && $object_utils:has_property(what, "object_size") && !is_clear_property(who, "size_quota"))
        object_size = what.object_size[1];
        if (object_size <= 0)
          who.size_quota[unmeasured_index] = who.size_quota[unmeasured_index] - 1;
        else
          who.size_quota[usage_index] = who.size_quota[usage_index] - object_size;
        endif
      endif
    else
      return E_PERM;
    endif
  endverb

  verb set_quota (this none this) owner: HACKER flags: "rxd"
    "Set args[1]'s quota to args[2]";
    if (caller_perms().wizard || caller == this || this:can_touch(caller_perms()))
      "Size_quota[1] is the total quota permitted.";
      return (args[1]).size_quota[1] = args[2];
    else
      return E_PERM;
    endif
  endverb

  verb get_size_quota (this none this) owner: HACKER flags: "rxd"
    "Return args[1]'s quotas.  second arg of 1 means add all second chars.";
    {who, ?all = 0} = args;
    if (all && (caller == this || this:can_peek(caller_perms(), who)))
      all = this:all_characters(who);
    else
      all = {who};
    endif
    baseline = {0, 0, 0, 0};
    for x in (all)
      baseline[1] = baseline[1] + x.size_quota[1];
      baseline[2] = baseline[2] + x.size_quota[2];
      baseline[3] = min(baseline[3], x.size_quota[3]) || x.size_quota[3];
      baseline[4] = baseline[4] + x.size_quota[4];
    endfor
    return baseline;
  endverb

  verb display_quota_summary (this none this) owner: HACKER flags: "rxd"
    {who, quota, usage, timestamp, unmeasured, unmeasurable} = args;
    player:tell(who.name, " has a total building quota of ", $string_utils:group_number(quota), " bytes.");
    player:tell($gender_utils:get_pronoun("P", who), " total usage was ", $string_utils:group_number(usage), " as of ", player:ctime(timestamp), ".");
    if (usage > quota)
      player:tell(who.name, " is over quota by ", $string_utils:group_number(usage - quota), " bytes.");
    else
      player:tell(who.name, " may create up to ", $string_utils:group_number(quota - usage), " more bytes of objects, properties, or verbs.");
    endif
    if (unmeasured)
      plural = unmeasured != 1;
      player:tell("There ", plural ? tostr("are ", unmeasured, " objects") | "is 1 object", " which ", plural ? "are" | "is", " not yet included in the tally; this tally may thus be inaccurate.");
      if (unmeasured >= this.max_unmeasured)
        player:tell("The number of unmeasured objects is too large; no objects may be created until @measure new is used.");
      endif
    endif
    if (unmeasurable)
      plural = unmeasurable != 1;
      player:tell("There ", plural ? tostr("are ", unmeasurable, " objects") | "is 1 object", " which do", plural ? "" | "es", " not have a .object_size property and will thus prevent additional building.", who == player ? "  Contact a wizard for assistance in having this situation repaired." | "");
    endif
  endverb

  verb quota_remaining (this none this) owner: HACKER flags: "rxd"
    "This wants to only be called by a wizard cuz I'm lazy.  This is just for @second-char anyway.";
    if (caller_perms().wizard)
      q = this:get_size_quota(args[1], 1);
      return q[1] - q[2];
    endif
  endverb

  verb preliminary_reimburse_quota (this none this) owner: HACKER flags: "rxd"
    "This does the reimbursement work of the recycler, since we ignore $garbage in ordinary reimbursement.";
    if (caller_perms().wizard)
      this:reimburse_quota(@args);
    else
      return E_PERM;
    endif
  endverb

  verb value_bytes (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return value_bytes(args[1]);
    set_task_perms(caller_perms());
    v = args[1];
    t = typeof(v);
    if (t == LIST)
      b = (length(v) + 1) * 2 * 4;
      for vv in (v)
        $command_utils:suspend_if_needed(2);
        b = b + this:value_bytes(vv);
      endfor
      return b;
    elseif (t == STR)
      return length(v) + 1;
    else
      return 0;
    endif
  endverb

  verb "object_bytes object_size" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "No need for lengthy algorithms to measure an object, we have a builtin now. Ho_Yan 10/31/96";
    set_task_perms($wiz_utils:random_wizard());
    o = args[1];
    if ($object_utils:has_property(o, "object_size") && o.object_size[1] > this.too_large && !caller_perms().wizard && caller_perms() != this.owner && caller_perms() != $hacker)
      return o.object_size[1];
    endif
    b = object_bytes(o);
    if ($object_utils:has_property(o, "object_size"))
      oldsize = is_clear_property(o, "object_size") ? 0 | o.object_size[1];
      if ($object_utils:has_property(o.owner, "size_quota"))
        "Update quota cache.";
        if (oldsize)
          o.owner.size_quota[2] = o.owner.size_quota[2] + (b - oldsize);
        else
          o.owner.size_quota[2] = o.owner.size_quota[2] + b;
          if (o.owner.size_quota[4] > 0)
            o.owner.size_quota[4] = o.owner.size_quota[4] - 1;
          endif
        endif
      endif
      o.object_size = {b, time()};
    endif
    if (b > this.too_large)
      this.large_objects = setadd(this.large_objects, o);
    endif
    return b;
  endverb

  verb do_summary (any with this) owner: HACKER flags: "rxd"
    who = args[1];
    results = this:summarize_one_user(who);
    {total, nuncounted, nzeros, oldest, eldest} = results;
    player:tell(who.name, " statistics:");
    player:tell("  ", $string_utils:group_number(total), " bytes of storage measured.");
    player:tell("  Oldest measurement date ", ctime(oldest), " (", $string_utils:from_seconds(time() - oldest), " ago) of object ", eldest, " (", valid(eldest) ? eldest.name | "$nothing", ")");
    if (nzeros || nuncounted)
      player:tell("  Number of objects with no statistics recorded:  ");
      player:tell("      ", nzeros, " recently created, ", nuncounted, " not descendents of #1");
    endif
  endverb

  verb summarize_one_user (this none this) owner: HACKER flags: "rxd"
    "Summarizes total space usage by one user (args[1]).  Optional second argument is a flag to say whether to re-measure all objects for this user; specify the number of seconds out of date you are willing to accept.  If negative, will only re-measure objects which have no recorded data.";
    "Returns a list of four values:";
    "  total : total measured space in bytes";
    "  uncounted : Number of objects that were not counted because they aren't descendents of #1";
    "  zeros : Number of objects which have been created too recently to have any measurement data at all (presumably none if re-measuring)";
    "  most-out-of-date : the time() the oldest actual measurement was taken";
    "  object-thereof: the object who had this time()'d measurement";
    who = args[1];
    if (length(args) == 2)
      if (args[2] < 0)
        earliest = 1;
      else
        earliest = time() - args[2];
      endif
    else
      earliest = 0;
    endif
    nzeros = 0;
    oldest = time();
    eldest = #-1;
    nuncounted = 0;
    total = 0;
    for x in (typeof(who.owned_objects) == LIST ? who.owned_objects | {})
      if (x.owner == who)
        "Bulletproofing against recycling during suspends!";
        "Leaves us open to unsummarized creation during this period, which is unfortunate.";
        if ($object_utils:has_property(x, "object_size"))
          size = x.object_size[1];
          time = x.object_size[2];
          if (time < earliest)
            "Re-measure.  This side-effects x.object_size.";
            this:object_bytes(x);
            size = x.object_size[1];
            time = x.object_size[2];
          endif
          if (time && time <= oldest)
            oldest = time;
            eldest = x;
          elseif (!time)
            nzeros = nzeros + 1;
          endif
          if (size >= 0)
            total = total + size;
          endif
        else
          nuncounted = nuncounted + 1;
        endif
      endif
      $command_utils:suspend_if_needed(0);
    endfor
    if (!is_clear_property(who, "size_quota"))
      "Cache the data, but only if they aren't scamming.";
      who.size_quota[2] = total;
      who.size_quota[3] = oldest;
      who.size_quota[4] = nuncounted * this.unmeasured_multiplier + nzeros;
    endif
    return {total, nuncounted, nzeros, oldest, eldest};
  endverb

  verb recent_object_bytes (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":recent_object_bytes(x, n) -- return object size of x, guaranteed to be no more than n days old.  N defaults to this.cycle_days.";
    {object, ?since = this.cycle_days} = args;
    if (!valid(object))
      return 0;
    elseif (`object.object_size[2] ! ANY => 0' > time() - since * 24 * 60 * 60)
      "Trap error when doesn't have .object_size for some oddball reason ($garbage). Ho_Yan 11/19/96";
      return object.object_size[1];
    else
      return this:object_bytes(object);
    endif
  endverb

  verb measurement_task (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      start_time = time();
      {num_processed, num_repetitions} = this:measurement_task_body(args[1]);
      players = players();
      lengthp = length(players);
      if (!num_repetitions && num_processed < lengthp / 2)
        "Add this in because we aren't getting people summarized like we should.  We're going to work for way longer now, cuz we're going to do a second pass, but we really need to get those summaries done.  Only do this if we hardly did any work.  Note the -1 here: measure all newly created objects as well.  More work, sigh.";
        extra_end = time() + 3600 * 3;
        for x in (players)
          if (is_player(x) && time() < extra_end)
            "Robustness as above, plus don't run all day.  My kingdom for a break statement";
            this:summarize_one_user(x, -1);
          endif
          $command_utils:suspend_if_needed(0);
        endfor
      endif
      $mail_agent:send_message(player, this.report_recipients, "quota-utils report", {tostr("About to measure objects of player ", this.working.name, " (", this.working, "), ", $string_utils:ordinal(this.working in players), " out of ", lengthp, ".  We processed ", num_processed + lengthp * num_repetitions, " players in this run in ", num_repetitions, " time", num_repetitions == 1 ? "" | "s", " through all players.  Total time spent:  ", $time_utils:dhms(time() - start_time), ".")});
    endif
  endverb

  verb can_peek (this none this) owner: HACKER flags: "rxd"
    return args[1] == this.owner || $perm_utils:controls(args[1], args[2]);
  endverb

  verb can_touch (this none this) owner: HACKER flags: "rxd"
    return (args[1]).wizard;
  endverb

  verb do_breakdown (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    dobj = args[1];
    who = valid(caller_perms()) ? caller_perms() | player;
    if (!this:can_peek(who, dobj.owner))
      return E_PERM;
    endif
    props = $object_utils:all_properties_suspended(dobj);
    grand_total = obj_over = this:object_overhead_bytes(dobj);
    output = {tostr("Object overhead:  ", obj_over)};
    if (props)
      total = 0;
      lines = {};
      output = {@output, "Properties, defined and inherited, sorted by size:"};
      for x in (props)
        $command_utils:suspend_if_needed(0, "...One moment. Working on the breakdown...");
        if (!is_clear_property(dobj, x))
          size = value_bytes(dobj.(x));
          total = total + size;
          if (size)
            lines = {@lines, {x, size}};
          endif
        endif
      endfor
      lines = $list_utils:reverse_suspended($list_utils:sort_suspended(0, lines, $list_utils:slice(lines, 2)));
      for x in (lines)
        $command_utils:suspend_if_needed(0, "...One moment. Working on the breakdown...");
        text = tostr("  ", x[1], ":  ", x[2]);
        output = {@output, text};
      endfor
      output = {@output, tostr("Total size of properties:  ", total)};
      grand_total = grand_total + total;
    endif
    prop_over = this:property_overhead_bytes(dobj, props);
    output = {@output, tostr("Property overhead:  ", prop_over)};
    grand_total = grand_total + prop_over;
    if (verbs(dobj))
      output = {@output, "Verbs, sorted by size:"};
      total = 0;
      lines = {};
      for x in [1..length(verbs(dobj))]
        $command_utils:suspend_if_needed(0, "...One moment. Working on the breakdown...");
        vname = verb_info(dobj, x)[3];
        size = value_bytes(verb_code(dobj, x, 0, 0)) + length(vname) + 1;
        total = total + size;
        lines = {@lines, {vname, size}};
      endfor
      lines = $list_utils:reverse_suspended($list_utils:sort_suspended(0, lines, $list_utils:slice(lines, 2)));
      for x in (lines)
        $command_utils:suspend_if_needed(0, "...One moment. Working on the breakdown...");
        text = tostr("  ", x[1], ":  ", x[2]);
        output = {@output, text};
      endfor
      output = {@output, tostr("Total size of verbs:  ", total)};
      grand_total = grand_total + total;
      verb_over = this:verb_overhead_bytes(dobj);
      output = {@output, tostr("Verb overhead:  ", verb_over)};
      grand_total = grand_total + verb_over;
    endif
    output = {@output, tostr("Grand total:  ", grand_total)};
    return output;
    "Last modified Sun Dec 31 10:12:14 2006 PST, by Roebare (#109000) @ LM.";
  endverb

  verb object_overhead_bytes (this none this) owner: HACKER flags: "rxd"
    object = args[1];
    return 13 * 4 + length(object.name) + 1;
  endverb

  verb property_overhead_bytes (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {o, ?ps = $object_utils:all_properties_suspended(o)} = args;
    return value_bytes(properties(o)) - 4 + length(ps) * 4 * 4;
  endverb

  verb verb_overhead_bytes (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    o = args[1];
    vs = verbs(o);
    return length(vs) * 5 * 4;
  endverb

  verb add_owned_object (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":add_owned_object(who, what) -- adds what to whose .owned_objects.";
    {who, what} = args;
    if (typeof(who.owned_objects) == LIST && what.owner == who)
      who.owned_objects = setadd(who.owned_objects, what);
    endif
  endverb

  verb measurement_task_nofork (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This is a one-shot run of the measurement task, as opposed to :measurement_task, which will fork once per day.";
    if (!caller_perms().wizard)
      return E_PERM;
    else
      {num_processed, num_repetitions} = this:measurement_task_body();
      $mail_agent:send_message(player, player, "quota-utils report", {"finished one shot run of measurement task: processed ", num_processed, " players in ", num_repetitions, " runs through all players."});
    endif
  endverb

  verb measurement_task_body (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      num_processed = 0;
      num_repetitions = 0;
      usage_index = 2;
      time_index = 3;
      unmeasured_index = 4;
      players = setremove(players(), $hacker);
      lengthp = length(players);
      index = this.working in players;
      keep_going = 1;
      if (!index)
        "Uh, oh, our guy got reaped while we weren't looking.  Better look for someone else.";
        index = 1;
        while (this.working > players[index] && index < lengthp)
          $command_utils:suspend_if_needed(0);
          index = index + 1;
        endwhile
        this.working = players[index];
      endif
      day = 60 * 60 * 24;
      stop = time() + args[1];
      early = time() - day * this.cycle_days;
      tooidle = day * this.cycle_days;
      "tooidletime is only used if !this.repeat_cycle.";
      tooidletime = time() - tooidle;
      local_per_player_hack = $object_utils:has_verb($local, "per_player_daily_scan");
      while (time() < stop && keep_going)
        who = players[index];
        if (is_player(who) && $object_utils:has_property(who, "size_quota"))
          "Robustness in the face of reaping...";
          if (!this.repeat_cycle || who.last_disconnect_time > tooidletime && who.last_disconnect_time != $maxint)
            "only measure people who login regularly if we're a big moo.";
            usage = 0;
            unmeasured = 0;
            earliest = time();
            for o in (who.owned_objects)
              if (valid(o) && o.owner == who && !(o in this.exempted))
                "sanity check: might have recycled while we suspended!";
                if ($object_utils:has_property(o, "object_size"))
                  if (o.object_size[2] < early)
                    usage = usage + this:object_bytes(o);
                  else
                    usage = usage + o.object_size[1];
                    earliest = min(earliest, o.object_size[2]);
                  endif
                else
                  unmeasured = unmeasured + 1;
                endif
              endif
              $command_utils:suspend_if_needed(3);
            endfor
            if (!is_clear_property(who, "size_quota"))
              who.size_quota[usage_index] = usage;
              who.size_quota[unmeasured_index] = this.unmeasured_multiplier * unmeasured;
              who.size_quota[time_index] = earliest;
            else
              $mail_agent:send_message(player, player, "Quota Violation", {tostr(who, " has a clear .size_quota property."), $string_utils:names_of({who, @$object_utils:ancestors(who)})});
            endif
          elseif (who.size_quota[unmeasured_index])
            "If they managed to create an object *despite* being too idle (presumably programmatically), measure it.";
            this:summarize_one_user(who, -1);
          endif
        elseif (is_player(who))
          "They don't have a size_quota property.  Whine.";
          $mail_agent:send_message(player, player, "Quota Violation", {tostr(who, " doesn't seem to have a .size_quota property."), $string_utils:names_of({who, @$object_utils:ancestors(who)})});
        endif
        if (local_per_player_hack)
          $local:per_player_daily_scan(who);
        endif
        if (index >= lengthp)
          index = 1;
        else
          index = index + 1;
        endif
        num_processed = num_processed + 1;
        if (num_processed > lengthp)
          if (this.repeat_cycle)
            "If we've gotten everyone up to threshold, try measuring some later than that.";
            early = early + 24 * 60 * 60;
            tooidle = tooidle * 4;
            tooidletime = tooidletime - tooidle;
            num_repetitions = num_repetitions + 1;
            num_processed = 0;
            if (early > time())
              "Don't spin our wheels when we've measured everything!";
              keep_going = 0;
            endif
          else
            keep_going = 0;
          endif
        endif
        this.working = players[index];
      endwhile
      return {num_processed, num_repetitions};
    endif
  endverb

  verb schedule_measurement_task (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this || caller_perms().wizard)
      day = 24 * 3600;
      hour_of_day_GMT = 8;
      fork (hour_of_day_GMT * 60 * 60 + day - time() % day)
        this:schedule_measurement_task();
        this.measurement_task_running = task_id();
        this:measurement_task(this.task_time_limit);
        this.measurement_task_running = 0;
      endfork
    endif
  endverb

  verb task_perms (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Put all your wizards in $byte_quota_utils.wizards.  Then various long-running tasks will cycle among the permissions, spreading out the scheduler-induced personal lag.";
    $wiz_utils.old_task_perms_user = setadd($wiz_utils.old_task_perms_user, caller);
    return $wiz_utils:random_wizard();
  endverb

  verb property_exists (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "this:property_exists(object, property)";
    " => does the specified property exist?";
    return !!`property_info(@args) ! ANY';
  endverb
endobject