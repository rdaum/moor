object EDIT_OPTIONS
  name: "Edit Options"
  parent: GENERIC_OPTIONS
  owner: HACKER
  readable: true

  property show_eval_subs (owner: HACKER, flags: "rc") = {
    "Ignore .eval_subs when compiling verbs.",
    "Use .eval_subs when compiling verbs."
  };
  property show_local (owner: HACKER, flags: "rc") = {"Use in-MOO text editors.", "Ship text to client for local editing."};
  property show_no_parens (owner: HACKER, flags: "rc") = {
    "include all parentheses when fetching verbs.",
    "includes only necessary parentheses when fetching verbs."
  };
  property show_quiet_insert (owner: HACKER, flags: "rc") = {"Report line numbers on insert or append.", "No echo on insert or append."};

  override _namelist = "!quiet_insert!eval_subs!local!no_parens!parens!noisy_insert!";
  override aliases = {"Edit Options"};
  override extras = {"parens", "noisy_insert"};
  override names = {"quiet_insert", "eval_subs", "local", "no_parens"};
  override namewidth = 20;
  override object_size = {1856, 1084848672};

  verb actual (this none this) owner: HACKER flags: "rxd"
    if (i = args[1] in {"parens", "noisy_insert"})
      return {{{"no_parens", "quiet_insert"}[i], !args[2]}};
    else
      return {args};
    endif
  endverb

  verb show (this none this) owner: HACKER flags: "rxd"
    if (o = (name = args[2]) in {"parens", "noisy_insert"})
      args[2] = {"no_parens", "quiet_insert"}[o];
      return {@pass(@args), tostr("(", name, " is a synonym for -", args[2], ")")};
    else
      return pass(@args);
    endif
  endverb
endobject