object GENERIC_DB
  name: "Generic Database"
  parent: ROOT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property " " (owner: HACKER, flags: "") = {"", "", {}, {}};
  property data (owner: HACKER, flags: "r") = 4;
  property node_perms (owner: HACKER, flags: "rc") = "r";

  override aliases = {"Generic Database"};
  override description = "A generic `database' (well, really more like a string-indexed array if you want the truth...). See `help $generic_db' for details.";
  override import_export_id = "generic_db";
  override object_size = {17214, 1084848672};

  verb "find find_key" (this none this) owner: HACKER flags: "rxd"
    "find(string[,n]) => datum corresponding to string with the search starting at node \" \"+string[1..n], n defaults to 0 (root node), $ambiguous_match or $failed_match";
    "find_key(string[,n]) is like :find but returns the full string key rather than the associated datum.  Note that if several string keys present in the db share a common prefix, :find_key(prefix) will return $ambiguous_match, but if there is a unique datum associated with all of these strings :find(prefix) will return it rather than $ambiguous_match.";
    "Assumes n<=length(string)";
    {search, ?sofar = 0} = args;
    rest = search;
    prefix = search[1..sofar];
    rest[1..sofar] = "";
    info = this.((" " + prefix));
    data = verb == "find" ? this.data | 3;
    if (i = search in info[3])
      "...exact match for one of the strings in this node...";
      return info[data][i];
    elseif (index(info[1], rest) == 1)
      "...ambiguous iff there's more than one object represented in this node..";
      return this:_only(prefix, data);
    elseif (index(rest, info[1]) != 1)
      "...search string doesn't agree with common portion...";
      return $failed_match;
    elseif (index(info[2], search[nsofar = sofar + length(info[1]) + 1]))
      "...search string follows one of continuations leading to other nodes...";
      return this:(verb)(search, nsofar);
    else
      "...search string may partially match one of the strings in this node...";
      for i in [1..length(exacts = info[3])]
        if (index(exacts[i], search) == 1)
          return info[data][i];
        endif
      endfor
      return $failed_match;
    endif
  endverb

  verb find_exact (this none this) owner: HACKER flags: "rxd"
    {search, ?sofar = 0} = args;
    rest = search;
    prefix = search[1..sofar];
    rest[1..sofar] = "";
    info = this.((" " + prefix));
    if (i = search in info[3])
      return info[this.data][i];
    elseif (length(rest) <= (common = length(info[1])) || rest[1..common] != info[1])
      return $failed_match;
    elseif (index(info[2], search[sofar + common + 1]))
      return this:find_exact(search, sofar + common + 1);
    else
      return $failed_match;
    endif
  endverb

  verb "find_all find_all_keys" (this none this) owner: HACKER flags: "rxd"
    ":find_all(string [,n=0])";
    "assumes n <= length(string)";
    {search, ?sofar = 0} = args;
    rest = search;
    prefix = search[1..sofar];
    rest[1..sofar] = "";
    info = this.((" " + prefix));
    data = verb == "find_all" ? this.data | 3;
    if (index(info[1], rest) == 1)
      "...return entire subtree.";
      return this:((data == 3 ? "_every_key" | "_every"))(prefix);
    elseif (index(rest, info[1]) != 1)
      "...common portion doesn't agree.";
      return {};
    elseif (index(info[2], rest[1 + (common = length(info[1]))]))
      "...matching strings are in a subnode.";
      return this:(verb)(search, sofar + common + 1);
    else
      "...matching string is in info[3].  length(rest) > common,";
      "...so there will be at most one matching string.";
      for i in [1..length(info[3])]
        if (index(info[3][i], search) == 1)
          return {info[data][i]};
        endif
      endfor
      return {};
    endif
  endverb

  verb _only (this none this) owner: HACKER flags: "rxd"
    ":_only(prefix,data) => if all strings in this node have the same datum, return it, otherwise, return $ambiguous_match.";
    if (caller != this)
      raise(E_PERM);
    endif
    {prefix, data} = args;
    info = this.((" " + prefix));
    if (data == 3)
      "... life is much simpler if there's no separate datum.";
      "... if there's more than one string here, we barf.";
      if (info[2] || length(info[3]) > 1)
        return $ambiguous_match;
      elseif (info[3])
        return info[3][1];
      else
        "..this can only happen with the root node of an empty db.";
        return $failed_match;
      endif
    elseif (info[2])
      what = this:_only(tostr(prefix, info[1], info[2][1]), data);
      if (what == $ambiguous_match)
        return what;
      endif
    elseif (info[data])
      what = info[data][1];
      info[data] = listdelete(info[data], 1);
    else
      "..this can only happen with the root node of an empty db.";
      return $failed_match;
    endif
    for x in (info[data])
      if (what != x)
        return $ambiguous_match;
      endif
    endfor
    for i in [2..length(info[2])]
      if (what != this:_only(tostr(prefix, info[1], info[2][i]), data))
        return $ambiguous_match;
      endif
    endfor
    return what;
  endverb

  verb _every (this none this) owner: HACKER flags: "rxd"
    if (caller != this)
      raise(E_PERM);
    endif
    info = this.((" " + args[1]));
    prefix = args[1] + info[1];
    r = $list_utils:remove_duplicates(info[4]);
    for i in [1..length(branches = info[2])]
      for new in (this:_every(prefix + branches[i]))
        r = setadd(r, new);
      endfor
    endfor
    return r;
  endverb

  verb _every_key (this none this) owner: HACKER flags: "rxd"
    if (caller != this)
      raise(E_PERM);
    endif
    info = this.((" " + args[1]));
    prefix = args[1] + info[1];
    r = info[3];
    for i in [1..length(branches = info[2])]
      for new in (this:_every_key(prefix + branches[i]))
        r = setadd(r, new);
        $command_utils:suspend_if_needed(0);
      endfor
      $command_utils:suspend_if_needed(0);
    endfor
    return r;
  endverb

  verb insert (this none this) owner: HACKER flags: "rxd"
    ":insert([n,]string,datum) -- inserts <string,datum> correspondence into tree starting at node \" \"+string[1..n], n defaulting to 0 (root node).";
    "Assumes length(string) >= n";
    "Returns {old_datum} (or 1) if there was a <string,old_datum> correspondence there before, otherwise returns 0";
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    has_datum = this.data > 3;
    if (typeof(sofar = args[1]) == INT)
      search = args[2];
      datum = has_datum ? args[3] | 0;
    else
      search = sofar;
      sofar = 0;
      datum = has_datum ? args[2] | 0;
    endif
    prefix = search[1..sofar];
    info = this.((" " + prefix));
    if (i = search in info[3])
      "... exact match ...";
      if (has_datum)
        previous = {info[this.data][i]};
        info[this.data][i] = datum;
        this:set_node(prefix, @info);
        return previous;
      else
        return 1;
      endif
    endif
    rest = search;
    rest[1..sofar] = "";
    if (index(rest, info[1]) != 1)
      "... find where new string disagrees with common portion...";
      c = $string_utils:common(rest, info[1]) + 1;
      "... make a new node with a shorter common portion....";
      this:make_node(prefix + (info[1])[1..c], @listset(info, (info[1])[c + 1..$], 1));
      this:set_node(prefix, (info[1])[1..c - 1], info[1][c], {search}, @has_datum ? {{datum}} | {});
      return 0;
    elseif (rest == info[1])
      ".. new string == common portion, insert...";
      info[3] = {@info[3], search};
      if (has_datum)
        info[this.data] = {@info[this.data], datum};
      endif
      this:set_node(prefix, @info);
      return 0;
    elseif (index(info[2], search[nsofar = sofar + length(info[1]) + 1]))
      "... new string matches pre-existing continuation. insert in subnode....";
      return this:insert(nsofar, search, datum);
    else
      "... new string may blow away one of the exact matches (i.e., matches one of them up to the first character beyond the common portion) in which case we need to create a new subnode....";
      s = search[1..nsofar];
      for m in (info[3])
        if (index(m, s) == 1)
          i = m in info[3];
          "... we know m != search ...";
          "... string m has been blown away.  create new node ...";
          cbegin = cafter = length(s) + 1;
          cend = $string_utils:common(search, m);
          this:make_node(s, m[cbegin..cend], "", {search, m}, @has_datum ? {{datum, info[this.data][i]}} | {});
          this:set_node(prefix, info[1], info[2] + s[nsofar], listdelete(info[3], i), @has_datum ? {listdelete(info[this.data], i)} | {});
          return 0;
        endif
      endfor
      "... new string hasn't blown away any of the exact matches, insert it as a new exact match...";
      info[3] = {search, @info[3]};
      if (has_datum)
        info[this.data] = {datum, @info[this.data]};
      endif
      this:set_node(prefix, @info);
      return 0;
    endif
  endverb

  verb delete (this none this) owner: HACKER flags: "rxd"
    ":delete(string[,n]) deletes any <string,something> pair from the tree starting at node \" \"+string[1..n], n defaulting to 0 (root node)";
    "Returns {something} if such a pair existed, otherwise returns 0";
    "If that node is not the root node and ends up containing only one string and no subnodes, we kill it and return {something,string2,something2} where <string2,something2> is the remaining pair.";
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    {search, ?sofar = 0} = args;
    rest = search;
    prefix = search[1..sofar];
    rest[1..sofar] = "";
    info = this.((" " + prefix));
    if (i = search in info[3])
      previous = {info[this.data][i]};
      info[3] = listdelete(info[3], i);
      if (this.data > 3)
        info[this.data] = listdelete(info[this.data], i);
      endif
    elseif (rest == info[1] || (index(rest, info[1]) != 1 || !index(info[2], search[d = sofar + length(info[1]) + 1])))
      "... hmm string isn't in here...";
      return 0;
    elseif ((previous = this:delete(search, d)) && length(previous) > 1)
      i = index(info[2], search[d]);
      (info[2])[i..i] = "";
      info[3] = {previous[2], @info[3]};
      if (this.data > 3)
        info[this.data] = {previous[3], @info[this.data]};
      endif
      previous = previous[1..1];
    else
      return previous;
    endif
    if (!prefix || length(info[3]) + length(info[2]) != 1)
      this:set_node(prefix, @info);
      return previous;
    elseif (info[3])
      this:kill_node(prefix);
      return {@previous, info[3][1], info[this.data][1]};
    else
      sub = this.((" " + (p = tostr(prefix, info[1], info[2]))));
      this:kill_node(p);
      this:set_node(prefix, @listset(sub, tostr(info[1], info[2], sub[1]), 1));
      return previous;
    endif
  endverb

  verb delete2 (this none this) owner: HACKER flags: "rxd"
    ":delete2(string,datum[,n]) deletes the pair <string,datum> from the tree starting at node \" \"+string[1..n], n defaulting to 0 (root node)";
    "Similar to :delete except that if the entry for that string has a different associated datum, it will not be removed.  ";
    ":delete2(string,datum) is equivalent to ";
    " ";
    "  if(this:find_exact(string)==datum) ";
    "    this:delete(string); ";
    "  endif";
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    {search, datum, ?sofar = 0} = args;
    rest = search;
    prefix = search[1..sofar];
    rest[1..sofar] = "";
    info = this.((" " + prefix));
    if (i = search in info[3])
      previous = {info[this.data][i]};
      if (previous[1] != datum)
        return previous;
      endif
      info[3] = listdelete(info[3], i);
      if (this.data > 3)
        info[this.data] = listdelete(info[this.data], i);
      endif
    elseif (rest == info[1] || (index(rest, info[1]) != 1 || !index(info[2], search[d = sofar + length(info[1]) + 1])))
      "... hmm string isn't in here...";
      return 0;
    elseif ((previous = this:delete2(search, datum, d)) && length(previous) > 1)
      i = index(info[2], search[d]);
      (info[2])[i..i] = "";
      info[3] = {previous[2], @info[3]};
      if (this.data > 3)
        info[this.data] = {previous[3], @info[this.data]};
      endif
      previous = previous[1..1];
    else
      return previous;
    endif
    if (!prefix || length(info[3]) + length(info[2]) != 1)
      this:set_node(prefix, @info);
      return previous;
    elseif (info[3])
      this:kill_node(prefix);
      return {@previous, info[3][1], info[this.data][1]};
    else
      sub = this.((" " + (p = tostr(prefix, info[1], info[2]))));
      this:kill_node(p);
      this:set_node(prefix, @listset(sub, tostr(info[1], info[2], sub[1]), 1));
      return previous;
    endif
  endverb

  verb set_node (this none this) owner: HACKER flags: "rxd"
    return caller != this ? E_PERM | (this.((" " + args[1])) = listdelete(args, 1));
  endverb

  verb make_node (this none this) owner: #2 flags: "rxd"
    "WIZARDLY";
    return caller != this ? E_PERM | add_property(this, " " + args[1], listdelete(args, 1), {$generic_db.owner, this.node_perms});
  endverb

  verb kill_node (this none this) owner: #2 flags: "rxd"
    "WIZARDLY";
    return caller != this ? E_PERM | delete_property(this, " " + args[1]);
  endverb

  verb clearall (this none this) owner: #2 flags: "rxd"
    "WIZARDLY";
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    if (args && (d = args[1]) in {3, 4})
      this.data = d;
    endif
    root = {"", "", {}, @this.data > 3 ? {{}} | {}};
    "...since the for loop contains a suspend, we want to keep people";
    "...from getting at properties which are now garbage but which we";
    "...haven't had a chance to wipe yet.  Somebody might yet succeed";
    "...in adding something; thus we have the outer while loop.";
    this:set_node("", 37);
    while (this.(" ") != root)
      this:set_node("", @root);
      for p in (properties(this))
        if (p[1] == " " && p != " ")
          delete_property(this, p);
        endif
        "...Bleah; db is inconsistent now....";
        "...At worst someone will add something that references an";
        "...existing property.  He will deserve to die...";
        $command_utils:suspend_if_needed(0);
      endfor
    endwhile
  endverb

  verb clearall_big (this none this) owner: HACKER flags: "rxd"
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    this:_kill_subtrees("", 0);
    this:clearall(@args);
  endverb

  verb _kill_subtrees (this none this) owner: HACKER flags: "rxd"
    ":_kill_subtree(node,count)...wipes out all subtrees";
    "...returns count + number of nodes removed...";
    if (!($perm_utils:controls(caller_perms(), this) || caller == this))
      return E_PERM;
    endif
    info = this.((" " + (prefix = args[1])));
    count = args[2];
    if (ticks_left() < 500 || seconds_left() < 2)
      player:tell("...", count);
      suspend(0);
    endif
    for i in [1..length(info[2])]
      count = this:_kill_subtrees(n = tostr(prefix, info[1], info[2][i]), count) + 1;
      this:kill_node(n);
    endfor
    return count;
  endverb

  verb depth (this none this) owner: HACKER flags: "rxd"
    info = this.((" " + (prefix = (args || {""})[1])));
    depth = 0;
    string = prefix;
    if (ticks_left() < 500 || seconds_left() < 2)
      player:tell("...", prefix);
      suspend(0);
    endif
    for i in [1..length(info[2])]
      if ((r = this:depth(tostr(prefix, info[1], info[2][i])))[1] > depth)
        depth = r[1];
        string = r[2];
      endif
    endfor
    return {depth + 1, string};
  endverb

  verb count_entries (this none this) owner: HACKER flags: "rxd"
    info = this.((" " + (prefix = args[1])));
    count = length(info[3]) + args[2];
    if (ticks_left() < 500 || seconds_left() < 2)
      player:tell("...", count);
      suspend(0);
    endif
    for i in [1..length(info[2])]
      count = this:count_entries(tostr(prefix, info[1], info[2][i]), count);
    endfor
    return count;
  endverb

  verb count_chars (this none this) owner: HACKER flags: "rxd"
    info = this.((" " + (prefix = args[1])));
    count = args[2];
    for s in (info[3])
      count = count + length(s);
    endfor
    if (ticks_left() < 500 || seconds_left() < 2)
      player:tell("...", count);
      suspend(0);
    endif
    for i in [1..length(info[2])]
      count = this:count_chars(tostr(prefix, info[1], info[2][i]), count);
    endfor
    return count;
  endverb

  verb count (any in this) owner: HACKER flags: "rd"
    "count [entries|chars] in <db>";
    "  reports on the number of distinct string keys or the number of characters";
    "  in all string keys in the db";
    if (index("entries", dobjstr) == 1)
      player:tell(this:count_entries("", 0), " strings in ", this.name, "(", this, ")");
    elseif (index("chars", dobjstr) == 1)
      player:tell(this:count_chars("", 0), " chars in ", this.name, "(", this, ")");
    else
      player:tell("Usage: ", verb, " entries|chars in <db>");
    endif
  endverb

  verb proxy_for_core (this none this) owner: #2 flags: "rxd"
    "Create a stand-in for the core-extraction process";
    "  (rather than change the ownership on 80000 properties only to delete them).";
    {core_variant, is_mcd} = args;
    if (!is_mcd)
      return this;
    elseif (caller != #0)
      raise(E_PERM);
    elseif (children(this) || length(properties(this)) < 100)
      return this;
    endif
    proxy = $recycler:_create(parent(this), this.owner);
    player:tell("Creating proxy object ", proxy, " for ", this.name, " (", this, ")");
    for p in ({"name", "r", "w", "f"})
      proxy.(p) = this.(p);
    endfor
    for p in ($object_utils:all_properties_suspended(parent(this)))
      if (!is_clear_property(this, p))
        $command_utils:suspend_if_needed(0, "...setting props from parent...");
        proxy.(p) = this.(p);
      endif
    endfor
    for p in (properties(this))
      $command_utils:suspend_if_needed(0);
      if (p[1] == " " && p != " ")
        continue;
      endif
      add_property(proxy, p, this.(p), property_info(this, p));
    endfor
    for v in [1..length(verbs(this))]
      add_verb(proxy, verb_info(this, v), verb_args(this, v));
      set_verb_code(proxy, v, verb_code(this, v));
      $command_utils:suspend_if_needed(0);
    endfor
    proxy:clearall();
    return proxy;
  endverb
endobject