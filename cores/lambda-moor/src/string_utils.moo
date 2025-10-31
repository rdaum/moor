object STRING_UTILS
  name: "string utilities"
  parent: GENERIC_UTILS
  owner: #2
  readable: true

  property alphabet (owner: #2, flags: "rc") = "abcdefghijklmnopqrstuvwxyz";
  property ascii (owner: #2, flags: "rc") = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
  property digits (owner: #2, flags: "rc") = "0123456789";
  property tab (owner: #2, flags: "rc") = "\t";
  property use_article_a (owner: HACKER, flags: "r") = {"unit", "unix", "one", "once", "utility"};
  property use_article_an (owner: HACKER, flags: "r") = {};

  override aliases = {"string", "utils"};
  override description = {
    "This is the string utilities utility package.  See `help $string_utils' for more details."
  };
  override help_msg = {
    "For a complete description of a given verb, do `help $string_utils:verbname'",
    "",
    "    Conversion routines:",
    "",
    ":from_list    (list [,sep])                          => \"foo1foo2foo3\"",
    ":english_list (str-list[,none-str[,and-str[, sep]]]) => \"foo1, foo2, and foo3\"",
    ":title_list*c (obj-list[,none-str[,and-str[, sep]]]) => \"foo1, foo2, and foo3\"",
    "                                                  or => \"Foo1, foo2, and foo3\"",
    ":from_value   (value [,quoteflag [,maxlistdepth]])   => \"{foo1, foo2, foo3}\"",
    ":print        (value)                                => value in string",
    ":abbreviated_value (value, options)                  => short value in string",
    "",
    ":to_value       (string)     => {success?, value or error message}",
    ":prefix_to_value(string)     => {rest of string, value} or {0, error message}",
    "",
    ":english_number(42)          => \"forty-two\"",
    ":english_ordinal(42)         => \"forty-second\"",
    ":ordinal(42)                 => \"42nd\"",
    ":group_number(42135 [,sep])  => \"42,135\"",
    ":from_ASCII(65)              => \"A\"",
    ":to_ASCII(\"A\")               => 65",
    ":from_seconds(number)        => string of rough time passed in large increments",
    "",
    ":name_and_number(obj [,sep]) => \"ObjectName (#obj)\"",
    ":name_and_number_list({obj1,obj2} [,sep])",
    "                             => \"ObjectName1 (#obj1) and ObjectName2 (#obj2)\"",
    ":nn is an alias for :name_and_number.",
    ":nn_list is an alias for :name_and_number_list.",
    "",
    "    Type checking:",
    "",
    ":is_integer   (string) => return true if string is composed entirely of digits",
    ":is_float     (string) => return true if string holds just a floating point",
    "",
    "    Parsing:",
    "",
    ":explode (string,char) -- string => list of words delimited by char",
    ":words   (string)      -- string => list of words (as with command line parser)",
    ":word_start (string)   -- string => list of start-end pairs.",
    ":first_word (string)   -- string => list {first word, rest of string} or {}",
    ":char_list  (string)   -- string => list of characters in string",
    "",
    ":parse_command (cmd_line [,player] => mimics action of builtin parser",
    "",
    "    Matching:",
    "",
    ":find_prefix  (prefix, string-list)=>list index of element starting with prefix",
    ":index_delimited(string,target[,case]) =>index of delimited string occurrence",
    ":index_all    (string, target string)          => list of all matched positions",
    ":common       (first string, second string)  => length of longest common prefix",
    ":match        (string, [obj-list, prop-name]+) => matching object",
    ":match_player (string-list[,me-object])        => list of matching players",
    ":match_object (string, location)               => default object match...",
    ":match_player_or_object (string, location) => object then player matching",
    ":literal_object (string)                       => match against #xxx, $foo",
    ":match_stringlist (string, targets)            => match against static strings",
    ":match_string (string, wildcard target,options)=> match against a wildcard",
    "",
    "    Pretty printing:",
    "",
    ":space         (n/string[,filler])     => n spaces",
    ":left          (string,width[,filler]) => left justified string in field ",
    ":right         (string,width[,filler]) => right justified string in field",
    ":center/re     (string,width[,lfiller[,rfiller]]) => centered string in field",
    ":columnize/se  (list,n[,width])        => list of strings in n columns",
    "",
    "    Substitutions",
    "",
    ":substitute (string,subst_list [,case])   -- general substitutions.",
    ":substitute_delimited (string,subst_list [,case])",
    "                                          -- like subst, but uses index_delim",
    ":pronoun_sub (string/list[,who[,thing[,location]]])",
    "                                          -- pronoun substitutions.",
    ":pronoun_sub_secure (string[,who[,thing[,location]]],default)",
    "                                          -- substitute and check for names.",
    ":pronoun_quote (string/list/subst_list)   -- quoting for pronoun substitutions.",
    "",
    "    Miscellaneous string munging:",
    "",
    ":trim         (string)       => string with outside whitespace removed.",
    ":triml        (string)       => string with leading whitespace removed.",
    ":trimr        (string)       => string with trailing whitespace removed.",
    ":strip_chars  (string,chars) => string with all chars in `chars' removed.",
    ":strip_all_but(string,chars) => string with all chars not in `chars' removed.",
    ":capitalize/se(string)       => string with first letter capitalized.",
    ":uppercase/lowercase(string) => string with all letters upper or lowercase.",
    ":names_of     (list of OBJ)  => string with names and object numbers of items.",
    ":a_or_an      (word)         => \"a\" or \"an\" as appropriate for that word.",
    ":reverse      (string)       => \"gnirts\"",
    ":incr_alpha   (string)       => \"increments\" the string alphabetically",
    "",
    "    A useful property:",
    "",
    ".alphabet                    => \"abcdefghijklmnopqrstuvwxyz\"",
    "",
    "Suspended versions (with _suspended at end of name) for",
    "     :print     :from_value     :columnize/se      :match"
  };
  override import_export_id = "string_utils";
  override object_size = {76712, 1084848672};

  verb space (this none this) owner: HACKER flags: "rxd"
    "space(len,fill) returns a string of length abs(len) consisting of copies of fill.  If len is negative, fill is anchored on the right instead of the left.";
    {n, ?fill = " "} = args;
    if (typeof(n) == STR)
      n = length(n);
    endif
    if (n > 1000)
      "Prevent someone from crashing the moo with $string_utils:space($maxint)";
      return E_INVARG;
    endif
    if (" " != fill)
      fill = fill + fill;
      fill = fill + fill;
      fill = fill + fill;
    elseif ((n = abs(n)) < 70)
      return "                                                                      "[1..n];
    else
      fill = "                                                                      ";
    endif
    m = (n - 1) / length(fill);
    while (m)
      fill = fill + fill;
      m = m / 2;
    endwhile
    return n > 0 ? fill[1..n] | fill[$ + 1 + n..$];
  endverb

  verb left (this none this) owner: HACKER flags: "rxd"
    "$string_utils:left(string,width[,filler])";
    "";
    "Assures that <string> is at least <width> characters wide.  Returns <string> if it is at least that long, or else <string> followed by enough filler to make it that wide. If <width> is negative and the length of <string> is greater than the absolute value of <width>, then the <string> is cut off at <width>.";
    "";
    "The <filler> is optional and defaults to \" \"; it controls what is used to fill the resulting string when it is too short.  The <filler> is replicated as many times as is necessary to fill the space in question.";
    {text, len, ?fill = " "} = args;
    abslen = abs(len);
    out = tostr(text);
    if (length(out) < abslen)
      return out + this:space(length(out) - abslen, fill);
    else
      return len > 0 ? out | out[1..abslen];
    endif
  endverb

  verb right (this none this) owner: HACKER flags: "rxd"
    "$string_utils:right(string,width[,filler])";
    "";
    "Assures that <string> is at least <width> characters wide.  Returns <string> if it is at least that long, or else <string> preceded by enough filler to make it that wide. If <width> is negative and the length of <string> is greater than the absolute value of <width>, then <string> is cut off at <width> from the right.";
    "";
    "The <filler> is optional and defaults to \" \"; it controls what is used to fill the resulting string when it is too short.  The <filler> is replicated as many times as is necessary to fill the space in question.";
    {text, len, ?fill = " "} = args;
    abslen = abs(len);
    out = tostr(text);
    if ((lenout = length(out)) < abslen)
      return this:space(abslen - lenout, fill) + out;
    else
      return len > 0 ? out | out[$ - abslen + 1..$];
    endif
  endverb

  verb "centre center" (this none this) owner: HACKER flags: "rxd"
    "$string_utils:center(string,width[,lfiller[,rfiller]])";
    "";
    "Assures that <string> is at least <width> characters wide.  Returns <string> if it is at least that long, or else <string> preceded and followed by enough filler to make it that wide.  If <width> is negative and the length of <string> is greater than the absolute value of <width>, then the <string> is cut off at <width>.";
    "";
    "The <lfiller> is optional and defaults to \" \"; it controls what is used to fill the left part of the resulting string when it is too short.  The <rfiller> is optional and defaults to the value of <lfiller>; it controls what is used to fill the right part of the resulting string when it is too short.  In both cases, the filler is replicated as many times as is necessary to fill the space in question.";
    {text, len, ?lfill = " ", ?rfill = lfill} = args;
    out = tostr(text);
    abslen = abs(len);
    if (length(out) < abslen)
      return this:space((abslen - length(out)) / 2, lfill) + out + this:space((abslen - length(out) + 1) / -2, rfill);
    else
      return len > 0 ? out | out[1..abslen];
    endif
  endverb

  verb "columnize columnise" (this none this) owner: HACKER flags: "rxd"
    "columnize (items, n [, width]) - Turn a one-column list of items into an n-column list. 'width' is the last character position that may be occupied; it defaults to a standard screen width. Example: To tell the player a list of numbers in three columns, do 'player:tell_lines ($string_utils:columnize ({1, 2, 3, 4, 5, 6, 7}, 3));'.";
    {items, n, ?width = 79} = args;
    height = (length(items) + n - 1) / n;
    items = {@items, @$list_utils:make(height * n - length(items), "")};
    colwidths = {};
    for col in [1..n - 1]
      colwidths = listappend(colwidths, 1 - (width + 1) * col / n);
    endfor
    result = {};
    for row in [1..height]
      line = tostr(items[row]);
      for col in [1..n - 1]
        line = tostr(this:left(line, colwidths[col]), " ", items[row + col * height]);
      endfor
      result = listappend(result, line[1..min($, width)]);
    endfor
    return result;
  endverb

  verb from_list (this none this) owner: HACKER flags: "rxd"
    "$string_utils:from_list(list [, separator])";
    "Return a string being the concatenation of the string representations of the elements of LIST, each pair separated by the string SEPARATOR, which defaults to the empty string.";
    {thelist, ?separator = ""} = args;
    if (separator == "")
      return tostr(@thelist);
    elseif (thelist)
      result = tostr(thelist[1]);
      for elt in (listdelete(thelist, 1))
        result = tostr(result, separator, elt);
      endfor
      return result;
    else
      return "";
    endif
  endverb

  verb english_list (this none this) owner: HACKER flags: "rxd"
    "Prints the argument (must be a list) as an english list, e.g. {1, 2, 3} is printed as \"1, 2, and 3\", and {1, 2} is printed as \"1 and 2\".";
    "Optional arguments are treated as follows:";
    "  Second argument is the string to use when the empty list is given.  The default is \"nothing\".";
    "  Third argument is the string to use in place of \" and \".  A typical application might be to use \" or \" instead.";
    "  Fourth argument is the string to use instead of a comma (and space).  Gary_Severn's deranged mind actually came up with an application for this.  You can ask him.";
    "  Fifth argument is a string to use after the penultimate element before the \" and \".  The default is to have a comma without a space.";
    {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
    nthings = length(things);
    if (nthings == 0)
      return nothingstr;
    elseif (nthings == 1)
      return tostr(things[1]);
    elseif (nthings == 2)
      return tostr(things[1], andstr, things[2]);
    else
      ret = "";
      for k in [1..nthings - 1]
        if (k == nthings - 1)
          commastr = finalcommastr;
        endif
        ret = tostr(ret, things[k], commastr);
      endfor
      return tostr(ret, andstr, things[nthings]);
    endif
  endverb

  verb names_of (this none this) owner: HACKER flags: "rxd"
    "Return a string of the names and object numbers of the objects in a list.";
    line = "";
    for item in (args[1])
      if (typeof(item) == OBJ && valid(item))
        line = line + item.name + "(" + tostr(item) + ")   ";
      endif
    endfor
    return $string_utils:trimr(line);
  endverb

  verb from_seconds (this none this) owner: HACKER flags: "rxd"
    ":from_seconds(number of seconds) => returns a string containing the rough increment of days, or hours if less than a day, or minutes if less than an hour, or lastly in seconds.";
    ":from_seconds(86400) => \"a day\"";
    ":from_seconds(7200)  => \"two hours\"";
    minute = 60;
    hour = 60 * minute;
    day = 24 * hour;
    secs = args[1];
    if (secs > day)
      count = secs / day;
      unit = "day";
      article = "a";
    elseif (secs > hour)
      count = secs / hour;
      unit = "hour";
      article = "an";
    elseif (secs > minute)
      count = secs / minute;
      unit = "minute";
      article = "a";
    else
      count = secs;
      unit = "second";
      article = "a";
    endif
    if (count == 1)
      time = tostr(article, " ", unit);
    else
      time = tostr(count, " ", unit, "s");
    endif
    return time;
  endverb

  verb trim (this none this) owner: HACKER flags: "rxd"
    ":trim (string [, space]) -- remove leading and trailing spaces";
    "";
    "`space' should be a character (single-character string); it defaults to \" \".  Returns a copy of string with all leading and trailing copies of that character removed.  For example, $string_utils:trim(\"***foo***\", \"*\") => \"foo\".";
    {string, ?space = " "} = args;
    m = match(string, tostr("[^", space, "]%(.*[^", space, "]%)?%|$"));
    return string[m[1]..m[2]];
  endverb

  verb triml (this none this) owner: HACKER flags: "rxd"
    ":triml(string [, space]) -- remove leading spaces";
    "";
    "`space' should be a character (single-character string); it defaults to \" \".  Returns a copy of string with all leading copies of that character removed.  For example, $string_utils:triml(\"***foo***\", \"*\") => \"foo***\".";
    {string, ?what = " "} = args;
    m = match(string, tostr("[^", what, "]%|$"));
    return string[m[1]..$];
  endverb

  verb trimr (this none this) owner: HACKER flags: "rxd"
    ":trimr(string [, space]) -- remove trailing spaces";
    "";
    "`space' should be a character (single-character string); it defaults to \" \".  Returns a copy of string with all trailing copies of that character removed.  For example, $string_utils:trimr(\"***foo***\", \"*\") => \"***foo\".";
    {string, ?what = " "} = args;
    return string[1..rmatch(string, tostr("[^", what, "]%|^"))[2]];
  endverb

  verb strip_chars (this none this) owner: HACKER flags: "rxd"
    ":strip_chars(string,chars) => string with chars removed";
    {subject, stripped} = args;
    for i in [1..length(stripped)]
      subject = strsub(subject, stripped[i], "");
    endfor
    return subject;
  endverb

  verb strip_all_but (this none this) owner: HACKER flags: "rxd"
    ":strip_all_but(string,keep) => string with chars not in `keep' removed.";
    "`keep' is used in match() so if it includes ], ^, or -,";
    "] should be first, ^ should be other from first, and - should be last.";
    string = args[1];
    wanted = "[" + args[2] + "]+";
    output = "";
    while (m = match(string, wanted))
      output = output + string[m[1]..m[2]];
      string = string[m[2] + 1..$];
    endwhile
    return output;
  endverb

  verb "uppercase lowercase" (this none this) owner: HACKER flags: "rxd"
    "lowercase(string) -- returns a lowercase version of the string.";
    "uppercase(string) -- returns the uppercase version of the string.";
    string = args[1];
    from = caps = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    to = lower = "abcdefghijklmnopqrstuvwxyz";
    if (verb == "uppercase")
      from = lower;
      to = caps;
    endif
    for i in [1..26]
      string = strsub(string, from[i], to[i], 1);
    endfor
    return string;
  endverb

  verb "capitalize capitalise" (this none this) owner: HACKER flags: "rxd"
    "capitalizes its argument.";
    if ((string = args[1]) && (i = index("abcdefghijklmnopqrstuvwxyz", string[1], 1)))
      string[1] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[i];
    endif
    return string;
  endverb

  verb literal_object (this none this) owner: HACKER flags: "rxd"
    "Matches args[1] against literal objects: #xxxxx, $variables, *mailing-lists, and username.  Returns the object if successful, $failed_match else.";
    string = args[1];
    if (!string)
      return $nothing;
    elseif (string[1] == "#" && E_TYPE != (object = $code_utils:toobj(string)))
      return object;
    elseif (string[1] == "~")
      return this:match_player(string[2..$], #0);
    elseif (string[1] == "*" && length(string) > 1)
      return $mail_agent:match_recipient(string);
    elseif (string[1] == "$")
      string[1..1] = "";
      object = #0;
      while (pn = string[1..(dot = index(string, ".")) ? dot - 1 | $])
        if (!$object_utils:has_property(object, pn) || typeof(object = object.(pn)) != OBJ)
          return $failed_match;
        endif
        string = string[length(pn) + 2..$];
      endwhile
      if (object == #0 || typeof(object) == ERR)
        return $failed_match;
      else
        return object;
      endif
    else
      return $failed_match;
    endif
  endverb

  verb match (this none this) owner: HACKER flags: "rxd"
    "$string_utils:match(string [, obj-list, prop-name]*)";
    "Each obj-list should be a list of objects or a single object, which is treated as if it were a list of that object.  Each prop-name should be string naming a property on every object in the corresponding obj-list.  The value of that property in each case should be either a string or a list of strings.";
    "The argument string is matched against all of the strings in the property values.";
    "If it exactly matches exactly one of them, the object containing that property is returned.  If it exactly matches more than one of them, $ambiguous_match is returned.";
    "If there are no exact matches, then partial matches are considered, ones in which the given string is a prefix of some property string.  Again, if exactly one match is found, the object with that property is returned, and if there is more than one match, $ambiguous_match is returned.";
    "Finally, if there are no exact or partial matches, then $failed_match is returned.";
    subject = args[1];
    if (subject == "")
      return $nothing;
    endif
    no_exact_match = no_partial_match = 1;
    for i in [1..length(args) / 2]
      prop_name = args[2 * i + 1];
      for object in (typeof(olist = args[2 * i]) == LIST ? olist | {olist})
        if (valid(object))
          if (typeof(str_list = `object.(prop_name) ! E_PERM, E_PROPNF => {}') != LIST)
            str_list = {str_list};
          endif
          if (subject in str_list)
            if (no_exact_match)
              no_exact_match = object;
            elseif (no_exact_match != object)
              return $ambiguous_match;
            endif
          else
            for string in (str_list)
              if (index(string, subject) != 1)
              elseif (no_partial_match)
                no_partial_match = object;
              elseif (no_partial_match != object)
                no_partial_match = $ambiguous_match;
              endif
            endfor
          endif
        endif
      endfor
    endfor
    return no_exact_match && (no_partial_match && $failed_match);
  endverb

  verb "match_str*ing" (this none this) owner: HACKER flags: "rxd"
    "* wildcard matching. Returns a list of what the *s actually matched. Won't cath every match, if there are several ways to parse it.";
    "Example: $string_utils:match_string(\"Jack waves to Jill\",\"* waves to *\") returns {\"Jack\", \"Jill\"}";
    "Optional arguments: numbers are interpreted as case-sensitivity, strings as alternative wildcards.";
    {what, targ, @rest} = args;
    wild = "*";
    case = ret = {};
    what = what + "&^%$";
    targ = targ + "&^%$";
    for y in (rest)
      if (typeof(y) == STR)
        wild = y;
      elseif (typeof(y) == INT)
        case = {y};
      endif
    endfor
    while (targ != "")
      if (z = index(targ, wild))
        part = targ[1..z - 1];
      else
        z = length(targ);
        part = targ;
      endif
      n = part == "" ? 1 | index(what, part, @case);
      if (n)
        ret = listappend(ret, what[1..n - 1]);
        what = what[z + n - 1..$];
        targ = targ[z + 1..$];
      else
        return 0;
      endif
    endwhile
    if (ret == {})
      return what == "";
    elseif (ret == {""})
      return 1;
    elseif (ret[1] == "")
      return ret[2..$];
    else
      return 0;
    endif
  endverb

  verb match_object (this none this) owner: HACKER flags: "rxd"
    ":match_object(string,location[,someone])";
    "Returns the object matching the given string for someone, on the assumption that s/he is in the given location.  `someone' defaults to player.";
    "This first tries :literal_object(string), \"me\"=>someone,\"here\"=>location, then player:match(string) and finally location:match(string) if location is valid.";
    "This is the default algorithm for use by room :match_object() and player :my_match_object() verbs.  Player verbs that are calling this directly should probably be calling :my_match_object instead.";
    {string, here, ?who = player} = args;
    if ($failed_match != (object = this:literal_object(string)))
      return object;
    elseif (string == "me")
      return who;
    elseif (string == "here")
      return here;
    elseif (valid(pobject = who:match(string)) && string in {@pobject.aliases, pobject.name} || !valid(here))
      "...exact match in player or room is bogus...";
      return pobject;
    elseif (valid(hobject = here:match(string)) && string in {@hobject.aliases, hobject.name} || pobject == $failed_match)
      "...exact match in room or match in player failed completely...";
      return hobject;
    else
      return pobject;
    endif
  endverb

  verb match_player (this none this) owner: HACKER flags: "rxd"
    "match_player(name,name,...)      => {obj,obj,...}";
    "match_player(name[,meobj])       => obj";
    "match_player({name,...}[,meobj]) => {obj,...}";
    "objs returned are either players, $failed_match, $ambiguous_match, or $nothing in the case of an empty string.";
    "meobj (what to return for instances of `me') defaults to player; if given and isn't actually a player, `me' => $failed_match";
    retstr = 0;
    me = player;
    if (length(args) < 2 || typeof(me = args[2]) == OBJ)
      me = valid(me) && is_player(me) ? me | $failed_match;
      if (typeof(args[1]) == STR)
        strings = {args[1]};
        retstr = 1;
        "return a string, not a list";
      else
        strings = args[1];
      endif
    else
      strings = args;
      me = player;
    endif
    found = {};
    for astr in (strings)
      if (!astr)
        aobj = $nothing;
      elseif (astr == "me")
        aobj = me;
      elseif (valid(aobj = $string_utils:literal_object(astr)) && is_player(aobj))
        "astr is a valid literal object number of some player, so we are done.";
      else
        aobj = $player_db:find(astr);
      endif
      found = {@found, aobj};
    endfor
    return retstr ? found[1] | found;
  endverb

  verb match_player_or_object (this none this) owner: HACKER flags: "rxd"
    "Accepts any number of strings, attempts to match those strings first against objects in the room, and if no objects by those names exist, matches against player names (and \"#xxxx\" style strings regardless of location).  Returns a list of valid objects so found.";
    "Unlike $string_utils:match_player, does not include in the list the failed and ambiguous matches; instead has built-in error messages for such objects.  This should probably be improved.  Volunteers?";
    if (!args)
      return;
    endif
    unknowns = {};
    objs = {};
    "We have to do something icky here.  Parallel walk the victims and args lists.  When it's a valid object, then it's a player.  If it's an invalid object, try to get an object match from the room.  If *that* fails, complain.";
    for i in [1..length(args)]
      if (valid(o = player.location:match_object(args[i])))
        objs = {@objs, o};
      else
        unknowns = {@unknowns, args[i]};
      endif
    endfor
    victims = $string_utils:match_player(unknowns);
    for i in [1..length(victims)]
      if (!valid(victims[i]))
        player:tell("Could not find ", unknowns[i], " as either an object or a player.");
      else
        objs = {@objs, victims[i]};
      endif
    endfor
    return objs;
  endverb

  verb find_prefix (this none this) owner: HACKER flags: "rxd"
    "find_prefix(prefix, string-list) => list index of something starting with prefix, or 0 or $ambiguous_match.";
    {subject, choices} = args;
    answer = 0;
    for i in [1..length(choices)]
      if (index(choices[i], subject) == 1)
        if (answer == 0)
          answer = i;
        else
          answer = $ambiguous_match;
        endif
      endif
    endfor
    return answer;
  endverb

  verb "index_d*elimited" (this none this) owner: HACKER flags: "rxd"
    "index_delimited(string,target[,case_matters]) is just like the corresponding call to the builtin index() but instead only matches on occurences of target delimited by word boundaries (i.e., not preceded or followed by an alphanumeric)";
    args[2] = "%(%W%|^%)" + $string_utils:regexp_quote(args[2]) + "%(%W%|$%)";
    return (m = match(@args)) ? m[3][1][2] + 1 | 0;
  endverb

  verb "is_integer is_numeric" (this none this) owner: HACKER flags: "rxd"
    "Usage:  is_numeric(string)";
    "        is_integer(string)";
    "Is string numeric (composed of one or more digits possibly preceded by a minus sign)? This won't catch floating points.";
    "Return true or false";
    return match(args[1], "^ *[-+]?[0-9]+ *$");
    digits = "1234567890";
    if (!(string = args[1]))
      return 0;
    endif
    if (string[1] == "-")
      string = string[2..length(string)];
    endif
    for i in [1..length(string)]
      if (!index(digits, string[i]))
        return 0;
      endif
    endfor
    return 1;
  endverb

  verb ordinal (this none this) owner: HACKER flags: "rxd"
    ":short_ordinal(1) => \"1st\",:short_ordinal(2) => \"2nd\",etc...";
    string = tostr(n = args[1]);
    n = abs(n) % 100;
    if (n / 10 != 1 && n % 10 in {1, 2, 3})
      return string + {"st", "nd", "rd"}[n % 10];
    else
      return string + "th";
    endif
  endverb

  verb group_number (this none this) owner: HACKER flags: "rxd"
    "$string_utils:group_number(INT n [, sep_char])";
    "$string_utils:group_number(FLOAT n, [INT precision [, scientific [, sep_char]]])";
    "";
    "Converts N to a string, inserting commas (or copies of SEP_CHAR, if given) every three digits, counting from the right.  For example, $string_utils:group_number(1234567890) returns the string \"1,234,567,890\".";
    "For floats, the arguements precision (defaulting to 4 in this verb) and scientific are the same as given in floatstr().";
    if (typeof(args[1]) == INT)
      {n, ?comma = ","} = args;
      result = "";
      sign = n < 0 ? "-" | "";
      n = tostr(abs(n));
    elseif (typeof(args[1]) == FLOAT)
      {n, ?prec = 4, ?scien = 0, ?comma = ","} = args;
      sign = n < 0.0 ? "-" | "";
      n = floatstr(abs(n), prec, scien);
      i = index(n, ".");
      result = n[i..$];
      n = n[1..i - 1];
    else
      return E_INVARG;
    endif
    while ((len = length(n)) > 3)
      result = comma + n[len - 2..len] + result;
      n = n[1..len - 3];
    endwhile
    return sign + n + result;
    "Code contributed by SunRay";
  endverb

  verb english_number (this none this) owner: HACKER flags: "rxd"
    "$string_utils:english_number(n) -- convert the integer N into English";
    "";
    "Produces a string containing the English phrase naming the given integer.  For example, $string_utils:english_number(-1234) returns the string `negative one thousand two hundred thirty-four'.";
    numb = toint(args[1]);
    if (numb == 0)
      return "zero";
    endif
    labels = {"", " thousand", " million", " billion"};
    numstr = "";
    mod = abs(numb);
    for n in [1..4]
      div = mod % 1000;
      if (div)
        hun = div / 100;
        ten = div % 100;
        outstr = this:english_tens(ten) + labels[n];
        if (hun)
          outstr = this:english_ones(hun) + " hundred" + (ten ? " " | "") + outstr;
        endif
        if (numstr)
          numstr = outstr + " " + numstr;
        else
          numstr = outstr;
        endif
      endif
      mod = mod / 1000;
    endfor
    return (numb < 0 ? "negative " | "") + numstr;
  endverb

  verb english_ordinal (this none this) owner: HACKER flags: "rxd"
    "$string_utils:english_ordinal(n) -- convert the integer N into an english ordinal (1 => \"first\", etc...)";
    numb = toint(args[1]);
    if (numb == 0)
      return "zeroth";
    elseif (numb % 100)
      hundreds = abs(numb) > 100 ? this:english_number(numb / 100 * 100) + " " | (numb < 0 ? "negative " | "");
      numb = abs(numb) % 100;
      specials = {1, 2, 3, 5, 8, 9, 12, 20, 30, 40, 50, 60, 70, 80, 90};
      ordinals = {"first", "second", "third", "fifth", "eighth", "ninth", "twelfth", "twentieth", "thirtieth", "fortieth", "fiftieth", "sixtieth", "seventieth", "eightieth", "ninetieth"};
      if (i = numb in specials)
        return hundreds + ordinals[i];
      elseif (numb > 20 && (i = numb % 10 in specials))
        return hundreds + this:english_tens(numb / 10 * 10) + "-" + ordinals[i];
      else
        return hundreds + this:english_number(numb) + "th";
      endif
    else
      return this:english_number(numb) + "th";
    endif
  endverb

  verb english_ones (this none this) owner: HACKER flags: "rxd"
    numb = args[1];
    ones = {"", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"};
    return ones[numb + 1];
  endverb

  verb english_tens (this none this) owner: HACKER flags: "rxd"
    numb = args[1];
    teens = {"ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"};
    others = {"twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"};
    if (numb < 10)
      return this:english_ones(numb);
    elseif (numb < 20)
      return teens[numb - 9];
    else
      return others[numb / 10 - 1] + (numb % 10 ? "-" | "") + this:english_ones(numb % 10);
    endif
  endverb

  verb "subst*itute" (this none this) owner: HACKER flags: "rxd"
    "subst(string,{{redex1,repl1},{redex2,repl2},{redex3,repl3}...}[,case])";
    "  => returns string with all instances of the strings redex<n> replaced respectively by the strings repl<n>.  If the optional argument `case' is given and nonzero, the search for instances of redex<n> is case sensitive.";
    "  Substitutions are done in parallel, i.e., instances of redex<n> that appear in any of the replacement strings are ignored.  In the event that two redexes overlap, whichever is leftmost in `string' takes precedence.  For two redexes beginning at the same position, the longer one takes precedence.";
    "";
    "subst(\"hoahooaho\",{{\"ho\",\"XhooX\"},{\"hoo\",\"mama\"}}) => \"XhooXamamaaXhooX\"";
    "subst(\"Cc: banana\",{{\"a\",\"b\"},{\"b\",\"c\"},{\"c\",\"a\"}},1) => \"Ca: cbnbnb\"";
    {ostr, subs, ?case = 0} = args;
    if (typeof(ostr) != STR)
      return ostr;
    endif
    len = length(ostr);
    " - - - find the first instance of each substitution - -";
    indices = {};
    substs = {};
    for s in (subs)
      if (i = index(ostr, s[1], case))
        fi = $list_utils:find_insert(indices, i = i - len) - 1;
        while (fi && (indices[fi] == i && length(substs[fi][1]) < length(s[1])))
          "...give preference to longer redexes...";
          fi = fi - 1;
        endwhile
        indices = listappend(indices, i, fi);
        substs = listappend(substs, s, fi);
      endif
    endfor
    "- - - - - perform substitutions - ";
    nstr = "";
    while (substs)
      ind = len + indices[1];
      sub = substs[1];
      indices = listdelete(indices, 1);
      substs = listdelete(substs, 1);
      if (ind > 0)
        nstr = nstr + ostr[1..ind - 1] + sub[2];
        ostr = ostr[ind + length(sub[1])..len];
        len = length(ostr);
      endif
      if (next = index(ostr, sub[1], case))
        fi = $list_utils:find_insert(indices, next = next - len) - 1;
        while (fi && (indices[fi] == next && length(substs[fi][1]) < length(sub[1])))
          "...give preference to longer redexes...";
          fi = fi - 1;
        endwhile
        indices = listappend(indices, next, fi);
        substs = listappend(substs, sub, fi);
      endif
    endwhile
    return nstr + ostr;
  endverb

  verb "substitute_d*elimited" (none none none) owner: #2 flags: "rxd"
    "subst(string,{{redex1,repl1},{redex2,repl2},{redex3,repl3}...}[,case])";
    "Just like :substitute() but it uses index_delimited() instead of index()";
    {ostr, subs, ?case = 0} = args;
    if (typeof(ostr) != STR)
      return ostr;
    endif
    len = length(ostr);
    " - - - find the first instance of each substitution - -";
    indices = {};
    substs = {};
    for s in (subs)
      if (i = this:index_delimited(ostr, s[1], case))
        fi = $list_utils:find_insert(indices, i = i - len) - 1;
        while (fi && (indices[fi] == i && length(substs[fi][1]) < length(s[1])))
          "...give preference to longer redexes...";
          fi = fi - 1;
        endwhile
        indices = listappend(indices, i, fi);
        substs = listappend(substs, s, fi);
      endif
    endfor
    "- - - - - perform substitutions - ";
    nstr = "";
    while (substs)
      ind = len + indices[1];
      sub = substs[1];
      indices = listdelete(indices, 1);
      substs = listdelete(substs, 1);
      if (ind > 0)
        nstr = nstr + ostr[1..ind - 1] + sub[2];
        ostr = ostr[ind + length(sub[1])..len];
        len = length(ostr);
      endif
      if (next = this:index_delimited(ostr, sub[1], case))
        fi = $list_utils:find_insert(indices, next = next - len) - 1;
        while (fi && (indices[fi] == next && length(substs[fi][1]) < length(sub[1])))
          "...give preference to longer redexes...";
          fi = fi - 1;
        endwhile
        indices = listappend(indices, next, fi);
        substs = listappend(substs, sub, fi);
      endif
    endwhile
    return nstr + ostr;
  endverb

  verb _cap_property (this none this) owner: #2 flags: "rxd"
    "cap_property(what,prop[,ucase]) returns what.(prop) but capitalized if either ucase is true or the prop name specified is capitalized.";
    "If prop is blank, returns what:title().";
    "If prop is bogus or otherwise irretrievable, returns the error.";
    "If capitalization is indicated, we return what.(prop+\"c\") if that exists, else we capitalize what.(prop) in the usual fashion.  There is a special exception for is_player(what)&&prop==\"name\" where we just return what.name if no .namec is provided --- ie., a player's .name is never capitalized in the usual fashion.";
    "If args[1] is a list, calls itself on each element of the list and returns $string_utils:english_list(those results).";
    {what, prop, ?ucase = 0} = args;
    set_task_perms(caller_perms());
    if (typeof(what) == LIST)
      result = {};
      for who in (what)
        result = {@result, this:_cap_property(who, prop, ucase)};
      endfor
      return $string_utils:english_list(result);
    endif
    ucase = prop && strcmp(prop, "a") < 0 || ucase;
    if (!prop)
      return valid(what) ? ucase ? what:titlec() | what:title() | (ucase ? "N" | "n") + "othing";
    elseif (!ucase || typeof(s = `what.((prop + "c")) ! ANY') == ERR)
      if (prop == "name")
        s = valid(what) ? what.name | "nothing";
        ucase = ucase && !is_player(what);
      else
        s = `$object_utils:has_property(what, prop) ? what.(prop) | $player.(prop) ! ANY';
      endif
      if (ucase && (s && (typeof(s) == STR && ((z = index(this.alphabet, s[1], 1)) < 27 && z > 0))))
        s[1] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[z];
      endif
    endif
    return typeof(s) == ERR ? s | tostr(s);
  endverb

  verb pronoun_sub (this none this) owner: #2 flags: "rxd"
    "Pronoun (and other things) substitution. See 'help pronouns' for details.";
    "syntax:  $string_utils:pronoun_sub(text[,who[,thing[,location[,dobj[,iobj]]]]])";
    "%s,%o,%p,%q,%r    => <who>'s pronouns.  <who> defaults to player.";
    "%n,%d,%i,%t,%l,%% => <who>, dobj, iobj, <thing>, location and %";
    "<thing> defaults to caller; <location> defaults to who.location";
    "%S,%O,%P,%Q,%R, %N,%D,%I,%T,%L have corresponding capitalized substitutions.";
    " %[#n], %[#d], ...  =>  <who>, dobj, etc.'s object number";
    "%(foo) => <who>.foo and %(Foo) => <who>.foo capitalized. %[dfoo] => dobj.foo, etc..";
    "%<foo> -> whatever <who> does when normal people foo. This is determined by calling :verb_sub() on the <who>.";
    "%<d:foo> -> whatever <dobj> does when normal people foo.";
    {string, ?who = player, ?thing = caller, ?where = $nothing, ?dobject = dobj, ?iobject = iobj} = args;
    where = valid(where) ? where | (valid(who) ? who.location | where);
    set_task_perms($no_one);
    if (typeof(string) == LIST)
      plines = {};
      for line in (string)
        plines = {@plines, this:(verb)(line, who, thing, where)};
      endfor
      return plines;
    endif
    old = tostr(string);
    new = "";
    objspec = "nditl";
    objects = {who, dobject, iobject, thing, where};
    prnspec = "sopqrSOPQR";
    prprops = {"ps", "po", "pp", "pq", "pr", "Ps", "Po", "Pp", "Pq", "Pr"};
    oldlen = length(old);
    while ((prcnt = index(old, "%")) && prcnt < oldlen)
      s = old[k = prcnt + 1];
      if (s == "<" && (gt = index(old[k + 2..$], ">")))
        "handling %<verb> ";
        gt = gt + k + 1;
        vb = old[k + 1..gt - 1];
        vbs = who;
        if (length(vb) > 2 && vb[2] == ":")
          " %<d:verb>";
          vbs = objects[index(objspec, vb[1]) || 1];
          vb = vb[3..$];
        endif
        vb = $object_utils:has_callable_verb(vbs, "verb_sub") ? vbs:verb_sub(vb) | $gender_utils:get_conj(vb, vbs);
        new = new + old[1..prcnt - 1] + vb;
        k = gt;
      else
        cp_args = {};
        if (brace = index("([", s))
          if (!(w = index(old[k + 1..oldlen], ")]"[brace])))
            return new + old;
          else
            p = old[prcnt + 2..(k = k + w) - 1];
            if (brace == 1)
              "%(property)";
              cp_args = {who, p};
            elseif (p[1] == "#")
              "%[#n] => object number";
              s = (o = index(objspec, p[2])) ? tostr(objects[o]) | "[" + p + "]";
            elseif (!(o = index(objspec, p[1])))
              s = "[" + p + "]";
            else
              " %[dproperty] ";
              cp_args = {objects[o], p[2..w - 1], strcmp(p[1], "a") < 0};
            endif
          endif
        elseif (o = index(objspec, s))
          cp_args = {objects[o], "", strcmp(s, "a") < 0};
        elseif (w = index(prnspec, s, 1))
          cp_args = {who, prprops[w]};
        elseif (s == "#")
          s = tostr(who);
        elseif (s != "%")
          s = "%" + s;
        endif
        new = new + old[1..prcnt - 1] + (!cp_args ? s | (typeof(sub = $string_utils:_cap_property(@cp_args)) != ERR ? sub | "%(" + tostr(sub) + ")"));
      endif
      old = old[k + 1..oldlen];
      oldlen = oldlen - k;
    endwhile
    return new + old;
  endverb

  verb pronoun_sub_secure (this none this) owner: HACKER flags: "rxd"
    "$string_utils:pronoun_sub_secure(string[,who[,thing[,location]]], default)";
    "Do pronoun_sub on string with the arguments given (see help";
    "string_utils:pronoun_sub for more information).  Return pronoun_subbed";
    "<default> if the subbed string does not contain <who>.name (<who>";
    "defaults to player).";
    who = length(args) > 2 ? args[2] | player;
    default = args[$];
    result = this:pronoun_sub(@args[1..$ - 1]);
    return this:index_delimited(result, who.name) ? result | this:pronoun_sub(@{default, @args[2..$ - 1]});
  endverb

  verb pronoun_quote (this none this) owner: HACKER flags: "rxd"
    " pronoun_quote(string) => quoted_string";
    " pronoun_quote(list of strings) => list of quoted_strings";
    " pronoun_quote(list of {key,string} pairs) => list of {key,quoted_string} pairs";
    "";
    "Here `quoted' means quoted in the sense of $string_utils:pronoun_sub, i.e., given a string X, the corresponding `quoted' string Y is such that pronoun_sub(Y) => X.  For example, pronoun_quote(\"--%Spam%--\") => \"--%%Spam%%--\".  This is for including literal text into a string that will eventually be pronoun_sub'ed, i.e., including it in such a way that the pronoun_sub will not expand anything in the included text.";
    "";
    "The 3rd form above (with {key,string} pairs) is for use with $string_utils:substitute().  If you have your own set of substitutions to be done in parallel with the pronoun substitutions, do";
    "";
    "  msg=$string_utils:substitute(msg,$string_utils:pronoun_quote(your_substs));";
    "  msg=$string_utils:pronoun_sub(msg);";
    if (typeof(what = args[1]) == STR)
      return strsub(what, "%", "%%");
    else
      ret = {};
      for w in (what)
        if (typeof(w) == LIST)
          ret = listappend(ret, listset(w, strsub(w[2], "%", "%%"), 2));
        else
          ret = listappend(ret, strsub(w, "%", "%%"));
        endif
      endfor
      return ret;
    endif
  endverb

  verb alt_pronoun_sub (none none none) owner: #2 flags: "rxd"
    "Pronoun (and other things) substitution. See 'help pronouns' for details.";
    "syntax:  $string_utils:pronoun_sub(text[,who[,thing[,location]]])";
    "%s,%o,%p,%q,%r    => <who>'s pronouns.  <who> defaults to player.";
    "%n,%d,%i,%t,%l,%% => <who>, dobj, iobj, this, <who>.location and %";
    "%S,%O,%P,%Q,%R, %N,%D,%I,%T,%L have corresponding capitalized substitutions.";
    " %[#n], %[#d], ...  =>  <who>, dobj, etc.'s object number";
    "%(foo) => <who>.foo and %(Foo) => <who>.foo capitalized. %[dfoo] => dobj.foo, etc..";
    "%<foo> -> whatever <who> does when normal people foo. This is determined by calling :verb_sub() on the <who>.";
    "%<d:foo> -> whatever <dobj> does when normal people foo.";
    set_task_perms($no_one);
    {string, ?who = player, ?thing = caller, ?where = $nothing} = args;
    where = valid(who) ? who.location | where;
    if (typeof(string) == LIST)
      plines = {};
      for line in (string)
        plines = {@plines, this:(verb)(line, who, thing, where)};
      endfor
      return plines;
    endif
    old = tostr(string);
    new = "";
    objspec = "nditl";
    objects = {who, dobj, iobj, thing, where};
    prnspec = "sopqrSOPQR";
    prprops = {"ps", "po", "pp", "pq", "pr", "Ps", "Po", "Pp", "Pq", "Pr"};
    oldlen = length(old);
    while ((prcnt = index(old, "%")) && prcnt < oldlen)
      s = old[k = prcnt + 1];
      if (s == "<" && (gt = index(old[k + 2..$], ">")))
        "handling %<verb> ";
        gt = gt + k + 1;
        vb = old[k + 1..gt - 1];
        vbs = who;
        if (length(vb) > 2 && vb[2] == ":")
          " %<d:verb>";
          vbs = objects[index(objspec, vb[1]) || 1];
          vb = vb[3..$];
        endif
        vb = $object_utils:has_verb(vbs, "verb_sub") ? vbs:verb_sub(vb) | this:(verb)(vb, vbs);
        new = new + old[1..prcnt - 1] + vb;
        k = gt;
      else
        cp_args = {};
        if (brace = index("([", s))
          if (!(w = index(old[k + 1..oldlen], ")]"[brace])))
            return new + old;
          else
            p = old[prcnt + 2..(k = k + w) - 1];
            if (brace == 1)
              "%(property)";
              cp_args = {who, p};
            elseif (p[1] == "#")
              "%[#n] => object number";
              s = (o = index(objspec, p[2])) ? tostr(objects[o]) | "[" + p + "]";
            elseif (!(o = index(objspec, p[1])))
              s = "[" + p + "]";
            else
              " %[dproperty] ";
              cp_args = {objects[o], p[2..w - 1], strcmp(p[1], "a") < 0};
            endif
          endif
        elseif (o = index(objspec, s))
          cp_args = {objects[o], "", strcmp(s, "a") < 0};
        elseif (w = index(prnspec, s, 1))
          cp_args = {who, prprops[w]};
        elseif (s == "#")
          s = tostr(who);
        elseif (s != "%")
          s = "%" + s;
        endif
        new = new + old[1..prcnt - 1] + (!cp_args ? s | (typeof(sub = $string_utils:_cap_property(@cp_args)) != ERR ? sub | "%(" + tostr(sub) + ")"));
      endif
      old = old[k + 1..oldlen];
      oldlen = oldlen - k;
    endwhile
    return new + old;
  endverb

  verb explode (this none this) owner: HACKER flags: "rxd"
    "$string_utils:explode(subject [, break])";
    "Return a list of those substrings of subject separated by runs of break[1].";
    "break defaults to space.";
    {subject, ?breakit = {" "}} = args;
    breakit = breakit[1];
    subject = subject + breakit;
    parts = {};
    while (subject)
      if ((i = index(subject, breakit)) > 1)
        parts = {@parts, subject[1..i - 1]};
      endif
      subject = subject[i + 1..$];
    endwhile
    return parts;
  endverb

  verb words (this none this) owner: HACKER flags: "rxd"
    "This breaks up the argument string into words, the resulting list being obtained exactly the way the command line parser obtains `args' from `argstr'.";
    rest = args[1];
    "...trim leading blanks...";
    if (0)
      rest[1..match(rest, "^ *")[2]] = "";
    endif
    rest = $string_utils:triml(rest);
    if (!rest)
      return {};
    endif
    quote = 0;
    toklist = {};
    token = "";
    pattern = " +%|\\.?%|\"";
    while (m = match(rest, pattern))
      "... find the next occurence of a special character, either";
      "... a block of spaces, a quote or a backslash escape sequence...";
      char = rest[m[1]];
      token = token + rest[1..m[1] - 1];
      if (char == " ")
        toklist = {@toklist, token};
        token = "";
      elseif (char == "\"")
        "... beginning or end of quoted string...";
        "... within a quoted string spaces aren't special...";
        pattern = (quote = !quote) ? "\\.?%|\"" | " +%|\\.?%|\"";
      elseif (m[1] < m[2])
        "... char has to be a backslash...";
        "... include next char literally if there is one";
        token = token + rest[m[2]];
      endif
      rest[1..m[2]] = "";
    endwhile
    return rest || char != " " ? {@toklist, token + rest} | toklist;
  endverb

  verb word_start (this none this) owner: HACKER flags: "rxd"
    "This breaks up the argument string into words, returning a list of indices into argstr corresponding to the starting points of each of the arguments.";
    rest = args[1];
    "... find first nonspace...";
    wstart = match(rest, "[^ ]%|$")[1];
    wbefore = wstart - 1;
    rest[1..wbefore] = "";
    if (!rest)
      return {};
    endif
    quote = 0;
    wslist = {};
    pattern = " +%|\\.?%|\"";
    while (m = match(rest, pattern))
      "... find the next occurence of a special character, either";
      "... a block of spaces, a quote or a backslash escape sequence...";
      char = rest[m[1]];
      if (char == " ")
        wslist = {@wslist, {wstart, wbefore + m[1] - 1}};
        wstart = wbefore + m[2] + 1;
      elseif (char == "\"")
        "... beginning or end of quoted string...";
        "... within a quoted string spaces aren't special...";
        pattern = (quote = !quote) ? "\\.?%|\"" | " +%|\\.?%|\"";
      endif
      rest[1..m[2]] = "";
      wbefore = wbefore + m[2];
    endwhile
    return rest || char != " " ? {@wslist, {wstart, wbefore + length(rest)}} | wslist;
  endverb

  verb to_value (this none this) owner: HACKER flags: "rxd"
    ":to_value(string) tries to parse string as a value (i.e., object, number, string, error, or list thereof).";
    "Returns {1,value} or {0,error_message} according as the attempt was successful or not.";
    result = this:_tolist(string = args[1] + "}");
    if (result[1] && result[1] != $string_utils:space(result[1]))
      return {0, tostr("after char ", length(string) - result[1], ":  ", result[2])};
    elseif (typeof(result[1]) == INT)
      return {0, "missing } or \""};
    elseif (length(result[2]) > 1)
      return {0, "comma unexpected."};
    elseif (result[2])
      return {1, result[2][1]};
    else
      return {0, "missing expression"};
    endif
  endverb

  verb prefix_to_value (this none this) owner: HACKER flags: "rxd"
    ":prefix_to_value(string) tries to parse string as a value (i.e., object, number, string, error, or list thereof).";
    "Returns {rest-of-string,value} or {0,error_message} according as the attempt was successful or not.";
    alen = length(args[1]);
    slen = length(string = this:triml(args[1]));
    if (!string)
      return {0, "empty string"};
    elseif (w = index("{\"", string[1]))
      result = this:(({"_tolist", "_unquote"}[w]))(string[2..slen]);
      if (typeof(result[1]) != INT)
        return result;
      elseif (result[1] == 0)
        return {0, "missing } or \""};
      else
        return {0, result[2], alen - result[1] + 1};
      endif
    else
      thing = string[1..tlen = index(string + " ", " ") - 1];
      if (typeof(s = this:_toscalar(thing)) != STR)
        return {string[tlen + 1..slen], s};
      else
        return {0, s, alen - slen + 1};
      endif
    endif
  endverb

  verb _tolist (this none this) owner: HACKER flags: "rxd"
    "_tolist(string) --- auxiliary for :to_value()";
    rest = this:triml(args[1]);
    vlist = {};
    if (!rest)
      return {0, {}};
    elseif (rest[1] == "}")
      return {rest[2..$], {}};
    endif
    while (1)
      rlen = length(rest);
      if (w = index("{\"", rest[1]))
        result = this:(({"_tolist", "_unquote"}[w]))(rest[2..rlen]);
        if (typeof(result[1]) == INT)
          return result;
        endif
        vlist = {@vlist, result[2]};
        rest = result[1];
      else
        thing = rest[1..tlen = min(index(rest + ",", ","), index(rest + "}", "}")) - 1];
        if (typeof(s = this:_toscalar(thing)) == STR)
          return {rlen, s};
        endif
        vlist = {@vlist, s};
        rest = rest[tlen + 1..rlen];
      endif
      if (!rest)
        return {0, vlist};
      elseif (rest[1] == "}")
        return {rest[2..$], vlist};
      elseif (rest[1] == ",")
        rest = this:triml(rest[2..$]);
      else
        return {length(rest), ", or } expected"};
      endif
    endwhile
  endverb

  verb _unquote (this none this) owner: HACKER flags: "rxd"
    "_unquote(string)   (auxiliary for :to_value())";
    "reads string as if it were preceded by a quote, reading up to the closing quote if any, then returns the corresponding unquoted string.";
    " => {0, string unquoted}  if there is no closing quote";
    " => {original string beyond closing quote, string unquoted}  otherwise";
    rest = args[1];
    result = "";
    while (m = match(rest, "\\.?%|\""))
      "Find the next special character";
      if (rest[pos = m[1]] == "\"")
        return {rest[pos + 1..$], result + rest[1..pos - 1]};
      endif
      result = result + rest[1..pos - 1] + rest[pos + 1..m[2]];
      rest = rest[m[2] + 1..$];
    endwhile
    return {0, result + rest};
  endverb

  verb _toscalar (this none this) owner: HACKER flags: "rxd"
    ":_toscalar(string)  --- auxiliary for :tovalue";
    " => value if string represents a number, object or error";
    " => string error message otherwise";
    thing = args[1];
    if (!thing)
      return "missing value";
    elseif ($code_utils:match_objid(thing))
      return toobj(thing);
    elseif (match(thing, "^[-+]?[0-9]+ *$"))
      return toint(thing);
    elseif (match(thing, "^[-+]?%([0-9]+%.[0-9]*%|[0-9]*%.[0-9]+%)%(e[-+]?[0-9]+%)? *$"))
      "matches 2. .2 3.2 3.2e3 .2e-3 3.e3";
      return `tofloat(thing) ! E_INVARG => tostr("Bad floating point value: ", thing)';
    elseif (match(thing, "^[-+]?[0-9]+e[-+]?[0-9]+ *$"))
      "matches 345e4. No decimal, but has an e so still a float";
      return `tofloat(thing) ! E_INVARG => tostr("Bad floating point value: ", thing)';
    elseif (thing[1] == "E")
      return (e = $code_utils:toerr(thing)) ? tostr("unknown error code `", thing, "'") | e;
    elseif (thing[1] == "#")
      return tostr("bogus objectid `", thing, "'");
    else
      return tostr("`", thing[1], "' unexpected");
    endif
  endverb

  verb parse_command (this none this) owner: #2 flags: "rxd"
    ":parse_command(cmd_line[,player])";
    " => {verb, {dobj, dobjstr}, {prep, prepstr}, {iobj, iobjstr}, {args, argstr},";
    "     {dobjset, prepset, iobjset}}";
    "This mimics the action of the builtin parser, returning what the values of the builtin variables `verb', `dobj', `dobjstr', `prepstr', `iobj', `iobjstr', `args', and `argstr' would be if `player' had typed `cmd_line'.  ";
    "`prep' is the shortened version of the preposition found.";
    "";
    "`dobjset' and `iobjset' are subsets of {\"any\",\"none\"} and are used to determine possible matching verbs, i.e., the matching verb must either be on `dobj' and have verb_args[1]==\"this\" or else it has verb_args[1] in `dobjset'; likewise for `iobjset' and verb_args[3]; similarly we must have verb_args[2] in `prepset'.";
    {c, ?who = player} = args;
    y = $string_utils:words(c);
    if (y == {})
      return {};
    endif
    vrb = y[1];
    y = y[2..$];
    as = y == {} ? "" | c[length(vrb) + 2..$];
    n = 1;
    while (!(gp = $code_utils:get_prep(@y[n..$]))[1] && n < length(y))
      n = n + 1;
    endwhile
    "....";
    really = player;
    player = who;
    loc = who.location;
    if (ps = gp[1])
      ds = $string_utils:from_list(y[1..n - 1], " ");
      is = $string_utils:from_list(listdelete(gp, 1), " ");
      io = valid(loc) ? loc:match_object(is) | $string_utils:match_object(is, loc);
    else
      ds = $string_utils:from_list(y, " ");
      is = "";
      io = $nothing;
    endif
    do = valid(loc) ? loc:match_object(ds) | $string_utils:match_object(ds, loc);
    player = really;
    "....";
    dset = {"any", @ds == "" ? {"none"} | {}};
    "\"this\" must be handled manually.";
    pset = {"any", @ps ? {$code_utils:full_prep(ps)} | {"none"}};
    iset = {"any", @is == "" ? {"none"} | {}};
    return {vrb, {do, ds}, {$code_utils:short_prep(ps), ps}, {io, is}, {y, as}, {dset, pset, iset}};
  endverb

  verb from_value (this none this) owner: #2 flags: "rxd"
    "$string_utils:from_value(value [, quote_strings = 0 [, list_depth = 1]])";
    "Print the given value into a string.";
    {value, ?quote_strings = 0, ?list_depth = 1} = args;
    if (typeof(value) == LIST)
      if (value)
        if (list_depth)
          result = "{" + this:from_value(value[1], quote_strings, list_depth - 1);
          for v in (listdelete(value, 1))
            result = tostr(result, ", ", this:from_value(v, quote_strings, list_depth - 1));
          endfor
          return result + "}";
        else
          return "{...}";
        endif
      else
        return "{}";
      endif
    elseif (quote_strings)
      if (typeof(value) == STR)
        result = "\"";
        while (q = index(value, "\"") || index(value, "\\"))
          if (value[q] == "\"")
            q = min(q, index(value + "\\", "\\"));
          endif
          result = result + value[1..q - 1] + "\\" + value[q];
          value = value[q + 1..$];
        endwhile
        return result + value + "\"";
      elseif (typeof(value) == ERR)
        return $code_utils:error_name(value);
      else
        return tostr(value);
      endif
    else
      return tostr(value);
    endif
  endverb

  verb "print print_suspended" (this none this) owner: HACKER flags: "rxd"
    "$string_utils:print(value)";
    "Print the given value into a string. == from_value(value,1,-1)";
    return toliteral(args[1]);
    value = args[1];
    if (typeof(value) == LIST)
      if (value)
        result = "{" + this:print(value[1]);
        for val in (listdelete(value, 1))
          result = tostr(result, ", ", this:print(val));
        endfor
        return result + "}";
      else
        return "{}";
      endif
    elseif (typeof(value) == STR)
      return tostr("\"", strsub(strsub(value, "\\", "\\\\"), "\"", "\\\""), "\"");
    elseif (typeof(value) == ERR)
      return $code_utils:error_name(value);
    else
      return tostr(value);
    endif
  endverb

  verb reverse (this none this) owner: HACKER flags: "rxd"
    ":reverse(string) => \"gnirts\"";
    "An example: :reverse(\"This is a test.\") => \".tset a si sihT\"";
    string = args[1];
    if ((len = length(string)) > 50)
      return this:reverse(string[$ / 2 + 1..$]) + this:reverse(string[1..$ / 2]);
    endif
    index = len;
    result = "";
    while (index > 0)
      result = result + string[index];
      index = index - 1;
    endwhile
    return result;
  endverb

  verb char_list (this none this) owner: HACKER flags: "rxd"
    ":char_list(string) => string as a list of characters.";
    "   e.g., :char_list(\"abad\") => {\"a\",\"b\",\"a\",\"d\"}";
    if (30 < (len = length(string = args[1])))
      return {@this:char_list(string[1..$ / 2]), @this:char_list(string[$ / 2 + 1..$])};
    else
      l = {};
      for c in [1..len]
        l = {@l, string[c]};
      endfor
      return l;
    endif
  endverb

  verb regexp_quote (this none this) owner: HACKER flags: "rxd"
    ":regexp_quote(string)";
    " => string with all of the regular expression special characters quoted with %";
    string = args[1];
    quoted = "";
    while (m = rmatch(string, "[][$^.*+?%].*"))
      quoted = "%" + string[m[1]..m[2]] + quoted;
      string = string[1..m[1] - 1];
    endwhile
    return string + quoted;
  endverb

  verb connection_hostname_bsd (this none this) owner: HACKER flags: "rxd"
    "Takes the output from connection_name() and returns just the host string portion of it.  Assumes you are using bsd_network style connection names.";
    s = args[1];
    return (m = `match(args[1], "^.* %(from%|to%) %([^, ]+%)") ! ANY') ? substitute("%2", m) | "";
  endverb

  verb connection_hostname (this none this) owner: HACKER flags: "rxd"
    "This is the function that should actually be called to get the host name from a connection name.  The archwizard should change _bsd so as to be calling the verb appropriate for his/her network interface.";
    return this:connection_hostname_bsd(@args);
  endverb

  verb from_value_suspended (this none this) owner: #2 flags: "rxd"
    "$string_utils:from_value(value [, quote_strings = 0 [, list_depth = 1]])";
    "Print the given value into a string.";
    "This verb suspends as necessary for large values.";
    set_task_perms(caller_perms());
    {value, ?quote_strings = 0, ?list_depth = 1} = args;
    if (typeof(value) == LIST)
      if (value)
        if (list_depth)
          result = "{" + this:from_value(value[1], quote_strings, list_depth - 1);
          for v in (listdelete(value, 1))
            $command_utils:suspend_if_needed(0);
            result = tostr(result, ", ", this:from_value(v, quote_strings, list_depth - 1));
          endfor
          return result + "}";
        else
          return "{...}";
        endif
      else
        return "{}";
      endif
    elseif (quote_strings)
      if (typeof(value) == STR)
        result = "\"";
        while (q = index(value, "\"") || index(value, "\\"))
          $command_utils:suspend_if_needed(0);
          if (value[q] == "\"")
            q = min(q, index(value + "\\", "\\"));
          endif
          result = result + value[1..q - 1] + "\\" + value[q];
          value = value[q + 1..$];
        endwhile
        return result + value + "\"";
      elseif (typeof(value) == ERR)
        return $code_utils:error_name(value);
      else
        return tostr(value);
      endif
    else
      return tostr(value);
    endif
  endverb

  verb end_expression (this none this) owner: HACKER flags: "rxd"
    ":end_expression(string[,stop_at])";
    "  assumes string starts with an expression; returns the index of the last char in expression or 0 if string appears not to be an expression.  Expression ends at any character from stop_at which occurs at top level.";
    {string, ?stop_at = " "} = args;
    gone = 0;
    paren_stack = "";
    inquote = 0;
    search = top_level_search = "[][{}()\"" + strsub(stop_at, "]", "") + "]";
    paren_search = "[][{}()\"]";
    while (m = match(string, search))
      char = string[m[1]];
      string[1..m[2]] = "";
      gone = gone + m[2];
      if (char == "\"")
        "...skip over quoted string...";
        char = "\\";
        while (char == "\\")
          if (!(m = match(string, "%(\\.?%|\"%)")))
            return 0;
          endif
          char = string[m[1]];
          string[1..m[2]] = "";
          gone = gone + m[2];
        endwhile
      elseif (index("([{", char))
        "... push parenthesis...";
        paren_stack[1..0] = char;
        search = paren_search;
      elseif (i = index(")]}", char))
        if (paren_stack && "([{"[i] == paren_stack[1])
          "... pop parenthesis...";
          paren_stack[1..1] = "";
          search = paren_stack ? paren_search | top_level_search;
        else
          "...parenthesis mismatch...";
          return 0;
        endif
      else
        "... stop character ...";
        return gone - 1;
      endif
    endwhile
    return !paren_stack && gone + length(string);
  endverb

  verb first_word (this none this) owner: HACKER flags: "rxd"
    ":first_word(string) => {first word, rest of string} or {}";
    rest = args[1];
    "...trim leading blanks...";
    rest[1..match(rest, "^ *")[2]] = "";
    if (!rest)
      return {};
    endif
    quote = 0;
    token = "";
    pattern = " +%|\\.?%|\"";
    while (m = match(rest, pattern))
      "... find the next occurence of a special character, either";
      "... a block of spaces, a quote or a backslash escape sequence...";
      char = rest[m[1]];
      token = token + rest[1..m[1] - 1];
      if (char == " ")
        rest[1..m[2]] = "";
        return {token, rest};
      elseif (char == "\"")
        "... beginning or end of quoted string...";
        "... within a quoted string spaces aren't special...";
        pattern = (quote = !quote) ? "\\.?%|\"" | " +%|\\.?%|\"";
      elseif (m[1] < m[2])
        "... char has to be a backslash...";
        "... include next char literally if there is one";
        token = token + rest[m[2]];
      endif
      rest[1..m[2]] = "";
    endwhile
    return {token + rest, ""};
  endverb

  verb common (this none this) owner: HACKER flags: "rxd"
    ":common(first,second) => length of longest common prefix";
    {first, second} = args;
    r = min(length(first), length(second));
    l = 1;
    while (r >= l)
      h = (r + l) / 2;
      if (first[l..h] == second[l..h])
        l = h + 1;
      else
        r = h - 1;
      endif
    endwhile
    return r;
  endverb

  verb "title_list*c list_title*c" (this none this) owner: HACKER flags: "rxd"
    "wr_utils:title_list/title_listc(<obj-list>[, @<args>)";
    "Creates an english list out of the titles of the objects in <obj-list>.  Optional <args> are passed on to $string_utils:english_list.";
    "title_listc uses :titlec() for the first item.";
    titles = $list_utils:map_verb(args[1], "title");
    if (verb[length(verb)] == "c")
      if (titles)
        titles[1] = args[1][1]:titlec();
      elseif (length(args) > 1)
        args[2] = $string_utils:capitalize(args[2]);
      else
        args = listappend(args, "Nothing");
      endif
    endif
    return $string_utils:english_list(titles, @args[2..$]);
  endverb

  verb "name_and_number nn name_and_number_list nn_list" (this none this) owner: HACKER flags: "rxd"
    "name_and_number(object [,sepr] [,english_list_args]) => \"ObjectName (#object)\"";
    "Return name and number for OBJECT.  Second argument is optional separator (for those who want no space, use \"\").  If OBJECT is a list of objects, this maps the above function over the list and then passes it to $string_utils:english_list.";
    "The third through nth arguments to nn_list corresponds to the second through nth arguments to English_list, and are passed along untouched.";
    {objs, ?sepr = " ", @eng_args} = args;
    if (typeof(objs) != LIST)
      objs = {objs};
    endif
    name_list = {};
    for what in (objs)
      name = valid(what) ? what.name | {"<invalid>", "$nothing", "$ambiguous_match", "$failed_match"}[1 + (what in {#-1, #-2, #-3})];
      name = tostr(name, sepr, "(", what, ")");
      name_list = {@name_list, name};
    endfor
    return this:english_list(name_list, @eng_args);
  endverb

  verb "columnize_suspended columnise_suspended" (this none this) owner: HACKER flags: "rxd"
    "columnize_suspended (interval, items, n [, width]) - Turn a one-column list of items into an n-column list, suspending for `interval' seconds as necessary. 'width' is the last character position that may be occupied; it defaults to a standard screen width. Example: To tell the player a list of numbers in three columns, do 'player:tell_lines ($string_utils:columnize_suspended(0, {1, 2, 3, 4, 5, 6, 7}, 3));'.";
    {interval, items, n, ?width = 79} = args;
    height = (length(items) + n - 1) / n;
    items = {@items, @$list_utils:make(height * n - length(items), "")};
    colwidths = {};
    for col in [1..n - 1]
      colwidths = listappend(colwidths, 1 - (width + 1) * col / n);
    endfor
    result = {};
    for row in [1..height]
      line = tostr(items[row]);
      for col in [1..n - 1]
        $command_utils:suspend_if_needed(interval);
        line = tostr(this:left(line, colwidths[col]), " ", items[row + col * height]);
      endfor
      result = listappend(result, line[1..min($, width)]);
    endfor
    return result;
  endverb

  verb a_or_an (this none this) owner: HACKER flags: "rxd"
    ":a_or_an(<noun>) => \"a\" or \"an\"";
    "To accomodate personal variation (e.g., \"an historical book\"), a player can override this by having a personal a_or_an verb.  If that verb returns 0 instead of a string, the standard algorithm is used.";
    noun = args[1];
    if ($object_utils:has_verb(player, "a_or_an") && (custom_result = player:a_or_an(noun)) != 0)
      return custom_result;
    endif
    if (noun in this.use_article_a)
      return "a";
    endif
    if (noun in this.use_article_an)
      return "an";
    endif
    a_or_an = "a";
    if (noun != "")
      if (index("aeiou", noun[1]))
        a_or_an = "an";
        "unicycle, unimplemented, union, united, unimpressed, unique";
        if (noun[1] == "u" && length(noun) > 2 && noun[2] == "n" && (index("aeiou", noun[3]) == 0 || (noun[3] == "i" && length(noun) > 3 && (index("aeioubcghqwyz", noun[4]) || (length(noun) > 4 && index("eiy", noun[5]))))))
          a_or_an = "a";
        endif
      endif
    endif
    return a_or_an;
    "Ported by Mickey with minor tweaks from a Moo far far away.";
    "Last modified Sun Aug  1 22:53:07 1993 EDT by BabyBriar (#2).";
  endverb

  verb index_all (this none this) owner: HACKER flags: "rxd"
    "index_all(string,target) -- returns list of positions of target in string.";
    "Usage: $string_utils:index_all(<string,pattern>)";
    "       $string_utils:index_all(\"aaabacadae\",\"a\")";
    {line, pattern} = args;
    if (typeof(line) != STR || typeof(pattern) != STR)
      return E_TYPE;
    else
      where = {};
      place = -1;
      next = 0;
      while ((place = index(line[next + 1..$], pattern)) != 0)
        where = {@where, place + next};
        next = place + next + length(pattern) - 1;
      endwhile
      return where;
    endif
  endverb

  verb "match_stringlist match_string_list" (this none this) owner: HACKER flags: "rx"
    "Copied from Puff (#1449):match_stringlist Tue Oct 19 08:18:13 1993 PDT";
    "$string_utils:match_stringlist(string, {list of strings})";
    "The list of strings should be just that, a list of strings.  The first string is matched against the list of strings.";
    "If it exactly matches exactly one of them, the index of the match is returned. If it exactly matches more than one of them, $ambiguous_match is returned.";
    "If there are no exact matches, then partial matches are considered, ones in which the given string is a prefix of one of the strings.";
    "Again, if exactly one match is found, the index of that string is returned, and if more than one match is found, $ambiguous match is returned.";
    "Finally, if there are no exact or partial matches, then $failed_match is returned.";
    {subject, stringlist} = args;
    if (subject == "" || length(stringlist) < 1)
      return $nothing;
    endif
    matches = {};
    "First check for exact matches.";
    for i in [1..length(stringlist)]
      if (subject == stringlist[i])
        matches = {@matches, i};
      endif
    endfor
    "Now return a match, or $ambiguous, or check for partial matches.";
    if (length(matches) == 1)
      return matches[1];
    elseif (length(matches) > 1)
      return $ambiguous_match;
    elseif (length(matches) == 0)
      "Checking for partial matches is almost identical to checking for exact matches, but we use index(list[i], target) instead of list[i] == target to see if they match.";
      for i in [1..length(stringlist)]
        if (index(stringlist[i], subject) == 1)
          matches = {@matches, i};
        endif
      endfor
      if (length(matches) == 1)
        return matches[1];
      elseif (length(matches) > 1)
        return $ambiguous_match;
      elseif (length(matches) == 0)
        return $failed_match;
      endif
    endif
  endverb

  verb from_ASCII (this none this) owner: HACKER flags: "rxd"
    "This converts a ASCII character code in the range [32..126] into the ASCII character with that code, represented as a one-character string.";
    "";
    "Example:   $string_utils:from_ASCII(65) => \"A\"";
    code = args[1];
    return this.ascii[code - 31];
  endverb

  verb to_ASCII (this none this) owner: HACKER flags: "rxd"
    "Convert a one-character string into the ASCII character code for that character.";
    "";
    "Example:  $string_utils:to_ASCII(\"A\") => 65";
    return (index(this.ascii, args[1], 1) || raise(E_INVARG)) + 31;
  endverb

  verb abbreviated_value (this none this) owner: HACKER flags: "rxd"
    "Copied from Mickey (#52413):abbreviated_value Fri Sep  9 08:52:41 1994 PDT";
    ":abbreviated_value(value,max_reslen,max_lstlev,max_lstlen,max_strlen,max_toklen)";
    "";
    "Gets the printed representation of value, subject to these parameters:";
    " max_reslen = Maximum desired result string length.";
    " max_lstlev = Maximum list level to show.";
    " max_lstlen = Maximum list length to show.";
    " max_strlen = Maximum string length to show.";
    " max_toklen = Maximum token length (e.g., numbers and errors) to show.";
    "";
    "A best attempt is made to get the exact target size, but in some cases the result is not exact.";
    {value, ?max_reslen = $maxint, ?max_lstlev = $maxint, ?max_lstlen = $maxint, ?max_strlen = $maxint, ?max_toklen = $maxint} = args;
    return this:_abbreviated_value(value, max_reslen, max_lstlev, max_lstlen, max_strlen, max_toklen);
    "Originally written by Mickey.";
  endverb

  verb _abbreviated_value (this none this) owner: HACKER flags: "rxd"
    "Copied from Mickey (#52413):_abbreviated_value Fri Sep  9 08:52:44 1994 PDT";
    "Internal to :abbreviated_value.  Do not call this directly.";
    {value, max_reslen, max_lstlev, max_lstlen, max_strlen, max_toklen} = args;
    if ((type = typeof(value)) == LIST)
      if (!value)
        return "{}";
      elseif (max_lstlev == 0)
        return "{...}";
      else
        n = length(value);
        result = "{";
        r = max_reslen - 2;
        i = 1;
        eltstr = "";
        while (i <= n && i <= max_lstlen && r > (x = i == 1 ? 0 | 2))
          eltlen = length(eltstr = this:(verb)(value[i], r, max_lstlev - 1, max_lstlen, max_strlen, max_toklen));
          lastpos = 1;
          if (r >= eltlen + x)
            comma = i == 1 ? "" | ", ";
            result = tostr(result, comma);
            if (r > 4)
              lastpos = length(result);
            endif
            result = tostr(result, eltstr);
            r = r - eltlen - x;
          elseif (i == 1)
            return "{...}";
          elseif (r > 4)
            return tostr(result, ", ...}");
          else
            return tostr(result[1..lastpos], "...}");
          endif
          i = i + 1;
        endwhile
        if (i <= n)
          if (i == 1)
            return "{...}";
          elseif (r > 4)
            return tostr(result, ", ...}");
          else
            return tostr(result[1..lastpos], "...}");
          endif
        else
          return tostr(result, "}");
        endif
      endif
    elseif (type == STR)
      result = "\"";
      while ((q = index(value, "\"")) ? q = min(q, index(value, "\\")) | (q = index(value, "\\")))
        result = result + value[1..q - 1] + "\\" + value[q];
        value = value[q + 1..$];
      endwhile
      result = result + value;
      if (length(result) + 1 > (z = max(min(max_reslen, max(max_strlen, max_strlen + 2)), 6)))
        z = z - 5;
        k = 0;
        while (k < z && result[z - k] == "\\")
          k = k + 1;
        endwhile
        return tostr(result[1..z - k % 2], "\"+...");
      else
        return tostr(result, "\"");
      endif
    else
      v = type == ERR ? $code_utils:error_name(value) | tostr(value);
      len = max(4, min(max_reslen, max_toklen));
      return length(v) > len ? v[1..len - 3] + "..." | v;
    endif
    "Originally written by Mickey.";
  endverb

  verb match_suspended (this none this) owner: HACKER flags: "rxd"
    "$string_utils:match_suspended(string [, obj-list, prop-name]*)";
    "Each obj-list should be a list of objects or a single object, which is treated as if it were a list of that object.  Each prop-name should be string naming a property on every object in the corresponding obj-list.  The value of that property in each case should be either a string or a list of strings.";
    "The argument string is matched against all of the strings in the property values.";
    "If it exactly matches exactly one of them, the object containing that property is returned.  If it exactly matches more than one of them, $ambiguous_match is returned.";
    "If there are no exact matches, then partial matches are considered, ones in which the given string is a prefix of some property string.  Again, if exactly one match is found, the object with that property is returned, and if there is more than one match, $ambiguous_match is returned.";
    "Finally, if there are no exact or partial matches, then $failed_match is returned.";
    "This verb will suspend as needed, and should be used if obj-list is very large.";
    subject = args[1];
    if (subject == "")
      return $nothing;
    endif
    no_exact_match = no_partial_match = 1;
    for i in [1..length(args) / 2]
      prop_name = args[2 * i + 1];
      for object in (typeof(olist = args[2 * i]) == LIST ? olist | {olist})
        if (valid(object))
          if (typeof(str_list = `object.(prop_name) ! E_PERM, E_PROPNF => {}') != LIST)
            str_list = {str_list};
          endif
          if (subject in str_list)
            if (no_exact_match)
              no_exact_match = object;
            elseif (no_exact_match != object)
              return $ambiguous_match;
            endif
          else
            for string in (str_list)
              if (index(string, subject) != 1)
              elseif (no_partial_match)
                no_partial_match = object;
              elseif (no_partial_match != object)
                no_partial_match = $ambiguous_match;
              endif
            endfor
          endif
        endif
        $command_utils:suspend_if_needed(5);
      endfor
    endfor
    return no_exact_match && (no_partial_match && $failed_match);
  endverb

  verb incr_alpha (this none this) owner: HACKER flags: "rxd"
    "args[1] is a string.  'increments' the string by one. E.g., aaa => aab, aaz => aba.  empty string => a, zzz => aaaa.";
    "args[2] is optional alphabet to use instead of $string_utils.alphabet.";
    {s, ?alphabet = this.alphabet} = args;
    index = length(s);
    if (!s)
      return alphabet[1];
    elseif (s[$] == alphabet[$])
      return this:incr_alpha(s[1..index - 1], alphabet) + alphabet[1];
    else
      t = index(alphabet, s[index]);
      return s[1..index - 1] + alphabet[t + 1];
    endif
  endverb

  verb is_float (this none this) owner: HACKER flags: "rxd"
    "Usage:  is_float(string)";
    "Is string composed of one or more digits possibly preceded by a minus sign either followed by a decimal or by an exponent?";
    "Return true or false";
    return match(args[1], "^ *[-+]?%(%([0-9]+%.[0-9]*%|[0-9]*%.[0-9]+%)%(e[-+]?[0-9]+%)?%)%|%([0-9]+e[-+]?[0-9]+%) *$");
  endverb

  verb inside_quotes (this none this) owner: HACKER flags: "rx"
    "Copied from Moo_tilities (#332):inside_quotes by Mooshie (#106469) Tue Dec 23 10:26:49 1997 PST";
    "Usage: inside_quotes(STR)";
    "Is the  end of the given string `inside' a doublequote?";
    "Called from $code_utils:substitute.";
    {string} = args;
    quoted = 0;
    while (i = index(string, "\""))
      if (!quoted || string[i - 1] != "\\")
        quoted = !quoted;
      endif
      string = string[i + 1..$];
    endwhile
    return quoted;
  endverb

  verb strip_all_but_seq (this none this) owner: HACKER flags: "rxd"
    ":strip_all_but_seq(string, keep) => chars in string not in exact sequence of keep removed.";
    ":strip_all_but() works similarly, only it does not concern itself with the sequence, just the specified chars.";
    string = args[1];
    wanted = args[2];
    output = "";
    while (m = match(string, wanted))
      output = output + string[m[1]..m[2]];
      string = string[m[2] + 1..length(string)];
    endwhile
    return output;
  endverb
endobject