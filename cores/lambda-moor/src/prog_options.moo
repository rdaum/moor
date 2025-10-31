object PROG_OPTIONS
  name: "Programmer Options"
  parent: GENERIC_OPTIONS
  owner: HACKER
  readable: true

  property show_copy_expert (owner: HACKER, flags: "rc") = {"@copy prints warning message.", "@copy omits warning message."};
  property show_eval_time (owner: HACKER, flags: "rc") = {
    "eval does not show ticks/seconds consumed.",
    "eval shows ticks/seconds consumed."
  };
  property show_list_all_parens (owner: HACKER, flags: "rc") = {
    "@list shows only necessary parentheses by default",
    "@list shows all parentheses by default"
  };
  property show_list_no_numbers (owner: HACKER, flags: "rc") = {"@list gives line numbers by default", "@list omits line numbers by default"};
  property show_list_show_permissions (owner: HACKER, flags: "rc") = {
    "@list does not display permissions in header",
    "@list displays permissions in header"
  };
  property show_rmverb_mail_backup (owner: HACKER, flags: "rc") = {
    "@rmverb does not email you a backup",
    "@rmverb emails you a backup before deleting the verb"
  };
  property "type_@prop_flags" (owner: HACKER, flags: "rc") = {2};

  override _namelist = "!list_all_parens!list_no_numbers!list_show_permissions!eval_time!copy_expert!list_numbers!verb_args!@prop_flags!rmverb_mail_backup!";
  override aliases = {"Programmer Options"};
  override description = {"Option package for $prog commands.  See `help @prog-options'."};
  override extras = {"list_numbers"};
  override import_export_id = "prog_options";
  override names = {
    "list_all_parens",
    "list_no_numbers",
    "eval_time",
    "copy_expert",
    "verb_args",
    "@prop_flags",
    "list_show_permissions",
    "rmverb_mail_backup"
  };
  override object_size = {5196, 1084848672};

  verb actual (this none this) owner: HACKER flags: "rxd"
    if (i = args[1] in {"list_numbers"})
      return {{{"list_no_numbers"}[i], !args[2]}};
    else
      return {args};
    endif
  endverb

  verb show (this none this) owner: HACKER flags: "rxd"
    if (o = (name = args[2]) in {"list_numbers"})
      args[2] = {"list_no_numbers"}[o];
      return {@pass(@args), tostr("(", name, " is a synonym for -", args[2], ")")};
    else
      return pass(@args);
    endif
  endverb

  verb show_verb_args (this none this) owner: HACKER flags: "rxd"
    if (value = this:get(@args))
      return {value, {tostr("Default args for @verb:  ", $string_utils:from_list(value, " "))}};
    else
      return {0, {"Default args for @verb:  none none none"}};
    endif
  endverb

  verb check_verb_args (this none this) owner: HACKER flags: "rxd"
    value = args[1];
    if (typeof(value) != LIST)
      return "List expected";
    elseif (length(value) != 3)
      return "List of length 3 expected";
    elseif (!(value[1] in {"this", "none", "any"}))
      return tostr("Invalid dobj specification:  ", value[1]);
    elseif (!((p = $code_utils:short_prep(value[2])) || value[2] in {"none", "any"}))
      return tostr("Invalid preposition:  ", value[2]);
    elseif (!(value[3] in {"this", "none", "any"}))
      return tostr("Invalid iobj specification:  ", value[3]);
    else
      if (p)
        value[2] = p;
      endif
      return {value};
    endif
  endverb

  verb parse_verb_args (this none this) owner: HACKER flags: "rxd"
    {oname, raw, data} = args;
    if (typeof(raw) == STR)
      raw = $string_utils:explode(raw, " ");
    elseif (typeof(raw) == INT)
      return raw ? {oname, {"this", "none", "this"}} | {oname, 0};
    endif
    value = $code_utils:parse_argspec(@raw);
    if (typeof(value) != LIST)
      return tostr(value);
    elseif (value[2])
      return tostr("I don't understand \"", $string_utils:from_list(value[2], " "), "\"");
    else
      value = {@value[1], "none", "none", "none"}[1..3];
      return {oname, value == {"none", "none", "none"} ? 0 | value};
    endif
  endverb

  verb "show_@prop_flags" (this none this) owner: HACKER flags: "rxd"
    value = this:get(@args);
    if (value)
      return {value, {tostr("Default permissions for @property=`", value, "'.")}};
    else
      return {0, {"Default permissions for @property=`rc'."}};
    endif
  endverb

  verb "check_@prop_flags" (this none this) owner: #2 flags: "rxd"
  endverb

  verb "parse_@prop_flags" (this none this) owner: #2 flags: "rxd"
    {oname, raw, data} = args;
    if (typeof(raw) != STR)
      return "Must be a string composed of the characters `rwc'.";
    endif
    len = length(raw);
    for x in [1..len]
      if (!(raw[x] in {"r", "w", "c"}))
        return "Must be a string composed of the characters `rwc'.";
      endif
    endfor
    return {oname, raw};
  endverb
endobject