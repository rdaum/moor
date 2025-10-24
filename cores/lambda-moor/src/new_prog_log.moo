object NEW_PROG_LOG
  name: "New-Prog-Log"
  parent: MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: #2

  property keyword (owner: #2, flags: "rc") = "PROGRAMMER";

  override aliases (owner: HACKER, flags: "r") = {"New-Prog-Log", "New_Prog_Log", "NPL"};
  override description = "Record of who's been made a @programmer.";
  override mail_forward = {};
  override mail_notify = {#2};
  override moderated = 1;
  override object_size = {6043, 1084848672};

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_notify = {player};
      player:set_current_message(this, 0, 0, 1);
      this.moderated = 1;
    else
      return E_PERM;
    endif
  endverb

  verb receive_message (this none this) owner: #2 flags: "rxd"
    if (!this:is_writable_by(caller_perms()))
      return E_PERM;
    else
      if (msgs = this.messages)
        new = msgs[$][1] + 1;
      else
        new = 1;
      endif
      if (rmsgs = this.messages_going)
        lbrm = rmsgs[$][2];
        new = max(new, lbrm[$][1] + 1);
      endif
      m = args[1];
      if (index(m[4], "@programmer ") == 1)
        m = {m[1], toobj(args[2]), o = $mail_agent:parse_address_field(m[4])[1], o.name};
      endif
      this.messages = {@msgs, {new, m}};
      this.last_msg_date = m[1];
      this.last_used_time = time();
      return new;
    endif
  endverb

  verb "display_seq_headers display_seq_full" (this none this) owner: #2 flags: "rxd"
    ":display_seq_headers(msg_seq[,cur])";
    ":display_seq_full(msg_seq[,cur]) => {cur}";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    {msg_seq, ?cur = 0, ?read_date = $maxint} = args;
    last = ldate = 0;
    player:tell("       WHEN           ", $string_utils:left(this.keyword, -30), "BY");
    for x in (msgs = this:messages_in_seq(args[1]))
      msgnum = $string_utils:right(last = x[1], 4, cur == x[1] ? ">" | " ");
      ldate = x[2][1];
      if (typeof(x[2][2]) != OBJ)
        hdr = this:msg_summary_line(@x[2]);
      else
        if (ldate < time() - 31536000)
          c = player:ctime(ldate);
          date = c[5..11] + c[21..25];
        else
          date = (player:ctime(ldate))[5..16];
        endif
        hdr = tostr(ctime(ldate)[5..16], "   ", $string_utils:left(tostr(x[2][4], " (", x[2][3], ")"), 30), valid(w = x[2][2]) ? w.name | "??", " (", x[2][2], ")");
      endif
      player:tell(msgnum, ldate > read_date ? ":+ " | ":  ", hdr);
      $command_utils:suspend_if_needed(0);
    endfor
    if (verb == "display_seq_full")
      return {last, ldate};
    else
      player:tell("----+");
    endif
  endverb

  verb from_msg_seq (this none this) owner: #2 flags: "rxd"
    ":from_msg_seq(object or list[,mask])";
    " => msg_seq of messages from any of these senders";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    {plist, ?mask = {1}} = args;
    if (typeof(plist) != LIST)
      plist = {plist};
    endif
    i = 1;
    fseq = {};
    for msg in (this.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        if (msg[2][2] in plist)
          fseq = $seq_utils:add(fseq, i, i);
        endif
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return fseq || "%f %<has> no messages from " + $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", plist), "no one", " or ");
  endverb

  verb to_msg_seq (this none this) owner: #2 flags: "rxd"
    ":to_msg_seq(object or list[,mask]) => msg_seq of messages to those people";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    {plist, ?mask = {1}} = args;
    if (typeof(plist) != LIST)
      plist = {plist};
    endif
    i = 1;
    fseq = {};
    for msg in (this.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        if (msg[2][3] in plist)
          fseq = $seq_utils:add(fseq, i, i);
        endif
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return fseq || "%f %<has> no messages about @programmer'ing " + $string_utils:english_list(plist, "no one", " or ");
  endverb

  verb "%to_msg_seq subject_msg_seq" (this none this) owner: #2 flags: "rxd"
    ":%to_msg_seq/subject_msg_seq(string or list of strings[,mask])";
    " => msg_seq of messages containing one of strings in the to line";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    {nlist, ?mask = {1}} = args;
    if (typeof(nlist) != LIST)
      nlist = {nlist};
    endif
    i = 1;
    fseq = {};
    for msg in (this.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        if (msg[2][4] in nlist)
          fseq = $seq_utils:add(fseq, i, i);
        endif
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return fseq || "%f %<has> no messages about @programmer'ing " + $string_utils:english_list(nlist, "no one", " or ");
  endverb

  verb "%from_msg_seq" (this none this) owner: #2 flags: "rxd"
    return this.name + " doesn't understand %%from:";
  endverb
endobject