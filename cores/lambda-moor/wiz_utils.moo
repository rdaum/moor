object WIZ_UTILS
  name: "Wizard Utilities"
  parent: GENERIC_UTILS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  property boot_exceptions (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property boot_task (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 585440461;
  property change_password_restricted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {};
  property chparent_restricted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {};
  property default_player_quota (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 7;
  property default_programmer_quota (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 7;
  property expiration_progress (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = LOCAL;
  property expiration_recipient (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {BYTE_QUOTA_UTILS_WORKING};
  property missed_help_counters (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {};
  property missed_help_strings (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {};
  property new_core_message (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {
    "Getting Started with your LambdaCore MOO",
    "========================================",
    "",
    "Thank you for choosing LambdaCore!",
    "",
    "Initial Setup Notes",
    "-------------------",
    "",
    "The \"welcome\" screen, seen when a player connects.",
    "  -- this is stored in $login.welcome_message",
    "",
    "Do you want on-line character creation?",
    "  -- this is stored in $login.create_enabled",
    "     for more detailed information, edit $login:player_creation_enabled",
    "",
    "Do you want to limit the number of players on the MOO at once?",
    "  -- look at $login.max_connections",
    "     the `connection_limit' message on $login is the message printed",
    "     when this limit is reached.",
    "",
    "Do you want a different default player class?",
    "  -- set $player_class to a different value",
    "     *do not* change $player",
    "",
    "You should also set the following:",
    "  $network.postmaster",
    "    -- your email address, or the email address of the person who will ",
    "       handle your email",
    "  $network.site",
    "    -- the machine your MOO is running on (e.g. \"lambda.moo.mud.org\")",
    "  $network.port",
    "    -- the port your MOO is running on (e.g. 8888)",
    "  $network.MOO_Name",
    "    -- the name of your MOO (e.g. \"LambdaMOO\")",
    "  $site_db.domain",
    "  -- this is set to the `domain' of your address",
    "     (eg `foo.com' for `moo.foo.com')",
    "",
    "If you compiled the server with open_network_connection() enabled (allowing the MOO to open up connections with other computers on the network), then you should set",
    "  $network.active = 1",
    "     This will enable @newpassword, @registerme, @password, @mailme, @netforward, and others to send mail from the MOO.",
    "",
    "-------------------------------------------------------------------",
    "",
    "Setting Yourself Up",
    "-------------------",
    "",
    "Set a password for yourself.",
    "  -- @password <new-password>",
    "",
    "Set a description for yourself.",
    "  -- @describe me as <anything>",
    "",
    "Set a gender for yourself.",
    "  -- @gender <gender>",
    "",
    "There are, also, a large number of messages you can set on yourself.  Setting them will enhance the virtual reality.",
    "",
    "-------------------------------------------------------------------",
    "",
    "About Guests",
    "------------",
    "",
    "To make a new Guest character:",
    "  -- @make-guest <guestname>",
    "     will make a new guest with the name you specify with `_Guest' appended",
    "     and some other standard but useful aliases",
    "",
    "This is the easiest way to make Guest characters.  The most important things to remember about Guests, if you want to make them yourself, are:",
    "  -- make them owned by nonwizards, and not owned by themselves",
    "  -- make sure they've got .password == 0, and that .password is nonclear",
    "  -- at least one Guest must always be named `Guest'; this can be an alias",
    "",
    "To set the default description and gender for a guest:",
    "  -- set .default_description to the description the guest should start with",
    "  -- set .default_gender to the gender the guest should start with",
    "  -- remember to set .description and .gender too, for the guest's first use",
    "",
    "-------------------------------------------------------------------",
    "",
    "Adding to the Newspaper",
    "-----------------------",
    "",
    "The newspaper is a special mailing list.  To add a post to the newspaper, send mail to it (as *News or $news), and then note the number of your post (let's call it <x> and:",
    "  -- @addnews <x> to *News",
    "... in general, `@addnews $ to *News' will work as well.",
    "",
    "-------------------------------------------------------------------",
    "",
    "Quota",
    "-----",
    "",
    "By default, LambdaCore runs with byte-based quota, an in-DB quota system, limiting users by total database space as opposed to total objects.  You'll need to do two things:",
    "  -- decide on the default quota:",
    "     ;$byte_quota_utils.default_quota[1] = <a number of bytes>",
    "  -- start the measurement task; see `help routine_tasks' for more information (Note: this help topic contains information about more than just the quota task; it should be read regardless of how quota is set).",
    "",
    "If you prefer the quota system documented in the LambdaMOO Programmer's Manual, directly supported by the server, you can enable object-based quota:",
    "  -- set $quota_utils to $object_quota_utils",
    "",
    "It's best that you make this switch before users start, because converting existing users is an awkward (and inherently arbitrary and political) move.",
    "",
    "-------------------------------------------------------------------",
    "",
    "Making Programmers",
    "------------------",
    "",
    "The command to turn someone into a programmer is `@programmer'  Its syntax is `@programmer <user>'.  For example:",
    "  -- @programmer Haakon",
    "The `@programmer' verb will prompt you if the user isn't set up with a description and a gender.",
    "",
    "No code to automatically grant programmer bits is included with LambdaCore.",
    "",
    "Making Wizards",
    "--------------",
    "",
    "THINK CAREFULLY.",
    "",
    "Be very careful before giving someone a wizard bit.  That person can do gross damage to your database, and fixable but serious damage to the machine it runs on.  That person can quite possibly open outbound network connections from your machine, and thus commit acts for which your host system will be blamed.  That person can ruin your MOO's as-yet-untarnished reputation.",
    "",
    "Wizards have technical power, the ability to change anything within the database, to create anything within the database.  Be careful with the idea of a `Social Wizard' -- a nontechnical person holding a wizard bit is fairly likely to, at some point, accidentally do something destructive.  It's a good idea not to socialize as your wizard character, for the same reason, to make it less likely to be accidentally destructive.",
    "",
    "That said, in general you don't turn an existing character into a wizard, you make a -new- character to be the wizard.  This is because the existing character probably owns code and objects which could be destructive if suddenly made wizardly; it's a good security measure to make a fresh player.  So, to make a fresh player:",
    "  -- @make-player (see `help @make-player' for more information)",
    "     this will make you a new player. for this example, #123",
    "",
    "To make #123 a wizard:",
    "  -- @programmer #123",
    "     (a nonprogrammer wizard is a truly strange beast)",
    "  -- ;#123.wizard = 1;",
    "  -- @chparent #123 to $wiz",
    "  -- ;#123.public_identity = <the player's nonwizard character's object number>",
    "",
    "-------------------------------------------------------------------",
    "",
    "Good luck with your new LambdaCore database!",
    "",
    "Visit us at LambdaMOO: lambda.moo.mud.org 8888",
    "",
    "Join the international mailing list for MOO coders: send an email message to moo-cows-request@the-b.org with the word `subscribe' as the body of your message.",
    "",
    "Do good things.",
    "",
    "The LambdaMOO Wizards",
    "[authored February 15, 1999]"
  };
  property next_perm_index (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 1;
  property old_task_perms_user (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {#8060};
  property programmer_restricted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property programmer_restricted_temp (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property record_missed_help (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property registration_domain_restricted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property rename_restricted (owner: HACKER, flags: "") = {};
  property suicide_string (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "You don't *really* want to commit suicide, do you?";
  property system_chars (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {HACKER, NO_ONE, HOUSEKEEPER};
  property wizards (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {BYTE_QUOTA_UTILS_WORKING};

  override aliases = {"Wizard Utilities"};
  override description = {
    "This is the Wizard Utilities utility package.  See `help $wiz_utils' for more details."
  };
  override help_msg = {
    "Wizard Utilities",
    "----------------",
    "The following functions are substitutes for various server builtins.",
    "Anytime one feel tempted to use one of the expressions on the right,",
    "use the corresponding one on the left instead.  This will take care",
    "of various things that the server (for whatever reason) does not handle.",
    "",
    ":set_programmer(object)             object.programmer = 1;",
    "    chparent object to $prog",
    "    send mail to $prog_log",
    "",
    ":set_player(object[,nochown])       set_player_flag(object,1);",
    "    set player flag, ",
    "    add name/aliases to $player_db,",
    "    and maybe do a self chown.",
    "",
    ":unset_player(object[,newowner])    set_player_flag(object,0);",
    "    unset player flag,",
    "    remove name/aliases from $player_db",
    "    chown to newowner if given",
    "",
    ":set_owner(object, newowner)        object.owner = newowner;",
    "    change ownership on object",
    "    change ownership on all +c properties",
    "    juggle .ownership_quotas",
    "",
    ":set_property_owner(object, property, newowner[, suspend-ok])",
    "    change owner on a given property",
    "    if this is a -c property, we change the owner on all descendants",
    "    for which this is also a -c property.",
    "    Polite protest if property is +c and newowner != object.owner.",
    "",
    ":set_property_flags(object, property, flags[, suspend-ok])",
    "    change the permissions on a given property and propagate these to ",
    "    *all descendants*.  property ownership is changed on descendants ",
    "    where necessary."
  };
  override object_size = {55744, 1084848672};

  verb set_programmer (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_programmer(victim[,mail from])  => 1 or error.";
    "Sets victim.programmer, chparents victim to $prog if necessary, and sends mail to $new_prog_log, mail is from optional second arg or caller_perms().";
    whodunnit = caller_perms();
    {victim, ?mailfrom = whodunnit} = args;
    if (!whodunnit.wizard)
      return E_PERM;
    elseif (!(valid(victim) && (is_player(victim) && $object_utils:isa(victim, $player))))
      return E_INVARG;
    elseif (victim.programmer)
      return E_NONE;
    elseif (this:check_prog_restricted(victim))
      return E_INVARG;
    elseif (typeof(e = `victim.programmer = 1 ! ANY') == ERR)
      return e;
    else
      $quota_utils:adjust_quota_for_programmer(victim);
      if (!$object_utils:isa(victim, $prog))
        if (typeof(e = `chparent(victim, $prog) ! ANY') == ERR)
          "...this isn't really supposed to happen but it could...";
          player:notify(tostr("chparent(", victim, ",", $prog, ") failed:  ", e));
          player:notify("Check for common properties.");
        endif
      else
        player:notify(tostr(victim.name, " was already a child of ", parent(victim).name, " (", parent(victim), ")"));
      endif
      if (!($mail_agent:send_message(mailfrom, {$new_prog_log, victim}, tostr("@programmer ", victim.name, " (", victim, ")"), tostr("I just gave ", victim.name, " a programmer bit."))[1]))
        $mail_agent:send_message(mailfrom, {$new_prog_log}, tostr("@programmer ", victim.name, " (", victim, ")"), tostr("I just gave ", victim.name, " a programmer bit."));
      endif
      return 1;
    endif
  endverb

  verb set_player (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_player(victim[,nochown]) => 1 or error";
    "Set victim's player flag, (maybe) chown to itself, add name and aliases to $player_db.";
    " E_NONE == already a player,";
    " E_NACC == player_db is frozen,";
    " E_RECMOVE == name is unavailable";
    {victim, ?nochown = 0} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    elseif (!(valid(victim) && $object_utils:isa(victim, $player)))
      return E_INVARG;
    elseif (is_player(victim))
      return E_NONE;
    elseif ($player_db.frozen)
      return E_NACC;
    elseif (!$player_db:available(name = victim.name))
      return E_RECMOVE;
    else
      set_player_flag(victim, 1);
      if (0 && $object_utils:isa(victim, $prog))
        victim.programmer = 1;
      else
        victim.programmer = $player.programmer;
      endif
      if (!nochown)
        $wiz_utils:set_owner(victim, victim);
      endif
      $player_db:insert(name, victim);
      for a in (setremove(aliases = victim.aliases, name))
        if (index(a, " "))
          "..ignore ..";
        elseif ($player_db:available(a) in {this, 1})
          $player_db:insert(a, victim);
        else
          aliases = setremove(aliases, a);
        endif
      endfor
      victim.aliases = setadd(aliases, name);
      return 1;
    endif
  endverb

  verb set_owner (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_owner(object,newowner[,suspendok])  does object.owner=newowner, taking care of c properties as well.  This should be used anyplace one is contemplating doing object.owner=newowner, since the latter leaves ownership of c properties unchanged.  (--Rog thinks this is a server bug).";
    {object, newowner, ?suspendok = 0} = args;
    if (!valid(object))
      return E_INVIND;
    elseif (!caller_perms().wizard)
      return E_PERM;
    elseif (!(valid(newowner) && is_player(newowner)))
      return E_INVARG;
    endif
    oldowner = object.owner;
    object.owner = newowner;
    for pname in ($object_utils:all_properties(object))
      if (suspendok && (ticks_left() < 5000 || seconds_left() < 2))
        suspend(0);
      endif
      perms = property_info(object, pname)[2];
      if (index(perms, "c"))
        set_property_info(object, pname, {newowner, perms});
      endif
    endfor
    if ($object_utils:isa(oldowner, $player))
      if (is_player(oldowner) && object != oldowner)
        $quota_utils:reimburse_quota(oldowner, object);
      endif
      if (typeof(oldowner.owned_objects) == LIST)
        oldowner.owned_objects = setremove(oldowner.owned_objects, object);
      endif
    endif
    if ($object_utils:isa(newowner, $player))
      if (object != newowner)
        $quota_utils:charge_quota(newowner, object);
      endif
      if (typeof(newowner.owned_objects) == LIST)
        newowner.owned_objects = setadd(newowner.owned_objects, object);
      endif
    endif
    return 1;
  endverb

  verb set_property_owner (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_property_owner(object,prop,newowner[,suspendok])  changes the ownership of object.prop to newowner.  If the property is !c, changes the ownership on all of the descendents as well.  Otherwise, we just chown the property on the object itself and give a warning if newowner!=object.owner (--Rog thinks this is a server bug that one is able to do this at all...).";
    {object, pname, newowner, ?suspendok = 0} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    elseif (!(info = `property_info(object, pname) ! ANY'))
      "... handles E_PROPNF and invalid object errors...";
      return info;
    elseif (!is_player(newowner))
      return E_INVARG;
    elseif (index(info[2], "c"))
      if (suspendok / 2)
        "...(recursive call)...";
        "...child property is +c while parent is -c??...RUN AWAY!!";
        return E_NONE;
      else
        set_property_info(object, pname, listset(info, newowner, 1));
        return newowner == object.owner || E_NONE;
      endif
    else
      set_property_info(object, pname, listset(info, newowner, 1));
      if (suspendok % 2 && (ticks_left() < 10000 || seconds_left() < 2))
        suspend(0);
      endif
      suspendok = 2 + suspendok;
      for c in (children(object))
        this:set_property_owner(c, pname, newowner, suspendok);
      endfor
      return 1;
    endif
  endverb

  verb unset_player (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":unset_player(victim[,newowner])  => 1 or error";
    "Reset victim's player flag, chown victim to newowner (if given), remove all of victim's names and aliases from $player_db.";
    {victim, ?newowner = 0} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    elseif (!valid(victim))
      return E_INVARG;
    elseif (!is_player(victim))
      return E_NONE;
    endif
    if (typeof(newowner) == OBJ)
      $wiz_utils:set_owner(victim, newowner);
    endif
    victim.programmer = 0;
    victim.wizard = 0;
    set_player_flag(victim, 0);
    if ($object_utils:has_property($local, "second_char_registry"))
      $local.second_char_registry:delete_player(victim);
      `$local.second_char_registry:delete_shared(victim) ! ANY';
    endif
    if ($player_db.frozen)
      player:tell("Warning:  player_db is in the middle of a :load().");
    endif
    $player_db:delete2(victim.name, victim);
    for a in (victim.aliases)
      $player_db:delete2(a, victim);
      "I don't *think* this is bad---we've already toaded the guy.  And folks with lots of aliases screw us. --Nosredna";
      $command_utils:suspend_if_needed(0);
    endfor
    return 1;
    "Paragraph (#122534) - Sat Nov 5, 2005 - Remove any shared character registry listings for `victim'.";
  endverb

  verb set_property_flags (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_property_flags(object,prop,flags[,suspendok])  changes the permissions on object.prop to flags.  Unlike a mere set_property_info, this changes the flags on all descendant objects as well.  We also change the ownership on the descendent properties where necessary.";
    {object, pname, flags, ?suspendok = 0} = args;
    perms = caller_perms();
    if (!(info = `property_info(object, pname) ! ANY'))
      "... handles E_PROPNF and invalid object errors...";
      return info;
    elseif ($set_utils:difference($string_utils:char_list(flags), {"r", "w", "c"}))
      "...not r, w, or c?...";
      return E_INVARG;
    elseif ((pinfo = `property_info(parent(object), pname) ! ANY') && flags != pinfo[2])
      "... property doesn't actually live here...";
      "... only allowed to correct so that this property matches parent...";
      return E_INVARG;
    elseif (!(perms.wizard || info[1] == perms))
      "... you have to own the property...";
      return E_PERM;
    elseif (!(!(c = index(flags, "c")) == !index(info[2], "c") || $perm_utils:controls(perms, object)))
      "... if you're changing the c flag, you have to own the object...";
      return E_PERM;
    else
      if (c)
        set_property_info(object, pname, {object.owner, kflags = flags});
      else
        set_property_info(object, pname, kflags = listset(info, flags, 2));
      endif
      for kid in (children(object))
        this:_set_property_flags(kid, pname, kflags, suspendok);
      endfor
      return 1;
    endif
  endverb

  verb _set_property_flags (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "_set_property_flags(object, pname, {owner, flags} or something+\"c\", suspendok)";
    "auxiliary to :set_property_flags... don't call this directly.";
    if (caller != this)
      return E_PERM;
    endif
    if (args[4] && $command_utils:running_out_of_time(0))
      suspend(0);
    endif
    object = args[1];
    if (typeof(args[3]) != LIST)
      set_property_info(object, args[2], {object.owner, args[3]});
    else
      set_property_info(@args[1..3]);
    endif
    for kid in (children(object))
      this:_set_property_flags(@listset(args, kid, 1));
    endfor
  endverb

  verb random_password (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Generate a random password of length args[1].  Alternates vowels and consonants, for maximum pronounceability.  Uses its own list of consonants which exclude F and C and K to prevent generating obscene sounding passwords.";
    "Capital I and lowercase L are excluded on the basis of looking like each other.";
    vowels = "aeiouyAEUY";
    consonants = "bdghjmnpqrstvwxzBDGHJLMNPQRSTVWXZ";
    len = toint(args[1]);
    if (len)
      alt = random(2) - 1;
      s = "";
      for i in [1..len]
        newchar = alt ? vowels[random($)] | consonants[random($)];
        s = s + newchar;
        alt = !alt;
      endfor
      return s;
    else
      return E_INVARG;
    endif
  endverb

  verb queued_tasks (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":queued_tasks(player) => list of queued tasks for that player.";
    "shouldn't the server builtin should work this way?  oh well";
    set_task_perms(caller_perms());
    if (typeof(e = `set_task_perms(who = args[1]) ! ANY') == ERR)
      return e;
    elseif (who.wizard)
      tasks = {};
      for t in (queued_tasks())
        if (t[5] == who)
          tasks = {@tasks, t};
        endif
      endfor
      return tasks;
    else
      return queued_tasks();
    endif
  endverb

  verb isnewt (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Return 1 if args[1] is a newted player.";
    if (!caller_perms().wizard)
      return E_PERM;
    else
      return args[1] in $login.newted;
    endif
  endverb

  verb initialize_owned (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      set_task_perms(caller_perms());
      player:tell("Beginning initialize_owned:  ", ctime());
      for o in [#0..max_object()]
        if (valid(o))
          if ($object_utils:isa(owner = o.owner, $player) && typeof(owner.owned_objects) == LIST)
            owner.owned_objects = setadd(owner.owned_objects, o);
          endif
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      player:tell("Done adding, beginning verification pass.");
      this:verify_owned_objects();
      player:tell("Finished:  ", ctime());
    endif
  endverb

  verb verify_owned_objects (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      for p in (players())
        if (typeof(p.owned_objects) == LIST)
          for o in (p.owned_objects)
            if (typeof(o) != OBJ || !valid(o) || o.owner != p)
              p.owned_objects = setremove(p.owned_objects, o);
              player:tell("Removed ", $string_utils:nn(o), " from ", $string_utils:nn(p), "'s .owned_objects list.");
              if (typeof(o) == OBJ && valid(o) && typeof(o.owner.owned_objects) == LIST)
                o.owner.owned_objects = setadd(o.owner.owned_objects, o);
              endif
            endif
            $command_utils:suspend_if_needed(0, p);
          endfor
        endif
      endfor
    endif
  endverb

  verb "connected_wizards connected_wizards_unadvertised" (this none this) owner: HACKER flags: "rxd"
    ":connected_wizards() => list of currently connected wizards and players mentioned in .public_identity properties as being wizard counterparts.";
    wizzes = $object_utils:leaves($wiz);
    wlist = {};
    everyone = verb == "connected_wizards_unadvertised";
    for w in (wizzes)
      if (w.wizard && (w.advertised || everyone))
        if (`connected_seconds(w) ! ANY => 0')
          wlist = setadd(wlist, w);
        endif
        if (`connected_seconds(w.public_identity) ! ANY => 0')
          wlist = setadd(wlist, w.public_identity);
        endif
      endif
    endfor
    return wlist;
  endverb

  verb "all_wizards_advertised all_wizards all_wizards_unadvertised" (this none this) owner: HACKER flags: "rxd"
    ":all_wizards_advertised() => list of all wizards who have set .advertised true and players mentioned their .public_identity properties as being wizard counterparts";
    wizzes = $object_utils:leaves($wiz);
    wlist = {};
    everyone = verb == "all_wizards_unadvertised";
    for w in (wizzes)
      if (w.wizard && (w.advertised || everyone))
        if (is_player(w))
          wlist = setadd(wlist, w);
        endif
        if (`is_player(w.public_identity) ! ANY')
          wlist = setadd(wlist, w.public_identity);
        endif
      endif
    endfor
    return wlist;
  endverb

  verb rename_all_instances (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":rename_all_instances(object,oldname,newname)";
    "Used to rename all instances of an unwanted verb (like recycle or disfunc)";
    "if said verb is actually defined on the object itself";
    if (caller_perms().wizard)
      found = 0;
      {object, oldname, newname} = args;
      while (info = `verb_info(object, oldname) ! ANY')
        `set_verb_info(object, oldname, listset(info, newname, 3)) ! ANY';
        found = 1;
      endwhile
      return found;
    else
      return E_PERM;
    endif
  endverb

  verb missed_help (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (this.record_missed_help && callers()[1][4] == $player)
      miss = args[1];
      if (!(index = miss in this.missed_help_strings))
        this.missed_help_strings = {miss, @this.missed_help_strings};
        this.missed_help_counters = {{0, 0}, @this.missed_help_counters};
        index = 1;
      endif
      which = args[2] ? 2 | 1;
      this.missed_help_counters[index][which] = this.missed_help_counters[index][which] + 1;
    endif
  endverb

  verb show_missing_help (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    mhs = this.missed_help_strings;
    cnt = this.missed_help_counters;
    "save values first, so subsequent changes during suspends wont affect it";
    thresh = args ? args[1] | 5;
    strs = {};
    for i in [1..length(mhs)]
      $command_utils:suspend_if_needed(0);
      if (cnt[i][1] + cnt[i][2] > thresh)
        strs = {@strs, $string_utils:right(tostr(cnt[i][1]), 5) + " " + $string_utils:right(tostr(cnt[i][2]), 5) + " " + mhs[i]};
      endif
    endfor
    sorted = $list_utils:sort_suspended(0, strs);
    len = length(sorted);
    player:tell(" miss ambig word");
    for x in [1..len]
      $command_utils:suspend_if_needed(0);
      player:tell(sorted[len - x + 1]);
    endfor
    player:tell(" - - - - - - - - -");
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      `delete_property(this, "guest_feature_restricted") ! ANY';
      this.boot_exceptions = {};
      this.programmer_restricted = {};
      this.programmer_restricted_temp = {};
      this.chparent_restricted = {};
      this.rename_restricted = {};
      this.change_password_restricted = {};
      this.record_missed_help = 0;
      this.missed_help_counters = this.missed_help_strings = {};
      this.suicide_string = "You don't *really* want to commit suicide, do you?";
      this.wizards = {#2};
      this.next_perm_index = 1;
      this.system_chars = {$hacker, $no_one, $housekeeper};
      this.expiration_progress = $nothing;
      this.expiration_recipient = {#2};
    endif
  endverb

  verb show_netwho_listing (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":show_netwho_listing(tell,player_list)";
    " prints a listing of the indicated players showing connect sites.";
    {who, unsorted} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    if (!unsorted)
      return;
    endif
    su = $string_utils;
    alist = {};
    footnotes = {};
    nwidth = length("Player name");
    for u in (unsorted)
      $command_utils:suspend_if_needed(0);
      if (u.programmer)
        pref = "% ";
        footnotes = setadd(footnotes, "prog");
      else
        pref = "  ";
      endif
      if (u in connected_players())
        lctime = ctime(time() - connected_seconds(u));
        where = connection_name(u);
      else
        lctime = ctime(u.last_connect_time);
        where = u.last_connect_place;
      endif
      name = u.name;
      if (length(name) > 15)
        name = name[1..13] + "..";
      endif
      u3 = {tostr(pref, u.name, " (", u, ")"), lctime[5..10] + lctime[20..24]};
      nwidth = max(length(u3[1]), nwidth);
      where = $string_utils:connection_hostname(where);
      if ($login:blacklisted(where))
        where = "(*) " + where;
        footnotes = setadd(footnotes, "black");
      elseif ($login:graylisted(where))
        where = "(+) " + where;
        footnotes = setadd(footnotes, "gray");
      endif
      alist = {@alist, {@u3, where}};
    endfor
    alist = $list_utils:sort_alist_suspended(0, alist, 3);
    $command_utils:suspend_if_needed(0);
    headers = {"Player name", "Last Login", "From Where"};
    before = {0, nwidth + 3, nwidth + length(ctime(0)) - 11};
    tell1 = "  " + headers[1];
    tell2 = "  " + su:space(headers[1], "-");
    for j in [2..3]
      tell1 = su:left(tell1, before[j]) + headers[j];
      tell2 = su:left(tell2, before[j]) + su:space(headers[j], "-");
    endfor
    who:notify(tell1);
    who:notify(tell2);
    for a in (alist)
      $command_utils:suspend_if_needed(0);
      tell1 = a[1];
      for j in [2..3]
        tell1 = su:left(tell1, before[j]) + a[j];
      endfor
      who:notify(tell1[1..min($, 79)]);
    endfor
    if (footnotes)
      who:notify("");
      if ("prog" in footnotes)
        who:notify(" %  == programmer.");
      endif
      if ("black" in footnotes)
        who:notify("(*) == blacklisted site.");
      endif
      if ("gray" in footnotes)
        who:notify("(+) == graylisted site.");
      endif
    endif
  endverb

  verb show_netwho_from_listing (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":show_netwho_from_listing(tell,site)";
    "@net-who from hoststring prints all players who have connected from that host or host substring.  Substring can include *'s, e.g. @net-who from *.foo.edu.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {tellwho, where} = args;
    su = $string_utils;
    if (!index(where, "*"))
      "Oh good... search for users from a site... the fast way.  No wild cards.";
      nl = 0;
      bozos = {};
      sites = $site_db:find_all_keys(where);
      while (sites)
        s = sites;
        sites = {};
        for domain in (s)
          "Temporary kluge until $site_db is repaired. --Nosredna";
          for b in ($site_db:find_exact(domain) || {})
            $command_utils:suspend_if_needed(0, "..netwho..");
            if (typeof(b) == STR)
              sites = setadd(sites, b + "." + domain);
            else
              bozos = setadd(bozos, b);
              nl = max(length(tostr(b, valid(b) && is_player(b) ? b.name | "*** recreated ***")), nl);
            endif
          endfor
        endfor
      endwhile
      if (bozos)
        tellwho:notify(tostr(su:left("  Player", nl + 7), "From"));
        tellwho:notify(tostr(su:left("  ------", nl + 7), "----"));
        for who in (bozos)
          st = su:left(tostr(valid(who) && is_player(who) ? (who.programmer ? "% " | "  ") + who.name | "", " (", who, ")"), nl + 7);
          comma = 0;
          if ($object_utils:isa(who, $player) && is_player(who))
            for p in ({$wiz_utils:get_email_address(who) || "*Unregistered*", @who.all_connect_places})
              if (comma && length(p) >= 78 - length(st))
                tellwho:notify(tostr(st, ","));
                st = su:space(nl + 7) + p;
              else
                st = tostr(st, comma ? ", " | "", p);
              endif
              comma = 1;
              $command_utils:suspend_if_needed(0);
            endfor
          else
            st = st + (valid(who) ? "*** recreated ***" | "*** recycled ***");
          endif
          tellwho:notify(st);
        endfor
        tellwho:notify("");
        tellwho:notify(tostr(length(bozos), " player", length(bozos) == 1 ? "" | "s", " found."));
      else
        tellwho:notify(tostr("No sites matching `", where, "'"));
      endif
    else
      "User typed 'from'.  Go search for users from this site.  (SLOW!)";
      howmany = 0;
      for who in (players())
        $command_utils:suspend_if_needed(0);
        matches = {};
        for name in (who.all_connect_places)
          if (index(where, "*") && su:match_string(name, where) || (!index(where, "*") && index(name, where)))
            matches = {@matches, name};
          endif
        endfor
        if (matches)
          howmany = howmany + 1;
          tellwho:notify(tostr(who.name, " (", who, "): ", su:english_list(matches)));
        endif
      endfor
      tellwho:notify(tostr(howmany || "No", " matches found."));
    endif
  endverb

  verb "check_player_request check_reregistration" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":check_player_request(name [,email [,connection]])";
    " check if the request for player and email address is valid;";
    " return empty string if it valid, or else a string saying why not.";
    " The result starts with - if this is a 'send email, don't try again' situation.";
    ":check_reregistration(who, email, connection)";
    "  Since name is ignored, only check the 'email' parts and use the first arg";
    "  for the re-registering player.";
    if (!caller_perms().wizard)
      return E_PERM;
      "accesses registration information -- wiz only";
    endif
    name = args[1];
    if (verb == "check_reregistration")
      "don't check player name";
    elseif (!name)
      return "A blank name isn't allowed.";
    elseif (name == "<>")
      return "Names with angle brackets aren't allowed.";
    elseif (index(name, " "))
      return "Names with spaces are not allowed. Use dashes or underscores.";
    elseif (match(name, "^<.*>$"))
      return tostr("Try using ", name[2..$ - 1], " instead of ", name, ".");
    elseif ($player_db.frozen)
      return "New players cannot be created at the moment, try again later.";
    elseif (!$player_db:available(name))
      return "The name '" + name + "' is not available.";
    elseif ($login:_match_player(name) != $failed_match)
      return "The name '" + name + "' doesn't seem to be available.";
    endif
    if (length(args) == 1)
      "no email address supplied.";
      return "";
    endif
    address = args[2];
    addrargs = verb == "check_reregistration" ? {name} | {};
    if ($registration_db:suspicious_address(address, @addrargs))
      return "-There has already been a character with that or a similar email address.";
    endif
    if (reason = $network:invalid_email_address(address))
      return reason + ".";
    endif
    parsed = $network:parse_address(address);
    if ($registration_db:suspicious_userid(parsed[1]))
      return tostr("-Automatic registration from an account named ", parsed[1], " is not allowed.");
    endif
    connection = length(args) > 2 ? args[3] | parsed[2];
    check_connection = $wiz_utils.registration_domain_restricted && verb == "check_player_request";
    if (connection[max($ - 2, 1)..$] == ".uk" && (parsed[2])[1..3] == "uk.")
      return tostr("Addresses must be in internet form. Try ", parsed[1], "@", $string_utils:from_list($list_utils:reverse($string_utils:explode(parsed[2], ".")), "."), ".");
    elseif (check_connection && match(connection, "^[0-9.]+$"))
      "Allow reregistration from various things we wouldn't allow registration from.  Let them register to their yahoo acct...";
      return "-The system cannot resolve the name of the system you're connected from.";
    elseif (check_connection && (a = $network:local_domain(connection)) != (b = $network:local_domain(parsed[2])))
      return tostr("-The connection is from '", a, "' but the mail address is '", b, "'; these don't seem to be the same place.");
    elseif (verb == "check_player_request" && $login:spooflisted(parsed[2]))
      return tostr("-Automatic registration is not allowed from ", parsed[2], ".");
    endif
    return "";
  endverb

  verb make_player (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "create a player named NAME with email address ADDRESS; return {object, password}.  Optional third arg is comment to be put in registration db.";
    "assumes $wiz_utils:check_player_request() has been called and it passes.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {name, address, @rest} = args;
    new = $quota_utils:bi_create($player_class, $nothing);
    new.name = name;
    new.aliases = {name};
    salt_str = salt();
    new.password = argon2(password = $wiz_utils:random_password(5), salt_str);
    new.last_password_time = time();
    new.last_connect_time = $maxint;
    "Last disconnect time is creation time, until they login.";
    new.last_disconnect_time = time();
    $quota_utils:initialize_quota(new);
    if (!(error = $wiz_utils:set_player(new)))
      return player:tell("An error, ", error, " occurred while trying to make ", new, " a player. The database is probably inconsistent.");
    endif
    $wiz_utils:set_email_address(new, address);
    $registration_db:add(new, address, @rest);
    move(new, $player_start);
    new.programmer = $player_class.programmer;
    return {new, password};
  endverb

  verb send_new_player_mail (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":send_new_player_mail(preface, name, address, character#, password)";
    "  used by $wiz:@make-player and $guest:@request";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {preface, name, address, new, password} = args;
    msg = {preface};
    msg = {@msg, tostr("A character has been created, with name \"", name, "\" and password \"", password, "\"."), "Passwords are case sensitive, which means you have to type it exactly as", "it appears here, including capital and lowercase letters.", "So, to log in, you would type:", tostr("  Connect ", name, " ", password)};
    if ($object_utils:has_property($local, "new_player_message"))
      msg = {@msg, @$local.new_player_message};
    endif
    return $network:sendmail(address, "Your " + $network.moo_name + " character, " + name, "Reply-to: " + $login.registration_address, @msg);
  endverb

  verb do_make_player (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "do_maker_player(name,email,[comment])";
    "Common code for @make-player";
    "If no password is given, generates a random password for the player.";
    "Email-address is stored in $registration_db and on the player object.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {name, email, @comments} = args;
    comments = $string_utils:from_list(comments, " ");
    reason = $wiz_utils:check_player_request(name, email);
    if (others = $registration_db:find_exact(email))
      player:notify(email + " is the registered address of the following characters:");
      for x in (others)
        player:notify(tostr(valid(x[1]) ? (x[1]).name | "<recycled>", valid(x[1]) && !is_player(x[1]) ? " {nonplayer}" | "", " (", x[1], ") ", length(x) > 1 ? "[" + tostr(@x[2..$]) + "]" | ""));
      endfor
      if (!reason)
        reason = "Already registered.";
      endif
    endif
    if (reason)
      player:notify(reason);
      if (!$command_utils:yes_or_no("Create character anyway? "))
        player:notify("Character not created.");
        return;
      endif
    endif
    new = $wiz_utils:make_player(name, email, comments);
    player:notify(tostr(name, " (", new[1], ") created with password `", new[2], "' for ", email, comments ? " [" + comments + "]" | ""));
    $mail_agent:send_message(player, $new_player_log, tostr(name, " (", new[1], ")"), tostr(email, comments ? " " + comments | ""));
    if ($network.active)
      if ($command_utils:yes_or_no("Send email to " + email + " with password? "))
        player:notify(tostr("Sending the password to ", email, "."));
        if ((result = $wiz_utils:send_new_player_mail(tostr("From ", player.name, "@", $network.moo_name, ":"), name, email, new[1], new[2])) == 0)
          player:notify(tostr("Mail sent successfully to ", email, "."));
        else
          player:tell("Cannot send mail: ", result);
        endif
      else
        player:notify("No mail sent.");
      endif
    else
      player:notify("Sorry, the network isn't active.");
    endif
  endverb

  verb do_register (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "do_register(name, email_address [,comments])";
    "change player's email address.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {whostr, email, @comments} = args;
    comments = $string_utils:from_list(comments);
    who = $string_utils:match_player(whostr);
    if ($command_utils:player_match_failed(who, whostr))
      return;
    endif
    if (whostr != who.name && !(whostr in who.aliases) && whostr != tostr(who))
      player:notify(tostr("Must be a full name or an object number:  ", who.name, "(", who, ")"));
      return;
    endif
    if (reason = $network:invalid_email_address(email))
      player:notify(reason);
      if (!$command_utils:yes_or_no("Register anyway?"))
        return player:notify("re-registration aborted.");
      endif
    endif
    if (comments)
      $registration_db:add(who, email, comments);
    else
      $registration_db:add(who, email);
    endif
    old = $wiz_utils:get_email_address(who);
    $wiz_utils:set_email_address(who, email);
    player:notify(tostr(who.name, " (", who, ") formerly ", old ? old | "unregistered", ", registered at ", email, ".", comments ? " [" + comments + "]" | ""));
  endverb

  verb do_new_password (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "do_new_password(who, [password])";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {who, ?password = $wiz_utils:random_password(6)} = args;
    if (!password)
      password = $wiz_utils:random_password(6);
    endif
    whostr = $string_utils:nn(who);
    player:notify(tostr("About to change password for ", whostr, ". Old encrypted password is \"", who.password, "\""));
    salt_str = salt();
    who.password = argon2(password, salt_str);
    who.last_password_time = time();
    player:notify(tostr(whostr, " new password is `", password, "'."));
    if (!$wiz_utils:get_email_address(who))
      player:notify(tostr(whostr, " doesn't have a registered email_address, cannot mail password; tell them some some other way."));
    elseif (who.last_connect_time == $maxint && $command_utils:yes_or_no(tostr(who.name, " has never logged in.  Send mail with the password as though this were a new player request?")))
      if ((result = $wiz_utils:send_new_player_mail(tostr("From ", player.name, "@", $network.moo_name, ":"), who.name, $wiz_utils:get_email_address(who), who, password)) == 0)
        player:tell("Mail sent.");
      else
        player:tell("Trouble sending mail: ", result);
      endif
    elseif ($command_utils:yes_or_no(tostr("Email new password to ", whostr, "?")))
      player:notify("Sending the password via email.");
      $network:adjust_postmaster_for_password("enter");
      if ((result = $network:sendmail($wiz_utils:get_email_address(who), "Your " + $network.moo_name + " password", "The password for your " + $network.moo_name + " character:", " " + whostr, "has been changed. The new password is:", " " + password, "", "Please note that passwords are case sensitive.")) == 0)
        player:tell("Mail sent.");
      else
        player:tell("Trouble sending mail: ", result);
      endif
      $network:adjust_postmaster_for_password("exit");
    else
      player:tell("No mail sent.");
    endif
  endverb

  verb set_owner_new (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_owner(object,newowner[,suspendok])  does object.owner=newowner, taking care of c properties as well.  This should be used anyplace one is contemplating doing object.owner=newowner, since the latter leaves ownership of c properties unchanged.  (--Rog thinks this is a server bug).";
    {object, newowner, ?suspendok = 0} = args;
    if (!valid(object))
      return E_INVIND;
    elseif (!caller_perms().wizard)
      return E_PERM;
    elseif (!(valid(newowner) && is_player(newowner)))
      return E_INVARG;
    endif
    oldowner = object.owner;
    object.owner = newowner;
    for pname in ($object_utils:all_properties(object))
      if (suspendok && (ticks_left() < 5000 || seconds_left() < 2))
        suspend(0);
      endif
      perms = property_info(object, pname)[2];
      if (index(perms, "c"))
        set_property_info(object, pname, {newowner, perms});
      endif
    endfor
    if ($object_utils:isa(oldowner, $player))
      if (is_player(oldowner) && object != oldowner)
        $quota_utils:reimburse_quota(oldowner, object);
      endif
      if (typeof(oldowner.owned_objects) == LIST)
        oldowner.owned_objects = setremove(oldowner.owned_objects, object);
      endif
    endif
    if ($object_utils:isa(newowner, $player))
      if (object != newowner)
        $quota_utils:charge_quota(newowner, object);
      endif
      if (typeof(newowner.owned_objects) == LIST)
        newowner.owned_objects = setadd(newowner.owned_objects, object);
      endif
    endif
    return 1;
  endverb

  verb boot_idlers (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    "------- constants ---- ";
    "20 minutes idle for regular players";
    mintime = 60 * 20;
    "10 minutes for guests";
    minguest = 60 * 10;
    "wait 3 minutes before actually booting";
    bootdelay = 3;
    "start booting when there are 20 less than max players";
    threshold = 20;
    " ----------------------";
    if ($code_utils:task_valid(this.boot_task) && task_id() != this.boot_task)
      "starting a new one: kill the old one";
      kill_task(this.boot_task);
      this.boot_task = 0;
    endif
    fork taskn (bootdelay * 60 * 3)
      maxplayers = $login:max_connections() - threshold;
      if (length(pl = connected_players()) > maxplayers)
        pll = {};
        plt = {};
        for x in (pl)
          suspend(0);
          min = $object_utils:isa(x, $guest) ? minguest | mintime;
          if ((idle = `idle_seconds(x) ! ANY => 0') > min && !x.wizard && !(x in this.boot_exceptions))
            pll = {x, @pll};
            plt = {idle, @plt};
          endif
        endfor
        if (pll)
          "Sort by idle time, and choose person who has been idle longest.";
          pll = $list_utils:sort(pll, plt);
          booted = pll[$];
          guest = $object_utils:isa(booted, $guest);
          min = guest ? minguest | mintime;
          if (`idle_seconds(booted) ! ANY => 0' > min)
            notify(booted, tostr("*** You've been idle more than ", min / 60, " minutes, and there are more than ", maxplayers, " players connected. If you're still idle and LambdaMOO is still busy in ", bootdelay, " minute", bootdelay == 1 ? "" | "s", ", you will be booted. ***"));
            fork (60 * bootdelay)
              idle = `idle_seconds(booted) ! ANY => 0';
              if (idle > min && length(connected_players()) > $login:max_connections() - threshold)
                notify(booted, "*** You've been idle too long and LambdaMOO is still too busy ***");
                server_log(tostr("IDLE: ", booted.name, " (", booted, ") idle ", idle / 60));
                boot_player(booted);
              endif
            endfork
          endif
        endif
      endif
      this:(verb)(@args);
    endfork
    this.boot_task = taskn;
    "This is set up so that it forks the task first, and this.boot_task is the task_id of whatever is running the idle booter";
  endverb

  verb grant_object (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":grant_object(what, towhom);";
    "Ownership of the object changes as in @chown and :set_owner (i.e., .owner and all c properties change).  In addition all verbs and !c properties owned by the original owner change ownership as well.  Finally, for !c properties, instances on descendant objects change ownership (as in :set_property_owner).";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {object, newowner} = args;
    if (!is_player(newowner))
      return E_INVARG;
    endif
    same = object.owner == newowner;
    for vnum in [1..length(verbs(object))]
      info = verb_info(object, vnum);
      if (!(info[1] != object.owner && (valid(info[1]) && is_player(info[1]))))
        same = same && info[1] == newowner;
        set_verb_info(object, vnum, listset(info, newowner, 1));
      endif
    endfor
    for prop in (properties(object))
      $command_utils:suspend_if_needed(0);
      info = property_info(object, prop);
      if (!(index(info[2], "c") || (info[1] != object.owner && valid(info[1]) && is_player(info[1]))))
        same = same && info[1] == newowner;
        $wiz_utils:set_property_owner(object, prop, newowner, 1);
      endif
    endfor
    suspend(0);
    $wiz_utils:set_owner(object, newowner, 1);
    return same ? "nothing changed" | "grant changed";
  endverb

  verb connection_hash (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "connection_hash(forwhom, host [,seed])";
    "Compute an encrypted hash of the host for 'forwhom', using 'crypt'.";
    {forwhom, host, @seed} = args;
    hash = toint(forwhom);
    for i in [1..length(host)]
      hash = hash * 14 + index($string_utils.ascii, host[i]);
    endfor
    return crypt(tostr(hash), @seed);
  endverb

  verb newt_player (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":newt_player(who [ , commentary] [, temporary])";
    {who, ?comment = "", ?temporary = 0} = args;
    if (!caller_perms().wizard)
      $error:raise(E_PERM);
    elseif (length(args) < 1)
      $error:raise(E_ARGS);
    elseif (typeof(who = args[1]) != OBJ || !is_player(who))
      $error:raise(E_INVARG);
    else
      if (!comment)
        player:notify("So why has this player been newted?");
        comment = $command_utils:read();
      endif
      if (temporary)
        comment = temporary + comment;
      endif
      $login.newted = setadd($login.newted, who);
      if (msg = player:newt_victim_msg())
        notify(who, msg);
      endif
      notify(who, $login:newt_registration_string());
      boot_player(who);
      player:notify(tostr(who.name, " (", who, ") has been turned into a newt."));
      $mail_agent:send_message(player, $newt_log, tostr("@newt ", who.name, " (", who, ")"), {$string_utils:from_list(who.all_connect_places, " "), @comment ? {comment} | {}});
      if ($object_utils:isa(who.location, $room) && (msg = player:newt_msg()))
        who.location:announce_all_but({who}, msg);
      endif
      player:notify(tostr("Mail sent to ", $mail_agent:name($newt_log), "."));
    endif
  endverb

  verb unset_programmer (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":unset_programmer(victim[,reason[,start time,duration]]) => 1 or error.";
    "Resets victim.programmer, adds victim to .programmer_restricted.";
    "Put into temporary list if 3rd and 4th arguments are given. Which restricts the victim for uptime duration since start time. Must give a reason, though it can be blank, in this case.";
    {victim, ?reason = "", ?start = 0, ?duration = 0} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    elseif (!valid(victim))
      return E_INVARG;
    elseif (!victim.programmer && this:check_prog_restricted(victim))
      return E_NONE;
    else
      victim.programmer = 0;
      if (is_player(victim) && $object_utils:isa(victim, $player))
        this.programmer_restricted = setadd(this.programmer_restricted, victim);
        if (start)
          this.programmer_restricted_temp = setadd(this.programmer_restricted_temp, {victim, start, duration});
        endif
      endif
      $mail_agent:send_message(caller_perms(), {$newt_log}, tostr("@deprogrammer ", victim.name, " (", victim, ")"), reason ? typeof(reason) == STR ? {reason} | reason | {});
      return 1;
    endif
  endverb

  verb is_wizard (this none this) owner: HACKER flags: "rxd"
    ":is_wizard(who) => whether `who' is a wizard or is the .public_identity of some wizard.";
    "This verb is used for permission checks on commands that should only be accessible to wizards or their ordinary-player counterparts.  It will return true for unadvertised wizards.";
    who = args[1];
    if (who.wizard)
      return 1;
    else
      for w in ($object_utils:leaves($wiz))
        if (w.wizard && is_player(w) && who == `w.public_identity ! ANY')
          return 1;
        endif
      endfor
    endif
    return 0;
  endverb

  verb expire_mail (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    this:expire_mail_lists();
    this:expire_mail_players();
  endverb

  verb expire_mail_weekly (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    fork (7 * 24 * 60 * 60)
      this:(verb)();
    endfork
    this:expire_mail();
  endverb

  verb check_prog_restricted (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Checks to see if args[1] is restricted from programmer either permanently or temporarily. Removes from temporary list if time is up";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    if ((who = args[1]) in this.programmer_restricted)
      "okay, who is restricted. Now check to see if it is temporary";
      if (entry = $list_utils:assoc(who, this.programmer_restricted_temp))
        if ($login:uptime_since(entry[2]) > entry[3])
          "It's temporary and the time is up, remove and return false";
          this.programmer_restricted_temp = setremove(this.programmer_restricted_temp, entry);
          this.programmer_restricted = setremove(this.programmer_restricted, who);
          return 0;
        else
          "time is not up";
          return 1;
        endif
      else
        return 1;
      endif
    else
      return 0;
    endif
  endverb

  verb expire_mail_players (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    s = 0;
    for p in (players())
      this.expiration_progress = p;
      if (p.owner == p && is_player(p))
        s = s + (p:expire_old_messages() || 0);
      endif
      if (ticks_left() < 10000)
        set_task_perms($wiz_utils:random_wizard());
        suspend(0);
      endif
    endfor
    $mail_agent:send_message(player, this.expiration_recipient, verb, tostr(s, " messages have been expired from players."));
    return s;
  endverb

  verb expire_mail_lists (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    sum = 0;
    for x in ($object_utils:leaves_suspended($mail_recipient))
      this.expiration_progress = x;
      temp = x:expire_old_messages();
      if (typeof(temp) == INT)
        sum = sum + temp;
      endif
      "just suspend for every fucker, I'm tired of losing.";
      set_task_perms($wiz_utils:random_wizard());
      suspend(0);
    endfor
    $mail_agent:send_message(player, this.expiration_recipient, verb, tostr(sum, " messages have been expired from mailing lists."));
    return sum;
  endverb

  verb flush_editors (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      fork (86400 * 7)
        this:(verb)();
      endfork
      player:tell("Flushing ancient editor sessions.");
      for x in ({$verb_editor, $note_editor, $mail_editor})
        x:do_flush(time() - 30 * 86400, 0);
        $command_utils:suspend_if_needed(0);
      endfor
    endif
  endverb

  verb random_wizard (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Put all your wizards in $wiz_utils.wizards.  Then various long-running tasks will cycle among the permissions, spreading out the scheduler-induced personal lag.";
    w = this.wizards;
    i = this.next_perm_index;
    if (i >= length(w))
      i = 1;
    else
      i = i + 1;
    endif
    this.next_perm_index = i;
    return w[i];
  endverb

  verb set_email_address (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    {who, email} = args;
    if (typeof(who.email_address) == LIST)
      who.email_address[1] = email;
    else
      who.email_address = email;
    endif
  endverb

  verb get_email_address (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    {who} = args;
    if (typeof(who.email_address) == LIST)
      return who.email_address[1];
    else
      return who.email_address;
    endif
  endverb
endobject