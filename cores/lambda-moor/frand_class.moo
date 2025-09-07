object FRAND_CLASS
  name: "Frand's player class"
  parent: MAIL_RECIPIENT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property at_number (owner: HACKER, flags: "rc") = 0;
  property at_room_width (owner: HACKER, flags: "rc") = 30;
  property default_refusal_time (owner: HACKER, flags: "r") = 604800;
  property join_msg (owner: HACKER, flags: "rc") = "You join %n.";
  property mail_refused_msg (owner: HACKER, flags: "rc") = "%N refuses your mail.";
  property object_port_msg (owner: HACKER, flags: "rc") = "teleports you.";
  property oplayer_port_msg (owner: HACKER, flags: "rc") = "%T teleports %n out.";
  property oself_port_msg (owner: HACKER, flags: "rc") = "%<teleports> out.";
  property othing_port_msg (owner: HACKER, flags: "rc") = "%T teleports %n out.";
  property page_refused (owner: HACKER, flags: "r") = 0;
  property page_refused_msg (owner: HACKER, flags: "rc") = "%N refuses your page.";
  property player_arrive_msg (owner: HACKER, flags: "rc") = "%T teleports %n in.";
  property player_port_msg (owner: HACKER, flags: "rc") = "You teleport %n.";
  property refused_actions (owner: HACKER, flags: "r") = {};
  property refused_extra (owner: HACKER, flags: "r") = {};
  property refused_origins (owner: HACKER, flags: "r") = {};
  property refused_until (owner: HACKER, flags: "r") = {};
  property report_refusal (owner: HACKER, flags: "r") = 0;
  property rooms (owner: HACKER, flags: "r") = {};
  property self_arrive_msg (owner: HACKER, flags: "rc") = "%<teleports> in.";
  property self_port_msg (owner: HACKER, flags: "rc") = "";
  property spurned_objects (owner: HACKER, flags: "r") = {};
  property thing_arrive_msg (owner: HACKER, flags: "rc") = "%T teleports %n in.";
  property thing_port_msg (owner: HACKER, flags: "rc") = "You teleport %n.";
  property victim_port_msg (owner: HACKER, flags: "rc") = "teleports you.";
  property whisper_refused_msg (owner: HACKER, flags: "rc") = "%N refuses your whisper.";

  override aliases (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {"Frand's player class", "player class"};
  override description = "You see a player who should type '@describe me as ...'.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override help = FRAND_HELP;
  override mail_notify (owner: HACKER, flags: "rc");
  override object_size = {69955, 1084848672};
  override size_quota = {50000, 0, 0, 1};

  verb "@rooms" (none none none) owner: HACKER flags: "rxd"
    "'@rooms' - List the rooms which are known by name.";
    line = "";
    for item in (this.rooms)
      line = line + item[1] + "(" + tostr(item[2]) + ")   ";
    endfor
    player:tell(line);
  endverb

  verb names_of (this none this) owner: HACKER flags: "rxd"
    "Return a string giving the names of the objects in a list. Now on $string_utils";
    return $string_utils:names_of(@args);
  endverb

  verb "@go" (any none none) owner: HACKER flags: "rxd"
    "'@go <place>' - Teleport yourself somewhere. Example: '@go liv' to go to the living room.";
    dest = this:lookup_room(dobjstr);
    if (dest == $failed_match)
      player:tell("There's no such place known.");
    else
      this:teleport(player, dest);
    endif
  endverb

  verb lookup_room (this none this) owner: HACKER flags: "rxd"
    "Look up a room in your personal database of room names, returning its object number. If it's not in your database, it checks to see if it's a number or a nearby object.";
    room = args[1];
    if (room == "home")
      return player.home;
    elseif (room == "me")
      return player;
    elseif (room == "here")
      return player.location;
    elseif (!room)
      return $failed_match;
    endif
    index = this:index_room(room);
    if (index)
      return this.rooms[index][2];
    else
      return this:my_match_object(room);
      "old code no longer used, 2/11/96 Heathcliff";
      source = player.location;
      if (!(valid(source) && $room in $object_utils:ancestors(source)))
        source = $room;
      endif
      return source:match_object(room);
    endif
  endverb

  verb teleport (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Teleport a player or object. For printing messages, there are three cases: (1) teleport self (2) teleport other player (3) teleport object. There's a spot of complexity for handling the invalid location #-1.";
    set_task_perms(caller == this ? this | $no_one);
    {thing, dest} = args;
    source = thing.location;
    if (valid(dest))
      dest_name = dest.name;
    else
      dest_name = tostr(dest);
    endif
    if (source == dest)
      player:tell(thing.name, " is already at ", dest_name, ".");
      return;
    endif
    thing:moveto(dest);
    if (thing.location == dest)
      tsd = {thing, source, dest};
      if (thing == player)
        this:teleport_messages(@tsd, this:self_port_msg(@tsd), this:oself_port_msg(@tsd), this:self_arrive_msg(@tsd), "");
      elseif (is_player(thing))
        this:teleport_messages(@tsd, this:player_port_msg(@tsd), this:oplayer_port_msg(@tsd), this:player_arrive_msg(@tsd), this:victim_port_msg(@tsd));
      else
        this:teleport_messages(@tsd, this:thing_port_msg(@tsd), this:othing_port_msg(@tsd), this:thing_arrive_msg(@tsd), this:object_port_msg(@tsd));
      endif
    elseif (thing.location == source)
      if ($object_utils:contains(thing, dest))
        player:tell("Ooh, it's all twisty. ", dest_name, " is inside ", thing.name, ".");
      else
        if ($object_utils:has_property(thing, "po"))
          pronoun = thing.po;
        else
          pronoun = "it";
        endif
        player:tell("Either ", thing.name, " doesn't want to go, or ", dest_name, " didn't accept ", pronoun, ".");
      endif
    else
      thing_name = thing == player ? "you" | thing.name;
      player:tell("A strange force deflects ", thing_name, " from the destination.");
    endif
  endverb

  verb teleport_messages (this none this) owner: HACKER flags: "rxd"
    "Send teleport messages. There's a slight complication in that the source and dest need not be valid objects.";
    {thing, source, dest, pmsg, smsg, dmsg, tmsg} = args;
    if (pmsg)
      "The player's own message.";
      player:tell(pmsg);
    endif
    if (smsg)
      `source:room_announce_all_but({thing, player}, smsg) ! E_VERBNF, E_INVIND';
    endif
    if (dmsg)
      `dest:room_announce_all_but({thing, player}, dmsg) ! E_VERBNF, E_INVIND';
    endif
    if (tmsg)
      "A message to the victim being teleported.";
      thing:tell(tmsg);
    endif
  endverb

  verb "@move" (any any any) owner: HACKER flags: "rxd"
    "'@move <object> to <place>' - Teleport an object. Example: '@move trash to #11' to move trash to the closet.";
    here = player.location;
    if (prepstr != "to" || !iobjstr)
      player:tell("Usage: @move <object> to <location>");
      return;
    endif
    if (!dobjstr || dobjstr == "me")
      thing = this;
    else
      thing = `here:match_object(dobjstr) ! E_VERBNF, E_INVIND => $failed_match';
      if (thing == $failed_match)
        thing = player:my_match_object(dobjstr);
      endif
    endif
    if ($command_utils:object_match_failed(thing, dobjstr))
      return;
    endif
    if (!player.programmer && (thing.owner != player && thing != player))
      player:tell("You can only move your own objects.");
      return;
    endif
    dest = this:lookup_room(iobjstr);
    if (dest == #-1 || !$command_utils:object_match_failed(dest, iobjstr))
      this:teleport(thing, dest);
    endif
  endverb

  verb index_room (this none this) owner: HACKER flags: "rxd"
    "'index_room (<room name>)' - Look up a room in your personal database of room names, returning its index in the list. Return 0 if it is not in the list. If the room name is the empty string, then only exact matches are considered; otherwise, a leading match is good enough.";
    room = tostr(args[1]);
    size = length(room);
    index = 1;
    match = 0;
    for item in (this.rooms)
      item_name = item[1];
      if (room == item_name)
        return index;
      elseif (size && length(item_name) >= size && room == item_name[1..size])
        match = index;
      endif
      index = index + 1;
    endfor
    return match;
  endverb

  verb "@addr*oom" (any none none) owner: HACKER flags: "rxd"
    "'@addroom <name> <object>', '@addroom <object> <name>', '@addroom <name>', '@addroom <object>', '@addroom' - Add a room to your personal database of teleport destinations. Example: '@addroom Kitchen #24'. Reasonable <object>s are numbers (#17) and 'here'. If you leave out <object>, the object is the current room. If you leave out <name>, the name is the specified room's name. If you leave out both, you get the current room and its name.";
    if (!caller && player != this || caller && callers()[1][3] != this)
      if (!caller)
        player:tell(E_PERM);
      endif
      return E_PERM;
    endif
    if (!dobjstr)
      object = this.location;
      name = valid(object) ? object.name | "Nowhere";
    elseif (command = this:parse_out_object(dobjstr))
      name = command[1];
      object = command[2];
    else
      name = dobjstr;
      object = this.location;
    endif
    if (!valid(object))
      player:tell("This is not a valid location.");
      return E_INVARG;
    endif
    player:tell("Adding ", name, "(", tostr(object), ") to your database of rooms.");
    this.rooms = {@this.rooms, {name, object}};
  endverb

  verb "@rmr*oom" (any none none) owner: HACKER flags: "rxd"
    "'@rmroom <roomname>' - Remove a room from your personal database of teleport destinations. Example: '@rmroom library'.";
    if (!caller && player != this || caller && callers()[1][3] != this)
      if (!caller)
        player:tell(E_PERM);
      endif
      return E_PERM;
    endif
    index = this:index_room(dobjstr);
    if (index)
      player:tell("Removing ", this.rooms[index][1], "(", this.rooms[index][2], ").");
      this.rooms = listdelete(this.rooms, index);
    else
      player:tell("That room is not in your database of rooms. Check '@rooms'.");
    endif
  endverb

  verb "@join" (any none none) owner: HACKER flags: "rxd"
    "'@join <player>' - Teleport yourself to the location of any player, whether connected or not.";
    if (dobjstr == "")
      player:tell("Usage: @join <player>. For example, '@join frand'.");
      return;
    endif
    target = $string_utils:match_player(dobjstr);
    $command_utils:player_match_result(target, dobjstr);
    if (valid(target))
      if (target == this)
        if (player == this)
          player:tell("There is little need to join yourself, unless you are split up.");
        else
          player:tell("No thank you. Please get your own join verb.");
        endif
        return;
      endif
      dest = target.location;
      msg = this:enlist(this:join_msg());
      editing = $object_utils:isa(dest, $generic_editor);
      if (editing)
        dest = dest.original[target in dest.active];
        editing_msg = "%N is editing at the moment. You can wait here until %s is done.";
        if (player.location == dest)
          msg = {editing_msg};
        else
          msg = {@msg, editing_msg};
        endif
      endif
      if (msg && (player.location != dest || editing))
        player:tell_lines($string_utils:pronoun_sub(msg, target));
      elseif (player.location == dest)
        player:tell("OK, you're there. You didn't need to actually move, though.");
        return;
      endif
      this:teleport(player, dest);
    endif
  endverb

  verb "@find" (any none none) owner: HACKER flags: "rxd"
    "'@find #<object>', '@find <player>', '@find :<verb>' '@find .<property>' - Attempt to locate things. Verbs and properties are found on any object in the player's vicinity, and some other places.  '@find ?<help>' looks for a help topic on any available help database.";
    if (!dobjstr)
      player:tell("Usage: '@find #<object>' or '@find <player>' or '@find :<verb>' or '@find .<property>' or '@find ?<help topic>'.");
      return;
    endif
    if (dobjstr[1] == ":")
      name = dobjstr[2..$];
      this:find_verb(name);
      return;
    elseif (dobjstr[1] == ".")
      name = dobjstr[2..$];
      this:find_property(name);
      return;
    elseif (dobjstr[1] == "#")
      target = toobj(dobjstr);
      if (!valid(target))
        player:tell(target, " does not exist.");
      endif
    elseif (dobjstr[1] == "?")
      name = dobjstr[2..$];
      this:find_help(name);
      return;
    else
      target = $string_utils:match_player(dobjstr);
      $command_utils:player_match_result(target, dobjstr);
    endif
    if (valid(target))
      player:tell(target.name, " (", target, ") is at ", valid(target.location) ? target.location.name | "Nowhere", " (", target.location, ").");
    endif
  endverb

  verb find_verb (this none this) owner: HACKER flags: "rxd"
    "'find_verb (<name>)' - Search for a verb with the given name. The objects searched are those returned by this:find_verbs_on(). The printing order relies on $list_utils:remove_duplicates to leave the *first* copy of each duplicated element in a list; for example, {1, 2, 1} -> {1, 2}, not to {2, 1}.";
    name = args[1];
    results = "";
    objects = $list_utils:remove_duplicates(this:find_verbs_on());
    for thing in (objects)
      if (valid(thing) && (mom = $object_utils:has_verb(thing, name)))
        results = results + "   " + thing.name + "(" + tostr(thing) + ")";
        mom = mom[1];
        if (thing != mom)
          results = results + "--" + mom.name + "(" + tostr(mom) + ")";
        endif
      endif
    endfor
    if (results)
      this:tell("The verb :", name, " is on", results);
    else
      this:tell("The verb :", name, " is nowhere to be found.");
    endif
  endverb

  verb "@ways" (any none none) owner: HACKER flags: "rxd"
    "'@ways', '@ways <room>' - List any obvious exits from the given room (or this room, if none is given).";
    if (dobjstr)
      room = dobj;
    else
      room = this.location;
    endif
    if (!valid(room) || !($room in $object_utils:ancestors(room)))
      player:tell("You can only pry into the exits of a room.");
      return;
    endif
    exits = {};
    if ($object_utils:has_verb(room, "obvious_exits"))
      exits = room:obvious_exits();
    endif
    exits = this:checkexits(this:obvious_exits(), room, exits);
    exits = this:findexits(room, exits);
    this:tell_ways(exits, room);
  endverb

  verb findexits (this none this) owner: HACKER flags: "rxd"
    "Add to the 'exits' list any exits in the room which have a single-letter alias.";
    {room, exits} = args;
    alphabet = "abcdefghijklmnopqrstuvwxyz0123456789";
    for i in [1..length(alphabet)]
      found = room:match_exit(alphabet[i]);
      if (valid(found) && !(found in exits))
        exits = {@exits, found};
      endif
    endfor
    return exits;
  endverb

  verb checkexits (this none this) owner: HACKER flags: "rxd"
    "Check a list of exits to see if any of them are in the given room.";
    {to_check, room, exits} = args;
    for word in (to_check)
      found = room:match_exit(word);
      if (valid(found) && !(found in exits))
        exits = {@exits, found};
      endif
    endfor
    return exits;
  endverb

  verb "self_port_msg player_port_msg thing_port_msg join_msg" (this none this) owner: HACKER flags: "rxd"
    "This verb returns messages that go only to you. You don't need to have your name tacked on to the beginning of these. Heh.";
    msg = this.(verb);
    if (msg && length(args) >= 3)
      msg = this:msg_sub(msg, @args);
    endif
    return msg;
  endverb

  verb "oself_port_msg self_arrive_msg oplayer_port_msg player_arrive_msg victim_port_msg othing_port_msg thing_arrive_msg object_port_msg" (this none this) owner: HACKER flags: "rxd"
    "This verb returns messages that go to other players. It does pronoun substitutions; if your name is not included in the final string, it adds the name in front.";
    msg = this.(verb);
    if (!msg)
      msg = $frand_class.(verb);
    endif
    if (length(args) >= 3)
      msg = this:msg_sub(msg, @args);
    endif
    if (!$string_utils:index_delimited(msg, player.name))
      msg = player.name + " " + msg;
    endif
    return msg;
  endverb

  verb msg_sub (this none this) owner: HACKER flags: "rxd"
    "Do pronoun and other substitutions on the teleport messages. The arguments are: 1. The original message, before any substitutions; 2. object being teleported; 3. from location; 4. to location. The return value is the final message.";
    {msg, thing, from, to} = args;
    msg = $string_utils:substitute(msg, $string_utils:pronoun_quote({{"%<from room>", valid(from) ? from.name | "Nowhere"}, {"%<to room>", valid(to) ? to.name | "Nowhere"}}));
    msg = $string_utils:pronoun_sub(msg, thing);
    return msg;
  endverb

  verb obvious_exits (this none this) owner: HACKER flags: "rxd"
    "'obvious_exits()' - Return a list of common exit names which are obviously worth looking for in a room.";
    return {"n", "ne", "e", "se", "s", "sw", "w", "nw", "north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest", "u", "d", "up", "down", "out", "exit", "leave", "enter"};
  endverb

  verb tell_ways (this none this) owner: HACKER flags: "rxd"
    ":tell_ways (<list of exits>)' - Tell yourself a list of exits, for @ways. You can override it to print the exits in any format.";
    exits = args[1];
    answer = {};
    for e in (exits)
      answer = {@answer, e.name + " (" + $string_utils:english_list(e.aliases) + ")"};
    endfor
    player:tell("Obvious exits: ", $string_utils:english_list(answer), ".");
  endverb

  verb tell_obj (this none this) owner: HACKER flags: "rxd"
    "Return the name and number of an object, e.g. 'Root Class (#1)'.";
    o = args[1];
    return (valid(o) ? o.name | "Nothing") + " (" + tostr(o) + ")";
  endverb

  verb parse_out_object (this none this) owner: HACKER flags: "rxd"
    "'parse_out_object (<string>)' -> {<name>, <object>}, or 0. Given a string, attempt to find an object at its beginning or its end. An object can be either an object number, or 'here'. If this succeeds, return a list of the object and the unmatched part of the string, called the name. If it fails, return 0.";
    words = $string_utils:words(args[1]);
    if (!length(words))
      return 0;
    endif
    word1 = words[1];
    wordN = words[$];
    if (length(word1) && word1[1] == "#")
      start = 2;
      finish = length(words);
      what = toobj(word1);
    elseif (word1 == "here")
      start = 2;
      finish = length(words);
      what = this.location;
    elseif (length(wordN) && wordN[1] == "#")
      start = 1;
      finish = length(words) - 1;
      what = toobj(wordN);
    elseif (wordN == "here")
      start = 1;
      finish = length(words) - 1;
      what = this.location;
    else
      return 0;
    endif
    "toobj() has the nasty property that invalid strings get turned into #0. Here we just pretend that all references to #0 are actually meant for #-1.";
    if (what == #0)
      what = $nothing;
    endif
    name = $string_utils:from_list(words[start..finish], " ");
    if (!name)
      name = valid(what) ? what.name | "Nowhere";
    endif
    return {name, what};
  endverb

  verb enlist (this none this) owner: HACKER flags: "rxd"
    "'enlist (<x>)' - If x is a list, just return it; otherwise, return {x}. The purpose here is to turn message strings into lists, so that lines can be added. It is not guaranteed to work for non-string non-lists.";
    x = args[1];
    if (!x)
      return {};
    elseif (typeof(x) == LIST)
      return x;
    else
      return {x};
    endif
  endverb

  verb "@spellm*essages @spellp*roperties" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@spellproperties <object>";
    "@spellmessages <object>";
    "Spell checks the string properties of an object, or the subset of said properties which are suffixed _msg, respectively.";
    set_task_perms(player);
    if (!dobjstr)
      player:notify(tostr("Usage: ", verb, " <object>"));
      return;
    elseif ($command_utils:object_match_failed(dobj = player:my_match_object(dobjstr), dobjstr))
      return;
    elseif (typeof(props = $object_utils:all_properties(dobj)) == ERR)
      player:notify("Permission denied to read properties on that object.");
      return;
    endif
    props = setremove(props, "messages");
    if (verb[1..7] == "@spellm")
      spell = {};
      for prop in (props)
        if (index(prop, "_msg") == length(prop) - 3 && index(prop, "_msg"))
          spell = {@spell, prop};
        endif
      endfor
      props = spell;
    endif
    if (props == {})
      player:notify(tostr("No ", verb[1..7] == "@spellm" ? "messages" | "properties", " found to spellcheck on ", dobj, "."));
      return;
    endif
    for data in (props)
      if (typeof(dd = `dobj.(data) ! ANY') == LIST)
        text = {};
        for linenum in (dd)
          text = listappend(text, linenum);
        endfor
      elseif (typeof(dd) == OBJ || typeof(dd) == INT || typeof(dd) == ERR || typeof(dd) == FLOAT)
        text = "";
      elseif (typeof(dd) == STR)
        text = dd;
      endif
      if (typeof(text) == STR)
        text = {text};
      endif
      linenumber = 0;
      for thisline in (text)
        $command_utils:suspend_if_needed(0);
        linenumber = linenumber + 1;
        if (typeof(thisline) != LIST && typeof(thisline) != OBJ && typeof(thisline) != INT && typeof(thisline) != FLOAT && typeof(thisline) != ERR)
          i = $string_utils:strip_chars(thisline, "!@#$%^&*()_+1234567890={}[]<>?:;,./|\"~'");
          if (i)
            i = $string_utils:words(i);
            for ii in [1..length(i)]
              $command_utils:suspend_if_needed(0);
              if (!$spell:valid(i[ii]))
                if (rindex(i[ii], "s") == length(i[ii]) && $spell:valid((i[ii])[1..$ - 1]))
                  msg = "Possible match: " + i[ii];
                elseif (rindex(i[ii], "'s") == length(i[ii]) - 1 && $spell:valid((i[ii])[1..$ - 2]))
                  msg = "Possible match: " + i[ii];
                else
                  msg = "Unknown word: " + i[ii];
                endif
                if (length(text) == 1)
                  foo = ": ";
                else
                  foo = " (line " + tostr(linenumber) + "): ";
                endif
                player:notify(tostr(dobj, ".", data, foo, msg));
              endif
            endfor
          endif
        endif
      endfor
    endfor
    player:notify(tostr("Done spellchecking ", dobj, "."));
  endverb

  verb "@at" (any any any) owner: HACKER flags: "rxd"
    "'@at' - Find out where everyone is. '@at <player>' - Find out where <player> is, and who else is there. '@at <obj>' - Find out who else is at the same place as <obj>. '@at <place>' - Find out who is at the place. The place can be given by number, or it can be a name from your @rooms list. '@at #-1' - Find out who is at #-1. '@at me' - Find out who is in the room with you. '@at home' - Find out who is at your home.";
    this:internal_at(argstr);
  endverb

  verb at_players (this none this) owner: HACKER flags: "rxd"
    "'at_players ()' - Return a list of players to be displayed by @at.";
    return connected_players();
  endverb

  verb do_at_all (this none this) owner: HACKER flags: "rxd"
    "'do_at_all ()' - List where everyone is, sorted by popularity of location. This is called when you type '@at'.";
    locations = {};
    parties = {};
    counts = {};
    for who in (this:at_players())
      loc = who.location;
      if (i = loc in locations)
        parties[i] = setadd(parties[i], who);
        counts[i] = counts[i] - 1;
      else
        locations = {@locations, loc};
        parties = {@parties, {who}};
        counts = {@counts, 0};
      endif
    endfor
    locations = $list_utils:sort(locations, counts);
    parties = $list_utils:sort(parties, counts);
    this:print_at_items(locations, parties);
  endverb

  verb do_at (this none this) owner: HACKER flags: "rxd"
    "'do_at (<location>)' - List the players at a given location.";
    loc = args[1];
    party = {};
    for who in (this:at_players())
      if (who.location == loc)
        party = setadd(party, who);
      endif
    endfor
    this:print_at_items({loc}, {party});
  endverb

  verb print_at_items (this none this) owner: HACKER flags: "rxd"
    "'print_at_items (<locations>, <parties>)' - Print a list of locations and people, for @at. Override this if you want to make a change to @at's output that you can't make in :at_item.";
    {locations, parties} = args;
    for i in [1..length(locations)]
      $command_utils:suspend_if_needed(0);
      player:tell_lines(this:at_item(locations[i], parties[i]));
    endfor
  endverb

  verb at_item (this none this) owner: HACKER flags: "rxd"
    "'at_item (<location>, <party>)' - Given a location and a list of the people there, return a string displaying the information. Override this if you want to change the format of each line of @at's output.";
    {loc, party} = args;
    su = $string_utils;
    if (this.at_number)
      number = su:right(tostr(loc), 7) + " ";
    else
      number = "";
    endif
    room = su:left(valid(loc) ? loc.name | "[Nowhere]", this.at_room_width);
    if (length(room) > this.at_room_width)
      room = room[1..this.at_room_width];
    endif
    text = number + room + " ";
    if (party)
      filler = su:space(length(text) - 2);
      line = text;
      text = {};
      for who in (party)
        name = " " + (valid(who) ? who.name | "[Nobody]");
        if (length(line) + length(name) > this:linelen())
          text = {@text, line};
          line = filler + name;
        else
          line = line + name;
        endif
      endfor
      text = {@text, line};
    else
      text = text + " [deserted]";
    endif
    return text;
  endverb

  verb internal_at (this none this) owner: HACKER flags: "rxd"
    "'internal_at (<argument string>)' - Perform the function of @at. The argument string is whatever the user typed after @at. This is factored out so that other verbs can call it.";
    where = $string_utils:trim(args[1]);
    if (where)
      if (where[1] == "#")
        result = toobj(where);
        if (!valid(result) && result != #-1)
          player:tell("That object does not exist.");
          return;
        endif
      else
        result = this:lookup_room(where);
        if (!valid(result))
          result = $string_utils:match_player(where);
          if (!valid(result))
            player:tell("That is neither a player nor a room name.");
            return;
          endif
        endif
      endif
      if (valid(result) && !$object_utils:isa(result, $room))
        result = result.location;
      endif
      this:do_at(result);
    else
      this:do_at_all();
    endif
  endverb

  verb confunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "'confunc ()' - Besides the inherited behavior, notify the player's feature objects that the player has connected.";
    if (valid(cp = caller_perms()) && caller != this && !$perm_utils:controls(cp, this))
      return E_PERM;
    endif
    pass(@args);
    set_task_perms(this);
    for feature in (this.features)
      try
        feature:player_connected(player, @args);
      except (E_VERBNF)
        continue feature;
      except id (ANY)
        player:tell("Feature initialization failure for ", feature, ": ", id[2], ".");
      endtry
      $command_utils:suspend_if_needed(0);
    endfor
  endverb

  verb disfunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "'disfunc ()' - Besides the inherited behavior, notify the player's feature objects that the player has disconnected.";
    if (valid(cp = caller_perms()) && caller != this && !$perm_utils:controls(cp, this))
      return E_PERM;
    endif
    pass(@args);
    "This is forked off to protect :disfunc from buggy :player_disconnected verbs.";
    set_task_perms(this);
    fork (max(0, $login:current_lag()))
      for feature in (this.features)
        try
          feature:player_disconnected(player, @args);
        except (ANY)
          continue feature;
        endtry
      endfor
    endfork
  endverb

  verb "@addword @adddict" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (verb == "@adddict" && !(player in $spell.trusted || player.wizard))
      player:tell("You may not add to the master dictionary. The following words will instead by put in a list of words to be approved for later addition to the dictionary. Thanks for your contribution.");
    endif
    if (!argstr)
      player:notify(tostr("Usage: ", verb, " one or more words"));
      player:notify(tostr("       ", verb, " object:verb"));
      player:notify(tostr("       ", verb, " object.prop"));
    elseif (!$perm_utils:controls(player, player))
      player:notify("Cannot modify dictionary on players who do not own themselves.");
    elseif (data = $spell:get_input(argstr))
      num_learned = 0;
      for i in [1..length(data)]
        line = $string_utils:words(data[i]);
        for ii in [1..length(line)]
          if (seconds_left() < 2)
            suspend(0);
          endif
          if (verb == "@adddict")
            result = $spell:add_word(line[ii]);
            if (result == E_PERM)
              if ($spell:find_exact(line[ii]) == $failed_match)
                player:notify(tostr("Submitted for approval:  ", line[ii]));
                $spell:submit(line[ii]);
              else
                player:notify(tostr("Already in dictionary:  " + line[ii]));
              endif
            elseif (typeof(result) == ERR)
              player:notify(tostr(result));
            elseif (result)
              player:notify(tostr("Word added:  ", line[ii]));
              num_learned = num_learned + 1;
            else
              player:notify(tostr("Already in dictionary:  " + line[ii]));
            endif
          elseif (!$spell:valid(line[ii]))
            player.dict = listappend(player.dict, line[ii]);
            player:notify(tostr("Word added:  ", line[ii]));
            num_learned = num_learned + 1;
          endif
        endfor
      endfor
      player:notify(tostr(num_learned ? num_learned | "No", " word", num_learned != 1 ? "s " | " ", "added to ", verb == "@adddict" ? "main " | "personal ", "dictionary."));
    endif
  endverb

  verb "@spell @cspell @complete" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "@spell a word or phrase  -- Spell check a word or phrase.";
    "@spell thing.prop  -- Spell check a property. The value must be a string or a list of strings.";
    "@spell thing:verb  -- Spell check a verb. Only the quoted strings in the verb are checked.";
    "@cspell word  -- Spell check a word, and if it is not in the dictionary, offset suggestions about what the right spelling might be. This actually works with thing.prop and thing:verb too, but it is too slow to be useful--it takes maybe 30 seconds per unknown word.";
    "@complete prefix  -- List all the word in the dictionary which begin with the given prefix. For example, `@complete zoo' lists zoo, zoologist, zoology, and zoom.";
    "";
    "Mr. Spell was written by waffle (waffle@euclid.humboldt.edu), for use by";
    "MOOers all over this big green earth. (....and other places....)";
    "This monstrosity programmed Sept-Oct 1991, when I should have been studying.";
    set_task_perms(player);
    if (!argstr)
      if (verb == "@complete")
        player:notify(tostr("Usage: ", verb, " word-prefix"));
      else
        player:notify(tostr("Usage: ", verb, " object.property"));
        player:notify(tostr("       ", verb, " object:verb"));
        player:notify(tostr("       ", verb, " one or more words"));
      endif
    elseif (verb == "@complete")
      if ((foo = $string_utils:from_list($spell:sort($spell:find_all(argstr)), " ")) == "")
        player:notify(tostr("No words found that begin with `", argstr, "'"));
      else
        player:notify(tostr(foo));
      endif
    else
      "@spell or @cspell.";
      corrected_words = {};
      data = $spell:get_input(argstr);
      if (data)
        misspelling = 0;
        for i in [1..length(data)]
          line = $string_utils:words(data[i]);
          for ii in [1..length(line)]
            $command_utils:suspend_if_needed(0);
            if (!$spell:valid(line[ii]))
              if (rindex(line[ii], "s") == length(line[ii]) && $spell:valid((line[ii])[1..$ - 1]))
                msg = "Possible match: " + line[ii];
                msg = msg + " " + (length(data) != 1 ? "(line " + tostr(i) + ")  " | "  ");
              elseif (rindex(line[ii], "'s") == length(line[ii]) - 1 && $spell:valid((line[ii])[1..$ - 2]))
                msg = "Possible match: " + line[ii];
                msg = msg + " " + (length(data) != 1 ? "(line " + tostr(i) + ")  " | "  ");
              else
                misspelling = misspelling + 1;
                msg = "Unknown word: " + line[ii] + (length(data) != 1 ? " (line " + tostr(i) + ")  " | "  ");
                if (verb == "@cspell" && !(line[ii] in corrected_words))
                  corrected_words = listappend(corrected_words, line[ii]);
                  guesses = $string_utils:from_list($spell:guess_words(line[ii]), " ");
                  if (guesses == "")
                    msg = msg + "-No guesses";
                  else
                    msg = msg + "-Possible correct spelling";
                    msg = msg + (index(guesses, " ") ? "s: " | ": ");
                    msg = msg + guesses;
                  endif
                endif
              endif
              player:notify(tostr(msg));
            endif
          endfor
        endfor
        player:notify(tostr("Found ", misspelling ? misspelling | "no", " misspelled word", misspelling == 1 ? "." | "s."));
      elseif (data != $failed_match)
        player:notify(tostr("Nothing found to spellcheck!"));
      endif
    endif
  endverb

  verb "@rmword" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    if (argstr in player.dict)
      player.dict = setremove(player.dict, argstr);
      player:notify(tostr("`", argstr, "' removed from personal dictionary."));
    else
      player:notify(tostr("`", argstr, "' not found in personal dictionary."));
    endif
  endverb

  verb "@rmdict" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    set_task_perms(player);
    result = $spell:remove_word(argstr);
    if (result == E_PERM)
      player:notify("You may not remove words from the main dictionary. Use `@rmword' to remove words from your personal dictionary.");
    elseif (typeof(result) == ERR)
      player:notify(tostr(result));
    elseif (result)
      player:notify(tostr("`", argstr, "' removed."));
    else
      player:notify(tostr("`", argstr, "' not found in dictionary."));
    endif
  endverb

  verb find_property (this none this) owner: HACKER flags: "rxd"
    "'find_property (<name>)' - Search for a property with the given name. The objects searched are those returned by this:find_properties_on(). The printing order relies on $list_utils:remove_duplicates to leave the *first* copy of each duplicated element in a list; for example, {1, 2, 1} -> {1, 2}, not to {2, 1}.";
    name = args[1];
    results = "";
    objects = $list_utils:remove_duplicates(this:find_properties_on());
    for thing in (objects)
      if (valid(thing) && (mom = $object_utils:has_property(thing, name)))
        results = results + "   " + thing.name + "(" + tostr(thing) + ")";
        mom = this:property_inherited_from(thing, name);
        if (thing != mom)
          if (valid(mom))
            results = results + "--" + mom.name + "(" + tostr(mom) + ")";
          else
            results = results + "--built-in";
          endif
        endif
      endif
    endfor
    if (results)
      this:tell("The property .", name, " is on", results);
    else
      this:tell("The property .", name, " is nowhere to be found.");
    endif
  endverb

  verb find_verbs_on (this none this) owner: HACKER flags: "rxd"
    "'find_verbs_on ()' -> list of objects - Return the objects that @find searches when looking for a verb. The objects are searched (and the results printed) in the order returned. Feature objects are included in the search. Duplicate entries are removed by the caller.";
    return {this, this.location, @valid(this.location) ? this.location:contents() | {}, @this:contents(), @this.features};
  endverb

  verb find_properties_on (this none this) owner: HACKER flags: "rxd"
    "'find_properties_on ()' -> list of objects - Return the objects that @find searches when looking for a property. The objects are searched (and the results printed) in the order returned. Feature objects are *not* included in the search. Duplicate entries are removed by the caller.";
    return {this, this.location, @valid(this.location) ? this.location:contents() | {}, @this:contents()};
  endverb

  verb property_inherited_from (this none this) owner: HACKER flags: "rxd"
    "'property_inherited_from (<object>, <property name>)' -> object - Return the ancestor of <object> on which <object>.<property> is originally defined. If <object>.<property> is not actually defined, return 0. The property is taken as originally defined on the earliest ancestor of <object> which has it. If the property is built-in, return $nothing.";
    {what, prop} = args;
    if (!$object_utils:has_property(what, prop))
      return 0;
    elseif (prop in $code_utils.builtin_props)
      return $nothing;
    endif
    ancestor = what;
    while ($object_utils:has_property(parent(ancestor), prop))
      ancestor = parent(ancestor);
    endwhile
    return ancestor;
  endverb

  verb "@ref*use" (any any any) owner: HACKER flags: "rd"
    "'@refuse <action(s)> [ from <player> ] [ for <time> ]' - Refuse all of a list of one or more actions. If a player is given, refuse actions from the player; otherwise, refuse all actions. If a time is specified, refuse the actions for the given amount of time; otherwise, refuse them for a week. If the actions are already refused, then the only their times are adjusted.";
    if (!argstr)
      player:tell("@refuse <action(s)> [ from <player> ] [ for <time> ]");
      return;
    endif
    stuff = this:parse_refuse_arguments(argstr);
    if (stuff)
      if (typeof(who = stuff[1]) == OBJ && who != $nothing && !is_player(who))
        player:tell("You must give the name of some player.");
      else
        "'stuff' is now in the form {<origin>, <actions>, <duration>}.";
        if (stuff[3] < 0 || stuff[3] > $maxint - time() - 2)
          stuff[3] = $maxint - time() - 2;
          player:tell("That amount of time is too large.  It has been capped at ", $time_utils:english_time(stuff[3]), ".");
        endif
        this:add_refusal(@stuff);
        player:tell("Refusal of ", this:refusal_origin_to_name(stuff[1]), " for ", $time_utils:english_time(stuff[3]), " added.");
      endif
    endif
  endverb

  verb "@unref*use @allow" (any any any) owner: HACKER flags: "rd"
    "'@unrefuse <action(s)> [ from <player> ]' - Stop refusing all of a list of actions. If a player is given, stop refusing actions by the player; otherwise, stop refusing all actions of the given kinds. '@unrefuse everything' - Remove all refusals.";
    if (argstr == "everything")
      if ($command_utils:yes_or_no("Do you really want to erase all your refusals?"))
        this:clear_refusals();
        player:tell("OK, they are gone.");
      else
        player:tell("OK, no harm done.");
      endif
      return;
    endif
    stuff = this:parse_refuse_arguments(argstr);
    if (!stuff)
      return;
    endif
    "'stuff' is now in the form {<origin>, <actions>, <duration>}.";
    origins = stuff[1];
    actions = stuff[2];
    if (typeof(origins) != LIST)
      origins = {origins};
    endif
    n = 0;
    for origin in (origins)
      n = n + this:remove_refusal(origin, actions);
    endfor
    plural = n == 1 && length(origins) == 1 ? "" | "s";
    if (n)
      player:tell("Refusal", plural, " removed.");
    else
      player:tell("You have no such refusal", plural, ".");
    endif
  endverb

  verb "@refusals" (none any any) owner: HACKER flags: "rd"
    "'@refusals' - List your refusals. '@refusals for <player>' - List the given player's refusals.";
    if (iobjstr)
      who = $string_utils:match_player(iobjstr);
      if ($command_utils:player_match_failed(who, iobjstr))
        return;
      endif
      if (!$object_utils:has_verb(who, "refusals_text"))
        player:tell("That player does not have the refusal facility.");
        return;
      endif
    else
      who = player;
    endif
    who:remove_expired_refusals();
    player:tell_lines(this:refusals_text(who));
  endverb

  verb "@refusal-r*eporting" (any any any) owner: HACKER flags: "rd"
    "'@refusal-reporting' - See if refusal reporting is on. '@refusal-reporting on', '@refusal-reporting off' - Turn it on or off..";
    if (!argstr)
      player:tell("Refusal reporting is ", this.report_refusal ? "on" | "off", ".");
    elseif (argstr in {"on", "yes", "y", "1"})
      this.report_refusal = 1;
      player:tell("Refusals will be reported to you as they happen.");
    elseif (argstr in {"off", "no", "n", "0"})
      this.report_refusal = 0;
      player:tell("Refusals will happen silently.");
    else
      player:tell("@refusal-reporting on     - turn on refusal reporting");
      player:tell("@refusal-reporting off    - turn it off");
      player:tell("@refusal-reporting        - see if it's on or off");
    endif
  endverb

  verb parse_refuse_arguments (this none this) owner: HACKER flags: "rxd"
    "'parse_refuse_arguments (<string>)' -> {<who>, <actions>, <duration>} - Parse the arguments of a @refuse or @unrefuse command. <who> is the player requested, or $nothing if none was. <actions> is a list of the actions asked for. <duration> is how long the refusal should last, or 0 if no expiration is given. <errors> is a list of actions (or other words) which are wrong. If there are any errors, this prints an error message and returns 0.";
    words = $string_utils:explode(args[1]);
    possible_actions = this:refusable_actions();
    who = $nothing;
    actions = {};
    until = this.default_refusal_time;
    errors = {};
    skip_to = 0;
    for i in [1..length(words)]
      word = words[i];
      if (i <= skip_to)
      elseif (which = $string_utils:find_prefix(word, possible_actions))
        actions = setadd(actions, possible_actions[which]);
      elseif (word[$] == "s" && (which = $string_utils:find_prefix(word[1..$ - 1], possible_actions)))
        "The word seems to be the plural of an action.";
        actions = setadd(actions, possible_actions[which]);
      elseif (results = this:translate_refusal_synonym(word))
        actions = $set_utils:union(actions, results);
      elseif (word == "from" && i < length(words))
        "Modified to allow refusals from all guests at once. 5-27-94, Gelfin";
        if (words[i + 1] == "guests")
          who = "all guests";
        elseif (!(typeof(who = $code_utils:toobj(words[i + 1])) == OBJ))
          who = $string_utils:match_player(words[i + 1]);
          if ($command_utils:player_match_failed(who, words[i + 1]))
            return 0;
          endif
        endif
        skip_to = i + 1;
      elseif (word == "for" && i < length(words))
        n_words = this:parse_time_length(words[i + 1..$]);
        until = this:parse_time(words[i + 1..i + n_words]);
        if (!until)
          return 0;
        endif
        skip_to = i + n_words;
      else
        errors = {@errors, word};
      endif
    endfor
    if (errors)
      player:tell(length(errors) > 1 ? "These parts of the command were not understood: " | "This part of the command was not understood: ", $string_utils:english_list(errors, 0, " ", " ", " "));
      return 0;
    endif
    return {this:player_to_refusal_origin(who), actions, until};
  endverb

  verb time_word_to_seconds (this none this) owner: HACKER flags: "rxd"
    "'time_word_to_seconds (<string>)' - The <string> is expected to be a time word, 'second', 'minute', 'hour', 'day', 'week', or 'month'. Return the number of seconds in that amount of time (a month is taken to be 30 days). If <string> is not a time word, return 0. This is used both as a test of whether a word is a time word and as a converter.";
    return $time_utils:parse_english_time_interval("1", args[1]);
  endverb

  verb parse_time_length (this none this) owner: HACKER flags: "rxd"
    "'parse_time_length (<words>)' -> n - Given a list of words which is expected to begin with a time expression, return how many of them belong to the time expression. A time expression can be a positive integer, a time word, or a positive integer followed by a time word. A time word is anything that this:time_word_to_seconds this is one. The return value is 0, 1, or 2.";
    words = {@args[1], "dummy"};
    n = 0;
    if (toint(words[1]) || this:time_word_to_seconds(words[1]))
      n = 1;
    endif
    if (this:time_word_to_seconds(words[n + 1]))
      n = n + 1;
    endif
    return n;
  endverb

  verb parse_time (this none this) owner: HACKER flags: "rxd"
    "'parse_time (<words>)' -> <seconds> - Given a list of zero or more words, either empty or a valid time expression, return the number of seconds that the time expression refers to. This is a duration, not an absolute time.";
    words = args[1];
    "If the list is empty, return the default refusal time.";
    if (!words)
      return this.default_refusal_time;
    endif
    "If the list has one word, either <units> or <n>.";
    "If it is a unit, like 'hour', return the time for 1 <unit>.";
    "If it is a number, return the time for <n> days.";
    if (length(words) == 1)
      return this:time_word_to_seconds(words[1]) || toint(words[1]) * this:time_word_to_seconds("days");
    endif
    "The list must contain two words, <n> <units>.";
    return toint(words[1]) * this:time_word_to_seconds(words[2]);
  endverb

  verb clear_refusals (this none this) owner: HACKER flags: "rxd"
    "'clear_refusals ()' - Erase all of this player's refusals.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    this.refused_origins = {};
    this.refused_actions = {};
    this.refused_until = {};
    this.refused_extra = {};
  endverb

  verb set_default_refusal_time (this none this) owner: HACKER flags: "rxd"
    "'set_default_refusal_time (<seconds>)' - Set the length of time that a refusal lasts if its duration isn't specified.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    endif
    this.default_refusal_time = toint(args[1]);
  endverb

  verb refusable_actions (this none this) owner: HACKER flags: "rxd"
    "'refusable_actions ()' -> {'page', 'whisper', ...} - Return a list of the actions that can be refused. This is a verb, rather than a property, so that it can be inherited properly. If you override this verb to add new refusable actions, write something like 'return {@pass (), 'action1', 'action2', ...}'. That way people can add new refusable actions at any level of the player class hierarchy, without clobbering any that were added higher up.";
    return {"page", "whisper", "move", "join", "accept", "mail"};
  endverb

  verb translate_refusal_synonym (this none this) owner: HACKER flags: "rxd"
    "'translate_refusal_synonym (<word>)' -> list - If the <word> is a synonym for some set of refusals, return the list of those refusals. Otherwise return the empty list, {}. Programmers can override this verb to provide more synonyms.";
    word = args[1];
    if (word == "all")
      return this:refusable_actions();
    endif
    return {};
  endverb

  verb default_refusals_text_filter (this none this) owner: HACKER flags: "rxd"
    "'default_refusals_text_filter (<origin>, <actions>)' - Return any actions by this <origin> which should be included in the text returned by :refusals_text. This is the default filter, which includes all actions.";
    return args[2];
  endverb

  verb refusals_text (this none this) owner: HACKER flags: "rxd"
    "'refusals_text (<player>, [<filter verb name>])' - Return text describing the given player's refusals. The filter verb name is optional; if it is given, this verb takes an origin and a list of actions and returns any actions which should be included in the refusals text. This verb works only if <player> is a player who has the refusals facility; it does not check for this itself.";
    who = args[1];
    "Used to allow you to supply the filter verb name, but that introduced a security hole. --Nosredna";
    filter_verb = "default_refusals_text_filter";
    text = {};
    for i in [1..length(who.refused_origins)]
      origin = who.refused_origins[i];
      actions = this:(filter_verb)(origin, who.refused_actions[i]);
      if (actions)
        line = "";
        for action in (actions)
          line = line + " " + action;
        endfor
        line = this:refusal_origin_to_name(origin) + ": " + line;
        line = ctime(who.refused_until[i]) + " " + line;
        text = {@text, line};
      endif
    endfor
    if (!text)
      text = {"No refusals."};
    endif
    return text;
  endverb

  verb player_to_refusal_origin (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "'player_to_refusal_origin (<player>)' -> <origin> - Convert a player to a unique identifier called the player's 'refusal origin'. For most players, it's just their object number. For guests, it is a hash of the site they are connecting from. Converting an origin to an origin is a safe no-op--the code relies on this.";
    set_task_perms(caller_perms());
    {who} = args;
    if (typeof(who) == OBJ && valid(who) && parent(who) == `$local.guest ! E_PROPNF, E_INVIND => $guest')
      return who:connection_name_hash("xx");
    else
      return who;
    endif
  endverb

  verb refusal_origin_to_name (this none this) owner: HACKER flags: "rxd"
    "'refusal_origin_to_name (<origin>)' -> string - Convert a refusal origin to a name.";
    origin = args[1];
    if (origin in {"all guests", "everybody"})
      return origin;
    elseif (typeof(origin) == STR && origin == "Permission denied")
      return "an errorful origin";
    elseif (typeof(origin) != OBJ)
      return "a certain guest";
    elseif (origin == #-1)
      return "Everybody";
    else
      return $string_utils:name_and_number(origin);
    endif
  endverb

  verb check_refusal_actions (this none this) owner: HACKER flags: "rxd"
    "'check_refusal_actions (<actions>)' - Check a list of refusal actions, and return whether they are all legal.";
    actions = args[1];
    legal_actions = this:refusable_actions();
    for action in (actions)
      if (!(action in legal_actions))
        return 0;
      endif
    endfor
    return 1;
  endverb

  verb add_refusal (this none this) owner: HACKER flags: "rxd"
    "'add_refusal (<origin>, <actions> [, <duration> [, <extra>]])' - Add refusal(s) to this player's list. <Actions> is a list of the actions to be refused. The list should contain only actions, no synonyms. <Origin> is the actor whose actions are to be refused. <Until> is the time that the actions are being refused until, in the form returned by time(). It is optional; if it's not given, it defaults to .default_refusal_time. <Extra> is any extra information; it can be used for comments, or to make finer distinctions about the actions being refused, or whatever. If it is not given, it defaults to 0. The extra information is per-action; that is, it is stored separately for each action that it applies to.";
    if (caller != this)
      return E_PERM;
    endif
    {orig, actions, ?duration = this.default_refusal_time, ?extra = 0} = args;
    origins = this:player_to_refusal_origin(orig);
    if (typeof(origins) != LIST)
      origins = {origins};
    endif
    if (typeof(actions) != LIST)
      actions = {actions};
    endif
    if (!this:check_refusal_actions(actions))
      return E_INVARG;
    endif
    until = time() + duration;
    for origin in (origins)
      if (i = origin in this.refused_origins)
        this.refused_until[i] = until;
        for action in (actions)
          if (j = action in this.refused_actions[i])
            this.refused_extra[i][j] = extra;
          else
            this.refused_actions[i] = {@this.refused_actions[i], action};
            this.refused_extra[i] = {@this.refused_extra[i], extra};
          endif
        endfor
      else
        this.refused_origins = {@this.refused_origins, origin};
        this.refused_actions = {@this.refused_actions, actions};
        this.refused_until = {@this.refused_until, until};
        this.refused_extra = {@this.refused_extra, $list_utils:make(length(actions), extra)};
      endif
    endfor
  endverb

  verb remove_refusal (this none this) owner: HACKER flags: "rxd"
    "'remove_refusal (<origin>, <actions>)' - Remove any refused <actions> by <origin>. The <actions> list should contain only actions, no synonyms. Return the number of such refusals found (0 if none).";
    if (caller != this)
      return E_PERM;
    endif
    {origin, actions} = args;
    if (typeof(actions) != LIST)
      actions = {actions};
    endif
    count = 0;
    i = origin in this.refused_origins;
    if (i)
      for action in (actions)
        if (j = action in this.refused_actions[i])
          this.refused_actions[i] = listdelete(this.refused_actions[i], j);
          this.refused_extra[i] = listdelete(this.refused_extra[i], j);
          count = count + 1;
        endif
      endfor
      if (!(this.refused_actions[i]))
        this.refused_origins = listdelete(this.refused_origins, i);
        this.refused_actions = listdelete(this.refused_actions, i);
        this.refused_until = listdelete(this.refused_until, i);
        this.refused_extra = listdelete(this.refused_extra, i);
      endif
    endif
    return count;
  endverb

  verb remove_expired_refusals (this none this) owner: HACKER flags: "rxd"
    "'remove_expired_refusals ()' - Remove refusal entries which are past their time limits.";
    origins = {};
    "Before removing any refusals, figure out which ones to remove. Removing one changes the indices and invalidates the loop invariant.";
    for i in [1..length(this.refused_origins)]
      if (time() >= this.refused_until[i] || typeof(this.refused_origins[i]) == OBJ && !$recycler:valid(this.refused_origins[i]))
        origins = {@origins, this.refused_origins[i]};
      endif
    endfor
    for origin in (origins)
      this:remove_refusal(origin, this:refusable_actions());
    endfor
  endverb

  verb refuses_action (this none this) owner: HACKER flags: "rxd"
    "'refuses_action (<origin>, <action>, ...)' - Return whether this object refuses the given <action> by <origin>. <Origin> is typically a player. Extra arguments after <origin>, if any, are used to further describe the action.";
    "Modified by Diopter (#98842) at LambdaMOO";
    {origin, action, @extra_args} = args;
    extra_args = {origin, @extra_args};
    rorigin = this:player_to_refusal_origin(origin);
    if ((which = rorigin in this.refused_origins) && action in this.refused_actions[which] && this:(("refuses_action_" + action))(which, @extra_args))
      return 1;
    elseif (typeof(rorigin) == OBJ && valid(rorigin) && (which = rorigin.owner in this.refused_origins) && action in this.refused_actions[which] && this:(("refuses_action_" + action))(which, @extra_args))
      return 1;
    elseif ((which = $nothing in this.refused_origins) && rorigin != this && action in this.refused_actions[which] && this:(("refuses_action_" + action))(which, @extra_args))
      return 1;
    elseif ((which = "all guests" in this.refused_origins) && $object_utils:isa(origin, $guest) && action in this.refused_actions[which] && this:(("refuses_action_" + action))(which, @extra_args))
      return 1;
    endif
    return 0;
  endverb

  verb "refuses_action_*" (this none this) owner: HACKER flags: "rxd"
    "'refuses_action_* (<which>, <origin>, ...)' - The action (such as 'whisper' for the verb :refuses_action_whisper) is being considered for refusal. Return whether the action should really be refused. <Which> is an index into this.refused_origins. By default, always refuse non-outdated actions that get this far.";
    {which, @junk} = args;
    if (time() >= this.refused_until[which])
      fork (0)
        "This <origin> is no longer refused. Remove any outdated refusals.";
        this:remove_expired_refusals();
      endfork
      return 0;
    else
      return 1;
    endif
  endverb

  verb report_refusal (this none this) owner: HACKER flags: "rxd"
    "'report_refusal (<player>, <message>, ...)' - If refusal reporting is turned on, print the given <message> to report the refusal of some action by <player>. The message may take more than one argument. You can override this verb to do more selective reporting.";
    if (this.report_refusal)
      this:tell(@listdelete(args, 1));
    endif
  endverb

  verb "wh*isper" (any at this) owner: HACKER flags: "rxd"
    "'whisper <message> to <this player>' - Whisper a message to this player which nobody else can see.";
    if (this:refuses_action(player, "whisper"))
      player:tell(this:whisper_refused_msg());
      this:report_refusal(player, "You just refused a whisper from ", player.name, ".");
    else
      pass(@args);
    endif
  endverb

  verb receive_page (this none this) owner: HACKER flags: "rxd"
    "'receive_page (<message>)' - Receive a page. If the page is accepted, pass(@args) shows it to the player.";
    if (this:refuses_action(player, "page"))
      this.page_refused = task_id();
      return 0;
    endif
    this.page_refused = 0;
    return pass(@args);
  endverb

  verb page_echo_msg (this none this) owner: HACKER flags: "rxd"
    "'page_echo_msg ()' - Return a message to inform the pager what happened to their page.";
    if (task_id() == this.page_refused)
      this:report_refusal(player, "You just refused a page from ", player.name, ".");
      return this:page_refused_msg();
    else
      return pass(@args);
    endif
  endverb

  verb "moveto acceptable" (this none this) owner: HACKER flags: "rxd"
    "'moveto (<destination>)', 'accept (<object>)' - Check whether this :moveto or :accept is allowed or refused. If it is allowed, do it. This code is slightly modified from an original verb by Grump.  Upgraded by Bits to account for forthcoming 1.8.0 behavior of callers().";
    by = callers();
    "Ignore all the verbs on this.";
    while ((y = by[1])[1] == this && y[2] == verb)
      by = listdelete(by, 1);
    endwhile
    act = verb == "moveto" ? "move" | "accept";
    if (player != this && this:refuses_action(player, act, args[1]))
      "check player";
      return 0;
    endif
    last = #-1;
    for k in (by)
      if ((perms = k[3]) == #-1 && k[2] != "" && k[1] == #-1)
      elseif (!perms.wizard && perms != this)
        if (perms != last)
          "check for possible malicious programmer";
          if (this:refuses_action(perms, act, args[1]))
            return 0;
          endif
          last = perms;
        endif
      endif
    endfor
    "Coded added 11/8/98 by TheCat, to refuse spurned objects.";
    if (act == "accept" && typeof(this.spurned_objects) == LIST)
      for item in (this.spurned_objects)
        if ($object_utils:isa(args[1], item))
          return 0;
        endif
      endfor
    endif
    "(end of code added by TheCat)";
    return pass(@args);
  endverb

  verb receive_message (this none this) owner: HACKER flags: "rxd"
    "'receive_message (<message>, <sender>)' - Receive the given mail message from the given sender. This version handles refusal of the message.";
    if (!$perm_utils:controls(caller_perms(), this) && caller != this)
      return E_PERM;
    elseif (this:refuses_action(args[2], "mail"))
      return this:mail_refused_msg();
    else
      return pass(@args);
    endif
  endverb

  verb "whisper_refused_msg page_refused_msg mail_refused_msg" (this none this) owner: HACKER flags: "rxd"
    "'whisper_refused_msg()', 'page_refused_msg()', etc. - Return a message string.";
    return $string_utils:pronoun_sub(this.(verb), this);
  endverb

  verb last_huh (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    if (pass(@args))
      return 1;
    endif
    {verb, args} = args;
    if (valid(dobj = $string_utils:literal_object(dobjstr)) && (r = $match_utils:match_verb(verb, dobj, args)))
      return r;
    elseif (valid(iobj = $string_utils:literal_object(iobjstr)) && (r = $match_utils:match_verb(verb, iobj, args)))
      return r;
    else
      return 0;
    endif
  endverb

  verb ping_features (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":ping_features()";
    " -- cleans up the .features list to remove !valid objects";
    " ==> cleaned-up .features list";
    features = this.features;
    for x in (features)
      if (!$recycler:valid(x))
        features = setremove(features, x);
      endif
    endfor
    return this.features = features;
  endverb

  verb set_owned_objects (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_owned_objects( LIST owned-objects list )";
    "  -- set your .owned_objects, ordered as you please";
    "  -- no, it will NOT let you set to to anything you want";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      new = args[1];
      old = this.owned_objects;
      "make sure they're the same";
      if (length(new) != length(old))
        return E_INVARG;
      endif
      for i in (new)
        old = setremove(old, i);
      endfor
      if (old)
        "something's funky";
        return E_INVARG;
      endif
      return this.owned_objects = new;
    else
      return E_PERM;
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      if ($code_utils:verb_location() == this)
        this.rooms = {};
      else
        clear_property(this, "rooms");
      endif
      this.features = {$pasting_feature, $stage_talk};
    endif
  endverb

  verb find_help (this none this) owner: HACKER flags: "rxd"
    "'find_help (<name>[, databases])'";
    "Search for a help topic with the given name. [<databases>] defaults to the ones returned by $code_utils:help_db_list().";
    {name, ?databases = $code_utils:help_db_list()} = args;
    if (!name)
      this:tell("What topic do you want to search for?");
    elseif (result = $code_utils:help_db_search(name, databases))
      {object, realname} = result;
      if (object == $ambiguous_match)
        this:tell("The help topic \"", name, "\" could refer to any of the following:  ", $string_utils:english_list(realname));
      elseif (object == $help && !$object_utils:has_property(object, realname) && valid(o = $string_utils:match_object(name, player.location)))
        if ($object_utils:has_callable_verb(o, "help_msg"))
          this:tell("That help topic was returned by ", $string_utils:nn(o), ":help_msg().");
        elseif ($object_utils:has_property(o, "help_msg"))
          this:tell("That help topic is located in ", $string_utils:nn(o), ".help_msg.");
        else
          this:tell("That help topic was matched by $help but there doesn't seem to be any help available for it.");
        endif
      elseif (object == $verb_help)
        if ((what = $code_utils:parse_verbref(realname)) && valid(what[1] = $string_utils:match_object(what[1], player.location)) && $object_utils:has_verb(@what))
          this:tell("That help topic is located at the beginning of the verb ", $string_utils:nn(what[1]), ":", what[2], ".");
        else
          this:tell("That help topic was matched by $verb_help but there doesn't seem to be any help available for it.");
        endif
      else
        where = {};
        for x in (databases)
          if ({realname} == x:find_topics(realname))
            where = setadd(where, x);
          endif
        endfor
        asname = name == realname ? "" | " as \"" + realname + "\"";
        if (where)
          this:tell("That help topic is located on ", $string_utils:nn(where), asname, ".");
        else
          "...this shouldn't happen unless $code_utils:help_db_search finds a match we weren't expecting";
          this:tell("That help topic appears to be located on ", $string_utils:nn(object), asname, ", although this command could not find it.");
        endif
      endif
    else
      this:tell("The help topic \"", name, "\" could not be found.");
    endif
  endverb

  verb "@spurn" (any none none) owner: HACKER flags: "rd"
    "Prevent an object or any of its descendents from going into your inventory, regardless of whose player perms sent it there.";
    "Syntax:  @spurn <object>";
    "         @spurn !<object>";
    "The second form removes an object from your list of spurned objects.";
    "Verb created by TheCat, 11/8/98";
    if (caller != this)
      return E_PERM;
    endif
    if (!argstr)
      this:tell("Spurn what?");
    elseif (argstr[1] == "!")
      "Stop spurning something.";
      item = this:my_match_object(argstr[2..$]);
      if (item in this.spurned_objects)
        this.spurned_objects = $list_utils:setremove_all(this.spurned_objects, item);
        this:tell("You are no longer spurning " + $string_utils:nn(item) + " or any kids of it.");
      else
        this:tell("You are not spurning " + $string_utils:nn(item) + ".");
      endif
    else
      "Spurn something.";
      item = this:my_match_object(argstr);
      if (!$command_utils:object_match_failed(item, argstr))
        if (item in this.spurned_objects)
          this:tell("You are already spurning " + $string_utils:nn(item) + " plus any and all kids of it.");
        else
          this.spurned_objects = setadd(this.spurned_objects, item);
          this:tell("You are now spurning " + $string_utils:nn(item) + " plus any and all kids of it.");
        endif
      endif
    endif
  endverb

  verb "@spurned" (none none none) owner: HACKER flags: "rd"
    "Displays a list of spurned objects.";
    "Verb created by TheCat, 11/8/98";
    if (this.spurned_objects)
      this:tell("You are spurning the following objects, including any and all descendents:  " + $string_utils:nn(this.spurned_objects));
    else
      this:tell("You are not spurning any objects.");
    endif
  endverb

  verb set_spurned_objects (this none this) owner: HACKER flags: "rxd"
    "Permits programmatic setting of .spurned_objects, which is -c.";
    {spurned_objects} = args;
    if ($perm_utils:controls(caller_perms(), this))
      "Note, the final result must be a list of objects, otherwise there's no point.";
      if (typeof(spurned_objects) != LIST)
        spurned_objects = {spurned_objects};
      endif
      this.spurned_objects = spurned_objects;
    endif
  endverb

  verb "@addsubmitted @rmsubmitted @submitted" (none none none) owner: HACKER flags: "rd"
    "Copied from Roebare (#109000):@submitted at Sat Feb 26 19:41:37 2005 PST";
    "Usage: @addsubmitted => Process submissions to the global spelling dictionary";
    "       @rmsubmitted  => Reject a submission";
    "       @submitted    => Review outstanding submissions";
    if (!(player in $spell.trusted || player.wizard))
      return player:tell("You may not process submissions to the master dictionary.");
    endif
    "...clean-up first...";
    $spell.submitted = pending = $list_utils:remove_duplicates($spell.submitted);
    "...nothing to do...";
    if (!pending)
      return player:notify("No submissions to the global spelling dictionary are pending.");
    endif
    "...do the work...";
    if (!(cmd = verb[2..index(verb, "submitted") - 1]))
      player:notify(tostr("The following ", length($spell.submitted), " words have been submitted to the master dictionary and await approval:"));
      player:notify_lines($string_utils:columnize($list_utils:sort($spell.submitted), abs(player.linelen) / (length($list_utils:longest($spell.submitted)) + 1)));
    elseif (cmd == "add")
      player:notify(tostr("A total of ", length($spell.submitted), " words have been submitted to the master dictionary and await approval."));
      if ($command_utils:yes_or_no("Do you wish to review the list first?"))
        return player:notify_lines($string_utils:columnize($list_utils:sort($spell.submitted), abs(player.linelen) / (length($list_utils:longest($spell.submitted)) + 1)));
      else
        num_learned = num_skipped = num_errors = num_rejects = 0;
        if ($command_utils:yes_or_no("Do you wish to process each word individually? Recommended, but may take a couple minutes."))
          for candidate in ($spell.submitted)
            $command_utils:suspend_if_needed(0);
            if (!$command_utils:yes_or_no(tostr("Submitted: `", candidate, "'. Add this word?")))
              num_skipped = num_skipped + 1;
              player:notify(tostr("The word `", candidate, "' was skipped."));
            else
              if (result = $spell:add_word(candidate))
                player:notify(tostr("Word added: ", candidate));
                num_learned = num_learned + 1;
                $spell.submitted = setremove($spell.submitted, candidate);
              elseif (result == E_PERM)
                return player:notify("Permissions error. Command cancelled.");
              elseif (typeof(result) == ERR)
                num_errors = num_errors + 1;
                player:notify(tostr(result));
              else
                player:notify(tostr("Already in dictionary: ", candidate));
                num_rejects = num_rejects + 1;
                $spell.submitted = setremove($spell.submitted, candidate);
              endif
            endif
            if ($command_utils:yes_or_no(tostr("Remove `", candidate, "' from the submission list?")))
              $spell.submitted = setremove($spell.submitted, candidate);
              player:notify(tostr("The word `", candidate, "' has been removed."));
            endif
            if (!$command_utils:yes_or_no("Continue on to the next word?"))
              return player:notify_lines({"Command aborted.", tostr(" ", num_learned, " words added"), tostr(" ", num_skipped, " words skipped"), tostr(" ", num_errors, " words errored"), tostr(" ", num_rejects, " words rejected")});
            endif
          endfor
          player:notify_lines({"End of submissions.", tostr(" ", num_learned, " words added"), tostr(" ", num_skipped, " words skipped"), tostr(" ", num_errors, " words errored"), tostr(" ", num_rejects, " words rejected")});
        else
          if ($command_utils:yes_or_no("Last chance. Do you wish to cancel?"))
            return player:notify(tostr("Command cancelled. ", $network.MOO_name, "'s lexicographers thank you."));
          else
            for candidate in ($spell.submitted)
              $command_utils:suspend_if_needed(0);
              result = $spell:add_word(candidate);
              if (result)
                num_learned = num_learned + 1;
                player:notify(tostr("Word added: ", candidate));
              else
                num_errors = num_errors + 1;
                player:notify(tostr(result));
                player:notify(tostr("The word `", candidate, "' was not added."));
              endif
            endfor
            player:notify_lines({"End of submissions.", tostr(" ", num_learned, " words added"), tostr(" ", num_errors, " words errored"), ""});
            if ($command_utils:yes_or_no("Clear the submission list?"))
              "...framing required for `-' call...";
              "$spell:(\"clear-submitted\")()";
              "...incomplete perms check, do it the long way...";
              $spell.submitted = {};
              player:notify("List cleared.");
            else
              player:notify("List unchanged. Please review and prune the submission list manually.");
            endif
          endif
        endif
      endif
    elseif (cmd == "rm")
      player:tell("Which word do you want removed from the submission list?");
      if ((reject = $command_utils:read()) in $spell.submitted)
        $spell.submitted = setremove($spell.submitted, reject);
        player:notify(tostr("The word `", reject, "' was rejected from the submission list."));
      else
        player:notify(tostr("The word `", reject, "' was not found in the submission list."));
      endif
    endif
    "Created Sat Feb 19 16:57:27 2005 PST, by CherLouis (#109000).";
    "Last modified Sat Feb 26 09:46:36 2005 PST, by CherLouis (#109000).";
  endverb
endobject