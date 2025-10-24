object BIGLIST
  name: "Generic BigList Utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property about (owner: HACKER, flags: "rc") = {
    "Implementation notes",
    "--------------------",
    "Each biglist is actually a tree (a kind of B-tree, actually).",
    "The routines above pass around handles of the form",
    "",
    "    {root_node, size, leftmost_ord}",
    "",
    "where root_node is the (string) name of a property that holds the root of the tree, size is the number of leaves in the tree, and leftmost_ord is the :_ord value of the leftmost element of the list (i.e., the leftmost leaf).",
    "Each node property has a value of the form ",
    "",
    "    {height,list of subtrees}.",
    "",
    "where the each of the subtrees is itself a 3-element list as above unless",
    "the height is 0, in which case the subtrees are actually biglist elements of the arbitrary form determined by the home object.",
    "At every level, each node except the rightmost has between this.maxfanout/2 and this.maxfanout subtrees; the rightmost is allowed to have as few as 1 subtree."
  };
  property maxfanout (owner: HACKER, flags: "rc") = 7;

  override aliases = {"ghblu", "biglist_utils"};
  override description = {
    "This is the Generic BigList Utilities utility package.  See `help $biglist' for more details."
  };
  override help_msg = {
    "Generic BigList Utilities",
    "----------------------------",
    "This is a package for maintaining huge persistent (sorted) lists in a format that is less likely to spam the server (which runs into a certain amount of trouble dealing with long ordinary lists --- btw we use `biglist' to refer to the huge data structure we're about to describe and `list' to refer to ordinary MOO lists {...}).  The biglist in question lives on a particular object, to which we will refer in the discussion below as the `home' object, and its various elements appear as leaves of a tree whose nodes are kept in properties of the home object.  It should be noted that the home object does not need to be (and in fact should *not* be) a descendant of this one; this object merely provides utilities for manipulating the properties on the home object that are used in a particular biglist manipulation.  ",
    "",
    "All of the utilities below refer to `caller' to locate the home object.  Thus verbs to manipulate a given biglist must be located on or inherited by its home object itself.  The home object needs to define the following verbs",
    "",
    "  :_make(@args)     => new property on home object with value args",
    "  :_kill(prop)      delete a given property that was created by :_make",
    "  :_get(prop)       => home.prop",
    "  :_put(prop,@args) set home.prop = args",
    "  :_ord(element)    given something that is of the form of a biglist element",
    "                    return the corresponding ordinal (for sorting purposes).",
    "                    If you never intend to use :find_ord, then this can be a ",
    "                    routine that always returns 0 or some other random value.",
    "",
    "See #5546 (Generic Biglist Resident) or $big_mail_recipient",
    "for examples.",
    "",
    "Those of the following routines that take a biglist argument are expecting",
    "either {} (empty biglist) or some biglist returned by one of the other routines",
    "",
    "  :length(biglist)          => length(biglist) (i.e., number of elements)",
    "  :find_nth(biglist,n)      => biglist[n]",
    "  :find_ord(biglist,k,comp) => n where n is",
    "     the largest such that home:(comp)(k,home:_ord(biglist[n])) is false, or",
    "     the smallest such that home:(comp)(k,home:_ord(biglist[n+1])) is true.",
    "     Always returns a value between 0 and length(biglist) inclusive.",
    "     This assumes biglist to be sorted in order of increasing :_ord values ",
    "     with respect to home:(comp)().",
    "     Standard situation is :_ord returns a number and comp is a < verb.",
    "",
    "  :start(biglist,s,e)  => {biglist[s..?],@handle} or {}",
    "  :next(@handle)       => {biglist[?+1..??],@newhandle} or {}",
    "     These two are used for iterating over a range of elements of a biglist",
    "     The canonical incantation for doing",
    "        for elt in (biglist[first..last])",
    "          ...",
    "        endfor",
    "     is",
    "        handle = :start(biglist,first,last);",
    "        while(handle)",
    "          for elt in (handle[1])",
    "            ...",
    "          endfor",
    "          handle = :next(@listdelete(handle,1));",
    "        endwhile",
    "",
    "The following all destructively modify their biglist argument(s) L (and M).",
    "",
    "  :set_nth(L,n,value)  =>  L[n] = value",
    "     replaces the indicated element",
    "",
    "  :insert_before(L,M,n) => {@L[1..n-1],@M,@L[n..length(L)]}",
    "  :insert_after (L,M,n) => {@L[1..n],  @M,@L[n+1..length(L)]}",
    "     takes two distinct biglists, inserts one into the other at the given point",
    "     returns the resulting consolidated biglist",
    "",
    "  :extract_range(L,m,n) => {{@L[1..m-1],@L[n+1..]}, L[m..n]} ",
    "     breaks the given biglist into two distinct biglists.",
    "",
    "  :delete_range(L,m,n[,leafkiller]) => {@L[1..m-1],@L[n+1..]}",
    "  :keep_range  (L,m,n[,leafkiller]) => L[m..n]",
    "     like extract_range only we destroy what we don't want.",
    "",
    "  :insertlast(L,value)  => {@L,value}",
    "     inserts a new element at the end of biglist.  ",
    "     If find_ord is to continue to work properly, it is assumed that the ",
    "     home:_ord(elt) is greater (comp-wise) than all of the :_ord values",
    "     of elements currently in the biglist.",
    "",
    "  :kill(L[,leafkiller]) ",
    "     destroys all nodes used by biglist.  ",
    "     Calls home:leafkiller on each element."
  };
  override object_size = {22666, 1084848672};

  verb length (this none this) owner: HACKER flags: "rxd"
    ":length(tree) => number of leaves in tree.";
    return args[1] ? args[1][2] | 0;
  endverb

  verb find_nth (this none this) owner: HACKER flags: "rxd"
    ":find_nth(tree,n) => nth leaf of tree.  Assumes n in [1..tree[2]]";
    return this:_find_nth(caller, @args);
  endverb

  verb find_ord (this none this) owner: HACKER flags: "rxd"
    ":_find_ord(tree,n,comp) ";
    " => index of rightmost leaf for which :(comp)(n,:_ord(leaf)) is false.";
    "returns 0 if true for all leaves.";
    return args[1] ? this:_find_ord(caller, @args) | 0;
  endverb

  verb set_nth (this none this) owner: HACKER flags: "rxd"
    ":set_nth(tree,n,value) => tree";
    "modifies tree so that nth leaf == value";
    if ((n = args[2]) < 1 || (!(tree = args[1]) || tree[2] < n))
      return E_RANGE;
    else
      this:_set_nth(caller, @args);
      return n != 1 ? tree | listset(tree, caller:_ord(args[3]), 3);
    endif
  endverb

  verb kill (this none this) owner: HACKER flags: "rxd"
    ":kill(tree[,leafverb]) deletes tree and _kills all of the nodes that it uses.";
    "if leafverb is given, caller:leafverb is called on all leaves in tree.";
    if (tree = args[1])
      lverb = {@args, ""}[2];
      this:_skill(caller, typeof(tree) == LIST ? tree[1] | tree, lverb);
    endif
    "... otherwise nothing to do...";
  endverb

  verb "insert_after insert_before" (this none this) owner: HACKER flags: "rxd"
    ":insert_after(tree,subtree,n)";
    ":insert_before(tree,subtree,n)";
    "  inserts subtree after (before) the nth leaf of tree,";
    "  returning the resulting tree.";
    subtree = args[2];
    if (tree = args[1])
      if (subtree)
        where = args[3] - (verb == "insert_before");
        if (where <= 0)
          return this:_merge(caller, subtree, tree);
        elseif (where >= tree[2])
          return this:_merge(caller, tree, subtree);
        else
          s = this:_split(caller, caller:_get(tree[1])[1], where, tree);
          return this:_merge(caller, this:_merge(caller, s[1], subtree), s[2]);
        endif
      else
        return tree;
      endif
    else
      return subtree;
    endif
  endverb

  verb extract_range (this none this) owner: HACKER flags: "rxd"
    ":extract_range(tree,first,last) => {newtree,extraction}";
    return this:_extract(caller, @args);
  endverb

  verb delete_range (this none this) owner: HACKER flags: "rxd"
    ":delete_range(tree,first,last[,leafkill]) => newtree";
    extract = this:_extract(caller, @args);
    if (die = extract[2])
      this:_skill(caller, die[1], {@args, ""}[4]);
    endif
    return extract[1];
  endverb

  verb keep_range (this none this) owner: HACKER flags: "rxd"
    ":keep_range(tree,first,last[,leafkill]) => range";
    extract = this:_extract(caller, @args);
    if (die = extract[1])
      this:_skill(caller, die[1], {@args, ""}[4]);
    endif
    return extract[2];
  endverb

  verb insert_last (this none this) owner: HACKER flags: "rxd"
    ":insert_last(tree,insert) => newtree";
    "insert a new leaf to be inserted at the righthand end of the tree";
    tree = args[1];
    insert = args[2];
    if (!tree)
      return {caller:_make(0, {insert}), 1, caller:_ord(insert)};
    endif
    hgt = caller:_get(tree[1]);
    rspine = {{tree, plen = length(kids = hgt[2])}};
    for i in [1..hgt[1]]
      parent = kids[plen];
      kids = caller:_get(parent[1])[2];
      plen = length(kids);
      rspine = {{parent, plen}, @rspine};
    endfor
    iord = caller:_ord(insert);
    for h in [1..length(rspine)]
      "... tree is the plen'th (rightmost) child of parent...";
      if (rspine[h][2] < this.maxfanout)
        parent = rspine[h][1];
        hgp = caller:_get(parent[1]);
        caller:_put(parent[1], @listset(hgp, {@hgp[2], insert}, 2));
        for p in (rspine[h + 1..length(rspine)])
          rkid = listset(parent, parent[2] + 1, 2);
          parent = p[1];
          hgp = caller:_get(parent[1]);
          caller:_put(parent[1], @listset(hgp, listset(hgp[2], rkid, p[2]), 2));
        endfor
        return listset(tree, tree[2] + 1, 2);
      endif
      insert = {caller:_make(h - 1, {insert}), 1, iord};
    endfor
    return {caller:_make(length(rspine), {tree, insert}), tree[2] + 1, tree[3]};
  endverb

  verb start (this none this) owner: HACKER flags: "rxd"
    ":start(tree,first,last) => {list of leaf nodes, @handle}";
    "handle is of the form {{node,next,size}...}";
    if (tree = args[1])
      before = max(0, args[2] - 1);
      howmany = min(args[3], tree[2]) - before;
      if (howmany <= 0)
        return {};
      else
        spine = {};
        for h in [1..caller:_get(tree[1])[1]]
          ik = this:_listfind_nth(kids = caller:_get(tree[1])[2], before);
          newh = kids[ik[1]][2] - ik[2];
          if (newh < howmany)
            spine = {{tree[1], ik[1] + 1, howmany - newh}, @spine};
            howmany = newh;
          endif
          tree = kids[ik[1]];
          before = ik[2];
        endfor
        return {(caller:_get(tree[1])[2])[before + 1..before + howmany], @spine};
      endif
    else
      return {};
    endif
  endverb

  verb next (this none this) owner: HACKER flags: "rxd"
    ":next(@handle) => {list of more leaf nodes, @newhandle}";
    if (args)
      spine = listdelete(args, 1);
      node = args[1][1];
      n = args[1][2];
      size = args[1][3];
      for h in [1..caller:_get(node)[1]]
        nnode = caller:_get(node)[2][n];
        if (size > nnode[2])
          spine = {{node, n + 1, size - nnode[2]}, @spine};
          size = nnode[2];
        endif
        n = 1;
        node = nnode[1];
      endfor
      test = caller:_get(node);
      return {(test[2])[n..size], @spine};
    else
      return {};
    endif
  endverb

  verb _find_nth (this none this) owner: HACKER flags: "rxd"
    ":_find_nth(home,tree,n) => nth leaf of tree.";
    "...Assumes n in [1..tree[2]]";
    if (caller != this)
      return E_PERM;
    endif
    {home, tree, n} = args;
    if ((p = home:_get(tree[1]))[1])
      for k in (p[2])
        if (n > k[2])
          n = n - k[2];
        else
          return this:_find_nth(home, k, n);
        endif
      endfor
      return E_RANGE;
    else
      return p[2][n];
    endif
  endverb

  verb _find_ord (this none this) owner: HACKER flags: "rxd"
    ":_find_ord(home,tree,n,less_than) ";
    " => index of rightmost leaf for which :(less_than)(n,:_ord(leaf)) is false.";
    "returns 0 if true for all leaves.";
    if (caller != this)
      return E_PERM;
    endif
    {home, tree, n, less_than} = args;
    if ((p = home:_get(tree[1]))[1])
      sz = tree[2];
      for i in [-length(p[2])..-1]
        k = p[2][-i];
        sz = sz - k[2];
        if (!this:_call(home, less_than, n, k[3]))
          return sz + this:_find_ord(home, k, n, less_than);
        endif
      endfor
      return 0;
    else
      for i in [1..r = length(p[2])]
        if (this:_call(home, less_than, n, home:_ord(p[2][i])))
          return i - 1;
        endif
      endfor
      return r;
    endif
  endverb

  verb _set_nth (this none this) owner: HACKER flags: "rxd"
    ":_set_nth(home,tree,n,value) => tree[n] = value";
    "Assumes n in [1..tree[2]]";
    if (caller != this)
      return E_PERM;
    endif
    {home, tree, n, value} = args;
    if ((p = home:_get(tree[1]))[1])
      ik = this:_listfind_nth(p[2], n - 1);
      this:_set_nth(home, p[2][ik[1]], ik[2] + 1, value);
      if (!ik[2])
        p[2][ik[1]][3] = home:_ord(value);
        home:_put(tree[1], @p);
      endif
    else
      p[2][n] = value;
      home:_put(tree[1], @p);
    endif
  endverb

  verb _skill (this none this) owner: HACKER flags: "rxd"
    ":_skill(home,node,kill_leaf)";
    "home:_kill's node and all descendants, home:(kill_leaf)'s all leaves";
    if (caller != this)
      return E_PERM;
    endif
    {home, node, kill_leaf} = args;
    try
      {height, subtrees} = home:_get(node) || {0, {}};
    except (E_PROPNF)
      return;
    endtry
    if (height)
      for kid in (subtrees)
        this:_skill(home, kid[1], kill_leaf);
      endfor
    elseif (kill_leaf)
      for kid in (subtrees)
        this:_call(home, kill_leaf, kid);
      endfor
    endif
    home:_kill(node);
  endverb

  verb _extract (this none this) owner: HACKER flags: "rxd"
    ":_extract(home,tree,first,last) => {newtree,extraction}";
    if (caller != this)
      return E_PERM;
    endif
    home = args[1];
    if (!(tree = args[2]))
      return {{}, {}};
    endif
    before = max(0, args[3] - 1);
    end = min(tree[2], args[4]);
    if (end <= 0 || before >= end)
      return {tree, {}};
    endif
    height = home:_get(tree[1])[1];
    if (end < tree[2])
      r = this:_split(home, height, end, tree);
      if (before)
        l = this:_split(home, height, before, r[1]);
        extract = l[2];
        newtree = this:_merge(home, l[1], r[2]);
      else
        extract = r[1];
        newtree = r[2];
      endif
    elseif (before)
      l = this:_split(home, height, before, tree);
      extract = l[2];
      newtree = l[1];
    else
      return {{}, tree};
    endif
    return {this:_scrunch(home, newtree), this:_scrunch(home, extract)};
  endverb

  verb _merge (this none this) owner: HACKER flags: "rxd"
    "_merge(home,ltree,rtree) => newtree";
    "assumes ltree and rtree to be nonempty.";
    if (caller != this)
      return E_PERM;
    endif
    {home, lnode, rnode} = args;
    lh = home:_get(lnode[1])[1];
    rh = home:_get(rnode[1])[1];
    if (lh > rh)
      return this:_rmerge(home, lnode, rnode);
    endif
    for h in [lh + 1..rh]
      lnode[1] = home:_make(h, {lnode});
    endfor
    m = this:_smerge(home, rh, lnode, rnode);
    return length(m) <= 1 ? m[1] | {home:_make(rh + 1, m), m[1][2] + m[2][2], m[1][3]};
  endverb

  verb _smerge (this none this) owner: HACKER flags: "rxd"
    "_smerge(home, height, ltree, rtree) =>{ltree[,rtree]}";
    "assumes ltree and rtree are at the given height.";
    "merges the trees if the combined number of children is <= maxfanout";
    "otherwise returns two trees where ltree is guaranteed minfanout children and rtree is guaranteed the minimum of minfanout and however many children it started with.";
    if (caller != this)
      return E_PERM;
    endif
    {home, height, ltree, rtree} = args;
    llen = length(lkids = home:_get(ltree[1])[2]);
    rlen = length(rkids = home:_get(rtree[1])[2]);
    if (height)
      m = this:_smerge(home, height - 1, lkids[llen], rkids[1]);
      mlen = length(mkids = {@listdelete(lkids, llen), @m, @listdelete(rkids, 1)});
      if (mlen <= this.maxfanout)
        home:_put(ltree[1], height, mkids);
        home:_kill(rtree[1]);
        ltree[2] = ltree[2] + rtree[2];
        return {ltree};
      else
        S = max(llen - 1, (mlen + 1) / 2);
        home:_put(ltree[1], height, mkids[1..S]);
        home:_put(rtree[1], height, mkids[S + 1..$]);
        xfer = -lkids[llen][2];
        for k in (mkids[llen..S])
          xfer = xfer + k[2];
        endfor
        ltree[2] = ltree[2] + xfer;
        rtree[2] = rtree[2] - xfer;
        rtree[3] = mkids[S + 1][3];
        return {ltree, rtree};
      endif
    elseif (llen * 2 >= this.maxfanout)
      return {ltree, rtree};
    elseif (this.maxfanout < llen + rlen)
      T = (rlen - llen + 1) / 2;
      home:_put(ltree[1], 0, {@lkids, @rkids[1..T]});
      home:_put(rtree[1], 0, rkids[T + 1..rlen]);
      ltree[2] = ltree[2] + T;
      rtree[2] = rtree[2] - T;
      rtree[3] = home:_ord(rkids[T + 1]);
      return {ltree, rtree};
    else
      home:_put(ltree[1], 0, {@lkids, @rkids});
      home:_kill(rtree[1]);
      ltree[2] = ltree[2] + rtree[2];
      return {ltree};
    endif
  endverb

  verb _split (this none this) owner: HACKER flags: "rxd"
    "_split(home, height,lmax,ltree[,@rtrees]}) => {ltree,[mtree,]@rtrees}";
    "ltree is split after the lmax'th leaf, the righthand portion grafted onto the leftmost of the rtrees, if possible.  Otherwise we create a new tree mtree, stealing from rtrees[1] if necessary.";
    "Assumes 1<=lmax<ltree[2]";
    if (caller != this)
      return E_PERM;
    endif
    {home, height, lmax, ltree, @rtrees} = args;
    llen = length(lkids = home:_get(ltree[1])[2]);
    rlen = length(rkids = rtrees ? home:_get(rtrees[1][1])[2] | {});
    if (height)
      ik = this:_listfind_nth(lkids, lmax);
      if (ik[2])
        llast = ik[1];
        m = this:_split(home, height - 1, ik[2], lkids[llast], @lkids[llast + 1..llen], @rkids);
        lkids[llast] = m[1];
        mkids = listdelete(m, 1);
      else
        llast = ik[1] - 1;
        mkids = {@lkids[ik[1]..llen], @rkids};
      endif
      home:_put(ltree[1], height, lkids[1..llast]);
      mlen = length(mkids);
      if ((mlen - rlen) * 2 >= this.maxfanout || !rtrees)
        "...residue left over from splitting ltree can stand by itself...";
        return {listset(ltree, lmax, 2), {home:_make(height, mkids[1..mlen - rlen]), ltree[2] - lmax, mkids[1][3]}, @rtrees};
      elseif (mlen <= this.maxfanout)
        "...residue left over from splitting ltree fits in rtrees[1]...";
        home:_put(rtrees[1][1], height, mkids);
        rtrees[1][2] = ltree[2] - lmax + rtrees[1][2];
        rtrees[1][3] = mkids[1][3];
        return {listset(ltree, lmax, 2), @rtrees};
      else
        "...need to steal from rtrees[1]...";
        if (llast < llen)
          msize = ltree[2] - lmax;
          R = mlen - rlen + 1;
        else
          msize = 0;
          R = 1;
        endif
        for k in (mkids[R..mlen / 2])
          msize = msize + k[2];
        endfor
        home:_put(rtrees[1][1], height, mkids[mlen / 2 + 1..mlen]);
        rtrees[1][2] = rtrees[1][2] + ltree[2] - (lmax + msize);
        rtrees[1][3] = mkids[mlen / 2 + 1][3];
        return {listset(ltree, lmax, 2), {home:_make(height, mkids[1..mlen / 2]), msize, mkids[1][3]}, @rtrees};
      endif
    else
      home:_put(ltree[1], 0, lkids[1..lmax]);
      if ((llen - lmax) * 2 >= this.maxfanout || !rtrees)
        "...residue left over from splitting ltree can stand by itself...";
        return {listset(ltree, lmax, 2), {home:_make(0, lkids[lmax + 1..llen]), llen - lmax, home:_ord(lkids[lmax + 1])}, @rtrees};
      elseif ((mlen = rlen + llen - lmax) <= this.maxfanout)
        "...residue left over from splitting ltree fits in rtrees[1]...";
        home:_put(rtrees[1][1], 0, {@lkids[lmax + 1..llen], @rkids});
        rtrees[1][2] = mlen;
        rtrees[1][3] = home:_ord(lkids[lmax + 1]);
        return {listset(ltree, lmax, 2), @rtrees};
      else
        "...need to steal from rtrees[1]...";
        home:_put(rtrees[1][1], 0, rkids[(R = (rlen - llen + lmax) / 2) + 1..rlen]);
        rtrees[1][2] = (mlen + 1) / 2;
        rtrees[1][3] = home:_ord(rkids[R + 1]);
        return {listset(ltree, lmax, 2), {home:_make(0, {@lkids[lmax + 1..llen], @rkids[1..R]}), mlen / 2, home:_ord(lkids[lmax + 1])}, @rtrees};
      endif
    endif
  endverb

  verb _rmerge (this none this) owner: HACKER flags: "rxd"
    ":_rmerge(home, tree, insertree) => newtree ";
    "(newtree is tree with insertree appended to the right)";
    "insertree is assumed to be of height < tree";
    if (caller != this)
      return E_PERM;
    endif
    {home, tree, insert} = args;
    if (!tree)
      return insert;
    elseif (!insert)
      return tree;
    endif
    iheight = home:_get(insert[1])[1];
    rspine = {};
    for i in [iheight + 1..home:_get(tree[1])[1]]
      kids = home:_get(tree[1])[2];
      tlen = length(kids);
      rspine = {{tree, tlen}, @rspine};
      tree = kids[tlen];
    endfor
    isize = insert[2];
    m = this:_smerge(home, iheight, tree, insert);
    for h in [1..length(rspine)]
      plen = rspine[h][2];
      parent = rspine[h][1];
      hgp = home:_get(parent[1]);
      if (length(m) - 1 + plen > this.maxfanout)
        home:_put(parent[1], @listset(hgp, listset(hgp[2], m[1], plen), 2));
        parent[2] = parent[2] + isize - m[2][2];
        m = {parent, listset(m[2], home:_make(h + iheight, {m[2]}), 1)};
      else
        home:_put(parent[1], @listset(hgp, {@(hgp[2])[1..plen - 1], @m}, 2));
        for p in (rspine[h + 1..length(rspine)])
          parent[2] = parent[2] + isize;
          tree = parent;
          parent = p[1];
          hgp = home:_get(parent[1]);
          home:_put(parent[1], @listset(hgp, listset(hgp[2], tree, p[2]), 2));
        endfor
        return listset(parent, parent[2] + isize, 2);
      endif
    endfor
    return {home:_make(length(rspine) + iheight + 1, m), m[1][2] + m[2][2], m[1][3]};
  endverb

  verb _scrunch (this none this) owner: HACKER flags: "rxd"
    ":_scrunch(home,tree) => newtree";
    "decapitates single-child nodes from the top of the tree, returns new root.";
    if (caller != this)
      return E_PERM;
    endif
    if (tree = args[2])
      home = args[1];
      while ((n = home:_get(tree[1]))[1] && length(n[2]) == 1)
        home:_kill(tree[1]);
        tree = n[2][1];
      endwhile
    endif
    return tree;
  endverb

  verb _listfind_nth (this none this) owner: HACKER flags: "rxd"
    "_listfind_nth(nodelist,key) => {i,k} where i is the smallest i such that the sum of the first i elements of intlist is > key, and k==key - sum(first i-1 elements).";
    "1 <= i <= length(intlist)+1";
    {lst, key} = args;
    for i in [1..length(lst)]
      key = key - lst[i][2];
      if (0 > key)
        return {i, key + lst[i][2]};
      endif
    endfor
    return {length(lst) + 1, key};
  endverb

  verb _insertfirst (this none this) owner: HACKER flags: "rxd"
    if (caller != this)
      return E_PERM;
    endif
  endverb

  verb debug (this none this) owner: HACKER flags: "rxd"
    return $perm_utils:controls(caller_perms(), this) ? this:((args[1]))(@listdelete(args, 1)) | E_PERM;
  endverb

  verb _call (this none this) owner: #2 flags: "rxd"
    ":_call(home,verb,@vargs) calls home:verb(@vargs) with $no_one's perms";
    set_task_perms($no_one);
    if (caller != this)
      raise(E_PERM);
    endif
    {home, vb, @vargs} = args;
    return home:(vb)(@vargs);
  endverb
endobject