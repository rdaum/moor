object BIG_MAIL_RECIPIENT
  name: "Generic Large-Capacity Mail Recipient"
  parent: MAIL_RECIPIENT
  owner: HACKER
  fertile: true
  readable: true

  property _genprop (owner: HACKER, flags: "r") = "";
  property _mgr (owner: HACKER, flags: "rc") = BIGLIST;
  property mowner (owner: HACKER, flags: "r") = HACKER;
  property summary_uses_body (owner: HACKER, flags: "rc") = 0;

  override aliases = {"Generic Large-Capacity Mail Recipient"};
  override description = {
    "Generic Large Capacity Mail Recipient",
    "-------------------------------------",
    "Since any modifications to large lists entail copying the entire list over, operations on ordinary mail recipients having large numbers of messages, that actually change the content of .messages will take inordinately long.  Thus we have this version which makes use of the $biglist package, scattering the messages onto numerous properties so that write operations involving only a few messages will not require recopying of the entire list.",
    "",
    "In nearly all respects it behaves as the ordinary Mail Recipient, except that it is faster for certain kinds of operations.",
    "",
    "Certain unimplemented verbs, like :date_sort(), and :messages() currently return E_VERBNF.",
    "",
    "To convert an existing $mail_recipient-child (call it #MR) into a $big_mail_recipient-child the basic procedure is",
    "",
    "    ;;something.foo= #MR:messages();",
    "    @rmm 1-$ from #MR",
    "    @unrmm expunge",
    "    @chparent #MR to $big_mail_recipient",
    "    ;#MR:receive_batch(@something.foo);",
    "",
    "Reconstructing Damaged Big Mail Recipients",
    "------------------------------------------",
    "On rare occasions, the tree structure created by $biglist can be corrupted (this can happen on lists sufficiently large that a list-modification operation (e.g., @rmm, @renumber) runs out of ticks/seconds).  In the vast majority of such cases, your messages are all still there; it's simply that the tree we use for finding/searching them is messed up.",
    "",
    "To recover messages from a damaged big mail recipient (#DBMR)",
    " --- read to the end before you start typing any commands ---",
    "",
    "create a fresh $big_mail_recipient (#NEWBMR) and then do the following:",
    "",
    "   ;#NEWBMR:restore_from(#DBMR)",
    "",
    "When this finishes, #NEWBMR will contain all of the mail messages we were able to find.  (note that this will include messages that you had deleted from #DBMR but not expunged).  #NEWMBR should thenceforth be useable in place of #DBMR, however if #DBMR contains custom verbs and non-clear properties, these will also need to be copied over.",
    "",
    "Alternatively, one may do",
    "",
    "   @copyobject #DBMR to #TEMPBMR",
    "   ;#DBMR:restore_from(#TEMPBMR)",
    "",
    "to rebuild #DBMR in place.  This, however, will take about twice as long.",
    "",
    "oooooooooooooooooooooooooooooooo",
    "WARNING!!! WARNING!!! WARNING!!!",
    "oooooooooooooooooooooooooooooooo",
    "",
    "Calling #OBJ:restore_from(...) COMPLETELY AND IRREVOCABLY REMOVES ALL MESSAGES from the object that it is run on (#OBJ); you MUST be sure to EITHER have made a copy of #OBJ OR be doing the restore to a DIFFERENT object."
  };
  override import_export_id = "big_mail_recipient";
  override object_size = {37437, 1084848672};

  verb _genprop (this none this) owner: HACKER flags: "rxd"
    gp = this._genprop;
    ngp = "";
    for i in [1..length(gp)]
      if (gp[i] != "z")
        ngp = ngp + "bcdefghijklmnopqrstuvwxyz"[index("abcdefghijklmnopqrstuvwxy", gp[i])] + gp[i + 1..length(gp)];
        return " " + (this._genprop = ngp);
      endif
      ngp = ngp + "a";
    endfor
    return " " + (this._genprop = ngp + "a");
  endverb

  verb _make (this none this) owner: #2 flags: "rxd"
    ":_make(...) => new node with value {...}";
    if (!(caller in {this._mgr, this}))
      return E_PERM;
    endif
    prop = this:_genprop();
    `add_property(this, prop, args, {this.mowner, ""}) ! ANY';
    return prop;
  endverb

  verb _kill (this none this) owner: #2 flags: "rxd"
    ":_kill(node) destroys the given node.";
    if (!(caller in {this, this._mgr}))
      return E_PERM;
    endif
    `delete_property(this, args[1]) ! ANY';
  endverb

  verb _get (this none this) owner: HACKER flags: "rxd"
    return caller == this._mgr ? `this.((args[1])) ! ANY' | E_PERM;
  endverb

  verb _put (this none this) owner: HACKER flags: "rxd"
    return caller == this._mgr ? this.((args[1])) = listdelete(args, 1) | E_PERM;
  endverb

  verb _ord (this none this) owner: HACKER flags: "rxd"
    return (args[1])[2..3];
  endverb

  verb _makemsg (this none this) owner: HACKER flags: "rxd"
    ":_makemsg(ord,msg) => leafnode for msg";
    "msg = $mail_agent:__convert_new(@args[2])";
    msg = args[2];
    if (caller != this)
      return E_PERM;
    elseif (h = "" in msg)
      return {this:_make(@msg[h + 1..$]), args[1], @msg[1..h - 1]};
    else
      return {0, args[1], @msg};
    endif
  endverb

  verb _killmsg (this none this) owner: HACKER flags: "rxd"
    if (caller != this._mgr)
      return E_PERM;
    elseif (node = args[1][1])
      this:_kill(node);
    endif
  endverb

  verb _message_num (this none this) owner: HACKER flags: "rxd"
    return args[2];
  endverb

  verb _message_date (this none this) owner: HACKER flags: "rxd"
    return args[3];
  endverb

  verb _message_hdr (this none this) owner: HACKER flags: "rxd"
    return args[3..$];
  endverb

  verb _message_text (this none this) owner: HACKER flags: "rxd"
    if (caller == this || this:is_readable_by(caller_perms()))
      "perms check added HTC 16 Feb 1999";
      return {@args[3..$], @args[1] ? {"", @this.((args[1]))} | {}};
    else
      return E_PERM;
    endif
  endverb

  verb _lt_msgnum (this none this) owner: HACKER flags: "rxd"
    return args[1] < args[2][1];
  endverb

  verb _lt_msgdate (this none this) owner: HACKER flags: "rxd"
    return args[1] < args[2][2];
  endverb

  verb receive_batch (this none this) owner: HACKER flags: "rxd"
    if (!this:is_writable_by(caller_perms()))
      return E_PERM;
    else
      new = this:new_message_num();
      msgtree = this.messages;
      for m in (args)
        msgtree = this._mgr:insert_last(msgtree, this:_makemsg(new, m[2]));
        new = new + 1;
        if ($command_utils:running_out_of_time())
          this.messages = msgtree;
          player:tell("... ", new);
          suspend(0);
          msgtree = this.messages;
          new = this:new_message_num();
        endif
      endfor
      this.messages = msgtree;
      this.last_used_time = time();
      return 1;
    endif
  endverb

  verb receive_message (this none this) owner: HACKER flags: "rxd"
    if (!this:is_writable_by(caller_perms()))
      return E_PERM;
    else
      this.messages = this._mgr:insert_last(this.messages, msg = this:_makemsg(new = this:new_message_num(), args[1]));
      this.last_msg_date = this:_message_date(@msg);
      this.last_used_time = time();
      return new;
    endif
  endverb

  verb messages_in_seq (this none this) owner: HACKER flags: "rxd"
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (typeof(seq = args[1]) != TYPE_LIST)
      x = this._mgr:find_nth(this.messages, seq);
      return {this:_message_num(@x), this:_message_text(@x)};
    else
      msgs = {};
      while (seq)
        handle = this._mgr:start(this.messages, seq[1], seq[2] - 1);
        while (handle)
          for x in (handle[1])
            msgs = {@msgs, {this:_message_num(@x), this:_message_text(@x)}};
          endfor
          handle = this._mgr:next(@listdelete(handle, 1));
          $command_utils:suspend_if_needed(0);
        endwhile
        seq = seq[3..$];
      endwhile
      return msgs;
    endif
  endverb

  verb display_seq_headers (this none this) owner: HACKER flags: "rxd"
    ":display_seq_headers(msg_seq[,cur[,last_read_date]])";
    "This is the default header display routine.";
    "Prints a list of headers of messages on this to player.  msg_seq is the handle returned by this:parse_message_seq(...).  cur is the player's current message.  last_read_date is the date of the last of the already-read messages.";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    getmsg = this.summary_uses_body ? "_message_text" | "_message_hdr";
    {seq, ?cur = 0, ?last_old = $maxint} = args;
    keep_seq = {@$seq_utils:contract(this:kept_msg_seq(), $seq_utils:complement(seq, 1, this:length_all_msgs())), $maxint};
    k = 1;
    mcount = 0;
    width = player:linelen();
    while (seq)
      handle = this._mgr:start(this.messages, seq[1], seq[2] - 1);
      while (handle)
        for x in (handle[1])
          $command_utils:suspend_if_needed(0);
          if (keep_seq[k] <= (mcount = mcount + 1))
            k = k + 1;
          endif
          annot = x[3] > last_old ? "+" | (k % 2 ? " " | "=");
          line = tostr($string_utils:right(x[2], 5, cur == x[2] ? ">" | " "), ":", annot, " ", this:msg_summary_line(@this:(getmsg)(@x)));
          player:tell(line[1..min(width, $)]);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
      seq = seq[3..$];
    endwhile
    player:tell("-----+");
  endverb

  verb display_seq_full (this none this) owner: HACKER flags: "rxd"
    ":display_seq_full(msg_seq[,preamble]) => {cur}";
    "This is the default message display routine.";
    "Prints the indicated messages on folder to player.  msg_seq is the handle returned by folder:parse_message_seq(...).  Returns the number of the final message in the sequence (to be the new current message number).";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    {seq, ?preamble = ""} = args;
    cur = date = 0;
    while (seq)
      handle = this._mgr:start(this.messages, seq[1], seq[2] - 1);
      while (handle)
        for x in (handle[1])
          cur = this:_message_num(@x);
          date = this:_message_date(@x);
          player:display_message(preamble ? strsub(preamble, "%d", tostr(cur)) | {}, this:msg_full_text(@this:_message_text(@x)));
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
        $command_utils:suspend_if_needed(0);
      endwhile
      seq = seq[3..$];
    endwhile
    return {cur, date};
  endverb

  verb list_rmm (this none this) owner: HACKER flags: "rxd"
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    len = 0;
    getmsg = this.summary_uses_body ? "_message_text" | "_message_hdr";
    going = this.messages_going;
    if (going && (!going[1] || typeof(going[1][2]) == TYPE_INT))
      kept = {@going[1], $maxint};
      going = going[2];
    else
      kept = {$maxint};
    endif
    k = 1;
    mcount = 0;
    for s in (going)
      if (kept[k] <= (mcount = mcount + s[1]))
        k = k + 1;
      endif
      len = len + s[2][2];
      handle = this._mgr:start(s[2], 1, s[2][2]);
      while (handle)
        for x in (handle[1])
          if (kept[k] <= (mcount = mcount + 1))
            k = k + 1;
          endif
          player:tell($string_utils:right(this:_message_num(@x), 4), k % 2 ? ":  " | ":= ", this:msg_summary_line(@this:(getmsg)(@x)));
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    if (len)
      player:tell("----+");
    endif
    return len;
  endverb

  verb undo_rmm (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    msgtree = this.messages;
    seq = {};
    last = 0;
    "there are two possible formats here:";
    "OLD: {{n,msgs},{n,msgs},...}";
    "NEW: {kept_seq, {{n,msgs},{n,msgs},...}}";
    going = this.messages_going;
    if (going && (!going[1] || typeof(going[1][2]) == TYPE_INT))
      kept = going[1];
      going = going[2];
    else
      kept = {};
    endif
    for s in (going)
      msgtree = this._mgr:insert_after(msgtree, s[2], last + s[1]);
      seq = {@seq, last + s[1] + 1, (last = last + s[1] + s[2][2]) + 1};
    endfor
    this.messages = msgtree;
    this.messages_going = {};
    this.messages_kept = $seq_utils:union(kept, $seq_utils:expand(this.messages_kept, seq));
    this:_fix_last_msg_date();
    return seq;
  endverb

  verb expunge_rmm (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    len = 0;
    going = this.messages_going;
    if (going && (!going[1] || typeof(going[1][2]) == TYPE_INT))
      going = going[2];
    endif
    for s in (going)
      len = len + s[2][2];
      this._mgr:kill(s[2], "_killmsg");
    endfor
    this.messages_going = {};
    return len;
  endverb

  verb rm_message_seq (this none this) owner: HACKER flags: "rxd"
    seq = args[1];
    if (!(this:ok_write(caller, caller_perms()) || (this:ok(caller, caller_perms()) && (seq = this:own_messages_filter(caller_perms(), @args)))))
      return E_PERM;
    endif
    msgtree = this.messages;
    save = nums = {};
    onext = 1;
    rmmed = 0;
    for i in [1..length(seq) / 2]
      if ($command_utils:suspend_if_needed(0))
        player:tell("... rmm ", onext);
        suspend(0);
      endif
      start = seq[2 * i - 1];
      next = seq[2 * i];
      {msgtree, zmsgs} = this._mgr:extract_range(msgtree, start - rmmed, next - 1 - rmmed);
      save = {@save, {start - onext, zmsgs}};
      nums = {@nums, this:_message_num(@this._mgr:find_nth(zmsgs, 1)), this:_message_num(@this._mgr:find_nth(zmsgs, zmsgs[2])) + 1};
      onext = next;
      rmmed = rmmed + next - start;
    endfor
    tmg = this.messages_going;
    save_kept = $seq_utils:intersection(this.messages_kept, seq);
    this.messages_kept = $seq_utils:contract(this.messages_kept, seq);
    this.messages_going = save_kept ? {save_kept, save} | save;
    fork (0)
      for s in (tmg)
        this._mgr:kill(s[2], "_killmsg");
      endfor
    endfork
    this.messages = msgtree;
    this:_fix_last_msg_date();
    return $seq_utils:tostr(nums);
  endverb

  verb renumber (this none this) owner: HACKER flags: "rxd"
    ":renumber([cur]) renumbers caller.messages, doing a suspend() if necessary.";
    "  => {number of messages,new cur}.";
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    {?cur = 0} = args;
    this:expunge_rmm();
    "... blow away @rmm'ed messages since there's no way to tell what their new numbers should be...";
    if (!(msgtree = this.messages))
      return {0, 0};
    endif
    if (cur)
      cur = this._mgr:find_ord(msgtree, cur - 1, "_lt_msgnum") + 1;
    endif
    while (1)
      "...find first out-of-sequence message...";
      n = 1;
      subtree = msgtree;
      if (msgtree[3][1] == 1)
        while ((node = this.((subtree[1])))[1])
          "...subtree[3][1]==n...";
          kids = node[2];
          n = n + subtree[2];
          i = length(kids);
          while ((n = n - kids[i][2]) != kids[i][3][1])
            i = i - 1;
          endwhile
          subtree = kids[i];
        endwhile
        leaves = node[2];
        n = (firstn = n) + length(leaves) - 1;
        while (n != leaves[n - firstn + 1][2])
          n = n - 1;
        endwhile
        n = n + 1;
      endif
      "... n == first out-of-sequence ...";
      "...renumber as many messages as we have time for...";
      while (n <= msgtree[2] && !$command_utils:running_out_of_time())
        msg = this._mgr:find_nth(msgtree, n);
        msgtree = this._mgr:set_nth(msgtree, n, listset(msg, n, 2));
        n = n + 1;
      endwhile
      this.messages = msgtree;
      if (n > msgtree[2])
        return {n - 1, cur};
      endif
      player:tell("...(renumbering to ", n - 1, ")");
      suspend(0);
      "...start over... may have received new mail, rmm'ed stuff, etc...";
      "...so who knows what's there now?...";
      if (this.messages_going)
        player:tell("Renumber aborted.");
        return;
      endif
      msgtree = this.messages;
    endwhile
  endverb

  verb length_all_msgs (this none this) owner: HACKER flags: "rxd"
    return this:ok(caller, caller_perms()) ? this.messages ? this.messages[2] | 0 | E_PERM;
  endverb

  verb length_num_le (this none this) owner: HACKER flags: "rxd"
    return this:ok(caller, caller_perms()) ? this._mgr:find_ord(this.messages, args[1], "_lt_msgnum") | E_PERM;
  endverb

  verb length_date_le (this none this) owner: HACKER flags: "rxd"
    return this:ok(caller, caller_perms()) ? this._mgr:find_ord(this.messages, args[1], "_lt_msgdate") | E_PERM;
  endverb

  verb exists_num_eq (this none this) owner: HACKER flags: "rxd"
    return this:ok(caller, caller_perms()) ? (i = this._mgr:find_ord(this.messages, args[1], "_lt_msgnum")) && (this:_message_num(@this._mgr:find_nth(this.messages, i)) == args[1] && i) | E_PERM;
  endverb

  verb new_message_num (this none this) owner: HACKER flags: "rxd"
    if (this:ok(caller, caller_perms()))
      new = (msgtree = this.messages) ? this:_message_num(@this._mgr:find_nth(msgtree, msgtree[2])) + 1 | 1;
      if (rmsgs = this.messages_going)
        lbrm = rmsgs[$][2];
        return max(new, this:_message_num(@this._mgr:find_nth(lbrm, lbrm[2])) + 1);
      else
        return new;
      endif
    else
      return E_PERM;
    endif
  endverb

  verb from_msg_seq (this none this) owner: HACKER flags: "rxd"
    ":from_msg_seq(object or list)";
    " => msg_seq of messages from any of these senders";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {plist, ?mask = {1, this.messages[2] + 1}} = args;
    if (typeof(plist) != TYPE_LIST)
      plist = {plist};
    endif
    fseq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          fromline = msg[4];
          for f in ($mail_agent:parse_address_field(fromline))
            if (f in plist)
              fseq = $seq_utils:add(fseq, i, i);
            endif
          endfor
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return fseq || "%f %<has> no messages from " + $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", plist), "no one", " or ");
  endverb

  verb "%from_msg_seq" (this none this) owner: HACKER flags: "rxd"
    ":%from_msg_seq(string or list of strings)";
    " => msg_seq of messages with one of these strings in the from line";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {nlist, ?mask = {1, this.messages[2] + 1}} = args;
    if (typeof(nlist) != TYPE_LIST)
      nlist = {nlist};
    endif
    fseq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          fromline = " " + msg[4];
          for n in (nlist)
            if (index(fromline, n))
              fseq = $seq_utils:add(fseq, i, i);
            endif
          endfor
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return fseq || "%f %<has> no messages from " + $string_utils:english_list($list_utils:map_arg($string_utils, "print", nlist), "no one", " or ");
  endverb

  verb to_msg_seq (this none this) owner: HACKER flags: "rxd"
    ":to_msg_seq(object or list) => msg_seq of messages to those people";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {plist, ?mask = {1, this.messages[2] + 1}} = args;
    if (typeof(plist) != TYPE_LIST)
      plist = {plist};
    endif
    seq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          toline = msg[5];
          for r in ($mail_agent:parse_address_field(toline))
            if (r in plist)
              seq = $seq_utils:add(seq, i, i);
            endif
          endfor
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return seq || "%f %<has> no messages to " + $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", plist), "no one", " or ");
  endverb

  verb "%to_msg_seq" (this none this) owner: HACKER flags: "rxd"
    ":%to_msg_seq(string or list of strings)";
    " => msg_seq of messages containing one of strings in the to line";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {nlist, ?mask = {1, this.messages[2] + 1}} = args;
    if (typeof(nlist) != TYPE_LIST)
      nlist = {nlist};
    endif
    seq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          toline = " " + msg[5];
          for n in (nlist)
            if (index(toline, n))
              seq = $seq_utils:add(seq, i, i);
            endif
          endfor
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return seq || "%f %<has> no messages to " + $string_utils:english_list($list_utils:map_arg($string_utils, "print", nlist), "no one", " or ");
  endverb

  verb subject_msg_seq (this none this) owner: HACKER flags: "rxd"
    ":subject_msg_seq(target) => msg_seq of messages with target in the Subject:";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {target, ?mask = {1, this.messages[2] + 1}} = args;
    seq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          if ((subject = msg[6]) != " " && index(subject, target))
            seq = $seq_utils:add(seq, i, i);
          endif
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return seq || "%f %<has> no messages with subjects containing `" + target + "'";
  endverb

  verb body_msg_seq (this none this) owner: HACKER flags: "rxd"
    ":body_msg_seq(target) => msg_seq of messages with target in the body";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!this.messages)
      return {};
    endif
    {target, ?mask = {1, this.messages[2] + 1}} = args;
    seq = {};
    for m in [1..length(mask) / 2]
      handle = this._mgr:start(this.messages, i = mask[2 * m - 1], mask[2 * m] - 1);
      while (handle)
        for msg in (handle[1])
          if (msg[1] && (body = this.((msg[1]))) && index(tostr(@body), target))
            seq = $seq_utils:add(seq, i, i);
            "Above saves ticks. Munges the whole message into one string and indexes it. Old code follows.";
            "l = length(body);";
            "while (!index(body[l], target) && (l = l - 1))";
            "$command_utils:suspend_if_needed(0);";
            "endwhile";
            "if (l)";
            "seq = $seq_utils:add(seq, i, i);";
            "endif";
          endif
          i = i + 1;
          $command_utils:suspend_if_needed(0);
        endfor
        handle = this._mgr:next(@listdelete(handle, 1));
      endwhile
    endfor
    return seq || tostr("%f %<has> no messages containing `", target, "' in the body.");
  endverb

  verb date_sort (this none this) owner: HACKER flags: "rxd"
    return E_VERBNF;
  endverb

  verb _fix_last_msg_date (this none this) owner: HACKER flags: "rxd"
    msgtree = this.messages;
    this.last_msg_date = msgtree && this:_message_hdr(@this._mgr:find_nth(msgtree, msgtree[2]))[1] || 0;
  endverb

  verb __fix (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    {?doit = 0} = args;
    msgtree = this.messages;
    for n in [1..msgtree[2]]
      msg = this._mgr:find_nth(msgtree, n);
      msg = {@msg[1..2], @$mail_agent:__convert_new(@msg[3..$])};
      if (doit)
        msgtree = this._mgr:set_nth(msgtree, n, msg);
      endif
      if ($command_utils:running_out_of_time())
        suspend(0);
        if (this.messages != msgtree)
          player:notify("urk.  someone played with this folder.");
          return 0;
        endif
      endif
    endfor
    return 1;
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      this._mgr = $biglist;
      this.mowner = $mail_recipient.owner;
      for p in (properties(this))
        $command_utils:suspend_if_needed(0);
        if (p && p[1] == " ")
          delete_property(this, p);
        endif
      endfor
      this.messages = this.messages_going = {};
      this:_fix_last_msg_date();
      this._genprop = "";
      pass(@args);
    endif
  endverb

  verb length_date_gt (this none this) owner: HACKER flags: "rxd"
    if (this:ok(caller, caller_perms()))
      date = args[1];
      return this.last_msg_date <= date ? 0 | this.messages[2] - this._mgr:find_ord(this.messages, args[1], "_lt_msgdate");
    else
      return E_PERM;
    endif
  endverb

  verb _repair (this none this) owner: #2 flags: "rx"
    c = callers();
    if (caller != this && !(length(c) > 1 && c[1][1] == $list_utils && c[1][2] == "map_arg" && c[2][1] == this))
      raise(E_PERM);
    endif
    $command_utils:suspend_if_needed(0);
    biglist = this;
    propname = args[1];
    if (!propname)
      bestlevel = -1;
      best = {};
      for prop in (properties(biglist))
        $command_utils:suspend_if_needed(0);
        if (index(prop, " ") == 1)
          val = biglist.(prop);
          if (typeof(val[1]) == TYPE_INT)
            if (bestlevel < val[1])
              bestlevel = val[1];
              best = {prop};
            elseif (bestlevel == val[1])
              best = {@best, prop};
            endif
          endif
        endif
      endfor
      if (!best)
        player:notify("Can't find a root.");
        raise(E_INVARG);
      elseif (length(best) == 1)
        propname = best[1];
      else
        propname = best[1];
        val = biglist.(propname);
        for prop in (best[2..$])
          $command_utils:suspend_if_needed(0);
          val[2] = {@val[2], @biglist.(prop)[2]};
        endfor
        biglist.(propname) = val;
        "Now that the new value is safely stored, delete old values.";
        for prop in (best[2..$])
          $command_utils:suspend_if_needed(0);
          player:notify(tostr("Removing property ", toliteral(prop), ".  Its value, ", toliteral(biglist.(prop)), ", has been merged with property ", toliteral(propname), "."));
          delete_property(biglist, prop);
        endfor
      endif
      maxlevel = biglist.(propname)[1];
      player:notify(tostr("Maximum level is ", maxlevel, "."));
      items = $list_utils:make(maxlevel, {});
      "Arrgh.  Even after finding the root, some nodes might be detached!";
      player:notify("Checking for orphans...");
      for prop in (properties(biglist))
        $command_utils:suspend_if_needed(0);
        if (prop && prop[1] == " ")
          val = biglist.(prop);
          if (typeof(val) == TYPE_LIST && typeof(level = val[1]) == TYPE_INT && level < maxlevel)
            items[level + 1] = {@items[level + 1], prop};
          endif
        endif
      endfor
      for prop in (properties(biglist))
        $command_utils:suspend_if_needed(0);
        if (prop && prop[1] == " ")
          val = biglist.(prop);
          if (typeof(val) == TYPE_LIST && typeof(level = val[1]) == TYPE_INT && level > 0)
            for item in (val[2])
              items[level] = setremove(items[level], item[1]);
            endfor
          endif
        endif
      endfor
      player:notify(tostr("Orphans: ", toliteral(items)));
      backbone_prop = propname;
      level = maxlevel;
      while (level)
        backbone = biglist.(backbone_prop);
        lastkid = backbone_prop;
        for prop in (props = items[level])
          backbone[2] = {@backbone[2], {lastkid = prop, 0, {0, 0}}};
        endfor
        player:notify(tostr("Attaching ", nn = length(props), " propert", nn == 1 ? "y" | "ies", " to property ", toliteral(backbone_prop), "..."));
        biglist.(backbone_prop) = backbone;
        backbone_prop = lastkid;
        level = level - 1;
      endwhile
      player:notify(tostr("Orphans repatriated."));
    endif
    toplevel = "(top level)";
    context = args[2] || toplevel;
    "This stuff is just paranoia in case something unexpected is in the data structure.  Normally there should be no blowouts here. --Minnie";
    if (typeof(propname) != TYPE_STR)
      player:notify(tostr("Context=", context, " Prop Name=", toliteral(propname), " -- bad property name."));
      raise(E_INVARG);
    endif
    val = biglist.(propname);
    if (typeof(val) != TYPE_LIST)
      player:notify(tostr("Context=", context, " Prop Name=", toliteral(propname), " -- contents invalid."));
      raise(E_INVARG);
    endif
    if (typeof(level = val[1]) != TYPE_INT)
      player:notify(tostr("Context=", context, " Prop Name=", toliteral(propname), " -- contents invalid (bad first argument)."));
      raise(E_INVARG);
    endif
    "This is where the real work starts. --Minnie";
    "First check that the properties referred to really exist.  This must be done for all levels.";
    for item in (val[2])
      try
        biglist.((item[1]));
      except (E_PROPNF)
        player:notify(tostr("Item ", toliteral(item), " is invalid in property ", toliteral(propname), ".  It is being removed."));
        val[2] = setremove(val[2], item);
        continue item;
      endtry
    endfor
    "Next, only for upper levels, check that the message count for inferior levels is correct, but only after recursing into those levels and making repairs.";
    if (level > 0)
      new = $list_utils:map_arg(this, verb, $list_utils:slice(val[2]), propname);
      if (val[2] != new)
        player:notify(tostr("Changing ", toliteral(val[2]), " to ", toliteral(new), "."));
        val[2] = new;
      endif
      "Now that everything is correct, count size of inferiors.";
    endif
    "Bravely stuff the result back into place.";
    biglist.(propname) = val;
    "The result will be of the form:                               ";
    "  {propname, inferior_msgcount, {first_msgnum, first_time}}  ";
    if (level == 0)
      "Count the messages for message count.";
      "Use first message number and time for first_msgnum and first_time.";
      result = {propname, length(val[2]), (val[2][1])[2..3]};
    else
      "Use message count that is sum of inferior counts.";
      "Just propagate first node's first_msgnum and first_time upward literally.";
      n = 0;
      for subnode in (val[2])
        n = n + subnode[2];
      endfor
      result = {propname, n, val[2][1][3]};
    endif
    if (context == toplevel)
      if (result != biglist.messages)
        biglist.messages = result;
        player:notify(tostr("Property ", biglist, ".messages updated."));
      endif
      player:tell(biglist.messages[2], " messages repaired in ", $mail_agent:name(biglist), ".");
    endif
    return result;
    "Last modified Thu Feb 15 23:13:44 1996 MST by Minnie (#123).";
  endverb

  verb repair (this none none) owner: #2 flags: "rd"
    "Syntax: repair <biglist>";
    "";
    "This tool makes a last-resort attempt to repair broken biglists (ones whose data structures are out of alignment due to an error such as \"out of ticks\" during some update operation leaving the b-tree in an inconsistent state).  This tool comes with no warranty of any kind.  You should only use it when you have no other choice, and you should make an attempt to @dump or fully copy or otherwise checkpoint your object before attempting to repair it so that you can recover from any failures this might produce.  This operation is NOT undoable.";
    if (!$perm_utils:controls(player, this))
      player:tell("You do not control that.");
    elseif (!$command_utils:yes_or_no("This tool can be used to repair some (but maybe not all) situations involving generic biglists that have had an error (usually \"out of ticks\") during an update operation and were left inconsistent.  Is this list really and truly broken in such a way?"))
      player:tell("No action taken.  PLEASE don't use this except in extreme cases.");
    elseif (!$command_utils:yes_or_no("Have you made a best effort to @dump or otherwise save the contents in case this make things worse?"))
      player:tell("No action taken.  PLEASE do any saving you can before proceeding.");
    elseif (!$command_utils:yes_or_no("This tool comes with no warranty of any kind.  Is this really your last resort and are you prepared to accept the consequences of utter failure?  There is no undoing the actions this takes.  Do you understand and accept the risks?"))
      player:tell("No action taken.  I'm not taking any responsibility for this failing.  It's gotta be your choice.");
    else
      player:tell("OK!  Going ahead with repair attempts...");
      this:_repair();
      player:tell("All done.  If this worked, you can thank Mickey.  If not, remember the promises you made above about accepting responsibility for failure.");
    endif
    "Last modified Fri Feb 16 08:36:27 1996 MST by Minnie (#123).";
  endverb

  verb restore_from (this none this) owner: #2 flags: "rxd"
    ":restore_from(OLD_MAIL_RECIPIENT, LOST_STRING)";
    "This clears all biglist properties from this object, then";
    "scans the properties of OLD_MAIL_RECIPIENT, which must be a descendant";
    "of $big_mail_recipient, looking for those corresponding to mail messages,";
    "and then rebuilds the message tree entirely from scratch.";
    "";
    "No attempt is made to preserve the original tree structure.";
    "The live/deleted state of any given message is lost;";
    "all messages, including formerly rmm-ed ones, are restored to .messages";
    "";
    "In the (unlikely) event that message-body properties have been lost, the";
    "affected messages are given a one-line body consisting of LOST_STRING";
    "";
    {old, ?lost_body = "###BODY-LOST###"} = args;
    if (!($perm_utils:controls(caller_perms(), this) && $perm_utils:controls(caller_perms(), old)))
      raise(E_PERM);
    elseif (!$object_utils:isa(old, $big_mail_recipient))
      raise(E_TYPE, "First argument must be a $big_mail_recipient.");
    elseif (typeof(lost_body) != TYPE_STR)
      raise(E_TYPE, "Second argument, if given, must be a string.");
    endif
    mgr = this._mgr;
    "...";
    "... destroy everything...";
    for p in (properties(this))
      delete_property(this, p);
    endfor
    this.messages = this.messages_going = {};
    "...";
    "... look at all properties...";
    msgcount = lostcount = 0;
    for p in (properties(old))
      if (index(p, " ") == 1)
        pvalue = old.(p);
        "... ignore everything except level-0 nodes...";
        if (pvalue[1..min(1, $)] == {0})
          for msg in (pvalue[2])
            if (ticks_left() < 6000 || seconds_left() < 2)
              player:tell("...", msgcount, " copied.");
              suspend(0);
            endif
            try
              body = old.((msg[1]));
            except e (E_PROPNF)
              body = {lost_body};
              lostcount = lostcount + 1;
            endtry
            msg[1] = this:_make(@body);
            msgtree = mgr:insert_last(this.messages, msg);
            msgcount = msgcount + 1;
            n = mgr:find_ord(msgtree, this:_message_num(@msg), "_lt_msgnum");
            if (n < msgcount)
              {msgtree, singleton} = mgr:extract_range(msgtree, msgcount, msgcount);
              msgtree = mgr:insert_after(msgtree, singleton, n);
            endif
            this.messages = msgtree;
          endfor
        endif
      endif
    endfor
    player:tell(msgcount, " messages installed on ", this.name, "(", this, ")");
    if (lostcount)
      player:tell(lostcount, " messages have missing bodies (indicated by ", toliteral(lost_body), ").");
    else
      player:tell("No message bodies were missing.");
    endif
  endverb

  verb set_message_body_by_index (this none this) owner: HACKER flags: "rxd"
    {i, body} = args;
    if (!this:ok_write(caller, caller_perms()))
      "... maybe someday let people edit messages they've sent?";
      "... && !(this:ok(caller, caller_perms()) && (seq = this:own_messages_filter(caller_perms(), @args))) ???";
      return E_PERM;
    endif
    {bodyprop, @rest} = this._mgr:find_nth(this.messages, i);
    if (!body)
      if (bodyprop)
        this:_kill(bodyprop);
        this._mgr:set_nth(this.messages, i, {0, @rest});
      endif
    elseif (bodyprop)
      if (typeof(body) != TYPE_LIST)
        raise(E_TYPE);
      endif
      this.(bodyprop) = body;
    else
      bodyprop = this:_make(@body);
      this._mgr:set_nth(this.messages, i, {bodyprop, @rest});
    endif
  endverb
endobject