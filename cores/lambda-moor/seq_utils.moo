object SEQ_UTILS
  name: "sequence utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  override aliases = {"sequence utilities", "seq_utils", "squ"};
  override description = {
    "This is the sequence utilities utility package.  See `help $seq_utils' for more details."
  };
  override help_msg = {
    "A sequence is a set of integers (*)",
    "This package supplies the following verbs:",
    "",
    "  :add      (seq,f,t)  => seq with [f..t] interval added",
    "  :remove   (seq,f,t)  => seq with [f..t] interval removed",
    "  :range    (f,t)      => sequence corresponding to [f..t]",
    "  {}                   => empty sequence",
    "  :contains (seq,n)    => n in seq",
    "  :size     (seq)      => number of elements in seq",
    "  :first    (seq)      => first integer in seq or E_NONE",
    "  :firstn   (seq,n)    => first n integers in seq (as a sequence)",
    "  :last     (seq)      => last integer in seq  or E_NONE",
    "  :lastn    (seq,n)    => last n integers in seq (as a sequence)",
    "",
    "  :complement   (seq)         => sequence consisting of integers not in seq",
    "  :union        (seq,seq,...) => union of all sequences",
    "  :intersection (seq,seq,...) => intersection of all sequences",
    "  :contract (seq,cseq)              (see `help $seq_utils:contract')",
    "  :expand   (seq,eseq[,include])    (see `help $seq_utils:expand')",
    "  ",
    "  :extract(seq,array)           => array[@seq]",
    "  :for([n,]seq,obj,verb,@args)  => for s in (seq) obj:verb(s,@args); endfor",
    "",
    "  :tolist(seq)            => list corresponding to seq",
    "  :tostr(seq)             => contents of seq as a string",
    "  :from_list(list)        => sequence corresponding to list",
    "  :from_sorted_list(list) => sequence corresponding to list (assumed sorted)",
    "  :from_string(string)    => sequence corresponding to string",
    "",
    "For boolean expressions, note that",
    "  the representation of the empty sequence is {} (boolean FALSE) and",
    "  all non-empty sequences are represented as nonempty lists (boolean TRUE).",
    "",
    "The representation used works better than the usual list implementation for sets consisting of long uninterrupted ranges of integers.  ",
    "For sparse sets of integers the representation is decidedly non-optimal (though it never takes more than double the space of the usual list representation).",
    "",
    "(*) i.e., integers in the range [$minint+1..$maxint].  The implementation depends on $minint never being included in a sequence.",
    ""
  };
  override object_size = {17130, 1084848672};

  verb "add remove" (this none this) owner: HACKER flags: "rxd"
    "   add(seq,start[,end]) => seq with range added.";
    "remove(seq,start[,end]) => seq with range removed.";
    "  both assume start<=end.";
    remove = verb == "remove";
    seq = args[1];
    start = args[2];
    s = start == $minint ? 1 | $list_utils:find_insert(seq, start - 1);
    if (length(args) < 3)
      return {@seq[1..s - 1], @(s + remove) % 2 ? {start} | {}};
    else
      e = $list_utils:find_insert(seq, after = args[3] + 1);
      return {@seq[1..s - 1], @(s + remove) % 2 ? {start} | {}, @(e + remove) % 2 ? {after} | {}, @seq[e..$]};
    endif
  endverb

  verb contains (this none this) owner: HACKER flags: "rxd"
    ":contains(seq,elt) => true iff elt is in seq.";
    return ($list_utils:find_insert(@args) + 1) % 2;
  endverb

  verb complement (this none this) owner: HACKER flags: "rxd"
    ":complement(seq[,lower[,upper]]) => the sequence containing all integers *not* in seq.";
    "If lower/upper are given, the resulting sequence is restricted to the specified range.";
    "Bad things happen if seq is not a subset of [lower..upper]";
    {seq, ?lower = $minint, ?upper = $nothing} = args;
    if (upper != $nothing)
      if (seq[$] >= (upper = upper + 1))
        seq[$..$] = {};
      else
        seq[$ + 1..$] = {upper};
      endif
    endif
    if (seq && seq[1] <= lower)
      return listdelete(seq, 1);
    else
      return {lower, @seq};
    endif
  endverb

  verb union (this none this) owner: HACKER flags: "rxd"
    ":union(seq1,seq2,...)        => union of all sequences...";
    if ({} in args)
      args = $list_utils:setremove_all(args, {});
    endif
    if (length(args) <= 1)
      return args ? args[1] | {};
    endif
    return this:_union(@args);
  endverb

  verb tostr (this none this) owner: HACKER flags: "rxd"
    "tostr(seq [,delimiter]) -- turns a sequence into a string, delimiting ranges with delimiter, defaulting to .. (e.g. 5..7)";
    {seq, ?separator = ".."} = args;
    if (!seq)
      return "empty";
    endif
    e = tostr(seq[1] == $minint ? "" | seq[1]);
    len = length(seq);
    for i in [2..len]
      e = e + (i % 2 ? tostr(", ", seq[i]) | (seq[i] == seq[i - 1] + 1 ? "" | tostr(separator, seq[i] - 1)));
    endfor
    return e + (len % 2 ? separator | "");
  endverb

  verb for (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":for([n,]seq,obj,verb,@args) => for s in (seq) obj:verb(s,@args); endfor";
    set_task_perms(caller_perms());
    if (typeof(n = args[1]) == INT)
      args = listdelete(args, 1);
    else
      n = 1;
    endif
    {seq, object, vname, @args} = args;
    if (seq[1] == $minint)
      return E_RANGE;
    endif
    for r in [1..length(seq) / 2]
      for i in [seq[2 * r - 1]..seq[2 * r] - 1]
        if (typeof(object:(vname)(@listinsert(args, i, n))) == ERR)
          return;
        endif
      endfor
    endfor
    if (length(seq) % 2)
      i = seq[$];
      while (1)
        if (typeof(object:(vname)(@listinsert(args, i, n))) == ERR)
          return;
        endif
        i = i + 1;
      endwhile
    endif
  endverb

  verb extract (this none this) owner: HACKER flags: "rxd"
    "extract(seq,array) => list of elements of array with indices in seq.";
    {seq, array} = args;
    if (alen = length(array))
      e = $list_utils:find_insert(seq, 1);
      s = $list_utils:find_insert(seq, alen);
      seq = {@e % 2 ? {} | {1}, @seq[e..s - 1], @s % 2 ? {} | {alen + 1}};
      ret = {};
      for i in [1..length(seq) / 2]
        $command_utils:suspend_if_needed(0);
        ret = {@ret, @array[seq[2 * i - 1]..seq[2 * i] - 1]};
      endfor
      return ret;
    else
      return {};
    endif
  endverb

  verb tolist (this none this) owner: HACKER flags: "rxd"
    seq = args[1];
    if (!seq)
      return {};
    else
      if (length(seq) % 2)
        seq = {@seq, $minint};
      endif
      l = {};
      for i in [1..length(seq) / 2]
        for j in [seq[2 * i - 1]..seq[2 * i] - 1]
          l = {@l, j};
        endfor
      endfor
      return l;
    endif
  endverb

  verb from_list (this none this) owner: HACKER flags: "rxd"
    ":fromlist(list) => corresponding sequence.";
    return this:from_sorted_list($list_utils:sort(args[1]));
  endverb

  verb from_sorted_list (this none this) owner: HACKER flags: "rxd"
    ":from_sorted_list(sorted_list) => corresponding sequence.";
    if (!(lst = args[1]))
      return {};
    else
      seq = {i = lst[1]};
      next = i + 1;
      for i in (listdelete(lst, 1))
        if (i != next)
          seq = {@seq, next, i};
        endif
        next = i + 1;
      endfor
      return next == $minint ? seq | {@seq, next};
    endif
  endverb

  verb first (this none this) owner: HACKER flags: "rxd"
    return (seq = args[1]) ? seq[1] | E_NONE;
  endverb

  verb last (this none this) owner: HACKER flags: "rxd"
    return (seq = args[1]) ? length(seq) % 2 ? $minint - 1 | seq[$] - 1 | E_NONE;
  endverb

  verb size (this none this) owner: HACKER flags: "rxd"
    ":size(seq) => number of elements in seq";
    "  for sequences consisting of more than half of the 4294967298 available integers, this returns a negative number, which can either be interpreted as (cardinality - 4294967298) or -(size of complement sequence)";
    n = 0;
    for i in (seq = args[1])
      n = i - n;
    endfor
    return length(seq) % 2 ? $minint - n | n;
  endverb

  verb from_string (this none this) owner: HACKER flags: "rxd"
    ":from_string(string) => corresponding sequence or E_INVARG";
    "  string should be a comma separated list of numbers and";
    "  number..number ranges";
    su = $string_utils;
    if (!(words = su:explode(su:strip_chars(args[1], " "), ",")))
      return {};
    endif
    parts = {};
    for word in (words)
      to = index(word, "..");
      if (!to && su:is_numeric(word))
        part = {toint(word), toint(word) + 1};
      elseif (to)
        if (to == 1)
          start = $minint;
        elseif (su:is_numeric(start = word[1..to - 1]))
          start = toint(start);
        else
          return E_INVARG;
        endif
        end = word[to + 2..length(word)];
        if (!end)
          part = {start};
        elseif (!su:is_numeric(end))
          return E_INVARG;
        elseif ((end = toint(end)) >= start)
          part = {start, end + 1};
        else
          part = {};
        endif
      else
        return E_INVARG;
      endif
      parts = {@parts, part};
    endfor
    return this:union(@parts);
  endverb

  verb firstn (this none this) owner: HACKER flags: "rxd"
    ":firstn(seq,n) => first n elements of seq as a sequence.";
    if ((n = args[2]) <= 0)
      return {};
    endif
    l = length(seq = args[1]);
    s = 1;
    while (s <= l)
      n = n + seq[s];
      if (s >= l || n <= seq[s + 1])
        return {@seq[1..s], n};
      endif
      n = n - seq[s + 1];
      s = s + 2;
    endwhile
    return seq;
  endverb

  verb lastn (this none this) owner: HACKER flags: "rxd"
    ":lastn(seq,n) => last n elements of seq as a sequence.";
    n = args[2];
    if ((l = length(seq = args[1])) % 2)
      return {$minint - n};
    else
      s = l;
      while (s)
        n = seq[s] - n;
        if (n >= seq[s - 1])
          return {n, @seq[s..l]};
        endif
        n = seq[s - 1] - n;
        s = s - 2;
      endwhile
      return seq;
    endif
  endverb

  verb range (this none this) owner: HACKER flags: "rxd"
    ":range(start,end) => sequence corresponding to [start..end] range";
    return (start = args[1]) <= (end = args[2]) ? {start, end + 1} | {};
  endverb

  verb expand (this none this) owner: HACKER flags: "rxd"
    ":expand(seq,eseq[,include=0])";
    "eseq is assumed to be a finite sequence consisting of intervals ";
    "[f1..a1-1],[f2..a2-1],...  We map each element i of seq to";
    "  i               if               i < f1";
    "  i+(a1-f1)       if         f1 <= i < f2-(a1-f1)";
    "  i+(a1-f1+a2-f2) if f2-(a1-f1) <= i < f3-(a2-f2)-(a1-f1)";
    "  ...";
    "returning the resulting sequence if include=0,";
    "returning the resulting sequence unioned with eseq if include=1;";
    {old, insert, ?include = 0} = args;
    exclude = !include;
    if (!insert)
      return old;
    elseif (length(insert) % 2 || insert[1] == $minint)
      return E_TYPE;
    endif
    olast = length(old);
    ilast = length(insert);
    "... find first o for which old[o] >= insert[1]...";
    ifirst = insert[i = 1];
    o = $list_utils:find_insert(old, ifirst - 1);
    if (o > olast)
      return olast % 2 == exclude ? {@old, @insert} | old;
    endif
    new = old[1..o - 1];
    oe = old[o];
    diff = 0;
    while (1)
      "INVARIANT: oe == old[o]+diff";
      "INVARIANT: oe >= ifirst == insert[i]";
      "... at this point we need to dispose of the interval ifirst..insert[i+1]";
      if (oe == ifirst)
        new = {@new, insert[i + (o % 2 == exclude)]};
        if (o >= olast)
          return olast % 2 == exclude ? {@new, @insert[i + 2..ilast]} | new;
        endif
        o = o + 1;
      else
        if (o % 2 != exclude)
          new = {@new, @insert[i..i + 1]};
        endif
      endif
      "... advance i...";
      diff = diff + insert[i + 1] - ifirst;
      if ((i = i + 2) > ilast)
        for oe in (old[o..olast])
          new = {@new, oe + diff};
        endfor
        return new;
      endif
      ifirst = insert[i];
      "... find next o for which old[o]+diff >= ifirst )...";
      while ((oe = old[o] + diff) < ifirst)
        new = {@new, oe};
        if (o >= olast)
          return olast % 2 == exclude ? {@new, @insert[i..ilast]} | new;
        endif
        o = o + 1;
      endwhile
    endwhile
  endverb

  verb contract (this none this) owner: HACKER flags: "rxd"
    ":contract(seq,cseq)";
    "cseq is assumed to be a finite sequence consisting of intervals ";
    "[f1..a1-1],[f2..a2-1],...  From seq, we remove any elements that ";
    "are in those ranges and map each remaining element i to";
    "  i               if       i < f1";
    "  i-(a1-f1)       if a1 <= i < f2";
    "  i-(a1-f1+a2-f2) if a2 <= i < f3 ...";
    "returning the resulting sequence.";
    "";
    "For any finite sequence cseq, the following always holds:";
    "  :contract(:expand(seq,cseq,include),cseq)==seq";
    {old, removed} = args;
    if (!removed)
      return old;
    elseif ((rlen = length(removed)) % 2 || removed[1] == $minint)
      return E_TYPE;
    endif
    rfirst = removed[1];
    ofirst = $list_utils:find_insert(old, rfirst - 1);
    new = old[1..ofirst - 1];
    diff = 0;
    rafter = removed[r = 2];
    for o in [ofirst..olast = length(old)]
      while (old[o] > rafter)
        if ((o - ofirst) % 2)
          new = {@new, rfirst - diff};
          ofirst = o;
        endif
        diff = diff + rafter - rfirst;
        if (r >= rlen)
          for oe in (old[o..olast])
            new = {@new, oe - diff};
          endfor
          return new;
        endif
        rfirst = removed[r + 1];
        rafter = removed[r = r + 2];
      endwhile
      if (old[o] < rfirst)
        new = {@new, old[o] - diff};
        ofirst = o + 1;
      endif
    endfor
    return (olast - ofirst) % 2 ? new | {@new, rfirst - diff};
  endverb

  verb _union (this none this) owner: HACKER flags: "rxd"
    ":_union(seq,seq,...)";
    "assumes all seqs are nonempty and that there are at least 2";
    nargs = length(args);
    "args  -- list of sequences.";
    "nexts -- nexts[i] is the index in args[i] of the start of the first";
    "         interval not yet incorporated in the return sequence.";
    "heap  -- a binary tree of indices into args/nexts represented as a list where";
    "         heap[1] is the root and the left and right children of heap[i]";
    "         are heap[2*i] and heap[2*i+1] respectively.  ";
    "         Parent index h is <= both children in the sense of args[h][nexts[h]].";
    "         heap[i]==0 indicates a nonexistant child; we fill out the array with";
    "         zeros so that length(heap)>2*length(args).";
    "...initialize heap...";
    heap = {0, 0, 0, 0, 0};
    nexts = {1, 1};
    hlen2 = 2;
    while (hlen2 < nargs)
      nexts = {@nexts, @nexts};
      heap = {@heap, @heap};
      hlen2 = hlen2 * 2;
    endwhile
    for n in [-nargs..-1]
      s1 = args[i = -n][1];
      while ((hleft = heap[2 * i]) && s1 > (m = min(la = args[hleft][1], (hright = heap[2 * i + 1]) ? args[hright][1] | $maxint)))
        if (m == la)
          heap[i] = hleft;
          i = 2 * i;
        else
          heap[i] = hright;
          i = 2 * i + 1;
        endif
      endwhile
      heap[i] = -n;
    endfor
    "...";
    "...find first interval...";
    h = heap[1];
    rseq = {args[h][1]};
    if (length(args[h]) < 2)
      return rseq;
    endif
    current_end = args[h][2];
    nexts[h] = 3;
    "...";
    while (1)
      if (length(args[h]) >= nexts[h])
        "...this sequence has some more intervals in it...";
      else
        "...no more intevals left in this sequence, grab another...";
        h = heap[1] = heap[nargs];
        heap[nargs] = 0;
        if ((nargs = nargs - 1) > 1)
        elseif (args[h][nexts[h]] > current_end)
          return {@rseq, current_end, @(args[h])[nexts[h]..$]};
        elseif ((i = $list_utils:find_insert(args[h], current_end)) % 2)
          return {@rseq, current_end, @(args[h])[i..$]};
        else
          return {@rseq, @(args[h])[i..$]};
        endif
      endif
      "...";
      "...sink the top sequence...";
      i = 1;
      first = args[h][nexts[h]];
      while ((hleft = heap[2 * i]) && first > (m = min(la = args[hleft][nexts[hleft]], (hright = heap[2 * i + 1]) ? args[hright][nexts[hright]] | $maxint)))
        if (m == la)
          heap[i] = hleft;
          i = 2 * i;
        else
          heap[i] = hright;
          i = 2 * i + 1;
        endif
      endwhile
      heap[i] = h;
      "...";
      "...check new top sequence ...";
      if (args[h = heap[1]][nexts[h]] > current_end)
        "...hey, a new interval! ...";
        rseq = {@rseq, current_end, args[h][nexts[h]]};
        if (length(args[h]) <= nexts[h])
          return rseq;
        endif
        current_end = args[h][nexts[h] + 1];
        nexts[h] = nexts[h] + 2;
      else
        "...first interval overlaps with current one ...";
        i = $list_utils:find_insert(args[h], current_end);
        if (i % 2)
          nexts[h] = i;
        elseif (i > length(args[h]))
          return rseq;
        else
          current_end = args[h][i];
          nexts[h] = i + 1;
        endif
      endif
    endwhile
  endverb

  verb intersection (this none this) owner: HACKER flags: "rxd"
    ":intersection(seq1,seq2,...) => intersection of all sequences...";
    if ((U = {$minint}) in args)
      args = $list_utils:setremove_all(args, U);
    endif
    if (length(args) <= 1)
      return args ? args[1] | U;
    endif
    return this:complement(this:_union(@$list_utils:map_arg(this, "complement", args)));
  endverb
endobject