object BUILDER
  name: "generic builder"
  parent: FRAND_CLASS
  owner: #2
  fertile: true
  readable: true

  property build_options (owner: #2, flags: "rc") = {};

  override aliases = {"generic builder"};
  override description = "You see a player who should type '@describe me as ...'.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override help = BUILDER_HELP;
  override import_export_id = "builder";
  override object_size = {36256, 1084848672};

  verb "@quota" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    if (dobjstr == "")
      dobj = player;
    else
      dobj = $string_utils:match_player(dobjstr);
    endif
    if (!valid(dobj))
      player:notify("Show whose quota?");
      return;
    endif
    $quota_utils:display_quota(dobj);
    try
      if (dobj in $local.informed_quota_consumers.uninformed_quota_consumers)
        player:notify(tostr("Note that quota is held in escrow -- `look ", $local.informed_quota_consumers, "' for more details."));
      endif
    except id (ANY)
    endtry
  endverb

  verb "@create" (any any any) owner: #2 flags: "rd"
    set_task_perms(player);
    nargs = length(args);
    pos = "named" in args;
    if (pos <= 1 || pos == nargs)
      pos = "called" in args;
    endif
    if (pos <= 1 || pos == nargs)
      player:notify("Usage:  @create <parent-class> named [name:]alias,...,alias");
      player:notify("   or:  @create <parent-class> named name-and-alias,alias,...,alias");
      player:notify("");
      player:notify("where <parent-class> is one of the standard classes ($note, $letter, $thing, or $container) or an object number (e.g., #999), or the name of some object in the current room.");
      player:notify("You can use \"called\" instead of \"named\", if you wish.");
      return;
    endif
    parentstr = $string_utils:from_list(args[1..pos - 1], " ");
    namestr = $string_utils:from_list(args[pos + 1..$], " ");
    if (!namestr)
      player:notify("You must provide a name.");
      return;
    endif
    if (parentstr[1] == "$")
      parent = $string_utils:literal_object(parentstr);
      if (parent == $failed_match || typeof(parent) != TYPE_OBJ)
        player:notify(tostr("\"", parentstr, "\" does not name an object."));
        return;
      endif
    else
      parent = player:my_match_object(parentstr);
      if ($command_utils:object_match_failed(parent, parentstr))
        return;
      endif
    endif
    object = player:_create(parent);
    if (typeof(object) == TYPE_ERR)
      player:notify(tostr(object));
      return;
    endif
    for f in ($string_utils:char_list(player:build_option("create_flags") || ""))
      object.(f) = 1;
    endfor
    "move() shouldn't, but could bomb. Say if player has a stupid :accept";
    `move(object, player) ! ANY';
    $building_utils:set_names(object, namestr);
    if ((other_names = setremove(object.aliases, object.name)) != {})
      aka = " (aka " + $string_utils:english_list(other_names) + ")";
    else
      aka = "";
    endif
    player:notify(tostr("You now have ", object.name, aka, " with object number ", object, " and parent ", parent.name, " (", parent, ")."));
  endverb

  verb "@recycle" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    dobj = player:my_match_object(dobjstr);
    if (dobj == $nothing)
      player:notify(tostr("Usage:  ", verb, " <object>"));
    elseif ($command_utils:object_match_failed(dobj, dobjstr))
      "...bogus object...";
    elseif (player == dobj)
      player:notify($wiz_utils.suicide_string);
    elseif (!$perm_utils:controls(player, dobj))
      player:notify(tostr(E_PERM));
    else
      name = dobj.name;
      result = player:_recycle(dobj);
      if (typeof(result) == TYPE_ERR)
        player:notify(tostr(result));
      else
        player:notify(tostr(name, " (", dobj, ") recycled."));
      endif
    endif
  endverb

  verb "@recreate" (any as any) owner: #2 flags: "rd"
    "@recreate <object> as <parent-class> named [name:]alias,alias,...";
    "  effectively recycles and creates <object> all over again.";
    set_task_perms(player);
    as = prepstr in args;
    named = "named" in args;
    if (named <= as + 1 || named == length(args))
      named = "called" in args;
    endif
    if (named <= as + 1 || named == length(args))
      player:notify_lines({tostr("Usage:  ", verb, " <object> as <parent-class> named [name:]alias,...,alias"), "", "where <parent-class> is one of the standard classes ($note, $letter, $thing, or $container) or an object number (e.g., #999), or the name of some object in the current room.  The [name:]alias... specification is as in @create.", "", "You can use \"called\" instead of \"named\", if you wish."});
      return;
    elseif ($command_utils:object_match_failed(dobj = player:my_match_object(dobjstr), dobjstr))
      return;
    elseif (is_player(dobj))
      player:notify("You really *don't* want to do that!");
      return;
    endif
    parentstr = $string_utils:from_list(args[as + 1..named - 1], " ");
    namestr = $string_utils:from_list(args[named + 1..$], " ");
    if (parentstr[1] == "$")
      parent = $string_utils:literal_object(parentstr);
      if (parent == $failed_match || typeof(parent) != TYPE_OBJ)
        player:notify(tostr("\"", parentstr, "\" does not name an object."));
        return;
      endif
    else
      parent = player:my_match_object(parentstr);
      if ($command_utils:object_match_failed(parent, parentstr))
        return;
      endif
    endif
    if (!(e = $building_utils:recreate(dobj, parent)))
      player:notify(tostr(e));
      return;
    endif
    for f in ($string_utils:char_list(player:build_option("create_flags") || ""))
      dobj.(f) = 1;
    endfor
    "move() shouldn't, but could, bomb. Say if player has a stupid :accept";
    `move(dobj, player) ! ANY';
    $building_utils:set_names(dobj, namestr);
    if ((other_names = setremove(dobj.aliases, dobj.name)) != {})
      aka = " (aka " + $string_utils:english_list(other_names) + ")";
    else
      aka = "";
    endif
    player:notify(tostr("Object number ", dobj, " is now ", dobj.name, aka, " with parent ", parent.name, " (", parent, ")."));
  endverb

  verb "@dig" (any any any) owner: #2 flags: "rd"
    set_task_perms(player);
    nargs = length(args);
    if (nargs == 1)
      room = args[1];
      exit_spec = "";
    elseif (nargs >= 3 && args[2] == "to")
      exit_spec = args[1];
      room = $string_utils:from_list(args[3..$], " ");
    elseif (argstr && !prepstr)
      room = argstr;
      exit_spec = "";
    else
      player:notify(tostr("Usage:  ", verb, " <new-room-name>"));
      player:notify(tostr("    or  ", verb, " <exit-description> to <new-room-name-or-old-room-object-number>"));
      return;
    endif
    if (room != tostr(other_room = toobj(room)))
      room_kind = player:build_option("dig_room");
      if (room_kind == 0)
        room_kind = $room;
      endif
      other_room = player:_create(room_kind);
      if (typeof(other_room) == TYPE_ERR)
        player:notify(tostr("Cannot create new room as a child of ", $string_utils:nn(room_kind), ": ", other_room, ".  See `help @build-options' for information on how to specify the kind of room this command tries to create."));
        return;
      endif
      for f in ($string_utils:char_list(player:build_option("create_flags") || ""))
        other_room.(f) = 1;
      endfor
      other_room.name = room;
      other_room.aliases = {room};
      move(other_room, $nothing);
      player:notify(tostr(other_room.name, " (", other_room, ") created."));
    elseif (nargs == 1)
      player:notify("You can't dig a room that already exists!");
      return;
    elseif (!valid(player.location) || !($room in $object_utils:ancestors(player.location)))
      player:notify(tostr("You may only use the ", verb, " command from inside a room."));
      return;
    elseif (!valid(other_room) || !($room in $object_utils:ancestors(other_room)))
      player:notify(tostr(other_room, " doesn't look like a room to me..."));
      return;
    endif
    if (exit_spec)
      exit_kind = player:build_option("dig_exit");
      if (exit_kind == 0)
        exit_kind = $exit;
      endif
      exits = $string_utils:explode(exit_spec, "|");
      if (length(exits) < 1 || length(exits) > 2)
        player:notify("The exit-description must have the form");
        player:notify("     [name:]alias,...,alias");
        player:notify("or   [name:]alias,...,alias|[name:]alias,...,alias");
        return;
      endif
      do_recreate = !player:build_option("bi_create");
      to_ok = $building_utils:make_exit(exits[1], player.location, other_room, do_recreate, exit_kind);
      if (to_ok && length(exits) == 2)
        $building_utils:make_exit(exits[2], other_room, player.location, do_recreate, exit_kind);
      endif
    endif
  endverb

  verb "@audit" (any any any) owner: #2 flags: "rd"
    "Usage:  @audit [player] [from <start>] [to <end>] [for <matching string>]";
    set_task_perms(player);
    dobj = $string_utils:match_player(dobjstr);
    if (!dobjstr)
      dobj = player;
    elseif ($command_utils:player_match_result(dobj, dobjstr)[1])
      return;
    endif
    dobjwords = $string_utils:words(dobjstr);
    if (args[1..length(dobjwords)] == dobjwords)
      args = args[length(dobjwords) + 1..$];
    endif
    if (!(parse_result = $code_utils:_parse_audit_args(@args)))
      player:notify(tostr("Usage:  ", verb, " [player] [from <start>] [to <end>] [for <match>]"));
      return;
    endif
    return $building_utils:do_audit(dobj, @parse_result);
  endverb

  verb "@count" (any none none) owner: #2 flags: "rd"
    if (!dobjstr)
      dobj = player;
    elseif ($command_utils:player_match_result(dobj = $string_utils:match_player(dobjstr), dobjstr)[1])
      return;
    endif
    set_task_perms(player);
    if (typeof(dobj.owned_objects) == TYPE_LIST)
      count = length(dobj.owned_objects);
      player:notify(tostr(dobj.name, " currently owns ", count, " object", count == 1 ? "." | "s."));
      if ($quota_utils.byte_based)
        player:notify(tostr("Total bytes consumed:  ", $string_utils:group_number($quota_utils:get_size_quota(dobj)[2]), "."));
      endif
    else
      player:notify(tostr(dobj.name, " is not enrolled in the object ownership system.  Use @countDB instead."));
    endif
  endverb

  verb "@countDB" (any none none) owner: #2 flags: "rd"
    if (!dobjstr)
      dobj = player;
    elseif ($command_utils:player_match_result(dobj = $string_utils:match_player(dobjstr), dobjstr)[1])
      return;
    endif
    set_task_perms(player);
    count = 0;
    for o in [#1..max_object()]
      if ($command_utils:running_out_of_time())
        player:notify("Counting...");
        suspend(5);
      endif
      if (valid(o) && o.owner == dobj)
        count = count + 1;
      endif
    endfor
    player:notify(tostr(dobj.name, " currently owns ", count, " object", count == 1 ? "." | "s."));
  endverb

  verb "@sort-owned*-objects" (any none none) owner: #2 flags: "rd"
    "$player:owned_objects -- sorts a players .owned_objects property in ascending";
    "order so it looks nice on @audit.";
    if (player != this)
      return E_PERM;
    endif
    if (typeof(player.owned_objects) == TYPE_LIST)
      if (!dobjstr || index("object", dobjstr) == 1)
        ret = $list_utils:sort_suspended(0, player.owned_objects);
      elseif (index("size", dobjstr) == 1)
        ret = $list_utils:reverse_suspended($list_utils:sort_suspended(0, player.owned_objects, $list_utils:slice($list_utils:map_prop(player.owned_objects, "object_size"))));
      endif
      if (typeof(ret) == TYPE_LIST)
        player.owned_objects = ret;
        player:tell("Your .owned_objects list has been sorted.");
        return 1;
      else
        player:tell("Something went wrong. .owned_objects not sorted.");
        return 0;
      endif
    else
      player:tell("You are not enrolled in .owned_objects scheme, sorry.");
    endif
  endverb

  verb "@add-owned" (any none none) owner: #2 flags: "rd"
    if (player != this)
      player:tell("Permission Denied");
      return E_PERM;
    endif
    if (!valid(dobj))
      player:tell("Don't understand `", dobjstr, "' as an object to add.");
    elseif (dobj.owner != player)
      player:tell("You don't own ", dobj.name, ".");
    elseif (dobj in player.owned_objects)
      player:tell(dobj.name, " is already recorded in your .owned_objects.");
    else
      player.owned_objects = setadd(player.owned_objects, dobj);
      player:tell("Added ", dobj, " to your .owned_objects.");
    endif
  endverb

  verb "@verify-owned" (none none none) owner: #2 flags: "rd"
    for x in (player.owned_objects)
      if (!valid(x) || x.owner != player)
        player.owned_objects = setremove(player.owned_objects, x);
        if (valid(x))
          player:tell("Removing ", x.name, "(", x, "), owned by ", valid(x.owner) ? x.owner.name | "<recycled player>", " from your .owned_objects property.");
        else
          player:tell("Removing invalid object ", x, " from your .owned_objects property.");
        endif
      endif
      $command_utils:suspend_if_needed(2, tostr("Suspending @verify-owned ... ", x));
    endfor
    player:tell(".owned_objects property verified.");
  endverb

  verb "@unlock" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    dobj = player:my_match_object(dobjstr);
    if ($command_utils:object_match_failed(dobj, dobjstr))
      return;
    endif
    try
      dobj.key = 0;
      player:notify(tostr("Unlocked ", dobj.name, "."));
    except error (ANY)
      player:notify(error[2]);
    endtry
  endverb

  verb "@lock" (any with any) owner: #2 flags: "rd"
    set_task_perms(player);
    dobj = player:my_match_object(dobjstr);
    if ($command_utils:object_match_failed(dobj, dobjstr))
      return;
    endif
    key = $lock_utils:parse_keyexp(iobjstr, player);
    if (typeof(key) == TYPE_STR)
      player:notify("That key expression is malformed:");
      player:notify(tostr("  ", key));
    else
      try
        dobj.key = key;
        player:notify(tostr("Locked ", dobj.name, " to this key:"));
        player:notify(tostr("  ", $lock_utils:unparse_key(key)));
      except error (ANY)
        player:notify(error[2]);
      endtry
    endif
  endverb

  verb "@newmess*age" (any any any) owner: #2 flags: "rd"
    "Usage:  @newmessage <message-name> [<message>] [on <object>]";
    "Add a message property to an object (default is player), and optionally";
    "set its value.  For use by non-programmers, who aren't allowed to add";
    "properties generally.";
    "To undo the effects of this, use @unmessage.";
    set_task_perms(player);
    dobjwords = $string_utils:words(dobjstr);
    if (!dobjwords)
      player:notify(tostr("Usage:  ", verb, " <message-name> [<message>] [on <object>]"));
      return;
    endif
    object = valid(iobj) ? iobj | player;
    name = this:_messagify(dobjwords[1]);
    value = dobjstr[length(dobjwords[1]) + 2..$];
    nickname = "@" + name[1..$ - 4];
    e = `add_property(object, name, value, {player, "rc"}) ! ANY';
    if (typeof(e) != TYPE_ERR)
      player:notify(tostr(nickname, " on ", object.name, " is now \"", object.(name), "\"."));
    elseif (e != E_INVARG)
      player:notify(tostr(e));
    elseif ($object_utils:has_property(object, name))
      "object already has property";
      player:notify(tostr(object.name, " already has a ", nickname, " message."));
    else
      player:notify(tostr("Unable to add ", nickname, " message to ", object.name, ": ", e));
    endif
  endverb

  verb "@unmess*age" (any any any) owner: #2 flags: "rd"
    "Usage:  @unmessage <message-name> [from <object>]";
    "Remove a message property from an object (default is player).";
    set_task_perms(player);
    if (!dobjstr || length($string_utils:words(dobjstr)) > 1)
      player:notify(tostr("Usage:  ", verb, " <message-name> [from <object>]"));
      return;
    endif
    object = valid(iobj) ? iobj | player;
    name = this:_messagify(dobjstr);
    nickname = "@" + name[1..$ - 4];
    try
      delete_property(object, name);
      player:notify(tostr(nickname, " message removed from ", object.name, "."));
    except (E_PROPNF)
      player:notify(tostr("No ", nickname, " message found on ", object.name, "."));
    except error (ANY)
      player:notify(error[2]);
    endtry
  endverb

  verb _messagify (this none this) owner: #2 flags: "rxd"
    "Given any of several formats people are likely to use for a @message";
    "property, return the canonical form (\"foobar_msg\").";
    name = args[1];
    if (name[1] == "@")
      name = name[2..$];
    endif
    if (length(name) < 4 || name[$ - 3..$] != "_msg")
      name = name + "_msg";
    endif
    return name;
  endverb

  verb "@kids" (any none none) owner: #2 flags: "rxd"
    "'@kids <obj>' - List the children of an object. This is handy for seeing whether anybody's actually using your carefully-wrought public objects.";
    thing = player:my_match_object(dobjstr);
    if (!$command_utils:object_match_failed(thing, dobjstr))
      kids = children(thing);
      if (kids)
        player:notify(tostr(thing:title(), "(", thing, ") has ", length(kids), " kid", length(kids) == 1 ? "" | "s", "."));
        player:notify(tostr($string_utils:names_of(kids)));
      else
        player:notify(tostr(thing:title(), "(", thing, ") has no kids."));
      endif
    endif
  endverb

  verb "@contents" (any none none) owner: #2 flags: "rd"
    "'@contents <obj> - list the contents of an object, with object numbers.";
    set_task_perms(player);
    if (!dobjstr)
      dobj = player.location;
    else
      dobj = player:my_match_object(dobjstr);
    endif
    if ($command_utils:object_match_failed(dobj, dobjstr))
    else
      contents = dobj.contents;
      if (contents)
        player:notify(tostr(dobj:title(), "(", dobj, ") contains:"));
        player:notify(tostr($string_utils:names_of(contents)));
      else
        player:notify(tostr(dobj:title(), "(", dobj, ") contains nothing."));
      endif
    endif
  endverb

  verb "@par*ents" (any none none) owner: #2 flags: "rd"
    "'@parents <thing>' - List <thing> and its ancestors, all the way back to the Root Class (#1).";
    if (player != this)
      return player:notify("Permission denied: not a builder.");
    elseif (!dobjstr)
      player:notify(tostr("Usage:  ", verb, " <object>"));
      return;
    endif
    set_task_perms(player);
    o = player:my_match_object(dobjstr);
    if (!$command_utils:object_match_failed(o, dobjstr))
      player:notify($string_utils:names_of({o, @$object_utils:ancestors(o)}));
    endif
  endverb

  verb "@location*s" (any none none) owner: #2 flags: "rd"
    "@locations <thing> - List <thing> and its containers, all the way back to the outermost one.";
    set_task_perms(player);
    if (!dobjstr)
      what = player;
    elseif (!valid(what = player:my_match_object(dobjstr)) && !valid(what = $string_utils:match_player(dobjstr)))
      $command_utils:object_match_failed(dobj, dobjstr);
      return;
    endif
    player:notify($string_utils:names_of({what, @$object_utils:locations(what)}));
  endverb

  verb "@cl*asses" (any any any) owner: #2 flags: "rd"
    "$class_registry is in the following format:";
    "        { {name, description, members}, ... }";
    "where `name' is the name of a particular class of objects, `description' is a one-sentence description of the membership of the class, and `members' is a list of object numbers, the members of the class.";
    "";
    if (!$command_utils:yes_or_no("This command can be very spammy.  Are you certain you need this information?"))
      return player:tell("OK, aborting.  The lag thanks you.");
    endif
    if (args)
      members = {};
      for name in (args)
        class = $list_utils:assoc_prefix(name, $class_registry);
        if (class)
          for o in (class[3])
            members = setadd(members, o);
          endfor
        else
          player:tell("There is no defined class of objects named `", name, "'; type `@classes' to see a complete list of defined classes.");
          return;
        endif
      endfor
      printed = {};
      for o in (members)
        what = o;
        while (valid(what))
          printed = setadd(printed, what);
          what = parent(what);
        endwhile
      endfor
      player:tell("Members of the class", length(args) > 1 ? "es" | "", " named ", $string_utils:english_list(args), ":");
      player:tell();
      set_task_perms(player);
      this:classes_2($root_class, "", members, printed);
      player:tell();
    else
      "List all class names and descriptions";
      player:tell("The following classes of objects have been defined:");
      for class in ($class_registry)
        name = class[1];
        description = class[2];
        player:tell();
        player:tell("-- ", name, ": ", description);
      endfor
      player:tell();
      player:tell("Type `@classes <name>' to see the members of the class with the given <name>.");
    endif
  endverb

  verb classes_2 (this none this) owner: #2 flags: "rxd"
    {root, indent, members, printed} = args;
    if (root in members)
      player:tell(indent, root.name, " (", root, ")");
    else
      player:tell(indent, "<", root.name, " (", root, ")>");
    endif
    printed = setremove(printed, root);
    indent = indent + "  ";
    set_task_perms(caller_perms());
    for c in ($list_utils:sort_suspended(2, $set_utils:intersection(children(root), printed)))
      $command_utils:suspend_if_needed(10);
      this:classes_2(c, indent, members, printed);
    endfor
  endverb

  verb _create (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    if (this:build_option("bi_create"))
      return $quota_utils:bi_create(@args);
    else
      return $recycler:(verb)(@args);
    endif
  endverb

  verb _recycle (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    if (this:build_option("bi_create") || is_uuobjid(@args))
      return recycle(@args);
    else
      return $recycler:(verb)(@args);
    endif
  endverb

  verb "@chparent" (any at any) owner: #2 flags: "rd"
    set_task_perms(player);
    if ($command_utils:object_match_failed(object = player:my_match_object(dobjstr), dobjstr))
      "...bogus object...";
    elseif ($command_utils:object_match_failed(parent = player:my_match_object(iobjstr), iobjstr))
      "...bogus new parent...";
    elseif (this != player && !$object_utils:isa(player, $player))
      "...They chparented to #1 and want to chparent back to $prog.  Probably for some nefarious purpose...";
      player:notify("You don't seem to already be a valid player class.  Perhaps chparenting away from the $player hierarchy was not such a good idea.  Permission denied.");
    elseif (is_player(object) && !$object_utils:isa(parent, $player))
      player:notify(tostr(object, " is a player and ", parent, " is not a player class."));
      player:notify("You really *don't* want to do this.  Trust me.");
    else
      if ($object_utils:isa(object, $mail_recipient))
        if (!$command_utils:yes_or_no("Chparenting a mailing list is usually a really bad idea.  Do you really want to do it?  (If you don't know why we're asking this question, please say 'no'.)"))
          return player:tell("Aborted.");
        endif
      endif
      try
        result = player:_chparent(object, parent);
        player:notify("Parent changed.");
      except (E_INVARG)
        if (valid(object) && valid(parent))
          player:notify(tostr("Some property existing on ", parent, " is defined on ", object, " or one of its descendants."));
          player:notify(tostr("Try @check-chparent ", dobjstr, " to ", iobjstr));
        else
          player:notify("Either that is not a valid object or not a valid parent");
        endif
      except (E_PERM)
        player:notify("Either you don't own the object, don't own the parent, or the parent is not fertile.");
      except (E_RECMOVE)
        player:notify("That parent object is a descendant of the object!");
      endtry
    endif
  endverb

  verb "@check-chp*arent" (any at any) owner: #2 flags: "rd"
    "Copied from generic programmer (#217):@check-chparent by ur-Rog (#6349) Sun Nov  8 22:13:53 1992 PST";
    "@check-chparent object to newparent";
    "checks for property name conflicts that would make @chparent bomb.";
    set_task_perms(player);
    if (!(dobjstr && iobjstr))
      player:notify(tostr("Usage:  ", verb, " <object> to <newparent>"));
    elseif ($command_utils:object_match_failed(object = player:my_match_object(dobjstr), dobjstr))
      "...bogus object...";
    elseif ($command_utils:object_match_failed(parent = player:my_match_object(iobjstr), iobjstr))
      "...bogus new parent...";
    elseif (player != this)
      player:notify(tostr(E_PERM));
    elseif (typeof(result = $object_utils:property_conflicts(object, parent)) == TYPE_ERR)
      player:notify(tostr(result));
    elseif (result)
      su = $string_utils;
      player:notify("");
      player:notify(su:left("Property", 30) + "Also Defined on");
      player:notify(su:left("--------", 30) + "---------------");
      for r in (result)
        player:notify(su:left(tostr(parent, ".", r[1]), 30) + su:from_list(listdelete(r, 1), " "));
        $command_utils:suspend_if_needed(0);
      endfor
    else
      player:notify("No property conflicts found.");
    endif
  endverb

  verb "@set*prop" (any at any) owner: #2 flags: "rd"
    "Syntax:  @set <object>.<prop-name> to <value>";
    "";
    "Changes the value of the specified object's property to the given value.";
    "You must have permission to modify the property, either because you own the property or if it is writable.";
    set_task_perms(player);
    if (this != player)
      return player:tell(E_PERM);
    endif
    l = $code_utils:parse_propref(dobjstr);
    if (l)
      dobj = player:my_match_object(l[1], player.location);
      if ($command_utils:object_match_failed(dobj, l[1]))
        return;
      endif
      prop = l[2];
      to_i = "to" in args;
      at_i = "at" in args;
      i = to_i && at_i ? min(to_i, at_i) | to_i || at_i;
      iobjstr = argstr[$string_utils:word_start(argstr)[i][2] + 1..$];
      iobjstr = $string_utils:trim(iobjstr);
      if (!iobjstr)
        try
          val = dobj.(prop) = "";
        except e (ANY)
          player:tell("Unable to set ", dobj, ".", prop, ": ", e[2]);
          return;
        endtry
        iobjstr = "\"\"";
        "elseif (iobjstr[1] == \"\\\"\")";
        "val = dobj.(prop) = iobjstr;";
        "iobjstr = \"\\\"\" + iobjstr + \"\\\"\";";
      else
        val = $string_utils:to_value(iobjstr);
        if (!val[1])
          player:tell("Could not parse: ", iobjstr);
          return;
        elseif (!$object_utils:has_property(dobj, prop))
          player:tell("That object does not define that property.");
          return;
        endif
        try
          val = dobj.(prop) = val[2];
        except e (ANY)
          player:tell("Unable to set ", dobj, ".", prop, ": ", e[2]);
          return;
        endtry
      endif
      player:tell("Property ", dobj, ".", prop, " set to ", $string_utils:print(val), ".");
    else
      player:tell("Property ", dobjstr, " not found.");
    endif
  endverb

  verb build_option (this none this) owner: #2 flags: "rxd"
    ":build_option(name)";
    "Returns the value of the specified builder option";
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      return $build_options:get(this.build_options, args[1]);
    else
      return E_PERM;
    endif
  endverb

  verb set_build_option (this none this) owner: #2 flags: "rxd"
    ":set_build_option(oname,value)";
    "Changes the value of the named option.";
    "Returns a string error if something goes wrong.";
    if (!(caller == this || $perm_utils:controls(caller_perms(), this)))
      return tostr(E_PERM);
    endif
    "...this is kludgy, but it saves me from writing the same verb n times.";
    "...there's got to be a better way to do this...";
    verb[1..4] = "";
    foo_options = verb + "s";
    "...";
    if (typeof(s = #0.(foo_options):set(this.(foo_options), @args)) == TYPE_STR)
      return s;
    elseif (s == this.(foo_options))
      return 0;
    else
      this.(foo_options) = s;
      return 1;
    endif
  endverb

  verb "@build-o*ptions @buildo*ptions @builder-o*ptions @buildero*ptions" (any any any) owner: #2 flags: "rd"
    "@<what>-option <option> [is] <value>   sets <option> to <value>";
    "@<what>-option <option>=<value>        sets <option> to <value>";
    "@<what>-option +<option>     sets <option>   (usually equiv. to <option>=1";
    "@<what>-option -<option>     resets <option> (equiv. to <option>=0)";
    "@<what>-option !<option>     resets <option> (equiv. to <option>=0)";
    "@<what>-option <option>      displays value of <option>";
    set_task_perms(player);
    what = "build";
    options = what + "_options";
    option_pkg = #0.(options);
    set_option = "set_" + what + "_option";
    if (!args)
      player:notify_lines({"Current " + what + " options:", "", @option_pkg:show(this.(options), option_pkg.names)});
      return;
    elseif (typeof(presult = option_pkg:parse(args)) == TYPE_STR)
      player:notify(presult);
      return;
    else
      if (length(presult) > 1)
        if (typeof(sresult = this:(set_option)(@presult)) == TYPE_STR)
          player:notify(sresult);
          return;
        elseif (!sresult)
          player:notify("No change.");
          return;
        endif
      endif
      player:notify_lines(option_pkg:show(this.(options), presult[1]));
    endif
  endverb

  verb "@meas*ure" (any any any) owner: HACKER flags: "rd"
    "Syntax:";
    "  @measure object <object name>";
    "  @measure summary [player]";
    "  @measure new [player]";
    "  @measure breakdown <object name>";
    "  @measure recent [number of days] [player]";
    if (length(args) < 1)
      player:tell_lines($code_utils:verb_documentation());
      return;
    endif
    if (index("object", args[1]) == 1)
      "Object.";
      what = player.location:match_object(name = $string_utils:from_list(args[2..$], " "));
      lag = $login:current_lag();
      if (!valid(what))
        player:tell("Sorry, I didn't understand `", name, "'");
      elseif ($object_utils:has_property(what, "object_size") && what.object_size[1] > $byte_quota_utils.too_large && !player.wizard && player != $byte_quota_utils.owner && player != $hacker && player != what.owner && lag > 0)
        player:tell($string_utils:nn(what), " when last measured was ", $string_utils:group_number(what.object_size[1]), " bytes.  To reduce lag induced by multiple players re-measuring large objects multiple times, you may not measure that object.");
      elseif (lag > 0 && `what.object_size[2] ! ANY => 0' > time() - 86400 && !$command_utils:yes_or_no(tostr("That object was measured only ", $string_utils:from_seconds(time() - what.object_size[2]), " ago.  Please don't lag the MOO by remeasuring things frequently.  Are you sure you want to remeasure it?")))
        return player:tell("Not measuring.  It was ", $string_utils:group_number(what.object_size[1]), " bytes when last measured.");
      else
        player:tell("Checking size of ", what.name, " (", what, ")...");
        player:tell("Size of ", what.name, " (", what, ") is ", $string_utils:group_number($byte_quota_utils:object_bytes(what)), " bytes.");
      endif
    elseif (index("summary", args[1]) == 1)
      "Summarize player.";
      if (length(args) == 1)
        what = player;
      else
        what = $string_utils:match_player(name = $string_utils:from_list(args[2..$], " "));
      endif
      if (!valid(what))
        player:tell("Sorry, I don't know who you mean by `", name, "'");
      else
        $byte_quota_utils:do_summary(what);
      endif
    elseif (index("new", args[1]) == 1)
      if (length(args) == 1)
        what = player;
      elseif (!valid(what = $string_utils:match_player(name = $string_utils:from_list(args[2..$], " "))))
        return $command_utils:player_match_failed(what, name);
      endif
      player:tell("Measuring the sizes of ", what.name, "'s recently created objects...");
      total = 0;
      unmeasured_index = 4;
      unmeasured_multiplier = 100;
      nunmeasured = 0;
      if (typeof(what.owned_objects) == TYPE_LIST)
        for x in (what.owned_objects)
          if (!$object_utils:has_property(x, "object_size"))
            nunmeasured = nunmeasured + 1;
          elseif (!x.object_size[1])
            player:tell("Measured ", $string_utils:nn(x), ":  ", size = $byte_quota_utils:object_bytes(x), " bytes.");
            total = total + size;
          endif
          $command_utils:suspend_if_needed(5);
        endfor
        if (nunmeasured && what.size_quota[unmeasured_index] < unmeasured_multiplier * nunmeasured)
          what.size_quota[unmeasured_index] = what.size_quota[unmeasured_index] % unmeasured_multiplier + nunmeasured * unmeasured_multiplier;
        endif
        player:tell("Total bytes used in new creations: ", total, ".", nunmeasured ? tostr("There were a total of ", nunmeasured, " object(s) found with no .object_size property.  This will prevent additional building.") | "");
      else
        player:tell("Sorry, ", what.name, " is not enrolled in the object measurement scheme.");
      endif
    elseif (index("recent", args[1]) == 1)
      "@measure recent days player";
      if (length(args) > 1)
        days = $code_utils:toint(args[2]);
      else
        days = $byte_quota_utils.cycle_days;
      endif
      if (!days)
        return player:tell("Couldn't understand `", args[2], "' as a positive integer.");
      endif
      if (length(args) > 2)
        if (!valid(who = $string_utils:match_player(name = $string_utils:from_list(args[3..$], " "))))
          return $command_utils:player_match_failed(who, name);
        endif
      else
        who = player;
      endif
      if (typeof(who.owned_objects) == TYPE_LIST)
        player:tell("Re-measuring objects of ", $string_utils:nn(who), " which have not been measured in the past ", days, " days.");
        when = time() - days * 86400;
        which = {};
        for x in (who.owned_objects)
          if (x.object_size[2] < when)
            $byte_quota_utils:object_size(x);
            which = setadd(which, x);
            $command_utils:suspend_if_needed(3, "...measuring");
          endif
        endfor
        player:tell("Done, re-measured ", length(which), " objects.", length(which) > 0 ? "  Recommend you use @measure summary to update the display of @quota." | "");
      else
        player:tell("Sorry, ", who.name, " is not enrolled in the object measurement scheme.");
      endif
    elseif (index("breakdown", args[1]) == 1)
      what = player.location:match_object(name = $string_utils:from_list(args[2..$], " "));
      if (!valid(what))
        player:tell("Sorry, I didn't understand `", name, "'");
      elseif (!$byte_quota_utils:can_peek(player, what.owner))
        return player:tell("Sorry, you don't control ", what.name, " (", what, ")");
      else
        if (mail = $command_utils:yes_or_no("This might be kinda long.  Want me to mail you the result?"))
          player:tell("Result will be mailed.");
        endif
        info = $byte_quota_utils:do_breakdown(what);
        if (typeof(info) == TYPE_ERR)
          player:tell(info);
        endif
        if (mail)
          $mail_agent:send_message($byte_quota_utils.owner, {player}, tostr("Object breakdown of ", what.name, " (", what, ")"), info);
        else
          player:tell_lines_suspended(info);
        endif
      endif
    else
      player:tell("Not a sub-command of @measure: ", args[1]);
      player:tell_lines($code_utils:verb_documentation());
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      if (this == $builder)
        this.build_options = {};
      else
        clear_property(this, "build_options");
      endif
      return pass(@args);
    endif
  endverb

  verb "@listedit @pedit" (any none none) owner: HACKER flags: "rd"
    "@listedit|@pedit object.prop -- invokes the list editor.";
    "   if you are editing a list of strings, you're better off using @notedit.";
    $list_editor:invoke(dobjstr, verb);
  endverb
endobject