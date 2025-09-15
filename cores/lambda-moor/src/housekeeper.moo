object HOUSEKEEPER
  name: "housekeeper"
  parent: PROG
  owner: HOUSEKEEPER
  player: true
  programmer: true
  readable: true

  property clean (owner: HOUSEKEEPER, flags: "r") = {};
  property cleaning (owner: HOUSEKEEPER, flags: "rc") = LOCAL;
  property cleaning_index (owner: HOUSEKEEPER, flags: "rc") = 0;
  property destination (owner: HOUSEKEEPER, flags: "rc") = {};
  property drop_off_msg (owner: HOUSEKEEPER, flags: "rc") = "%[tpsc] arrives to drop off %n, who is sound asleep.";
  property eschews (owner: HOUSEKEEPER, flags: "rc") = {};
  property litter (owner: HOUSEKEEPER, flags: "rc") = {};
  property move_player_task (owner: HOUSEKEEPER, flags: "r") = 0;
  property moveto_task (owner: HOUSEKEEPER, flags: "rc") = 0;
  property owners (owner: HOUSEKEEPER, flags: "rc") = {BYTE_QUOTA_UTILS_WORKING};
  property player_queue (owner: HOUSEKEEPER, flags: "r") = {};
  property public_places (owner: HOUSEKEEPER, flags: "rc") = {};
  property recycle_bins (owner: HOUSEKEEPER, flags: "rc") = {};
  property requestors (owner: HOUSEKEEPER, flags: "rc") = {};
  property take_away_msg (owner: HOUSEKEEPER, flags: "rc") = "%[tpsc] arrives to cart %n off to bed.";
  property task (owner: HOUSEKEEPER, flags: "rc") = 0;
  property testing (owner: HOUSEKEEPER, flags: "rc") = 0;

  override aliases (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {"housekeeper"};
  override description = "A very clean, neat, tidy person who doesn't mind lugging players and their gear all over the place.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override last_disconnect_time = 2147483647;
  override linelen = -80;
  override mail_forward = {BYTE_QUOTA_UTILS_WORKING};
  override object_size = {21397, 1084848672};
  override owned_objects = {HOUSEKEEPER};
  override ownership_quota = -9993;
  override page_absent_msg = "The housekeeper is too busy putting away all of the junk all over LambdaMoo that there isn't time to listen to pages and stuff like that so your page isn't listened to, too bad.";
  override po = "the housekeeper";
  override poc = "The housekeeper";
  override pp = "the housekeeper's";
  override ppc = "The housekeeper's";
  override pq = "the housekeeper's";
  override pqc = "The housekeeper's";
  override pr = "'self";
  override prc = "'Self";
  override ps = "the housekeeper";
  override psc = "The housekeeper";
  override size_quota = {183000, 34096, 1084780981, 0};

  verb look_self (this none this) owner: HOUSEKEEPER flags: "rxd"
    player:tell_lines(this:description());
    player:tell($string_utils:pronoun_sub("%S %<is> moving around from room to room, cleaning up.", this));
  endverb

  verb cleanup (this none this) owner: HOUSEKEEPER flags: "rxd"
    "$housekeeper:cleanup([insist]) => clean up player's objects. Argument is 'up' or 'up!' for manually requested cleanups (notify player differently)";
    if (caller_perms() != this)
      return E_PERM;
    endif
    for object in (this.clean)
      x = object in this.clean;
      if (this.requestors[x] == player)
        if (result = this:replace(object, @args))
          player:tell(result, ".");
        endif
      endif
      $command_utils:suspend_if_needed(0);
    endfor
    player:tell("The housekeeper has finished cleaning up your objects.");
  endverb

  verb replace (this none this) owner: HOUSEKEEPER flags: "rxd"
    "replace the object given to its proper spot (if there is one).";
    {object, ?insist = 0} = args;
    i = object in this.clean;
    if (!i)
      return tostr(object, " is not on the ", this.name, "'s cleanup list");
    endif
    place = this.destination[i];
    if (!($recycler:valid(object) && ($recycler:valid(r = this.requestors[i]) && is_player(r)) && ($recycler:valid(place) || place == #-1) && !(object.location in this.recycle_bins)))
      "object no longer valid (recycled or something), remove it.";
      this.clean = listdelete(this.clean, i);
      this.requestors = listdelete(this.requestors, i);
      this.destination = listdelete(this.destination, i);
      return tostr(object) + " is no longer valid, removed from cleaning list";
    endif
    oldloc = loc = object.location;
    if (object.location == place)
      "already in its place";
      return "";
    endif
    requestor = $recycler:valid(tr = this.requestors[i]) ? tr | $no_one;
    if (insist != "up!")
      if ($code_utils:verb_or_property(object, "in_use"))
        return "Not returning " + object.name + " because it claims to be in use";
      endif
      for thing in (object.contents)
        if (thing:is_listening())
          return "Not returning " + object.name + " because " + thing.name + " is inside";
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      if (valid(loc) && loc != $limbo)
        if (loc:is_listening())
          return "Not returning " + object.name + " because " + loc.name + " is holding it";
        endif
        for y in (loc:contents())
          if (y != object && y:is_listening())
            return "Not returning " + object.name + " because " + y.name + " is in " + loc.name;
          endif
          $command_utils:suspend_if_needed(0);
        endfor
      endif
    endif
    if (valid(place) && !place:acceptable(object))
      return place.name + " won't accept " + object.name;
    endif
    try
      requestor:tell("As you requested, the housekeeper tidies ", $string_utils:nn(object), " from ", $string_utils:nn(loc), " to ", $string_utils:nn(place), ".");
      if ($object_utils:has_verb(loc, "announce_all_but"))
        loc:announce_all_but({requestor, object}, "At ", requestor.name, "'s request, the ", this.name, " sneaks in, picks up ", object.name, " and hurries off to put ", $object_utils:has_property(object, "po") && typeof(object.po) == STR ? object.po | "it", " away.");
      endif
    except (ANY)
      "Ignore errors";
    endtry
    fork (0)
      this:moveit(object, place, requestor);
      if ((loc = object.location) == oldloc)
        return object.name + " wouldn't go; " + (!place:acceptable(object) ? " perhaps " + $string_utils:nn(place) + " won't let it in" | " perhaps " + $string_utils:nn(loc) + " won't let go of it");
      endif
      try
        object:tell("The housekeeper puts you away.");
        if ($object_utils:isa(loc, $room))
          loc:announce_all_but({object}, "At ", requestor.name, "'s request, the housekeeper sneaks in, deposits ", object:title(), " and leaves.");
        else
          loc:tell("You notice the housekeeper sneak in, give you ", object:title(), " and leave.");
        endif
      except (ANY)
        "Ignore errors";
      endtry
    endfork
    return "";
  endverb

  verb cleanup_list (any none none) owner: HOUSEKEEPER flags: "rxd"
    if (args)
      if (!valid(who = args[1]))
        return;
      endif
      player:tell(who.name, "'s personal cleanup list:");
    else
      who = 0;
      player:tell("Housekeeper's complete cleanup list:");
    endif
    player:tell("------------------------------------------------------------------");
    printed_anything = 0;
    objs = this.clean;
    reqs = this.requestors;
    dest = this.destination;
    objfieldwid = length(tostr(max_object())) + 1;
    for i in [1..length(objs)]
      $command_utils:suspend_if_needed(2);
      req = $recycler:valid(tr = reqs[i]) ? tr | $no_one;
      ob = objs[i];
      place = dest[i];
      if (who == 0 || req == who || ob.owner == who)
        if (!valid(ob))
          player:tell($string_utils:left(tostr(ob), objfieldwid), $string_utils:left("** recycled **", 50), "(", req.name, ")");
        else
          player:tell($string_utils:left(tostr(ob), objfieldwid), $string_utils:left(ob.name, 26), "=>", $string_utils:left(tostr(place), objfieldwid), (valid(place) ? place.name | "nowhere") || "nowhere", " (", req.name, ")");
        endif
        printed_anything = 1;
      endif
    endfor
    if (!printed_anything)
      player:tell("** The housekeeper has nothing in the cleanup list.");
    endif
    player:tell("------------------------------------------------------------------");
  endverb

  verb add_cleanup (any any any) owner: HOUSEKEEPER flags: "rxd"
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    {what, ?who = player, ?where = what.location} = args;
    if (what < #1 || !valid(what))
      return "invalid object";
    endif
    if ($object_utils:isa(who, $guest))
      return tostr("Guests can't use the ", this.name, ".");
    endif
    if (!is_player(who))
      return tostr("Non-players can't use the ", this.name, ".");
    endif
    if (!valid(where))
      return tostr("The ", this.name, " doesn't know how to find ", where, " in order to put away ", what.name, ".");
    endif
    if (is_player(what))
      return "The " + this.name + " doesn't do players, except to cart them home when they fall asleep.";
    endif
    for x in (this.eschews)
      if ($object_utils:isa(what, x[1]))
        ok = 0;
        for y in [3..length(x)]
          if ($object_utils:isa(what, x[y]))
            ok = 1;
          endif
        endfor
        if (!ok)
          return tostr("The ", this.name, " doesn't do ", x[2], "!");
        endif
      endif
    endfor
    if ($object_utils:has_callable_verb(where, "litterp") ? where:litterp(what) | where in this.public_places && !(what in where.residents))
      return tostr("The ", this.name, " won't litter ", where.name, "!");
    endif
    if (i = what in this.clean)
      if (!this:controls(i, who) && valid(this.destination[i]))
        return tostr($recycler:valid(tr = this.requestors[i]) ? tr.name | "Someone", " already asked that ", what.name, " be kept at ", (this.destination[i]).name, "!");
      endif
      this.requestors[i] = who;
      this.destination[i] = where;
    else
      this.clean = {what, @this.clean};
      this.requestors = {who, @this.requestors};
      this.destination = {where, @this.destination};
    endif
    return tostr("The ", this.name, " will keep ", what.name, " (", what, ") at ", valid(where) ? where.name + " (" + tostr(where) + ")" | where, ".");
  endverb

  verb remove_cleanup (any none none) owner: HOUSEKEEPER flags: "rxd"
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    {what, ?who = player} = args;
    if (i = what in this.clean)
      if (!this:controls(i, who))
        return tostr("You may remove an object from ", this.name, " list only if you own the object, the place it is kept, or if you placed the original cleaning order.");
      endif
      this.clean = listdelete(this.clean, i);
      this.destination = listdelete(this.destination, i);
      this.requestors = listdelete(this.requestors, i);
      return tostr(what.name, " (", what, ") removed from cleanup list.");
    else
      return tostr(what.name, " not in cleanup list.");
    endif
  endverb

  verb controls (this none this) owner: HOUSEKEEPER flags: "rxd"
    "does player control entry I?";
    {i, who} = args;
    if (who in {this.owner, @this.owners} || who.wizard)
      return "Yessir.";
    endif
    cleanable = this.clean[i];
    if (this.requestors[i] == who)
      return "you asked for the previous result, you can change this one.";
    elseif (who == cleanable.owner || !valid(dest = this.destination[i]) || who == dest.owner)
      return "you own the object or the place where it is being cleaned to, or the destination is no longer valid.";
    else
      return "";
    endif
  endverb

  verb continuous (this none this) owner: HOUSEKEEPER flags: "rxd"
    "start the housekeeper cleaning continuously. Kill any previous continuous";
    "task. Not meant to be called interactively.";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    if ($code_utils:task_valid(this.task))
      taskn = this.task;
      this.task = 0;
      kill_task(taskn);
    endif
    fork taskn (0)
      while (1)
        index = 1;
        while (index <= length(this.clean))
          this.cleaning = x = this.clean[index];
          this.cleaning_index = index;
          index = index + 1;
          fork (0)
            `this:replace(x) ! ANY';
          endfork
          suspend(this.testing ? 2 | this:time());
        endwhile
        suspend(5);
        this:litterbug();
      endwhile
    endfork
    this.task = taskn;
  endverb

  verb litterbug (this none this) owner: HOUSEKEEPER flags: "rxd"
    for room in (this.public_places)
      for thingy in (room.contents)
        suspend(10);
        if (thingy.location == room && this:is_litter(thingy) && !this:is_watching(thingy, $nothing))
          "if it is litter and no-one is watching";
          fork (0)
            this:send_home(thingy);
          endfork
          suspend(0);
        endif
      endfor
    endfor
  endverb

  verb is_watching (this none this) owner: HOUSEKEEPER flags: "rxd"
    return valid(thing = args[1]) && thing:is_listening();
  endverb

  verb send_home (this none this) owner: HOUSEKEEPER flags: "rxd"
    if (caller != this)
      return E_PERM;
    endif
    litter = args[1];
    littering = litter.location;
    this:ejectit(litter, littering);
    home = litter.location;
    if ($object_utils:isa(home, $room))
      home:announce_all("The ", this.name, " sneaks in, deposits ", litter:title(), " and leaves.");
    else
      home:tell("You notice the ", this.name, " sneak in, give you ", litter:title(), " and leave.");
    endif
    if ($object_utils:has_callable_verb(littering, "announce_all_but"))
      littering:announce_all_but({litter}, "The ", this.name, " sneaks in, picks up ", litter:title(), " and rushes off to put it away.");
    endif
  endverb

  verb moveit (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Wizardly verb to move object with requestor's permission";
    if (caller != this)
      return E_PERM;
    else
      set_task_perms(player = args[3]);
      return (args[1]):moveto(args[2]);
    endif
  endverb

  verb ejectit (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "this:ejectit(object,room): Eject args[1] from args[2].  Callable only by housekeeper's quarters verbs.";
    if (caller == this)
      (args[2]):eject(args[1]);
    endif
  endverb

  verb is_object_cleaned (this none this) owner: HOUSEKEEPER flags: "rxd"
    what = args[1];
    if (!(where = what in this.clean))
      return 0;
    else
      return {this.destination[where], this.requestors[where]};
    endif
  endverb

  verb is_litter (this none this) owner: HOUSEKEEPER flags: "rxd"
    thingy = args[1];
    for x in (this.litter)
      if ($object_utils:isa(thingy, x[1]) && !$object_utils:isa(thingy, x[2]))
        return 1;
      endif
    endfor
    return 0;
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      this.password = "Impossible password to type";
      this.last_password_time = 0;
      this.litter = {};
      this.public_places = {};
      this.requestors = {};
      this.destination = {};
      this.clean = {};
      this.eschews = {};
      this.recycle_bins = {};
      this.cleaning = #-1;
      this.task = 0;
      this.owners = {#2};
      this.mail_forward = {#2};
      this.player_queue = {};
      this.move_player_task = 0;
      this.moveto_task = 0;
      pass(@args);
    endif
  endverb

  verb clean_status (this none this) owner: HOUSEKEEPER flags: "rxd"
    count = 0;
    for i in (this.requestors)
      if (i == player)
        count = count + 1;
      endif
      $command_utils:suspend_if_needed(1);
    endfor
    player:tell("Number of items in cleanup list: ", tostr(length(this.clean)));
    player:tell("Number of items you requested to be tidied: ", tostr(count));
    player:tell("Number of requestors: ", tostr(length($list_utils:remove_duplicates(this.requestors))));
    player:tell("Time to complete one cleaning circuit: ", $time_utils:english_time(length(this.clean) * this:time()));
    player:tell("The Housekeeper is in " + ($housekeeper.testing == 0 ? "normal, non-testing mode." | "testing mode. "));
    if (!$code_utils:task_valid($housekeeper.task))
      player:tell("The Housekeeper task has died. Restarting...");
      $housekeeper:continuous();
    else
      player:tell("The Housekeeper is actively cleaning.");
    endif
  endverb

  verb is_cleaning (this none this) owner: HOUSEKEEPER flags: "rxd"
    "return a string status if the hosuekeeper is cleaning this object";
    cleanable = args[1];
    info = this:is_object_cleaned(cleanable);
    if (info == 0)
      return tostr(cleanable.name, " is not cleaned by the ", this.name, ".");
    else
      return tostr(cleanable.name, " is kept tidy at ", $string_utils:nn(info[1]), " at the request of ", $string_utils:nn(info[2]), ".");
    endif
  endverb

  verb time (this none this) owner: HOUSEKEEPER flags: "rxd"
    "Returns the amount of time to suspend between objects while continuous cleaning.";
    "Currently set to try to complete cleaning circuit in one hour, but not exceed one object every 20 seconds.";
    return max(20 + $login:current_lag(), length(this.clean) ? 3600 / length(this.clean) | 0);
  endverb

  verb acceptable (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return caller == this;
  endverb

  verb move_players_home (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!$perm_utils:controls(caller_perms(), this))
      "perms don't control the $housekeeper; probably not called by $room:disfunc then. Used to let args[1] call this. No longer.";
      return E_PERM;
    endif
    this.player_queue = {@this.player_queue, {args[1], time() + 300}};
    if ($code_utils:task_valid(this.move_player_task))
      "the move-players-home task is already running";
      return;
    endif
    fork tid (10)
      while (this.player_queue)
        if ((mtime = this.player_queue[1][2]) < time() + 10)
          who = this.player_queue[1][1];
          "Remove from queue first so that if they do something malicious, like put a kill_task in a custom :accept_for_abode, they won't be in the queue when the task restarts with the next player disconnect. Ho_Yan 12/3/98";
          this.player_queue = listdelete(this.player_queue, 1);
          if (is_player(who) && !$object_utils:connected(who))
            dest = `who.home:accept_for_abode(who) ! ANY => 0' ? who.home | $player_start;
            if (who.location != dest)
              player = who;
              this:move_em(who, dest);
            endif
          endif
        else
          suspend(mtime - time());
        endif
        $command_utils:suspend_if_needed(1);
      endwhile
    endfork
    this.move_player_task = tid;
  endverb

  verb move_em (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this)
      {who, dest} = args;
      set_task_perms(who);
      fork (0)
        fork (0)
          "This is forked so that it's protected from aborts due to errors in the player's :moveto verb.";
          if (who.location != dest)
            "Unfortunately, if who is -already- at $player_start, move() won't call :enterfunc and the sleeping body never goes to $limbo. Have to call explicitly for that case. Ho_Yan 11/2/95";
            if (who.location == $player_start)
              $player_start:enterfunc(who);
            else
              "Nosredna, 5/4/01: but wait, why don't we just moved them straight to limbo?";
              move(who, $limbo);
            endif
          endif
        endfork
        start = who.location;
        this:set_moveto_task();
        who:moveto(dest);
        if (who.location != start)
          start:announce(this:take_away_msg(who));
        endif
        if (who.location == dest)
          dest:announce(this:drop_off_msg(who));
        endif
      endfork
    else
      return E_PERM;
    endif
  endverb

  verb "take_away_msg drop_off_msg" (this none this) owner: HOUSEKEEPER flags: "rxd"
    return $string_utils:pronoun_sub(this.(verb), args[1], this);
  endverb

  verb set_moveto_task (this none this) owner: HOUSEKEEPER flags: "rxd"
    "sets $housekeeper.moveto_task to the current task_id() so player:moveto's can check for validity.";
    if (caller != this)
      return E_PERM;
    endif
    this.moveto_task = task_id();
  endverb
endobject