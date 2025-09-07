object MATCH_UTILS
  name: "matching utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property matching_room (owner: HACKER, flags: "r") = LOCAL;
  property ordinal_regexp (owner: HACKER, flags: "rc") = "%<%(first%|second%|third%|fourth%|fifth%|sixth%|seventh%|eighth%|ninth%|tenth%|1st%|2nd%|3rd%|4th%|5th%|6th%|7th%|8th%|9th%|10th%)%>";
  property ordn (owner: HACKER, flags: "rc") = {
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth"
  };
  property ordw (owner: HACKER, flags: "rc") = {"1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "9th", "10th"};

  override aliases = {"matching utilities"};
  override help_msg = {
    "$match_utils defines the following verbs:",
    "",
    "match",
    "match_nth",
    "match_verb",
    "match_list",
    "parse_ordinal_reference (alias parse_ordref)",
    "parse_possessive_reference",
    "object_match_failed",
    "",
    "For more documentation, see help $match_utils:<specific verb>."
  };
  override object_size = {9401, 1084848672};

  verb match (this none this) owner: HACKER flags: "rxd"
    ":match(string, object-list)";
    "Return object in 'object-list' aliased to 'string'.";
    "Matches on a wide variety of syntax, including:";
    " \"5th axe\" -- The fifth object matching \"axe\" in the object list.";
    " \"where's sai\" -- The only object contained in 'where' matching \"sai\" (possible $ambiguous_match).";
    " \"where's second staff\" -- The second object contained in 'where' matching \"staff\".";
    " \"my third dagger\" -- The third object in your inventory matching \"dagger\".";
    "Ordinal matches are determined according to the match's position in 'object-list' or, if a possessive (such as \"where\" above) is given, then the ordinal is the nth match in that object's inventory.";
    "In the matching room (#3879@LambdaMOO), the 'object-list' consists of first the player's contents, then the room's, and finally all exits leading from the room.";
    {string, olist} = args;
    if (!string)
      return $nothing;
    elseif (string == "me")
      return player;
    elseif (string == "here")
      return player.location;
    elseif (valid(object = $string_utils:literal_object(string)))
      return object;
    elseif (valid(object = $string_utils:match(string, olist, "aliases")))
      return object;
    elseif (parsed = this:parse_ordinal_reference(string))
      return this:match_nth(parsed[2], olist, parsed[1]);
    elseif (parsed = this:parse_possessive_reference(string))
      {whostr, objstr} = parsed;
      if (valid(whose = this:match(whostr, olist)))
        return this:match(objstr, whose.contents);
      else
        return whose;
      endif
    else
      return object;
    endif
    "Profane (#30788) - Sat Jan  3, 1998 - Changed so literals get returned ONLY if in the passed object list.";
    "Profane (#30788) - Sat Jan  3, 1998 - OK, that broke lots of stuff, so changed it back.";
  endverb

  verb match_nth (this none this) owner: HACKER flags: "rxd"
    ":match_nth(string, objlist, n)";
    "Find the nth object in 'objlist' that matches 'string'.";
    {what, where, n} = args;
    for v in (where)
      z = 0;
      for q in (v.aliases)
        z = z || index(q, what) == 1;
      endfor
      if (z && !(n = n - 1))
        return v;
      endif
    endfor
    return $failed_match;
  endverb

  verb match_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$match_utils:match_verb(verbname, object) => Looks for a command-line style verb named <verbname> on <object> with current values of prepstr, dobjstr, dobj, iobjstr, and iobj.  If a match is made, the verb is called with @args[3] as arguments and 1 is returned.  Otherwise, 0 is returned.";
    {vrb, what, rest} = args;
    if (where = $object_utils:has_verb(what, vrb))
      if ((vargs = verb_args(where[1], vrb)) != {"this", "none", "this"})
        if ((vargs[2] == "any" || !prepstr && vargs[2] == "none" || index("/" + vargs[2] + "/", "/" + prepstr + "/")) && (vargs[1] == "any" || !dobjstr && vargs[1] == "none" || dobj == what && vargs[1] == "this") && (vargs[3] == "any" || !iobjstr && vargs[3] == "none" || iobj == what && vargs[3] == "this") && index(verb_info(where[1], vrb)[2], "x") && verb_code(where[1], vrb))
          set_task_perms(caller_perms());
          what:(vrb)(@rest);
          return 1;
        endif
      endif
    endif
  endverb

  verb match_list (this none this) owner: HACKER flags: "rxd"
    ":match_list(string, object_list) -> List of all matches.";
    {what, where} = args;
    if (!what)
      return {};
    endif
    r = {};
    for v in (where)
      if (!(v in r))
        z = 0;
        for q in (v.aliases)
          z = z || q && index(q, what) == 1;
        endfor
        if (z)
          "r = listappend(r, v);";
          r = {@r, v};
        endif
      endif
    endfor
    return r;
    "Hydros (#106189) - Sun Jul 3, 2005 - Changed listappend to a splice to save ticks. Old code commented above.";
  endverb

  verb "parse_ordinal_reference parse_ordref" (this none this) owner: HACKER flags: "rxd"
    ":parse_ordref(string)";
    "Parses strings referring to an 'nth' object.";
    "=> {INT n, STR object} Where 'n' is the number the ordinal represents, and 'object' is the rest of the string.";
    "=> 0 If the given string is not an ordinal reference.";
    "  Example:";
    ":parse_ordref(\"second broadsword\") => {2, \"broadsword\"}";
    ":parse_ordref(\"second\") => 0";
    "  Note that there must be more to the string than the ordinal alone.";
    if (m = match(args[1], "^" + this.ordinal_regexp + " +%([^ ].+%)$"))
      o = substitute("%1", m);
      n = o in this.ordn || o in this.ordw;
      return n && {n, substitute("%2", m)};
    else
      return 0;
    endif
  endverb

  verb parse_possessive_reference (this none this) owner: HACKER flags: "rxd"
    ":parse_possessive_reference(string)";
    "Parses strings in a possessive format.";
    "=> {STR whose, STR object}  Where 'whose' is the possessor of 'object'.";
    "If the string consists only of a possessive string (ie: \"my\", or \"yduJ's\"), then 'object' will be an empty string.";
    "=> 0 If the given string is not a possessive reference.";
    "  Example:";
    ":parse_possessive_reference(\"joe's cat\") => {\"joe\", \"cat\"}";
    ":parse_possessive_reference(\"sis' fish\") => {\"sis\", \"fish\"}";
    "  Strings are returned as a value suitable for a :match routine, thus 'my' becoming 'me'.";
    ":parse_possessive_reference(\"my dog\") => {\"me\", \"dog\"}";
    string = args[1];
    if (m = match(string, "^my$%|^my +%(.+%)?"))
      return {"me", substitute("%1", m)};
    elseif (m = match(string, "^%(.+s?%)'s? *%(.+%)?"))
      return {substitute("%1", m), substitute("%2", m)};
    else
      return 0;
    endif
    "Profane (#30788) - Sun Jun 21, 1998 - changed first parenthetical match bit from %([^ ]+s?%) to %(.+s?%)";
  endverb

  verb object_match_failed (this none this) owner: HACKER flags: "rx"
    "Usage: object_match_failed(object, string[, ambigs])";
    "Prints a message if string does not match object.  Generally used after object is derived from a :match_object(string).";
    "ambigs is an optional list of the objects that were matched upon.  If given, the message printed will list the ambiguous among them as choices.";
    {match_result, string, ?ambigs = 0} = args;
    tell = 0 && $perm_utils:controls(caller_perms(), player) ? "notify" | "tell";
    if (index(string, "#") == 1 && $code_utils:toobj(string) != E_TYPE)
      "...avoid the `I don't know which `#-2' you mean' message...";
      if (!valid(match_result))
        player:(tell)(tostr("There is no \"", string, "\" that you can see."));
      endif
      return !valid(match_result);
    elseif (match_result == $nothing)
      player:(tell)("You must give the name of some object.");
    elseif (match_result == $failed_match)
      player:(tell)(tostr("There is no \"", string, "\" that you can see."));
    elseif (match_result == $ambiguous_match)
      if (typeof(ambigs) != LIST)
        player:(tell)(tostr("I don't know which \"", string, "\" you mean."));
        return 1;
      endif
      ambigs = $match_utils:match_list(string, ambigs);
      ambigs = $list_utils:map_property(ambigs, "name");
      if (length($list_utils:remove_duplicates(ambigs)) == 1 && $object_utils:isa(player.location, this.matching_room))
        player:(tell)(tostr("I don't know which \"", string, "\" you mean.  Try using \"first ", string, "\", \"second ", string, "\", etc."));
      else
        player:(tell)(tostr("I don't know which \"", string, "\" you mean: ", $string_utils:english_list(ambigs, "nothing", " or "), "."));
      endif
      return 1;
    elseif (!valid(match_result))
      player:(tell)(tostr("The object you specified does not exist.  Seeing ghosts?"));
    else
      return 0;
    endif
    return 1;
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.matching_room = $nothing;
    endif
  endverb
endobject