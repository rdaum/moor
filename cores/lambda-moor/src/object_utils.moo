object OBJECT_UTILS
  name: "object utilities"
  parent: GENERIC_UTILS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  override aliases = {"object utilities"};
  override description = {
    "This is the object utilities utility package.  See `help $object_utils' for more details."
  };
  override help_msg = {
    "These routines are useful for finding out information about individual objects.",
    "",
    "Examining everything an object has defined on it:",
    "  all_verbs          (object) => like it says",
    "  all_properties     (object) => likewise",
    "  findable_properties(object) => tests to see if caller can \"find\" them",
    "  owned_properties   (object[, owner]) => tests for ownership",
    "",
    "Investigating inheritance:",
    "  ancestors(object[,object...]) => all ancestors",
    "  descendants      (object)     => all descendants",
    "  ordered_descendants(object)   => descendants, in a different order",
    "  leaves           (object)     => descendants with no children",
    "  branches         (object)     => descendants with children ",
    "  isa        (object,class) => true iff object is a descendant of class (or ==)",
    "  property_conflicts (object,newparent) => can object chparent to newparent?",
    "  isoneof     (object,list)     => true if object :isa class in list of parents",
    "",
    "Considering containment:",
    "  contains      (obj1, obj2) => Does obj1 contain obj2 (nested)?",
    "  all_contents      (object) => return all the (nested) contents of object",
    "  locations         (object) => list of location hierarchy above object",
    "",
    "Verifying verbs and properties:",
    "  has_property(object,pname) => false/true   according as object.(pname) exists",
    "  has_readable_property(object,pname) => false/true if prop exists and is +r",
    "  defines_property(object,pname) => does object *define* this property",
    "  has_verb    (object,vname) => false/{#obj} according as object:(vname) exists",
    "  has_callable_verb          => same, but verb must be callable from a program",
    "  defines_verb(object,vname) => does this object *define* this verb",
    "  match_verb  (object,vname) => false/{location, newvname}",
    "                               (identify location and usable name of verb)",
    "  accessible_verbs(object)   => a list of verb names (or E_PERM) regardless of ",
    "                                readability of object",
    "",
    "Player checking:",
    "  connected         (object) => true if object is a player and is connected",
    "",
    "Suspending:",
    "  Many of the above verbs have ..._suspended versions to assist with very large object hierarchies.  The following exist:",
    "   descendants_suspended              ",
    "   branches_suspended                 ",
    "   leaves_suspended                   ",
    "   all_properties_suspended           ",
    "   descendants_with_property_suspended"
  };
  override object_size = {21564, 1084848672};

  verb has_property (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Syntax:  has_property(OBJ, STR) => INT 0|1";
    "";
    "Does object have the specified property? Returns true if it is defined on the object or a parent.";
    {object, prop} = args;
    try
      object.(prop);
      return 1;
    except (E_PROPNF, E_INVIND)
      return 0;
    endtry
    "Old code...Ho_Yan 10/22/96";
    if (prop in $code_utils.builtin_props)
      return valid(object);
    else
      return !!property_info(object, prop);
    endif
  endverb

  verb "all_properties all_verbs" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Syntax:  all_properties (OBJ what)";
    "         all_verbs      (OBJ what)";
    "";
    "Returns all properties or verbs defined on `what' and all of its ancestors. Uses wizperms to get properties or verbs if the caller of this verb owns what, otherwise, uses caller's perms.";
    what = args[1];
    if (what.owner != caller_perms())
      set_task_perms(caller_perms());
    endif
    bif = verb == "all_verbs" ? "verbs" | "properties";
    res = `call_function(bif, what) ! E_PERM => {}';
    while (valid(what = parent(what)))
      res = {@`call_function(bif, what) ! E_PERM => {}', @res};
    endwhile
    return res;
  endverb

  verb has_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":has_verb(OBJ object, STR verbname)";
    "Find out if an object has a verb matching the given verbname.";
    "Returns {location} if so, 0 if not, where location is the object or the ancestor on which the verb is actually defined.";
    {object, verbname} = args;
    while (valid(object))
      try
        if (verb_info(object, verbname))
          return {object};
        endif
      except (E_VERBNF)
        object = parent(object);
      endtry
    endwhile
    return 0;
    "OLD CODE BELOW:";
    while (E_VERBNF == (vi = `verb_info(object, verbname) ! E_VERBNF, E_INVARG'))
      object = parent(object);
    endwhile
    return vi ? {object} | 0;
  endverb

  verb has_callable_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Usage:  has_callable_verb(object, verb)";
    "See if an object has a verb that can be called by another verb (i.e., that has its x permission bit set).";
    "Return {location}, where location is the object that defines the verb, or 0 if the object doesn't have the verb.";
    {object, verbname} = args;
    while (valid(object))
      if (`index(verb_info(object, verbname)[2], "x") ! E_VERBNF => 0' && verb_code(object, verbname))
        "Don't need to catch E_VERBNF in verb_code(), since it will never get there with the 0 &&";
        return {object};
      endif
      object = parent(object);
    endwhile
    return 0;
  endverb

  verb match_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":match_verb(OBJ object, STR verb)";
    "Find out if an object has a given verb, and some information about it.";
    "Returns {OBJ location, STR verb} if matched, 0 if not.";
    "Location is the object on which it is actually defined, verb is a name";
    "for the verb which can subsequently be used in verb_info (i.e., no";
    "asterisks).";
    verbname = strsub(args[2], "*", "");
    object = args[1];
    while (E_VERBNF == (info = `verb_info(object, verbname) ! E_VERBNF, E_INVARG'))
      object = parent(object);
    endwhile
    return info ? {object, verbname} | 0;
  endverb

  verb isa (this none this) owner: HACKER flags: "rxd"
    ":isa(x,y) == valid(x) && (y==x || y in :ancestors(x))";
    {what, targ} = args;
    while (valid(what))
      if (what == targ)
        return 1;
      endif
      what = parent(what);
    endwhile
    return 0;
  endverb

  verb ancestors (this none this) owner: HACKER flags: "rxd"
    "Usage:  ancestors(object[, object...])";
    "Return a list of all ancestors of the object(s) in args, with no duplicates.";
    "If called with a single object, the result will be in order ascending up the inheritance hierarchy.  If called with multiple objects, it probably won't.";
    ret = {};
    for o in (args)
      what = o;
      while (valid(what = parent(what)))
        ret = setadd(ret, what);
      endwhile
    endfor
    return ret;
  endverb

  verb ordered_descendants (this none this) owner: HACKER flags: "rxd"
    r = {what = args[1]};
    for k in (children(what))
      if (children(k))
        r = {@r, @this:(verb)(k)};
      else
        r = {@r, k};
      endif
    endfor
    return r;
  endverb

  verb contains (this none this) owner: HACKER flags: "rxd"
    "$object_utils:contains(obj1, obj2) -- does obj1 contain obj2?";
    "";
    "Return true iff obj2 is under obj1 in the containment hierarchy; that is, if obj1 is obj2's location, or its location's location, or ...";
    {loc, what} = args;
    while (valid(what))
      what = what.location;
      if (what == loc)
        return valid(loc);
      endif
    endwhile
    return 0;
  endverb

  verb all_contents (this none this) owner: HACKER flags: "rxd"
    "all_contents(object)";
    "Return a list of all objects contained (at some level) by object.";
    for y in (res = (args[1]).contents)
      y.contents && (res = {@res, @this:all_contents(y)});
    endfor
    return res;
  endverb

  verb findable_properties (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "findable_properties(object)";
    "Return a list of properties on those members of object's ancestor list that are readable or are owned by the caller (or all properties if the caller is a wizard).";
    what = args[1];
    props = {};
    who = caller_perms();
    while (what != $nothing)
      if (what.r || who == what.owner || who.wizard)
        props = {@properties(what), @props};
      endif
      what = parent(what);
    endwhile
    return props;
  endverb

  verb owned_properties (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "owned_properties(what[, who])";
    "Return a list of all properties on WHAT owned by WHO.";
    "Only wizardly verbs can specify WHO; mortal verbs can only search for properties owned by their own owners.  For more information, talk to Gary_Severn.";
    what = anc = args[1];
    who = (c = caller_perms()).wizard && length(args) > 1 ? args[2] | c;
    props = {};
    while (anc != $nothing)
      for k in (properties(anc))
        if (property_info(what, k)[1] == who)
          props = listappend(props, k);
        endif
      endfor
      anc = parent(anc);
    endwhile
    return props;
  endverb

  verb property_conflicts (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":property_conflicts(object,newparent)";
    "Looks for propertyname conflicts that would keep chparent(object,newparent)";
    "  from working.";
    "Returns a list of elements of the form {<propname>, @<objectlist>}";
    "where <objectlist> is list of descendents of object defining <propname>.";
    if (!valid(object = args[1]))
      return E_INVARG;
    elseif (!valid(newparent = args[2]))
      return newparent == #-1 ? {} | E_INVARG;
    elseif (!($perm_utils:controls(caller_perms(), object) && (newparent.f || $perm_utils:controls(caller_perms(), newparent))))
      "... if you couldn't chparent anyway, you don't need to know...";
      return E_PERM;
    endif
    "... properties existing on newparent";
    "... cannot be present on object or any descendent...";
    props = conflicts = {};
    for o in ({object, @$object_utils:descendents_suspended(object)})
      for p in (properties(o))
        if (`property_info(newparent, p) ! E_PROPNF => 0')
          if (i = p in props)
            conflicts[i] = {@conflicts[i], o};
          else
            props = {@props, p};
            conflicts = {@conflicts, {p, o}};
          endif
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      $command_utils:suspend_if_needed(0);
    endfor
    return conflicts;
  endverb

  verb descendants_with_property_suspended (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":descendants_with_property_suspended(object,property)";
    " => list of descendants of object on which property is defined.";
    "calls suspend(0) as needed";
    {object, prop} = args;
    if (caller == this || (object.w || $perm_utils:controls(caller_perms(), object)))
      $command_utils:suspend_if_needed(0);
      if (`property_info(object, prop) ! E_PROPNF => 0')
        return {object};
      endif
      r = {};
      for c in (children(object))
        r = {@r, @this:descendants_with_property_suspended(c, prop)};
      endfor
      return r;
    else
      return E_PERM;
    endif
  endverb

  verb locations (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Usage:  locations(object)";
    "Return a listing of the location hierarchy above object.";
    ret = {};
    what = args[1];
    while (valid(what = what.location))
      ret = {@ret, what};
    endwhile
    return ret;
  endverb

  verb "all_properties_suspended all_verbs_suspended" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Syntax:  all_properties_suspended (OBJ what)";
    "         all_verbs_suspended      (OBJ what)";
    "";
    "Returns all properties or verbs defined on `what' and all of its ancestors. Uses wizperms to get properties or verbs if the caller of this verb owns what, otherwise, uses caller's perms. Suspends as necessary";
    what = args[1];
    if (what.owner != caller_perms())
      set_task_perms(caller_perms());
    endif
    bif = verb == "all_verbs_suspended" ? "verbs" | "properties";
    res = `call_function(bif, what) ! E_PERM => {}';
    while (valid(what = parent(what)))
      res = {@`call_function(bif, what) ! E_PERM => {}', @res};
      $command_utils:suspend_if_needed(0);
    endwhile
    return res;
  endverb

  verb connected (this none this) owner: HACKER flags: "rxd"
    ":connected(object) => true if object is a connected player.";
    "equivalent to (object in connected_players()) for valid players, perhaps with less server overhead.";
    "use object:is_listening() if you want to allow for puppets and other non-player objects that still 'care' about what's said.";
    return typeof(`connected_seconds(@args) ! E_INVARG') == INT;
  endverb

  verb isoneof (this none this) owner: HACKER flags: "rxd"
    ":isoneof(x,y) = x isa z, for some z in list y";
    {what, targ} = args;
    while (valid(what))
      if (what in targ)
        return 1;
      endif
      what = parent(what);
    endwhile
    return 0;
  endverb

  verb defines_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Returns 1 if the verb is actually *defined* on this object, 0 else.";
    "Use this instead of :has_verb if your aim is to manipulate that verb code or whatever.";
    return `verb_info(@args) ! ANY => 0' && 1;
    "Old code below...Ho_Yan 10/22/96";
    info = verb_info(@args);
    return typeof(info) != ERR;
  endverb

  verb defines_property (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":defines_property(OBJ object, STR property name) => Returns 1 if the property is actually *defined* on the object given";
    if (!valid(o = args[1]))
      return 0;
    elseif (!valid(p = parent(o)))
      return this:has_property(o, args[2]);
    else
      return !this:has_property(p, args[2]) && this:has_property(o, args[2]);
    endif
  endverb

  verb "has_any_verb has_any_property" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":has_any_verb(object) / :has_any_property(object)";
    " -- does `object' have any verbs/properties?";
    return !!`verb == "has_any_verb" ? verbs(args[1]) | properties(args[1]) ! E_INVARG => 0';
  endverb

  verb "has_readable_prop*erty hrp" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":has_readable_property(OBJ object, STR property name) => 1 if property exists and is publically readable (has the r flag set true).";
    {object, prop} = args;
    try
      pinfo = property_info(object, prop);
      return index(pinfo[2], "r") != 0;
    except (E_PROPNF)
      return prop in $code_utils.builtin_props > 0;
    endtry
  endverb

  verb "descendants descendents" (this none this) owner: HACKER flags: "rxd"
    ":descendants (OBJ object) => {OBJs} all nested children of <object>";
    r = children(args[1]);
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r = {@r, @kids};
      endif
      i = i + 1;
    endwhile
    return r;
  endverb

  verb leaves (this none this) owner: HACKER flags: "rxd"
    ":leaves (OBJ object) => {OBJs} descendants of <object> that have no children";
    r = {args[1]};
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r[i..i] = kids;
      else
        i = i + 1;
      endif
    endwhile
    return r;
  endverb

  verb branches (this none this) owner: HACKER flags: "rxd"
    ":branches (OBJ object) => {OBJs} descendants of <object> that have children";
    r = args[1..1];
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r[i + 1..i] = kids;
        i = i + 1;
      else
        r[i..i] = {};
      endif
    endwhile
    return r;
  endverb

  verb "descendants_suspended descendents_suspended" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":descendants_suspended (OBJ object) => {OBJs} all nested children of <object>";
    set_task_perms(caller_perms());
    r = children(args[1]);
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r = {@r, @kids};
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endwhile
    return r;
  endverb

  verb leaves_suspended (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":leaves_suspended (OBJ object) => {OBJs} descendants of <object> that have";
    "                                         no children";
    set_task_perms(caller_perms());
    r = {args[1]};
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r[i..i] = kids;
      else
        i = i + 1;
      endif
      $command_utils:suspend_if_needed(0);
    endwhile
    return r;
  endverb

  verb branches_suspended (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":branches_suspended (OBJ object) => {OBJs} all descendants of <object> that";
    "                                           have children.";
    set_task_perms(caller_perms());
    r = args[1..1];
    i = 1;
    while (i <= length(r))
      if (kids = children(r[i]))
        r[i + 1..i] = kids;
        i = i + 1;
      else
        r[i..i] = {};
      endif
      $command_utils:suspend_if_needed(0);
    endwhile
    return r;
  endverb

  verb "disown disinherit" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":disown(object) / :disinherit(object)";
    " => 1 (for a successful disinheritance)";
    " raises E_PERM, E_INVARG, E_ARGS";
    cp = caller_perms();
    "     no set_task_perms() because we need to be able to";
    "     chparent() an object we don't own";
    {victim} = args;
    parent = parent(victim);
    if ($perm_utils:controls(cp, victim))
      raise(E_INVARG, tostr(victim.name, " (", victim, ") is yours.  Use @chparent."));
    elseif (!valid(parent))
      raise(E_INVARG, tostr(victim.name, " (", victim, ") is already an orphan."));
    elseif (!$perm_utils:controls(cp, parent))
      raise(E_PERM, tostr(parent.name, " (", parent, "), the parent of ", victim.name, " (", victim, "), is not yours."));
    elseif (!valid(grandparent = parent(parent)))
      "... still not sure about this... do we care?  --Rog...";
      raise(E_INVARG, tostr(victim.name, " (", victim, ") has no grandparent to take custody."));
    else
      chparent(victim, grandparent);
      return 1;
    endif
  endverb

  verb accessible_verbs (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "  accessible_verbs(object)   => a list of verb names (or E_PERM) regardless of readability of object";
    {thing} = args;
    valid(thing) || raise(E_INVARG, "Invalid object argument");
    {num_verbs, verbs} = {length(verbs(thing)), {}};
    set_task_perms(caller_perms());
    "... caching num of verbs before for loop saves us ticks ...";
    for i in [1..num_verbs]
      verbs = {@verbs, `verb_info(thing, i)[3] ! E_PERM'};
    endfor
    return verbs;
  endverb

  verb "accessible_prop*erties accessible_props" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    " :accessible_props(object)   => a list of property names (or E_PERM), regardless of the readability of the object.";
    thing = args[1];
    all = properties(thing);
    props = {};
    set_task_perms(caller_perms());
    for i in (all)
      $command_utils:suspend_if_needed(0);
      if ((info = `property_info(thing, i) ! ANY') != E_PROPNF)
        props = {@props, info ? i | E_PERM};
      endif
    endfor
    return props;
    "Last modified Mon Nov 28 06:19:35 2005 PST, by Roebare (#109000).";
  endverb
endobject