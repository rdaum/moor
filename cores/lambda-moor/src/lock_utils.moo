object LOCK_UTILS
  name: "lock utilities"
  parent: GENERIC_UTILS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  property index_incremented (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property input_index (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property input_length (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property input_string (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "";
  property player (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;

  override aliases = {"lock utilities"};
  override description = "This the lock utilities package, used by the MOOwide locking mechanisms. See `help $lock_utils' for more details.";
  override help_msg = {
    "These routines are used when locking objects, and when testing an object's lock before allowing use (such as in an exit).",
    "",
    ":parse_keyexp   (string keyexpression, object player)",
    "        => returns an object or list for the new key as defined by the",
    "           keyexpression or a string describing the error if it failed.",
    "",
    ":eval_key       (LIST|OBJ key, testobject)",
    "        => returns true if the given testobject satisfies the key.",
    "",
    ":unparse_key    (LIST|OBJ key)",
    "        => returns a string describing the key in english/moo-code terms.",
    "",
    "For more information on keys and locking, read `help locking', `help keys', and `help @lock'."
  };
  override object_size = {9664, 1084848672};

  verb init_scanner (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    this.input_string = args[1];
    this.input_length = length(args[1]);
    this.input_index = 1;
    this.index_incremented = 0;
  endverb

  verb scan_token (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    string = this.input_string;
    len = this.input_length;
    i = this.input_index;
    while (i <= len && string[i] == " ")
      i = i + 1;
    endwhile
    if (i > len)
      this.index_incremented = 0;
      return "";
    elseif ((ch = string[i]) in {"(", ")", "!", "?"})
      this.input_index = i + 1;
      this.index_incremented = 1;
      return ch;
    elseif (ch in {"&", "|"})
      this.input_index = i = i + 1;
      this.index_incremented = 1;
      if (i <= len && string[i] == ch)
        this.input_index = i + 1;
        this.index_incremented = 2;
      endif
      return ch + ch;
    else
      start = i;
      while (i <= len && !((ch = string[i]) in {"(", ")", "!", "?", "&", "|"}))
        i = i + 1;
      endwhile
      this.input_index = i;
      i = i - 1;
      while (string[i] == " ")
        i = i - 1;
      endwhile
      this.index_incremented = i - start + 1;
      return this:canonicalize_spaces(string[start..i]);
    endif
  endverb

  verb canonicalize_spaces (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    name = args[1];
    while (index(name, "  "))
      name = strsub(name, "  ", " ");
    endwhile
    return name;
  endverb

  verb parse_keyexp (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "parse_keyexp(STRING keyexpression, OBJ player) => returns a list containing the coded key, or a string containing an error message if the attempt failed.";
    "";
    "Grammar for key expressions:";
    "";
    "    E ::= A       ";
    "       |  E || A  ";
    "       |  E && A  ";
    "    A ::= ( E )   ";
    "       |  ! A     ";
    "       |  object  ";
    "       |  ? object  ";
    this:init_scanner(args[1]);
    this.player = args[2];
    return this:parse_E();
  endverb

  verb parse_E (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    exp = this:parse_A();
    if (typeof(exp) != STR)
      while ((token = this:scan_token()) in {"&&", "||"})
        rhs = this:parse_A();
        if (typeof(rhs) == STR)
          return rhs;
        endif
        exp = {token, exp, rhs};
      endwhile
      "The while loop above always eats a token. Reset it back so the iteration can find it again. Always losing `)'. Ho_Yan 3/9/95";
      this.input_index = this.input_index - this.index_incremented;
    endif
    return exp;
  endverb

  verb parse_A (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    token = this:scan_token();
    if (token == "(")
      exp = this:parse_E();
      if (typeof(exp) != STR && this:scan_token() != ")")
        return "Missing ')'";
      else
        return exp;
      endif
    elseif (token == "!")
      exp = this:parse_A();
      if (typeof(exp) == STR)
        return exp;
      else
        return {"!", exp};
      endif
    elseif (token == "?")
      next = this:scan_token();
      if (next in {"(", ")", "!", "&&", "||", "?"})
        return "Missing object-name before '" + token + "'";
      elseif (next == "")
        return "Missing object-name at end of key expression";
      else
        what = this:match_object(next);
        if (typeof(what) == OBJ)
          return {"?", this:match_object(next)};
        else
          return what;
        endif
      endif
    elseif (token in {"&&", "||"})
      return "Missing expression before '" + token + "'";
    elseif (token == "")
      return "Missing expression at end of key expression";
    else
      return this:match_object(token);
    endif
  endverb

  verb eval_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "eval_key(LIST|OBJ coded key, OBJ testobject) => returns true if testobject will solve the provided key.";
    {key, who} = args;
    type = typeof(key);
    if (!(type in {LIST, OBJ}))
      return 1;
    elseif (typeof(key) == OBJ)
      return who == key || $object_utils:contains(who, key);
    endif
    op = key[1];
    if (op == "!")
      return !this:eval_key(key[2], who);
    elseif (op == "?")
      return (key[2]):is_unlocked_for(who);
    elseif (op == "&&")
      return this:eval_key(key[2], who) && this:eval_key(key[3], who);
    elseif (op == "||")
      return this:eval_key(key[2], who) || this:eval_key(key[3], who);
    else
      raise(E_DIV);
    endif
  endverb

  verb match_object (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "used by $lock_utils to unparse a key expression so one can use `here' and `me' as well as doing the regular object matching.";
    token = args[1];
    if (token == "me")
      return this.player;
    elseif (token == "here")
      if (valid(this.player.location))
        return this.player.location;
      else
        return "'here' has no meaning where " + this.player.name + " is";
      endif
    else
      what = this.player.location:match_object(token);
      if (what == $failed_match)
        return "Can't find an object named '" + token + "'";
      elseif (what == $ambiguous_match)
        return "Multiple objects named '" + token + "'";
      else
        return what;
      endif
    endif
  endverb

  verb unparse_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":unparse_key(LIST|OBJ coded key) => returns a string describing the key in english/moo-code terms.";
    "Example:";
    "$lock_utils:unparse_key({\"||\", $hacker, $housekeeper}) => \"#18105[Hacker] || #36830[housekeeper]\"";
    key = args[1];
    type = typeof(key);
    if (!(type in {LIST, OBJ}))
      return "(None.)";
    elseif (type == OBJ)
      if (valid(key))
        return tostr(key, "[", key.name, "]");
      else
        return tostr(key);
      endif
    else
      op = key[1];
      arg1 = this:unparse_key(key[2]);
      if (op == "?")
        return "?" + arg1;
      elseif (op == "!")
        if (typeof(key[2]) == LIST)
          return "!(" + arg1 + ")";
        else
          return "!" + arg1;
        endif
      elseif (op in {"&&", "||"})
        other = op == "&&" ? "||" | "&&";
        lhs = arg1;
        rhs = this:unparse_key(key[3]);
        if (typeof(key[2]) == OBJ || key[2][1] != other)
          exp = lhs;
        else
          exp = "(" + lhs + ")";
        endif
        exp = exp + " " + op + " ";
        if (typeof(key[3]) == OBJ || key[3][1] != other)
          exp = exp + rhs;
        else
          exp = exp + "(" + rhs + ")";
        endif
        return exp;
      else
        raise(E_DIV);
      endif
    endif
  endverb

  verb eval_key_new (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms($no_one);
    {key, who} = args;
    type = typeof(key);
    if (!(type in {LIST, OBJ}))
      return 1;
    elseif (typeof(key) == OBJ)
      return who == key || $object_utils:contains(who, key);
    endif
    op = key[1];
    if (op == "!")
      return !this:eval_key(key[2], who);
    elseif (op == "?")
      return (key[2]):is_unlocked_for(who);
    elseif (op == "&&")
      return this:eval_key(key[2], who) && this:eval_key(key[3], who);
    elseif (op == "||")
      return this:eval_key(key[2], who) || this:eval_key(key[3], who);
    elseif (op == ".")
      if ($object_utils:has_property(who, key[2]) && who.((key[2])))
        return 1;
      else
        for thing in ($object_utils:all_contents(who))
          if ($object_utils:has_property(thing, key[2]) && thing.((key[2])))
            return 1;
          endif
        endfor
      endif
      return 0;
    elseif (op == ":")
      if ($object_utils:has_verb(who, key[2]) && who:((key[2]))())
        return 1;
      else
        for thing in ($object_utils:all_contents(who))
          if ($object_utils:has_verb(thing, key[2]) && thing:((key[2]))())
            return 1;
          endif
        endfor
      endif
      return 0;
    else
      raise(E_DIV);
    endif
  endverb

  verb parse_A_new (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    token = this:scan_token();
    if (token == "(")
      exp = this:parse_E();
      if (typeof(exp) != STR && this:scan_token() != ")")
        return "Missing ')'";
      else
        return exp;
      endif
    elseif (token == "!")
      exp = this:parse_A();
      if (typeof(exp) == STR)
        return exp;
      else
        return {"!", exp};
      endif
    elseif (token == "?")
      next = this:scan_token();
      if (next in {":", ".", "(", ")", "!", "&&", "||", "?"})
        return "Missing object-name before '" + token + "'";
      elseif (next == "")
        return "Missing object-name at end of key expression";
      else
        what = this:match_object(next);
        if (typeof(what) == OBJ)
          return {"?", this:match_object(next)};
        else
          return what;
        endif
      endif
    elseif (token in {":", "."})
      next = this:scan_token();
      if (next in {":", ".", "(", ")", "!", "&&", "||", "?"})
        return "Missing verb-or-property-name before '" + token + "'";
      elseif (next == "")
        return "Missing verb-or-property-name at end of key expression";
      elseif (typeof(next) != STR)
        return "Non-string verb-or-property-name at end of key expression";
      else
        return {token, next};
      endif
    elseif (token in {"&&", "||"})
      return "Missing expression before '" + token + "'";
    elseif (token == "")
      return "Missing expression at end of key expression";
    else
      return this:match_object(token);
    endif
  endverb
endobject