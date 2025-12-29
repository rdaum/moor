object LIST_UTILS
  name: "list utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property nonstring_tell_lines (owner: HACKER, flags: "r") = {};

  override aliases = {"list_utilities"};
  override description = {
    "This is the list utilities utility package.  See `help $list_utils' for more details."
  };
  override help_msg = {
    "append            (list,list,..) => result of concatenating the given lists",
    "reverse           (list)         => reversed list",
    "remove_duplicates (list)         => list with all duplicates removed",
    "compress          (list)         => list with consecutive duplicates removed",
    "setremove_all     (list,elt)     => list with all occurrences of elt removed",
    "find_insert       (sortedlist,e) => index of first element > e in sortedlist",
    "sort              (list[,keys])  => sorted list",
    "count             (elt,list)     => count of elt found in list.",
    "flatten           (list)         => flatten all recursive lists into one list",
    "randomly_permute  (list)         => list with elements randomly permuted",
    "longest           (list)         => longest in list (consisting of str or list)",
    "shortest          (list)         => shortest in list (as above)",
    "",
    "make              (n[,e])        => list of n copies of e",
    "range             (m,n)          => {m,m+1,...,n}",
    "",
    "arrayset   (list,val,i[,j,k...]) => array modified so that list[i][j][k]==val",
    "",
    "-- Mapping functions (take a list and do something to each element):",
    "",
    "map_prop ({o...},prop)              => list of o.(prop)            for all o",
    "map_verb ({o...},verb[,args])        => list of o:(verb)(@args)     for all o",
    "map_arg  ([n,]obj,verb,{a...},args) => list of obj:(verb)(a,@args) for all a",
    "map_builtin (objectlist, function)  => applies function to all in objectlist",
    "",
    "-- Association list functions --",
    "",
    "An association list (alist) is a list of pairs (2-element lists), though the following functions have been generalized for lists of n-tuples (n-element lists).  In each case i defaults to 1.",
    "",
    "assoc        (targ,alist[,i]) => 1st tuple in alist whose i-th element is targ",
    "iassoc       (targ,alist[,i]) => index of same.",
    "assoc_prefix (targ,alist[,i]) => ... whose i-th element has targ as a prefix",
    "iassoc_prefix(targ,alist[,i]) => index of same.",
    "iassoc_sorted(targ,slist[,i]) => index of last element in sortedlist <= targ",
    "slice             (alist[,i]) => list of i-th elements",
    "sort_alist        (alist[,i]) => alist sorted on i-th elements.",
    "amerge  (alist,[tind,[dind]]) => merges tuples of alist with matching i-th elt",
    "build_alist          (list,N) => make an alist of N-intervals from list",
    "",
    "-- Functions that suspend --",
    "",
    "Each of these either suspends(0) as needed or takes an interval in seconds for the suspend as a first argument. See help $list_utils:<verb>.",
    "",
    "sort_suspended          iassoc_suspended          sort_alist_suspended",
    "reverse_suspended       randomly_permute_suspended"
  };
  override import_export_id = "list_utils";
  override object_size = {29031, 1084848672};

  verb make (this none this) owner: HACKER flags: "rxd"
    ":make(n[,elt]) => a list of n elements, each of which == elt. elt defaults to 0.";
    {n, ?elt = 0} = args;
    if (n < 0)
      return E_INVARG;
    endif
    ret = {};
    build = {elt};
    while (1)
      if (n % 2)
        ret = {@ret, @build};
      endif
      if (n = n / 2)
        build = {@build, @build};
      else
        return ret;
      endif
    endwhile
  endverb

  verb range (this none this) owner: HACKER flags: "rxd"
    ":range([m,]n) => {m,m+1,...,n}";
    {?m = 1, n} = args;
    ret = {};
    for k in [m..n]
      ret = {@ret, k};
    endfor
    return ret;
  endverb

  verb "map_prop*erty" (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    {objs, prop} = args;
    if (length(objs) > 50)
      return {@this:map_prop(objs[1..$ / 2], prop), @this:map_prop(objs[$ / 2 + 1..$], prop)};
    endif
    strs = {};
    for foo in (objs)
      strs = {@strs, foo.(prop)};
    endfor
    return strs;
  endverb

  verb map_verb (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    {objs, vrb, @rest} = args;
    if (length(objs) > 50)
      return {@this:map_verb(@listset(args, objs[1..$ / 2], 1)), @this:map_verb(@listset(args, objs[$ / 2 + 1..$], 1))};
    endif
    strs = {};
    for o in (objs)
      strs = {@strs, o:(vrb)(@rest)};
    endfor
    return strs;
  endverb

  verb "map_arg*s" (this none this) owner: #2 flags: "rxd"
    "map_arg([n,]object,verb,@args) -- assumes the nth element of args is a list, calls object:verb(@args) with each element of the list substituted in turn, returns the list of results.  n defaults to 1.";
    "map_verb_arg(o,v,{a...},a2,a3,a4,a5)={o:v(a,a2,a3,a4,a5),...}";
    "map_verb_arg(4,o,v,a1,a2,a3,{a...},a5)={o:v(a1,a2,a3,a,a5),...}";
    set_task_perms(caller_perms());
    if (n = args[1])
      {object, verb, @rest} = args[2..$];
    else
      object = n;
      n = 1;
      {verb, @rest} = args[2..$];
    endif
    results = {};
    for a in (rest[n])
      results = listappend(results, object:(verb)(@listset(rest, a, n)));
    endfor
    return results;
  endverb

  verb map_builtin (this none this) owner: #2 flags: "rxd"
    ":map_builtin(objectlist,func) applies func to each of the objects in turn and returns the corresponding list of results.  This function is mainly here for completeness -- in the vast majority of situations, a simple for loop is better.";
    set_task_perms(caller_perms());
    {objs, builtin} = args;
    if (!`function_info(builtin) ! E_INVARG => 0')
      return E_INVARG;
    endif
    if (length(objs) > 100)
      return {@this:map_builtin(objs[1..$ / 2], builtin), @this:map_builtin(objs[$ / 2 + 1..$], builtin)};
    endif
    strs = {};
    for foo in (objs)
      strs = {@strs, call_function(builtin, foo)};
    endfor
    return strs;
  endverb

  verb find_insert (this none this) owner: HACKER flags: "rxd"
    "find_insert(sortedlist,key) => index of first element in sortedlist > key";
    "  sortedlist is assumed to be sorted in increasing order and the number returned is anywhere from 1 to length(sortedlist)+1, inclusive.";
    {lst, key} = args;
    if ((r = length(lst)) < 25)
      for l in [1..r]
        if (lst[l] > key)
          return l;
        endif
      endfor
      return r + 1;
    else
      l = 1;
      while (r >= l)
        if (key < lst[i = (r + l) / 2])
          r = i - 1;
        else
          l = i + 1;
        endif
      endwhile
      return l;
    endif
  endverb

  verb remove_duplicates (this none this) owner: HACKER flags: "rxd"
    "remove_duplicates(list) => list as a set, i.e., all repeated elements removed.";
    out = {};
    for x in (args[1])
      out = setadd(out, x);
    endfor
    return out;
  endverb

  verb arrayset (this none this) owner: HACKER flags: "rxd"
    "arrayset(list,value,pos1,...,posn) -- returns list modified such that";
    "  list[pos1][pos2][...][posn] == value";
    if (length(args) > 3)
      return listset(@listset(args[1..3], this:arrayset(@listset(listdelete(args, 3), args[1][args[3]], 1)), 2));
      "... Rog's entry in the Obfuscated MOO-Code Contest...";
    else
      return listset(@args);
    endif
  endverb

  verb setremove_all (this none this) owner: HACKER flags: "rxd"
    ":setremove_all(set,elt) => set with *all* occurences of elt removed";
    {set, what} = args;
    while (w = what in set)
      set[w..w] = {};
    endwhile
    return set;
  endverb

  verb append (this none this) owner: HACKER flags: "rxd"
    "append({a,b,c},{d,e},{},{f,g,h},...) =>  {a,b,c,d,e,f,g,h}";
    if (length(args) > 50)
      return {@this:append(@args[1..$ / 2]), @this:append(@args[$ / 2 + 1..$])};
    endif
    l = {};
    for a in (args)
      l = {@l, @a};
    endfor
    return l;
  endverb

  verb reverse (this none this) owner: HACKER flags: "rxd"
    "reverse(list) => reversed list";
    return reverse(args[1]);
  endverb

  verb compress (this none this) owner: HACKER flags: "rxd"
    "compress(list) => list with consecutive repeated elements removed, e.g.,";
    "compress({a,b,b,c,b,b,b,d,d,e}) => {a,b,c,b,d,e}";
    if (l = args[1])
      out = {last = l[1]};
      for x in (listdelete(l, 1))
        if (x != last)
          out = listappend(out, x);
          last = x;
        endif
      endfor
      return out;
    else
      return l;
    endif
  endverb

  verb sort (this none this) owner: HACKER flags: "rxd"
    "sort(list[,keys][,natural][,reverse]) => sorts list, optionally using parallel keys for sort order. Strings are compared case-insensitively. If natural is true, uses natural string ordering (e.g., file2.txt < file10.txt). If reverse is true, reverses the sort order.";
    "sort({x1,x3,x2},{1,3,2}) => {x1,x2,x3}";
    return sort(@args);
  endverb

  verb sort_suspended (this none this) owner: #2 flags: "rxd"
    ":sort_suspended(interval,list[,keys]) => sorts keys (assumed to be all numbers or strings) and returns list with the corresponding permutation applied to it.  keys defaults to the list itself.";
    "does suspend(interval) as needed.";
    set_task_perms(caller_perms());
    interval = args[1];
    if (typeof(interval) != TYPE_INT)
      return E_ARGS;
    endif
    lst = args[2];
    unsorted_keys = (use_sorted_lst = length(args) >= 3) ? args[3] | lst;
    sorted_lst = sorted_keys = {};
    for e in (unsorted_keys)
      l = this:find_insert(sorted_keys, e);
      sorted_keys[l..l - 1] = {e};
      if (use_sorted_lst)
        sorted_lst[l..l - 1] = {lst[length(sorted_keys)]};
      endif
      $command_utils:suspend_if_needed(interval);
    endfor
    return sorted_lst || sorted_keys;
  endverb

  verb slice (this none this) owner: HACKER flags: "rxd"
    "slice(alist[,index]) returns a list of the index-th elements of the elements of alist, e.g., ";
    "    slice({{\"z\",1},{\"y\",2},{\"x\",5}},2) => {1,2,5}.";
    "index defaults to 1 and may also be a nonempty list, e.g., ";
    "    slice({{\"z\",1,3},{\"y\",2,4}},{2,1}) => {{1,\"z\"},{2,\"y\"}}";
    {thelist, ?ind = 1} = args;
    slice = {};
    if (typeof(ind) == TYPE_LIST)
      for elt in (thelist)
        s = {elt[ind[1]]};
        for i in (listdelete(ind, 1))
          s = {@s, elt[i]};
        endfor
        slice = {@slice, s};
      endfor
    else
      for elt in (thelist)
        slice = {@slice, elt[ind]};
      endfor
    endif
    return slice;
  endverb

  verb assoc (this none this) owner: HACKER flags: "rxd"
    "assoc(target,list[,index]) returns the first element of `list' whose own index-th element is target.  Index defaults to 1.";
    "returns {} if no such element is found";
    {target, thelist, ?indx = 1} = args;
    for t in (thelist)
      if (typeof(t) == TYPE_LIST && `t[indx] == target ! E_RANGE => 0')
        return t;
      endif
    endfor
    return {};
  endverb

  verb iassoc (this none this) owner: HACKER flags: "rxd"
    "Copied from Moo_tilities (#332):iassoc by Mooshie (#106469) Wed Mar 18 19:27:51 1998 PST";
    "Usage: iassoc(ANY target, LIST list [, INT index ]) => Returns the index of the first element of `list' whose own index-th element is target.  Index defaults to 1.";
    "Returns 0 if no such element is found.";
    {target, thelist, ?indx = 1} = args;
    for element in (thelist)
      if (`element[indx] == target ! E_RANGE, E_TYPE => 0')
        if (typeof(element) == TYPE_LIST)
          return element in thelist;
        endif
      endif
    endfor
    return 0;
  endverb

  verb iassoc_suspended (this none this) owner: #2 flags: "rxd"
    "Copied from Moo_tilities (#332):iassoc_suspended by Mooshie (#106469) Wed Mar 18 19:27:53 1998 PST";
    "Usage: iassoc_suspended(ANY target, LIST list [, INT index [, INT suspend-for ]]) => Returns the index of the first element of `list' whose own index-th element is target. Index defaults to 1.";
    "Returns 0 if no such element is found.";
    "Suspends as needed. Suspend length defaults to 0.";
    set_task_perms(caller_perms());
    {target, thelist, ?indx = 1, ?suspend_for = 0} = args;
    cu = $command_utils;
    for element in (thelist)
      if (`element[indx] == target ! E_RANGE, E_TYPE => 0' && typeof(element) == TYPE_LIST)
        return element in thelist;
      endif
      cu:suspend_if_needed(suspend_for);
    endfor
    return 0;
    "Mooshie (#106469) - Tue Feb 10, PST - :assoc_suspended does a set_task_perms, why shouldn't this?";
  endverb

  verb assoc_prefix (this none this) owner: HACKER flags: "rxd"
    "assoc_prefix(target,list[,index]) returns the first element of `list' whose own index-th element has target as a prefix.  Index defaults to 1.";
    {target, thelist, ?indx = 1} = args;
    for t in (thelist)
      if (typeof(t) == TYPE_LIST && (length(t) >= indx && index(t[indx], target) == 1))
        return t;
      endif
    endfor
    return {};
  endverb

  verb iassoc_prefix (this none this) owner: HACKER flags: "rxd"
    "iassoc_prefix(target,list[,index]) returns the index of the first element of `list' whose own index-th element has target as a prefix.  Index defaults to 1.";
    {target, lst, ?indx = 1} = args;
    for i in [1..length(lst)]
      if (typeof(lsti = lst[i]) == TYPE_LIST && (length(lsti) >= indx && index(lsti[indx], target) == 1))
        return i;
      endif
    endfor
    return 0;
  endverb

  verb iassoc_sorted (this none this) owner: HACKER flags: "rxd"
    "iassoc_sorted(target,sortedlist[,i]) => index of last element in sortedlist whose own i-th element is <= target.  i defaults to 1.";
    "  sortedlist is assumed to be sorted in increasing order and the number returned is anywhere from 0 to length(sortedlist), inclusive.";
    {target, lst, ?indx = 1} = args;
    if ((r = length(lst)) < 25)
      for l in [1..r]
        if (target < lst[l][indx])
          return l - 1;
        endif
      endfor
      return r;
    else
      l = 0;
      r = r + 1;
      while (r - 1 > l)
        if (target < lst[i = (r + l) / 2][indx])
          r = i;
        else
          l = i;
        endif
      endwhile
      return l;
    endif
  endverb

  verb sort_alist (this none this) owner: HACKER flags: "rxd"
    ":sort_alist(alist[,n]) sorts a list of tuples by n-th (1st) element.";
    {alist, ?sort_on = 1} = args;
    if ((alist_length = length(alist)) < 25)
      "use insertion sort on short lists";
      return this:sort(alist, this:slice(@args));
    endif
    left_index = alist_length / 2;
    right_index = (alist_length + 1) / 2;
    left_sublist = this:sort_alist(alist[1..left_index], sort_on);
    right_sublist = this:sort_alist(alist[left_index + 1..alist_length], sort_on);
    "...";
    "... merge ...";
    "...";
    left_key = left_sublist[left_index][sort_on];
    right_key = right_sublist[right_index][sort_on];
    if (left_key > right_key)
      merged_list = {};
    else
      "... alist_length >= 25 implies right_index >= 2...";
      "... move right_index downward until left_key > right_key...";
      r = right_index - 1;
      while (left_key <= (right_key = right_sublist[r][sort_on]))
        if (r = r - 1)
        else
          return {@left_sublist, @right_sublist};
        endif
      endwhile
      merged_list = right_sublist[r + 1..right_index];
      right_index = r;
    endif
    while (l = left_index - 1)
      "... left_key > right_key ...";
      "... move left_index downward until left_key <= right_key...";
      while ((left_key = left_sublist[l][sort_on]) > right_key)
        if (l = l - 1)
        else
          return {@right_sublist[1..right_index], @left_sublist[1..left_index], @merged_list};
        endif
      endwhile
      merged_list[1..0] = left_sublist[l + 1..left_index];
      left_index = l;
      "... left_key <= right_key ...";
      if (r = right_index - 1)
        "... move right_index downward until left_key > right_key...";
        while (left_key <= (right_key = right_sublist[r][sort_on]))
          if (r = r - 1)
          else
            return {@left_sublist[1..left_index], @right_sublist[1..right_index], @merged_list};
          endif
        endwhile
        merged_list[1..0] = right_sublist[r + 1..right_index];
        right_index = r;
      else
        return {@left_sublist[1..left_index], right_sublist[1], @merged_list};
      endif
    endwhile
    return {@right_sublist[1..right_index], left_sublist[1], @merged_list};
  endverb

  verb sort_alist_suspended (this none this) owner: #2 flags: "rxd"
    "sort_alist_suspended(interval,alist[,n]) sorts a list of tuples by n-th element.  n defaults to 1.  Calls suspend(interval) as necessary.";
    set_task_perms(caller_perms());
    "... so it can be killed...";
    {interval, alist, ?sort_on = 1} = args;
    if ((alist_length = length(alist)) < 10)
      "insertion sort on short lists";
      $command_utils:suspend_if_needed(interval);
      return this:sort(alist, this:slice(@listdelete(args, 1)));
    endif
    "variables specially expanded for the anal-retentive";
    left_index = alist_length / 2;
    right_index = (alist_length + 1) / 2;
    left_sublist = this:sort_alist_suspended(interval, alist[1..left_index], sort_on);
    right_sublist = this:sort_alist_suspended(interval, alist[left_index + 1..alist_length], sort_on);
    left_element = left_sublist[left_index];
    right_element = right_sublist[right_index];
    merged_list = {};
    while (1)
      $command_utils:suspend_if_needed(interval);
      if (left_element[sort_on] > right_element[sort_on])
        merged_list = {left_element, @merged_list};
        if (left_index = left_index - 1)
          left_element = left_sublist[left_index];
        else
          return {@right_sublist[1..right_index], @merged_list};
        endif
      else
        merged_list = {right_element, @merged_list};
        if (right_index = right_index - 1)
          right_element = right_sublist[right_index];
        else
          return {@left_sublist[1..left_index], @merged_list};
        endif
      endif
    endwhile
  endverb

  verb randomly_permute (this none this) owner: HACKER flags: "rxd"
    ":randomly_permute(list) => list with its elements randomly permuted";
    "  each of the length(list)! possible permutations is equally likely";
    plist = {};
    for i in [1..length(ulist = args[1])]
      plist = listinsert(plist, ulist[i], random(i));
    endfor
    return plist;
  endverb

  verb count (this none this) owner: #2 flags: "rxd"
    "$list_utils:count(item, list)";
    "Returns the number of occurrences of item in list.";
    {x, xlist} = args;
    if (typeof(xlist) != TYPE_LIST)
      return E_INVARG;
    endif
    counter = 0;
    while (loc = x in xlist)
      counter = counter + 1;
      xlist = xlist[loc + 1..$];
    endwhile
    return counter;
  endverb

  verb flatten (this none this) owner: HACKER flags: "rxd"
    "Copied from $quinn_utils (#34283):unroll by Quinn (#19845) Mon Mar  8 09:29:03 1993 PST";
    ":flatten(LIST list_of_lists) => LIST of all lists in given list `flattened'";
    newlist = {};
    for elm in (args[1])
      if (typeof(elm) == TYPE_LIST)
        newlist = {@newlist, @this:flatten(elm)};
      else
        newlist = {@newlist, elm};
      endif
    endfor
    return newlist;
  endverb

  verb "longest shortest" (this none this) owner: HACKER flags: "rxd"
    "Copied from APHiD (#33119):longest Sun May  9 21:00:18 1993 PDT";
    "$list_utils:longest(<list>)";
    "$list_utils:shortest(<list>)";
    "             - Returns the shortest or longest element in the list.  Elements may be either strings or lists.  Returns E_TYPE if passed a non-list or a list containing non-string/list elements.  Returns E_RANGE if passed an empty list.";
    if (typeof(all = args[1]) != TYPE_LIST)
      return E_TYPE;
    elseif (all == {})
      return E_RANGE;
    else
      result = all[1];
      for things in (all)
        if (typeof(things) != TYPE_LIST && typeof(things) != TYPE_STR)
          return E_TYPE;
        else
          result = verb == "longest" && length(result) < length(things) || (verb == "shortest" && length(result) > length(things)) ? things | result;
        endif
      endfor
    endif
    return result;
  endverb

  verb check_nonstring_tell_lines (this none this) owner: HACKER flags: "rxd"
    "check_nonstring_tell_lines(lines)";
    if (caller_perms().wizard)
      "don't let a nonwizard mess up our stats";
      for line in (args[1])
        if (typeof(line) != TYPE_STR)
          this.nonstring_tell_lines = listappend(this.nonstring_tell_lines, callers());
          return;
        endif
      endfor
    endif
  endverb

  verb reverse_suspended (this none this) owner: #2 flags: "rxd"
    "reverse(list) => reversed list.  Uses the fast builtin reverse() so no suspend is needed.";
    return reverse(args[1]);
  endverb

  verb randomly_permute_suspended (this none this) owner: #2 flags: "rxd"
    ":randomly_permute_suspended(list) => list with its elements randomly permuted";
    "  each of the length(list)! possible permutations is equally likely";
    set_task_perms(caller_perms());
    plist = {};
    for i in [1..length(ulist = args[1])]
      plist = listinsert(plist, ulist[i], random(i));
      $command_utils:suspend_if_needed(0);
    endfor
    return plist;
  endverb

  verb swap_elements (this none this) owner: HACKER flags: "rxd"
    "swap_elements -- exchange two elements in a list";
    "Usage:  $list_utils:swap_elements(<list/LIST>,<index/INT>,<index/INT>)";
    "        $list_utils:swap_elements({\"a\",\"b\"},1,2);";
    {l, i, j} = args;
    if (typeof(l) == TYPE_LIST && typeof(i) == TYPE_INT && typeof(j) == TYPE_INT)
      ll = length(l);
      if (i > 0 && i <= ll && (j > 0 && j <= ll))
        t = l[i];
        l[i] = l[j];
        l[j] = t;
        return l;
      else
        return E_RANGE;
      endif
    else
      return E_TYPE;
    endif
  endverb

  verb "random_item random_element" (this none this) owner: HACKER flags: "rxd"
    "random_item -- returns a random element of the input list.";
    if (length(args) == 1)
      if (typeof(l = args[1]) == TYPE_LIST)
        if (length(l) > 0)
          return l[random($)];
        else
          return E_RANGE;
        endif
      else
        return E_TYPE;
      endif
    else
      return E_ARGS;
    endif
  endverb

  verb assoc_suspended (this none this) owner: #2 flags: "rxd"
    "Copied from Moo_tilities (#332):assoc_suspended by Mooshie (#106469) Wed Mar 18 19:27:54 1998 PST";
    "Usage: assoc_suspended(ANY target, LIST list [, INT index [, INT suspend-for ])) => Returns the first element of `list' whose own index-th element is target.  Index defaults to 1.";
    "Returns {} if no such element is found.";
    "Suspends as necessary. Suspend length defaults to 0.";
    set_task_perms(caller_perms());
    {target, thelist, ?indx = 1, ?suspend_for = 0} = args;
    cu = $command_utils;
    for t in (thelist)
      if (`t[indx] == target ! E_TYPE => 0')
        if (typeof(t) == TYPE_LIST && length(t) >= indx)
          return t;
        endif
      endif
      cu:suspend_if_needed(suspend_for);
    endfor
    return {};
  endverb

  verb amerge (this none this) owner: HACKER flags: "rxd"
    "Copied from Uther's_Ghost (#93141):amerge Tue May 27 20:28:18 1997 PDT";
    "amerge(list[,tindex[,dindex]]) returns an associated list such that all the tuples in the original list with the same tindex-th element are merged. Useful for merging alists ( amerge({@alist1, @alist2, ...}) ) and for ensuring that each tuple has a unique index. Tindex defaults to 1. Dindex defaults to 1 and refers to the position in the tuple where the tindex-th element will land in the new tuple.";
    {alist, ?tidx = 1, ?didx = 1} = args;
    if (alist)
      alist = this:sort_alist(alist, tidx);
      i = 1;
      res = {{cur = alist[1][tidx]}};
      for tuple in (alist)
        if (tuple[tidx] == cur)
          res[i] = {@res[i], @listdelete(tuple, tidx)};
        else
          if (didx != 1)
            res[i] = this:swap_elements(res[i], 1, min(didx, length(res[i])));
          endif
          i = i + 1;
          res = {@res, {cur = tuple[tidx], @listdelete(tuple, tidx)}};
        endif
      endfor
      return res;
    endif
    return alist;
  endverb

  verb passoc (this none this) owner: HACKER flags: "rx"
    "passoc -- essentially a hashtable lookup for parallel lists.  Parallel lists are an efficient way to store and access key/value pairs.";
    "Usage:  $list_utils:passoc(<key>,<key-list>,<value-list>)";
    "        avalue=$list_utils:passoc(\"akey\",keys,values)";
    return args[3][args[1] in args[2]];
  endverb

  verb setmove (this none this) owner: HACKER flags: "rxd"
    "Copied from Moo_tilities (#332):setmove by Mooshie (#106469) Mon Sep 22 21:07:25 1997 PDT";
    "Usage: setmove(LIST elements, INT from, INT to)";
    "Moves element in list from one position in list to another.";
    "";
    "Example: setmove({x, y, z}, 1, 3) => {y, z, x}";
    "         setmove({x, y, z}, 2, 1} => {y, x, z}";
    {start, from, to} = args;
    what = start[from];
    return listinsert(listdelete(start, from), what, to);
    "  Written by Mooshie (#106469) @ Lambda - Mon Sep 22 21:03:26 1997 PDT -  ";
  endverb

  verb iassoc_new (this none this) owner: HACKER flags: "rxd"
    "Copied from Moo_tilities (#332):iassoc_new by Mooshie (#106469) Wed Mar 18 19:27:52 1998 PST";
    "Usage: iassoc_new(ANY target, LIST list [, INT index ]) => Returns the index of the first element of `list' whose own index-th element is target.  Index defaults to 1.";
    "Returns 0 if no such element is found.";
    "NOTE: expects that each index in the given list will be a list with at least as many elements as the indicated `index' argument. Otherwise will return E_RANGE";
    {target, thelist, ?indx = 1} = args;
    try
      for element in (thelist)
        if (element[indx] == target)
          if (typeof(element) == TYPE_LIST)
            return element in thelist;
          endif
        endif
      endfor
    except e (ANY)
      return e[1];
    endtry
    return 0;
  endverb

  verb build_alist (this none this) owner: HACKER flags: "rxd"
    "Syntax:  build_alist(list, N) =>";
    "{list[1..N], list[N+1..N*2], list[N*2+1..N*3], ..., list[N*(N-1)+1..N*N]}";
    "";
    "Creates an associated list from a flat list at every Nth interval. If the list doesn't have a multiple of N elements, E_RANGE is returned.";
    "Example:  build_alist({a,b,c,d,e,f,g,h,i},3)=>{{a,b,c},{d,e,f},{g,h,i}}";
    {olist, interval} = args;
    if ((tot = length(olist)) % interval)
      return E_RANGE;
    endif
    nlist = {};
    d = 1;
    while (d <= tot)
      nlist = {@nlist, olist[1..interval]};
      olist[1..interval] = {};
      d = d + interval;
    endwhile
    return nlist;
  endverb

  verb flatten_suspended (this none this) owner: HACKER flags: "rxd"
    "Copied from list utilities (#372):flatten [verb author Hacker (#18105)] at Sun Jul  3 07:46:47 2011 PDT";
    "Copied from $quinn_utils (#34283):unroll by Quinn (#19845) Mon Mar  8 09:29:03 1993 PST";
    ":flatten(LIST list_of_lists) => LIST of all lists in given list `flattened'";
    newlist = {};
    for elm in (args[1])
      if (typeof(elm) == TYPE_LIST)
        $command_utils:suspend_if_needed(0);
        newlist = {@newlist, @this:flatten_suspended(elm)};
      else
        $command_utils:suspend_if_needed(0);
        newlist = {@newlist, elm};
      endif
    endfor
    return newlist;
  endverb
endobject