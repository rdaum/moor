object SET_UTILS
  name: "Set Utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  override aliases = {"Set Utilities", "set_utilities"};
  override description = {
    "This is the Set Utilities utility package.  See `help $set_utils' for more details."
  };
  override help_msg = {
    "This object is useful for operations that treat lists as sets (i.e.,",
    "without concern about order and assuming no duplication).",
    "",
    " union(set, set, ...)        => union",
    " intersection(set, set, ...) => intersection",
    " intersection_preserve_case(base set, set, set, ...)",
    "        => intersection with the case of the base set's elements preserved",
    "",
    " diff*erence(set1, set2, ..., setn)",
    "        => result of removing all elements of sets 2..n from set 1.",
    "",
    " difference_suspended(set1, set2, ..., setn)",
    "        => same as above except it suspends as needed.",
    "",
    " exclusive_or(set, set, set, ...)",
    "        => all elements that are contained in exactly one of the sets",
    "",
    " contains(set1, set2, ..., setn)",
    "        => true if and only if all of sets 2..n are subsets of set 1",
    "",
    " equal(set1, set2)",
    "        => true if and only if set1 and set2 are equal"
  };
  override object_size = {5574, 1084848672};

  verb union (this none this) owner: HACKER flags: "rxd"
    "Returns the set union of all of the lists provided as arguments.";
    if (!args)
      return {};
    endif
    {set, @rest} = args;
    for l in (rest)
      for x in (l)
        set = setadd(set, x);
      endfor
    endfor
    return set;
  endverb

  verb intersection (this none this) owner: HACKER flags: "rxd"
    "Returns the set intersection of all the lists provided as arguments.";
    if (!args)
      return {};
    endif
    max = 0;
    {result, @rest} = args;
    for set in (rest)
      if (length(result) < length(set))
        set1 = result;
        set2 = set;
      else
        set1 = set;
        set2 = result;
      endif
      for x in (set1)
        if (!(x in set2))
          set1 = setremove(set1, x);
        endif
      endfor
      result = set1;
    endfor
    return result;
  endverb

  verb "diff*erence" (this none this) owner: HACKER flags: "rxd"
    "Usage:  diff(set 1, set 2, ..., set n)";
    "Returns all elements of set 1 that are not in sets 2..n";
    {set, @rest} = args;
    for l in (rest)
      for x in (l)
        set = setremove(set, x);
      endfor
    endfor
    return set;
  endverb

  verb contains (this none this) owner: HACKER flags: "rxd"
    "True if the first list given is a superset of all subsequent lists.";
    "False otherwise.  {} is a superset of {} and nothing else; anything is";
    "a superset of {}.  If only one list is given, return true.";
    {?super = {}, @rest} = args;
    for l in (rest)
      for x in (l)
        if (!(x in super))
          return 0;
        endif
      endfor
    endfor
    return 1;
  endverb

  verb "exclusive_or xor" (this none this) owner: HACKER flags: "rxd"
    "Usage:  exclusive_or(set, set, ...)";
    "Return the set of all elements that are in exactly one of the input sets";
    "For two sets, this is the equivalent of (A u B) - (A n B).";
    if (!args)
      return {};
    endif
    {set, @rest} = args;
    so_far = set;
    for l in (rest)
      for x in (l)
        if (x in so_far)
          set = setremove(set, x);
        else
          set = setadd(set, x);
        endif
      endfor
      so_far = {@so_far, @l};
    endfor
    return set;
  endverb

  verb "difference_suspended diff_suspended" (this none this) owner: HACKER flags: "rxd"
    "Usage:  diff_suspended(set 1, set 2, ..., set n)";
    "Returns all elements of set 1 that are not in sets 2..n";
    "Suspends as needed if the lists are large.";
    {set, @rest} = args;
    for l in (rest)
      for x in (l)
        set = setremove(set, x);
        $command_utils:suspend_if_needed(0);
      endfor
    endfor
    return set;
  endverb

  verb equal (this none this) owner: HACKER flags: "rxd"
    "True if the two lists given contain the same elements.";
    "False otherwise.";
    {set1, set2} = args;
    while (set1)
      {elt, @set1} = set1;
      if (elt in set2)
        set2 = setremove(set2, elt);
        while (elt in set2)
          set2 = setremove(set2, elt);
        endwhile
        while (elt in set1)
          set1 = setremove(set1, elt);
        endwhile
      else
        return 0;
      endif
    endwhile
    if (set2)
      return 0;
    else
      return 1;
    endif
  endverb

  verb intersection_preserve_case (this none this) owner: HACKER flags: "rxd"
    "Copied from Fox (#54902):intersection Mon Dec 27 17:02:57 1993 PST";
    "a version of $set_utils:intersection that maintains the property that everything in the return value is in the first argument, even considering case";
    if (!args)
      return {};
    endif
    {result, @rest} = args;
    for s in (rest)
      for x in (result)
        if (!(x in s))
          result = setremove(result, x);
        endif
      endfor
    endfor
    return result;
  endverb
endobject