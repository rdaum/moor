object PERM_UTILS
  name: "permissions utilities"
  parent: GENERIC_UTILS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  override description = {
    "This is the permissions utilities utility package.  See `help $perm_utils' for more details."
  };
  override help_msg = {
    "Miscellaneous routines for permissions checking",
    "",
    "For a complete description of a given verb, do `help $perm_utils:verbname'",
    "",
    ":controls(who,what) -- can who write on object what",
    ":controls_property(who,what,propname) -- can who write on what.propname",
    "These routines check write flags and also the wizardliness of `who'.",
    "",
    "(these last two probably belong on $code_utils)",
    "",
    ":apply(permstring,mods)",
    "  -- used by @chmod to apply changes (e.g., +x) ",
    "     to a given permissions string",
    "",
    ":caller()",
    "  -- returns the first caller in the callers() stack distinct from `this'"
  };
  override object_size = {3491, 1084848672};

  verb controls (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$perm_utils:controls(who, what)";
    "Is WHO allowed to hack on WHAT?";
    {who, what} = args;
    return valid(who) && valid(what) && (who.wizard || who == what.owner);
  endverb

  verb apply (this none this) owner: HACKER flags: "rxd"
    ":apply(permstring,mods) => new permstring.";
    "permstring is a permissions string, mods is a concatenation of strings of the form +<letters>, !<letters>, or -<letters>, where <letters> is a string of letters as might appear in a permissions string (`+' adds the specified permissions, `-' or `!' removes them; `-' and `!' are entirely equivalent).";
    {perms, mods} = args;
    if (!mods || !index("!-+", mods[1]))
      return mods;
    endif
    i = 1;
    while (i <= length(mods))
      if (mods[i] == "+")
        while ((i = i + 1) <= length(mods) && !index("!-+", mods[i]))
          if (!index(perms, mods[i]))
            perms = perms + mods[i];
          endif
        endwhile
      else
        "mods[i] must be ! or -";
        while ((i = i + 1) <= length(mods) && !index("!-+", mods[i]))
          perms = strsub(perms, mods[i], "");
        endwhile
      endif
    endwhile
    return perms;
  endverb

  verb caller (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":caller([include line numbers])";
    "  -- returns the first caller in the callers() stack distinct from `this'";
    {?lineno = 0} = args;
    c = callers(lineno);
    {stage, lc, nono} = {1, length(c), {c[1][1], $nothing}};
    while ((stage = stage + 1) <= lc && c[stage][1] in nono)
    endwhile
    return c[stage];
  endverb

  verb "controls_prop*erty controls_verb" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Syntax:  controls_prop(OBJ who, OBJ what, STR propname)   => 0 | 1";
    "         controls_verb(OBJ who, OBJ what, STR verbname)   => 0 | 1";
    "";
    "Is WHO allowed to hack on WHAT's PROPNAME? Or VERBNAME?";
    {who, what, name} = args;
    bi = verb == "controls_verb" ? "verb_info" | "property_info";
    return who.wizard || who == call_function(bi, what, name)[1];
  endverb
endobject