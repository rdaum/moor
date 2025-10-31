object BUILDING_UTILS
  name: "building utilities"
  parent: GENERIC_UTILS
  owner: #2
  readable: true

  property class_string (owner: #2, flags: "rc") = {"p", "R", "E", "N", "C", "T", "F", "M", "H", "D", "U", "O"};
  property classes (owner: #2, flags: "rc") = {
    PLAYER,
    ROOM,
    EXIT,
    NOTE,
    CONTAINER,
    THING,
    FEATURE,
    MAIL_RECIPIENT,
    GENERIC_HELP,
    GENERIC_DB,
    GENERIC_UTILS,
    GENERIC_OPTIONS
  };

  override aliases = {"building", "utils"};
  override description = {
    "This is the building utilities utility package.  See `help $building_utils' for more details."
  };
  override help_msg = {
    "Verbs useful for building.  For a complete description of a given verb, do `help $building_utils:verbname'.",
    "",
    "make_exit(spec,source,dest[,don't-really-create]) => a new exit",
    "          spec is an exit-spec as described in `help @dig'",
    "",
    "set_names(object, spec) - sets name and aliases for an object",
    "parse_names(spec) => list of {name, aliases}",
    "          in both of these, spec is of the form",
    "            <name>[[,:]<alias>,<alias>,...]",
    "          (as described in `help @rename')",
    "",
    "recreate(object, newparent) - effectively recycle and recreate object",
    "          as a child of newparent"
  };
  override import_export_id = "building_utils";
  override object_size = {12705, 1084848672};

  verb make_exit (this none this) owner: #2 flags: "rxd"
    "make_exit(spec, source, dest[, use-$recycler-pool [, kind]])";
    "";
    "Uses $recycler by default; supplying fourth arg as 0 suppresses this.";
    "Optional 5th arg gives a parent for the object to be created";
    "(i.e., distinct from $exit)";
    "Returns the object number as a list if successful, 0 if not.";
    set_task_perms(caller_perms());
    {spec, source, dest, ?use_recycler, ?exit_kind = $exit} = args;
    exit = player:_create(exit_kind);
    if (typeof(exit) == ERR)
      player:notify(tostr("Cannot create new exit as a child of ", $string_utils:nn(exit_kind), ": ", exit, ".  See `help @build-options' for information on how to specify the kind of exit this command tries to create."));
      return;
    endif
    for f in ($string_utils:char_list(player:build_option("create_flags") || ""))
      exit.(f) = 1;
    endfor
    $building_utils:set_names(exit, spec);
    exit.source = source;
    exit.dest = dest;
    source_ok = source:add_exit(exit);
    dest_ok = dest:add_entrance(exit);
    move(exit, $nothing);
    via = $string_utils:from_value(setadd(exit.aliases, exit.name), 1);
    if (source_ok)
      player:tell("Exit from ", source.name, " (", source, ") to ", dest.name, " (", dest, ") via ", via, " created with id ", exit, ".");
      if (!dest_ok)
        player:tell("However, I couldn't add ", exit, " as a legal entrance to ", dest.name, ".  You may have to get its owner, ", dest.owner.name, " to add it for you.");
      endif
      return {exit};
    elseif (dest_ok)
      player:tell("Exit to ", dest.name, " (", dest, ") via ", via, " created with id ", exit, ".  However, I couldn't add ", exit, " as a legal exit from ", source.name, ".  Get its owner, ", source.owner.name, " to add it for you.");
      return {exit};
    else
      player:_recycle(exit);
      player:tell("I couldn't add a new exit as EITHER a legal exit from ", source.name, " OR as a legal entrance to ", dest.name, ".  Get their owners, ", source.owner.name, " and ", dest.owner.name, ", respectively, to add it for you.");
      return 0;
    endif
  endverb

  verb set_names (this none this) owner: #2 flags: "rxd"
    "$building_utils:set_names(object, spec)";
    set_task_perms(caller_perms());
    object = args[1];
    names = this:parse_names(args[2]);
    name = names[1] || object.name;
    return object:set_name(name) && object:set_aliases(names[2]);
  endverb

  verb recreate (this none this) owner: #2 flags: "rxd"
    ":recreate(object,newparent) -- effectively recycle and recreate the specified object as a child of parent.  Returns true if successful.";
    {object, parent} = args;
    who = caller_perms();
    if (!(valid(object) && valid(parent)))
      return E_INVARG;
    elseif (who.wizard)
      "no problemo";
    elseif (who != object.owner || (who != parent.owner && !parent.f))
      return E_PERM;
    endif
    "Chparent any children to their grandparent instead of orphaning them horribly.  Have to do the chparent with wizperms, in case the children are owned by others, so do this before set_task_perms.";
    "Because this is done before set_task_perms() -- thus with wizard perms -- we save ticks and use chparent() instead of #0:chparent().  This will save many more ticks, if this is an object with many children.";
    grandpa = parent(object);
    for c in (children(object))
      chparent(c, grandpa);
    endfor
    for item in (object.contents)
      if (!is_player(item))
        move(item, #-1);
      else
        move(item, $player_start);
      endif
    endfor
    set_task_perms(who);
    if ($object_utils:has_callable_verb(object, "recycle"))
      object:recycle();
    endif
    chparent(object, #-1);
    for p in (properties(object))
      delete_property(object, p);
    endfor
    for v in (verbs(object))
      delete_verb(object, 1);
    endfor
    chparent(object, parent);
    object.name = "";
    object.r = 0;
    object.f = 0;
    object.w = 0;
    if ($object_utils:has_callable_verb(parent, "initialize"))
      object:initialize();
    endif
    return 1;
  endverb

  verb parse_names (this none this) owner: #2 flags: "rxd"
    "$building_utils:parse_names(spec)";
    "Return {name, {alias, alias, ...}} from name,alias,alias or name:alias,alias";
    spec = args[1];
    if (!(colon = index(spec, ":")))
      aliases = $string_utils:explode(spec, ",");
      if (!aliases)
        aliases = {spec};
      endif
      name = aliases[1];
    else
      aliases = $string_utils:explode(spec[colon + 1..$], ",");
      name = spec[1..colon - 1];
    endif
    return {name, $list_utils:map_arg($string_utils, "trim", aliases)};
  endverb

  verb audit_object_category (this none this) owner: #2 flags: "rxd"
    if (is_player(what = args[1]))
      return "P";
    endif
    while (valid(what))
      if (i = what in this.classes)
        return this.class_string[i];
      endif
      what = parent(what);
    endwhile
    return " ";
  endverb

  verb object_audit_string (this none this) owner: #2 flags: "rxd"
    ":object_audit_string(object [,prospectus-style])";
    {o, ?prospectus = 0} = args;
    olen = length(tostr(max_object()));
    if (!$recycler:valid(o))
      return tostr(prospectus ? "          " | "", $quota_utils.byte_based ? "    " | "", $string_utils:right(o, olen), " Invalid Object!");
    endif
    if (prospectus)
      kids = 0;
      for k in (children(o))
        $command_utils:suspend_if_needed(0);
        if (k.owner != o.owner)
          kids = 2;
          break k;
        elseif (kids == 0)
          kids = 1;
        endif
      endfor
      "The verbs() call below might fail, but that's OK";
      "Well, actually it won't cuz we seem to be a wizard.  Since you can get the number of verbs information from @verbs anyway, it seems kind of pointless to hide it here.";
      v = verbs(o);
      if (v)
        vstr = tostr("[", $string_utils:right(length(v), 3), "] ");
      else
        vstr = "      ";
      endif
      if (o.r && o.f)
        r = "f";
      elseif (o.r)
        r = "r";
      elseif (o.f)
        r = "F";
      else
        r = " ";
      endif
      vstr = tostr(" kK"[kids + 1], r, $building_utils:audit_object_category(o), vstr);
    else
      vstr = "";
    endif
    if ($quota_utils.byte_based)
      vstr = tostr(this:size_string(`o.object_size[1] ! ANY => 0'), " ", vstr);
      name_field_len = 26;
    else
      name_field_len = 30;
    endif
    if (valid(o.location))
      loc = (o.location.owner == o.owner ? " " | "*") + "[" + o.location.name + "]";
    elseif ($object_utils:has_property(o, "dest") && $object_utils:has_property(o, "source"))
      if (typeof(o.source) != OBJ)
        source = " <non-object> ";
      elseif (!valid(o.source))
        source = "<invalid>";
      else
        source = o.source.name;
        if (o.source.owner != o.owner)
          source = "*" + source;
        endif
      endif
      if (typeof(o.dest) != OBJ)
        destin = " <non-object> ";
      elseif (!valid(o.dest))
        destin = "<invalid>";
      else
        destin = o.dest.name;
        if (o.dest.owner != o.owner)
          destin = "*" + destin;
        endif
      endif
      srclen = min(length(source), 19);
      destlen = min(length(destin), 19);
      loc = " " + source[1..srclen] + "->" + destin[1..destlen];
    elseif ($object_utils:isa(o, $room))
      loc = "";
      try
        for x in (o.entrances)
          if (typeof(x) == OBJ && valid(x) && x.owner != o.owner && $object_utils:has_property(x, "dest") && x.dest == o)
            loc = loc + (loc ? ", " | "") + "<-*" + x.name;
          endif
        endfor
      except (ANY)
        if ($perm_utils:controls(player, o))
          loc = " BROKEN PROPERTY: .entrances";
        endif
      endtry
    else
      loc = " [Nowhere]";
    endif
    if (length(loc) > 41)
      loc = loc[1..37] + "..]";
    endif
    namelen = min(length(o.name), name_field_len - 1);
    return tostr(vstr, $string_utils:right(o, olen), " ", $string_utils:left((o.name)[1..namelen], name_field_len), loc);
  endverb

  verb "do_audit do_prospectus" (this none this) owner: #2 flags: "rxd"
    ":do_audit(who, start, end, match)";
    "audit who, with objects from start to end that match 'match'";
    ":do_prospectus(...)";
    "same, but with verb counts";
    {who, start, end, match} = args;
    pros = verb == "do_prospectus";
    "the set_task_perms is to make the task owned by the player. There are no other security aspects";
    set_task_perms(caller_perms());
    if (start == 0 && end == toint(max_object()) && !match && typeof(who.owned_objects) == LIST && length(who.owned_objects) > 100 && !$command_utils:yes_or_no(tostr(who.name, " has ", length(who.owned_objects), " objects.  This will be a very long list.  Do you wish to proceed?")))
      v = pros ? "@prospectus" | "@audit";
      return player:tell(v, " aborted.  Usage:  ", v, " [player] [from <start>] [to <end>] [for <match>]");
    endif
    player:tell(tostr("Objects owned by ", who.name, " (from #", start, " to #", end, match ? " matching " + match | "", ")", ":"));
    count = bytes = 0;
    if (typeof(who.owned_objects) == LIST)
      for o in (who.owned_objects)
        $command_utils:suspend_if_needed(0);
        if (!player:is_listening())
          return;
        endif
        if (toint(o) >= start && toint(o) <= end)
          didit = this:do_audit_item(o, match, pros);
          count = count + didit;
          if (didit && $quota_utils.byte_based && $object_utils:has_property(o, "object_size"))
            bytes = bytes + o.object_size[1];
          endif
        endif
      endfor
    else
      for i in [start..end]
        $command_utils:suspend_if_needed(0);
        o = toobj(i);
        if ($recycler:valid(o) && o.owner == who)
          didit = this:do_audit_item(o, match, pros);
          count = count + didit;
          if (didit && $quota_utils.byte_based && $object_utils:has_property(o, "object_size"))
            bytes = bytes + o.object_size[1];
          endif
        endif
      endfor
    endif
    player:tell($string_utils:left(tostr("-- ", count, " object", count == 1 ? "." | "s.", $quota_utils.byte_based ? tostr("  Total bytes: ", $string_utils:group_number(bytes), ".") | ""), player:linelen() - 1, "-"));
  endverb

  verb do_audit_item (this none this) owner: #2 flags: "rxd"
    ":do_audit_item(object, match-name-string, prospectus-flag)";
    {o, match, pros} = args;
    found = match ? 0 | 1;
    names = `{o.name, @o.aliases} ! ANY => {o.name}';
    "Above to get rid of screwed up aliases";
    while (names && !found)
      if (index(names[1], match) == 1)
        found = 1;
      endif
      names = listdelete(names, 1);
    endwhile
    if (found)
      "From Dred---don't wrap long lines.";
      line = $building_utils:object_audit_string(o, pros);
      player:tell(line[1..min($, player:linelen())]);
      return 1;
    endif
    return 0;
  endverb

  verb size_string (this none this) owner: #2 flags: "rxd"
    "Copied from Roebare (#109000):size_string at Sat Nov 26 18:41:12 2005 PST";
    size = args[1];
    if (typeof(size) != INT)
      return E_INVARG;
    endif
    if (`!player:build_option("audit_float") ! ANY')
      "...use integers to determine a four-char string...";
      factor = 1000;
      threshold = {{1000, "B"}, {1000000, "K"}, {1000000000, "M"}};
      if (!size)
        return " ???";
      elseif (size < 0 || size > threshold[$][1])
        if (size < 0 || size > $maxint)
          return " >2G";
        else
          "...floats still required to factor over $maxint...";
          return tostr($string_utils:right(floatstr(tofloat(size) / 1000000000.0, 0), 3), "G");
        endif
      elseif (size < threshold[1][1] && `!player:build_option("audit_bytes") ! ANY')
        return " <1K";
      endif
      for entry in ($list_utils:slice(threshold, 1))
        $command_utils:suspend_if_needed(0);
        i = $list_utils:iassoc(entry, threshold);
        if (size == entry)
          size = "1";
          try
            unit = threshold[i + 1][2];
          except error (E_RANGE)
            unit = "G";
          endtry
          break;
        elseif (size < entry)
          size = tostr(size / (entry / factor));
          unit = threshold[i][2];
          break;
        endif
      endfor
      return tostr($string_utils:right(size, 3), unit);
    else
      "...use floats to determine a six-char string...";
      size = tofloat(size);
      factor = 1024.0;
      "...be precise, `((1024.00 * 1024.00) * 1024.00) * 1024.00'...";
      threshold = {{1048576.0, "K"}, {1073741824.0, "M"}, {1099511627776.0, "G"}};
      if (!size)
        return "   ???";
      elseif (size < 0.0 || size > threshold[$][1])
        "...special handling for bad conversions & big numbers...";
        if (size < 0.0 || size > tofloat($maxint))
          return "   >2G";
        else
          return tostr($string_utils:right(floatstr(size / 1000000000.0, 1), 3), "G");
        endif
      endif
      for entry in ($list_utils:slice(threshold, 1))
        $command_utils:suspend_if_needed(0);
        i = $list_utils:iassoc(entry, threshold);
        if (size == entry)
          size = "1";
          try
            unit = threshold[i + 1][2];
          except error (E_RANGE)
            "...in another decade, maybe...";
            unit = "T";
          endtry
          break;
        elseif (size < entry)
          size = floatstr(size / (entry / factor), 1);
          unit = threshold[i][2];
          break;
        endif
      endfor
      return tostr($string_utils:right(size, 5), unit);
    endif
    "Rewritten by Roebare (#109000), 051119-26";
    "With inspiration from Miral (#107983) and assistance from Diopter (#98842)";
    "Byte & float display optional, per Nosredna (#2487), 051120-24";
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.classes = {$player, $room, $exit, $note, $container, $thing, $feature, $mail_recipient, $generic_help, $generic_db, $generic_utils, $generic_options};
    endif
  endverb
endobject